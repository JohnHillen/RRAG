use crate::llm::{ChatMessage, LlmClient, LlmRuntimeConfig};
use anyhow::{Context, Result};
use serde_json::{json, Value};

pub(crate) fn get_ollama_num_ctx() -> u64 {
    std::env::var("OLLAMA_NUM_CTX")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(16384)
}

pub(crate) fn should_disable_think() -> bool {
    std::env::var("OLLAMA_THINK")
        .map(|v| v == "false" || v == "0")
        .unwrap_or(false)
}

impl LlmClient {
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
            "ollama" => {
                self.call_ollama_native(&cfg, messages, model, temp, num_ctx, think)
                    .await
            }
            _ => {
                if cfg.llm_base_url.contains(":11434") {
                    self.call_ollama_native(&cfg, messages, model, temp, num_ctx, think)
                        .await
                } else {
                    self.call_openai_compatible(&cfg, messages, model, temp)
                        .await
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
        let disable_think = think.map(|t| !t).unwrap_or_else(should_disable_think);
        let mut chat_msgs: Vec<ChatMessage> = messages.to_vec();
        if disable_think && is_thinking_model(model) {
            // Prefill empty `<think></think>` so DeepSeek-R1 / QwQ skip reasoning tokens and respond in <1s
            chat_msgs.push(ChatMessage {
                role: "assistant".to_string(),
                content: "<think>\n</think>\n".to_string(),
            });
        }

        let mut payload = json!({
            "model": model,
            "messages": chat_msgs,
            "stream": false,
            "keep_alive": -1,
            "options": {
                "temperature": temperature,
                "num_ctx": effective_num_ctx
            }
        });

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
}

/// Checks whether a model supports explicit Ollama parameter `think: false` (e.g. Qwen3/3.5, DeepSeek-R1, QwQ).
pub(crate) fn is_thinking_model(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("qwen3") || m.contains("deepseek-r1") || m.contains("qwq") || m.contains("think")
}

/// Strips empty `<think></think>` blocks when a reasoning model returns with thinking suppressed.
pub(crate) fn strip_empty_think_tags(text: &str) -> String {
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
