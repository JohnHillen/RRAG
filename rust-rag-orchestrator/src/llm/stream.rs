use super::{
    providers::{get_ollama_num_ctx, is_thinking_model, should_disable_think},
    ChatMessage, LlmClient, LlmRuntimeConfig,
};
use anyhow::Result;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc;

impl LlmClient {
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
                || (provider == "openai_compatible"
                    && (cfg.llm_base_url.contains(":11434") || cfg.llm_base_url.contains(":11435")))
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
        let disable_think = think.map(|t| !t).unwrap_or_else(should_disable_think);
        let mut chat_msgs: Vec<ChatMessage> = messages.to_vec();
        if disable_think && is_thinking_model(model) {
            chat_msgs.push(ChatMessage {
                role: "assistant".to_string(),
                content: "<think>\n</think>\n".to_string(),
            });
        }

        let mut payload = json!({
            "model": model,
            "messages": chat_msgs,
            "stream": true,
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
        let mut in_thinking = false;

        while let Some(chunk_res) = stream.next().await {
            let bytes = chunk_res?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if let Some(data_str) = line.strip_prefix("data: ") {
                    let trimmed = data_str.trim();
                    if trimmed == "[DONE]" {
                        if in_thinking {
                            let _ = tx.send(Ok("\n</think>\n\n".to_string())).await;
                        }
                        return Ok(());
                    }
                    if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                        let delta = &val["choices"][0]["delta"];
                        if let Some(think_tok) = delta["reasoning_content"]
                            .as_str()
                            .or_else(|| delta["reasoning"].as_str())
                        {
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
                        if let Some(tok) = delta["content"].as_str() {
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
        }
        if in_thinking {
            let _ = tx.send(Ok("\n</think>\n\n".to_string())).await;
        }
        Ok(())
    }
}
