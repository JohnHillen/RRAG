pub mod providers;
pub mod stream;

use crate::config::AppConfig;
use crate::db::RetrievedChunk;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRuntimeConfig {
    pub llm_provider: String,
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
}

#[derive(Clone)]
pub struct LlmClient {
    pub(crate) http: Client,
    pub(crate) runtime_cfg: Arc<RwLock<LlmRuntimeConfig>>,
    pub(crate) enable_graph_extraction: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ExtractedGraphData {
    #[serde(default)]
    pub entities: Vec<ExtractedEntity>,
    #[serde(default)]
    pub relations: Vec<ExtractedRelation>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractedRelation {
    pub source: String,
    pub relation: String,
    pub target: String,
}

impl LlmClient {
    pub fn new(http: Client, config: AppConfig) -> Self {
        let initial_cfg = LlmRuntimeConfig {
            llm_provider: config.llm_provider.clone(),
            llm_base_url: normalize_docker_localhost(&config.llm_base_url),
            llm_api_key: config.llm_api_key.clone(),
            llm_model: config.llm_model.clone(),
        };
        Self {
            http,
            runtime_cfg: Arc::new(RwLock::new(initial_cfg)),
            enable_graph_extraction: config.enable_graph_extraction,
        }
    }

    /// Returns the currently active LLM runtime configuration (thread-safe, editable at runtime).
    pub fn get_runtime_config(&self) -> LlmRuntimeConfig {
        self.runtime_cfg
            .read()
            .map(|c| c.clone())
            .unwrap_or_else(|_| LlmRuntimeConfig {
                llm_provider: "openai_compatible".into(),
                llm_base_url: "http://host.docker.internal:11434/v1".into(),
                llm_api_key: "ollama".into(),
                llm_model: "qwen2.5:7b".into(),
            })
    }

    /// Dynamically updates provider, server URL, API key, and active model at runtime.
    pub fn update_runtime_config(
        &self,
        provider: String,
        base_url: String,
        api_key: Option<String>,
        model: String,
    ) -> LlmRuntimeConfig {
        let mut guard = self.runtime_cfg.write().unwrap();
        if !provider.trim().is_empty() {
            guard.llm_provider = provider.trim().to_string();
        }
        if !base_url.trim().is_empty() {
            guard.llm_base_url = normalize_docker_localhost(base_url.trim());
        }
        if let Some(k) = api_key {
            if !k.is_empty() {
                guard.llm_api_key = k;
            }
        }
        if !model.trim().is_empty() {
            guard.llm_model = model.trim().to_string();
        }
        guard.clone()
    }

    /// Live discovers available models from the target LLM server.
    pub async fn discover_models(&self, custom_cfg: Option<&LlmRuntimeConfig>) -> Vec<String> {
        let cfg = custom_cfg
            .cloned()
            .unwrap_or_else(|| self.get_runtime_config());
        let provider = cfg.llm_provider.to_lowercase();
        let base_url = normalize_docker_localhost(cfg.llm_base_url.trim());
        let mut models: Vec<String> = Vec::new();

        if provider == "none" {
            return vec!["none (Retrieval only)".to_string()];
        }

        // 1st attempt: If Ollama (native or port 11434 / 11435), query `/api/tags`
        if provider == "ollama" || base_url.contains(":11434") || base_url.contains(":11435") {
            let ollama_root = base_url.trim_end_matches('/').trim_end_matches("/v1");
            let tags_url = format!("{}/api/tags", ollama_root);
            if let Ok(resp) = self
                .http
                .get(&tags_url)
                .timeout(Duration::from_millis(3500))
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(body) = resp.json::<Value>().await {
                        if let Some(arr) = body["models"].as_array() {
                            for item in arr {
                                if let Some(name) = item["name"].as_str() {
                                    let s = name.trim().to_string();
                                    if !s.is_empty() && !models.contains(&s) {
                                        models.push(s);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2nd attempt: If Google Gemini API (googleapis.com), query `/v1beta/models?key=...`
        if models.is_empty() && base_url.contains("googleapis.com") && !cfg.llm_api_key.is_empty() {
            let gemini_models_url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}&pageSize=100",
                cfg.llm_api_key.trim()
            );
            if let Ok(resp) = self
                .http
                .get(&gemini_models_url)
                .timeout(Duration::from_millis(4500))
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(body) = resp.json::<Value>().await {
                        if let Some(arr) = body["models"].as_array() {
                            for item in arr {
                                let supports_chat = item["supportedGenerationMethods"]
                                    .as_array()
                                    .map(|methods| {
                                        methods
                                            .iter()
                                            .any(|m| m.as_str() == Some("generateContent"))
                                    })
                                    .unwrap_or(true);
                                if !supports_chat {
                                    continue;
                                }
                                if let Some(raw_name) = item["name"].as_str() {
                                    let clean_name = raw_name
                                        .trim()
                                        .strip_prefix("models/")
                                        .unwrap_or(raw_name.trim())
                                        .to_string();
                                    let lower = clean_name.to_lowercase();
                                    let is_non_chat = lower.contains("-tts")
                                        || lower.starts_with("lyria")
                                        || lower.starts_with("imagen")
                                        || lower.starts_with("veo")
                                        || lower.contains("embedding")
                                        || lower.starts_with("aqa");
                                    if !clean_name.is_empty()
                                        && !is_non_chat
                                        && !models.contains(&clean_name)
                                    {
                                        models.push(clean_name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3rd attempt: OpenAI-compatible `/models` endpoint
        if models.is_empty() && (provider == "openai_compatible" || provider == "ollama") {
            let base_trimmed = base_url.trim_end_matches('/');
            let models_url =
                if base_trimmed.ends_with("/v1") || base_trimmed.ends_with("/openai") {
                    format!("{}/models", base_trimmed)
                } else {
                    format!("{}/v1/models", base_trimmed)
                };

            let mut req = self
                .http
                .get(&models_url)
                .timeout(Duration::from_millis(4000));
            if !cfg.llm_api_key.is_empty() {
                req = req.bearer_auth(&cfg.llm_api_key);
            }

            if let Ok(resp) = req.send().await {
                if resp.status().is_success() {
                    if let Ok(body) = resp.json::<Value>().await {
                        if let Some(arr) = body["data"].as_array() {
                            for item in arr {
                                if let Some(id) = item["id"].as_str() {
                                    let clean_id = id
                                        .trim()
                                        .strip_prefix("models/")
                                        .unwrap_or(id.trim())
                                        .to_string();
                                    if !clean_id.is_empty() && !models.contains(&clean_id) {
                                        models.push(clean_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if models.is_empty() {
            if provider == "anthropic" || base_url.contains("anthropic.com") {
                models.extend([
                    "claude-3-7-sonnet-latest".to_string(),
                    "claude-3-5-sonnet-latest".to_string(),
                    "claude-3-5-haiku-latest".to_string(),
                ]);
            } else if provider == "gemini_native" || base_url.contains("googleapis.com") {
                models.extend([
                    "gemini-2.5-flash".to_string(),
                    "gemini-2.5-pro".to_string(),
                    "gemini-2.0-flash".to_string(),
                ]);
            } else if base_url.contains("openai.com") {
                models.extend([
                    "gpt-4o".to_string(),
                    "gpt-4o-mini".to_string(),
                    "o3-mini".to_string(),
                ]);
            }
        }

        if !cfg.llm_model.trim().is_empty() && !models.iter().any(|m| m == cfg.llm_model.trim()) {
            models.insert(0, cfg.llm_model.trim().to_string());
        }

        models
    }

    /// Formats retrieved candidate chunks and graph entities into a structured RAG context block.
    pub fn format_rag_context(chunks: &[RetrievedChunk]) -> String {
        if chunks.is_empty() {
            return "No relevant documents found in the knowledge base.".to_string();
        }

        let mut out = String::from("### RELEVANT KNOWLEDGE BASE CONTEXT (RAG & GraphRAG)\n\n");
        for (i, c) in chunks.iter().enumerate() {
            out.push_str(&format!(
                "[Source {} | Document: '{}' | Chunk #{} | Score: {:.4}]\n{}\n",
                i + 1,
                c.doc_title,
                c.chunk_index,
                c.rerank_score.unwrap_or(c.rrf_score),
                c.content
            ));

            if !c.entities.is_empty() {
                let ent_names: Vec<String> = c
                    .entities
                    .iter()
                    .take(15)
                    .map(|e| format!("{} ({})", e.name, e.entity_type))
                    .collect();
                out.push_str(&format!("  -> Mentioned entities: {}\n", ent_names.join(", ")));
            }
            if !c.related_entities.is_empty() {
                let rel_names: Vec<String> = c
                    .related_entities
                    .iter()
                    .take(15)
                    .map(|e| format!("{} ({})", e.name, e.entity_type))
                    .collect();
                out.push_str(&format!(
                    "  -> Linked graph entities: {}\n",
                    rel_names.join(", ")
                ));
            }
            if !c.relations.is_empty() {
                let rel_triplets: Vec<String> = c
                    .relations
                    .iter()
                    .take(18)
                    .map(|r| format!("({} --[{}]--> {})", r.from, r.relation, r.to))
                    .collect();
                out.push_str(&format!(
                    "  -> Knowledge Graph Relations: {}\n",
                    rel_triplets.join(", ")
                ));
            }
            out.push('\n');
        }
        out
    }

    /// Optionally extracts entities and relations for GraphRAG using the configured LLM.
    /// Contains zero domain- or dataset-specific code.
    pub async fn extract_graph_entities(
        &self,
        text: &str,
        force_extract: Option<bool>,
    ) -> Result<(
        Vec<(String, String, Option<String>)>,
        Vec<(String, String, String)>,
    )> {
        let cfg = self.get_runtime_config();
        let should_extract = force_extract.unwrap_or(self.enable_graph_extraction);
        if !should_extract || cfg.llm_provider == "none" {
            return Ok((vec![], vec![]));
        }

        let sample: String = text.chars().take(2500).collect();
        let prompt = format!(
            r#"Extract up to 8 key entities and their relationships from the following text as a pure JSON object:
{{"entities": [{{"name": "...", "entity_type": "Organization|Person|Concept|Document|System|Location", "description": "..."}}], "relations": [{{"source": "...", "relation": "relates_to|references|requires|belongs_to|regulates", "target": "..."}}]}}

Text:
{}"#,
            sample
        );

        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "Respond strictly with valid JSON without markdown wrapping or prose."
                    .into(),
            },
            ChatMessage {
                role: "user".into(),
                content: prompt,
            },
        ];

        if let Ok(raw) = self
            .chat_completion(&messages, None, Some(0.0), Some(4096), Some(false))
            .await
        {
            let after_think = match raw.rfind("</think>") {
                Some(pos) => &raw[pos + 8..],
                None => raw.as_str(),
            };
            let trimmed = if let (Some(start), Some(end)) =
                (after_think.find('{'), after_think.rfind('}'))
            {
                if start < end {
                    &after_think[start..=end]
                } else {
                    after_think.trim()
                }
            } else {
                after_think.trim()
            };

            if let Ok(parsed) = serde_json::from_str::<ExtractedGraphData>(trimmed) {
                let entities = parsed
                    .entities
                    .into_iter()
                    .filter(|e| !e.name.trim().is_empty())
                    .map(|e| (e.name.trim().to_string(), e.entity_type, e.description))
                    .collect();
                let relations = parsed
                    .relations
                    .into_iter()
                    .filter(|r| !r.source.trim().is_empty() && !r.target.trim().is_empty())
                    .map(|r| {
                        (
                            r.source.trim().to_string(),
                            r.relation.trim().to_string(),
                            r.target.trim().to_string(),
                        )
                    })
                    .collect();
                return Ok((entities, relations));
            }
        }

        Ok((vec![], vec![]))
    }
}

/// If running inside a Docker container (`/.dockerenv`) and the user specifies
/// `localhost` or `127.0.0.1`, automatically map to `host.docker.internal`
/// (except for port `:11435` or `:18081` when the container-internal RunPod SSH tunnel is active).
pub(crate) fn normalize_docker_localhost(url: &str) -> String {
    if std::path::Path::new("/.dockerenv").exists() {
        if (url.contains(":11435") || url.contains(":18081"))
            && std::path::Path::new("/tmp/runpod_ui_tunnel.active").exists()
        {
            return url
                .replace("://localhost", "://127.0.0.1")
                .replace("://host.docker.internal", "://127.0.0.1");
        }
        url.replace("://localhost", "://host.docker.internal")
            .replace("://127.0.0.1", "://host.docker.internal")
    } else {
        url.to_string()
    }
}
