use crate::config::AppConfig;
use crate::db::RetrievedChunk;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRuntimeConfig {
    pub llm_provider: String,
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
}

#[derive(Clone)]
pub struct LlmClient {
    http: Client,
    runtime_cfg: Arc<RwLock<LlmRuntimeConfig>>,
    enable_graph_extraction: bool,
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

fn get_ollama_num_ctx() -> u64 {
    std::env::var("OLLAMA_NUM_CTX")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(16384)
}

fn should_disable_think() -> bool {
    std::env::var("OLLAMA_THINK")
        .map(|v| v == "false" || v == "0")
        .unwrap_or(false)
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
            // Only overwrite if a new non-empty key was supplied
            if !k.is_empty() {
                guard.llm_api_key = k;
            }
        }
        if !model.trim().is_empty() {
            guard.llm_model = model.trim().to_string();
        }
        guard.clone()
    }

    /// Live discovers available models from the target LLM server:
    /// - Local or remote Ollama (`/api/tags`)
    /// - OpenAI-compatible endpoints like RunPod vLLM, llama.cpp, LM Studio, OpenAI Cloud (`/v1/models`)
    /// - Google Gemini catalog (`/v1beta/models?key=...`)
    /// - Curated fallback suggestions for Claude & Gemini
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

        // 1st attempt: If Ollama (native or port 11434), query `/api/tags`
        if provider == "ollama" || base_url.contains(":11434") {
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

        // 2nd attempt: If Google Gemini API (googleapis.com), query the official catalog at `/v1beta/models?key=...`
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
                                // Only retain models supporting text/chat generation ("generateContent")
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
                                    // Filter out pure audio/TTS/music/vision specialist models
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

        // 3rd attempt: OpenAI-compatible `/models` endpoint (RunPod vLLM, OpenAI, LM Studio, Ollama /v1)
        if models.is_empty() && (provider == "openai_compatible" || provider == "ollama") {
            let base_trimmed = base_url.trim_end_matches('/');
            let models_url = if base_trimmed.ends_with("/v1") || base_trimmed.ends_with("/openai") {
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

        // Fallback: Curated cloud model suggestions if endpoint does not provide a listing
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

        // Ensure currently configured model is always present at position 0
        if !cfg.llm_model.trim().is_empty()
            && !models.iter().any(|m| m == cfg.llm_model.trim())
        {
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
                    .map(|e| format!("{} ({})", e.name, e.entity_type))
                    .collect();
                out.push_str(&format!("  -> Mentioned entities: {}\n", ent_names.join(", ")));
            }
            if !c.related_entities.is_empty() {
                let rel_names: Vec<String> = c
                    .related_entities
                    .iter()
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

    /// Generic non-streaming chat completion call (Local via Ollama/vLLM, Remote RunPod, or Cloud via OpenAI, Gemini, Claude).
    pub async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        model_override: Option<&str>,
        temperature: Option<f32>,
        num_ctx: Option<u64>,
        think: Option<bool>,
    ) -> Result<String> {
        let cfg = self.get_runtime_config();
        let provider = cfg.llm_provider.to_lowercase();
        let model = model_override
            .filter(|m| !m.is_empty() && *m != "rag-default")
            .unwrap_or(&cfg.llm_model);
        let temp = temperature.unwrap_or(0.2);

        match provider.as_str() {
            "none" => {
                // Retrieval-only mode: returns formatted context directly without LLM invocation
                let summary = messages
                    .iter()
                    .map(|m| format!("[{}]: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                Ok(format!(
                    "LLM_PROVIDER is set to 'none' (Retrieval-only mode).\n\n{}",
                    summary
                ))
            }
            "anthropic" => self.call_anthropic(&cfg, messages, model, temp).await,
            "gemini_native" => self.call_gemini_native(&cfg, messages, model, temp).await,
            "ollama" => self.call_ollama_native(&cfg, messages, model, temp, num_ctx, think).await,
            _ => {
                // If an Ollama instance (:11434) is configured under openai_compatible,
                // automatically use native /api/chat to reliably support `keep_alive: -1` and `think: false`
                if cfg.llm_base_url.contains(":11434") {
                    self.call_ollama_native(&cfg, messages, model, temp, num_ctx, think).await
                } else {
                    self.call_openai_compatible(&cfg, messages, model, temp).await
                }
            }
        }
    }

    async fn call_ollama_native(
        &self,
        cfg: &LlmRuntimeConfig,
        messages: &[ChatMessage],
        model: &str,
        temperature: f32,
        num_ctx: Option<u64>,
        think: Option<bool>,
    ) -> Result<String> {
        let base = cfg
            .llm_base_url
            .trim_end_matches('/')
            .trim_end_matches("/v1");
        let url = format!("{}/api/chat", base);

        let effective_num_ctx = num_ctx.unwrap_or_else(get_ollama_num_ctx);
        let mut payload = json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "keep_alive": -1,
            "options": {
                "temperature": temperature,
                "num_ctx": effective_num_ctx
            }
        });

        let disable_think = think.map(|t| !t).unwrap_or_else(should_disable_think);
        if disable_think && is_thinking_model(model) {
            payload["think"] = json!(false);
        }

        let mut req = self.http.post(&url).json(&payload);
        if !cfg.llm_api_key.is_empty() && cfg.llm_api_key != "ollama" {
            req = req.bearer_auth(&cfg.llm_api_key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Ollama at {}: {:?}", url, e))?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("Ollama HTTP {}: {}", status, body);
        }

        let eval_count = body["eval_count"].as_u64().unwrap_or(0);
        let prompt_eval_count = body["prompt_eval_count"].as_u64().unwrap_or(0);
        let load_dur_ms = body["load_duration"].as_u64().unwrap_or(0) / 1_000_000;
        let prompt_dur_ms = body["prompt_eval_duration"].as_u64().unwrap_or(0) / 1_000_000;
        let eval_dur_ms = body["eval_duration"].as_u64().unwrap_or(0) / 1_000_000;
        let has_thinking = body["message"]["thinking"]
            .as_str()
            .map(|s| s.len())
            .unwrap_or(0);

        tracing::info!(
            "Ollama metrics ({}): load={}ms | prompt_eval={}ms ({} tok) | eval={}ms ({} tok) | thinking_chars={}",
            model,
            load_dur_ms,
            prompt_dur_ms,
            prompt_eval_count,
            eval_dur_ms,
            eval_count,
            has_thinking
        );

        let content = body["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();

        let thinking = body["message"]["thinking"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();

        if !thinking.is_empty() && !content.is_empty() {
            Ok(format!("<think>\n{}\n</think>\n\n{}", thinking, content))
        } else if !content.is_empty() {
            Ok(strip_empty_think_tags(&content))
        } else if !thinking.is_empty() {
            Ok(thinking)
        } else {
            Ok(String::new())
        }
    }

    /// Streams completion tokens in real time (token by token) over a Tokio channel.
    pub async fn chat_completion_stream(
        &self,
        messages: Vec<ChatMessage>,
        model_override: Option<String>,
        temperature: Option<f32>,
        num_ctx: Option<u64>,
        think: Option<bool>,
    ) -> mpsc::Receiver<Result<String>> {
        let (tx, rx) = mpsc::channel(64);
        let client = self.clone();

        tokio::spawn(async move {
            let cfg = client.get_runtime_config();
            let provider = cfg.llm_provider.to_lowercase();
            let model = model_override
                .as_deref()
                .filter(|m| !m.is_empty() && *m != "rag-default")
                .unwrap_or(&cfg.llm_model)
                .to_string();
            let temp = temperature.unwrap_or(0.2);

            if provider == "ollama"
                || (provider == "openai_compatible" && cfg.llm_base_url.contains(":11434"))
            {
                if let Err(e) = client
                    .stream_ollama_native(&cfg, &messages, &model, temp, num_ctx, think, &tx)
                    .await
                {
                    let _ = tx.send(Err(e)).await;
                }
            } else if provider == "openai_compatible" {
                if let Err(e) = client
                    .stream_openai_compatible(&cfg, &messages, &model, temp, &tx)
                    .await
                {
                    let _ = tx.send(Err(e)).await;
                }
            } else {
                // Non-streaming fallback for providers without SSE parser (e.g. Anthropic, Gemini Native, None)
                match client
                    .chat_completion(&messages, Some(&model), Some(temp), num_ctx, think)
                    .await
                {
                    Ok(full) => {
                        let _ = tx.send(Ok(full)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                    }
                }
            }
        });

        rx
    }

    async fn stream_ollama_native(
        &self,
        cfg: &LlmRuntimeConfig,
        messages: &[ChatMessage],
        model: &str,
        temperature: f32,
        num_ctx: Option<u64>,
        think: Option<bool>,
        tx: &mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        let base = cfg
            .llm_base_url
            .trim_end_matches('/')
            .trim_end_matches("/v1");
        let url = format!("{}/api/chat", base);

        let effective_num_ctx = num_ctx.unwrap_or_else(get_ollama_num_ctx);
        let mut payload = json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "keep_alive": -1,
            "options": {
                "temperature": temperature,
                "num_ctx": effective_num_ctx
            }
        });

        let disable_think = think.map(|t| !t).unwrap_or_else(should_disable_think);
        if disable_think && is_thinking_model(model) {
            payload["think"] = json!(false);
        }

        let mut req = self.http.post(&url).json(&payload);
        if !cfg.llm_api_key.is_empty() && cfg.llm_api_key != "ollama" {
            req = req.bearer_auth(&cfg.llm_api_key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Ollama at {}: {:?}", url, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama HTTP {}: {}", status, err_txt);
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut in_thinking = false;

        while let Some(chunk_res) = stream.next().await {
            let bytes = chunk_res?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                    // Check thinking tokens first
                    if let Some(think_tok) = val["message"]["thinking"].as_str() {
                        if !think_tok.is_empty() {
                            if !in_thinking {
                                in_thinking = true;
                                if tx.send(Ok("<think>\n".to_string())).await.is_err() {
                                    return Ok(());
                                }
                            }
                            if tx.send(Ok(think_tok.to_string())).await.is_err() {
                                return Ok(());
                            }
                        }
                    }

                    // Check content tokens
                    if let Some(tok) = val["message"]["content"].as_str() {
                        if !tok.is_empty() {
                            if in_thinking {
                                in_thinking = false;
                                if tx.send(Ok("\n</think>\n\n".to_string())).await.is_err() {
                                    return Ok(());
                                }
                            }
                            if tx.send(Ok(tok.to_string())).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        if in_thinking {
            let _ = tx.send(Ok("\n</think>\n\n".to_string())).await;
        }
        Ok(())
    }

    async fn stream_openai_compatible(
        &self,
        cfg: &LlmRuntimeConfig,
        messages: &[ChatMessage],
        model: &str,
        temperature: f32,
        tx: &mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        let base_trimmed = cfg.llm_base_url.trim_end_matches('/');
        let url = if base_trimmed.ends_with("/v1") || base_trimmed.ends_with("/openai") {
            format!("{}/chat/completions", base_trimmed)
        } else {
            format!("{}/v1/chat/completions", base_trimmed)
        };

        let payload = json!({
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "stream": true
        });

        let mut req = self.http.post(&url).json(&payload);
        if !cfg.llm_api_key.is_empty() {
            req = req.bearer_auth(&cfg.llm_api_key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to LLM at {}: {:?}", url, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM HTTP {}: {}", status, err_txt);
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            let bytes = chunk_res?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if let Some(data_str) = line.strip_prefix("data: ") {
                    let trimmed = data_str.trim();
                    if trimmed == "[DONE]" {
                        return Ok(());
                    }
                    if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                        if let Some(tok) = val["choices"][0]["delta"]["content"].as_str() {
                            if !tok.is_empty() && tx.send(Ok(tok.to_string())).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn call_openai_compatible(
        &self,
        cfg: &LlmRuntimeConfig,
        messages: &[ChatMessage],
        model: &str,
        temperature: f32,
    ) -> Result<String> {
        let base_trimmed = cfg.llm_base_url.trim_end_matches('/');
        let url = if base_trimmed.ends_with("/v1") || base_trimmed.ends_with("/openai") {
            format!("{}/chat/completions", base_trimmed)
        } else {
            format!("{}/v1/chat/completions", base_trimmed)
        };

        let payload = json!({
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "stream": false
        });

        let mut req = self.http.post(&url).json(&payload);
        if !cfg.llm_api_key.is_empty() {
            req = req.bearer_auth(&cfg.llm_api_key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to LLM at {}: {:?}", url, e))?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("LLM HTTP {}: {}", status, body);
        }

        let raw_content = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default();
        Ok(strip_empty_think_tags(raw_content))
    }

    async fn call_anthropic(
        &self,
        cfg: &LlmRuntimeConfig,
        messages: &[ChatMessage],
        model: &str,
        temperature: f32,
    ) -> Result<String> {
        let base_trimmed = cfg.llm_base_url.trim_end_matches('/');
        let url = if base_trimmed.ends_with("/v1") {
            format!("{}/messages", base_trimmed)
        } else {
            format!("{}/v1/messages", base_trimmed)
        };

        let system_prompt = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let user_messages: Vec<Value> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();

        let payload = json!({
            "model": model,
            "max_tokens": 2048,
            "temperature": temperature,
            "system": system_prompt,
            "messages": user_messages
        });

        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &cfg.llm_api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&payload)
            .send()
            .await
            .context("Failed to call Anthropic API")?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("Anthropic HTTP {}: {}", status, body);
        }

        let content = body["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        Ok(content)
    }

    async fn call_gemini_native(
        &self,
        cfg: &LlmRuntimeConfig,
        messages: &[ChatMessage],
        model: &str,
        temperature: f32,
    ) -> Result<String> {
        let base = cfg.llm_base_url.trim_end_matches('/');
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            base, model, cfg.llm_api_key
        );

        let combined_prompt = messages
            .iter()
            .map(|m| format!("{}: {}", m.role.to_uppercase(), m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let payload = json!({
            "contents": [{
                "parts": [{ "text": combined_prompt }]
            }],
            "generationConfig": {
                "temperature": temperature
            }
        });

        let resp = self
            .http
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to call Google Gemini Native API")?;

        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("Gemini HTTP {}: {}", status, body);
        }

        let content = body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        Ok(content)
    }

    /// Optionally extracts entities and relations for GraphRAG using the configured LLM.
    /// `force_extract`: `Some(true)` forces extraction (e.g. from UI checkbox),
    /// `Some(false)` skips extraction, `None` uses `.env` default (`ENABLE_GRAPH_EXTRACTION`).
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

        let sample: String = text.chars().take(3500).collect();
        let prompt = format!(
            r#"Extract key entities (persons, organizations, projects, technical skills/programming languages/databases, language proficiency levels, standards/certifications) and their relationships from the following text as a pure JSON object:
{{"entities": [{{"name": "...", "entity_type": "Person|Organization|Project|Skill|Certification|Document", "description": "..."}}], "relations": [{{"source": "...", "relation": "works_on|belongs_to|has_skill|requires_skill|guarantees_for|references|regulates", "target": "..."}}]}}

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

        if let Ok(raw) = self.chat_completion(&messages, None, Some(0.0), None, None).await {
            let trimmed = if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) {
                &raw[start..=end]
            } else {
                raw.trim()
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
/// `localhost` or `127.0.0.1`, automatically map to `host.docker.internal`.
fn normalize_docker_localhost(url: &str) -> String {
    if std::path::Path::new("/.dockerenv").exists() {
        url.replace("://localhost", "://host.docker.internal")
            .replace("://127.0.0.1", "://host.docker.internal")
    } else {
        url.to_string()
    }
}

/// Checks whether a model supports explicit Ollama parameter `think: false` (e.g. Qwen3/3.5, DeepSeek-R1, QwQ).
fn is_thinking_model(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("qwen3") || m.contains("deepseek-r1") || m.contains("qwq") || m.contains("think")
}

/// Strips empty `<think></think>` blocks when a reasoning model returns with thinking suppressed.
fn strip_empty_think_tags(text: &str) -> String {
    if let Some(end_idx) = text.find("</think>") {
        let after = &text[end_idx + "</think>".len()..];
        let before = &text[..text.find("<think>").unwrap_or(0)];
        let combined = format!("{}{}", before, after).trim().to_string();
        if !combined.is_empty() {
            return combined;
        }
    }
    text.trim().to_string()
}
