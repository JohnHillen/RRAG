use crate::{
    attachments::{prepare_ephemeral_attachment_chunks, ChatAttachment},
    db::RetrievedChunk,
    llm::{ChatMessage, LlmClient, LlmRuntimeConfig},
    retrieval::{
        build_conversational_retrieval_query, execute_retrieval_pipeline, RetrievalMetrics,
    },
    AppState,
};
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct OpenAiChatRequest {
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    #[serde(default = "default_true")]
    pub use_rag: bool,
    pub top_n: Option<usize>,
    pub reranker: Option<String>,
    #[serde(default)]
    pub num_ctx: Option<u64>,
    #[serde(default)]
    pub think: Option<bool>,
    #[serde(default)]
    pub attachment: Option<ChatAttachment>,
    #[serde(default)]
    pub attachment_only: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct LlmConfigUpdateRequest {
    pub llm_provider: String,
    pub llm_base_url: String,
    #[serde(default)]
    pub llm_api_key: Option<String>,
    pub llm_model: String,
}

/// Returns currently active LLM configuration and discovered models (`GET /v1/llm/config`)
pub async fn get_llm_config_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.llm.get_runtime_config();
    let available_models = state.llm.discover_models(Some(&cfg)).await;
    let has_key = !cfg.llm_api_key.is_empty() && cfg.llm_api_key != "ollama";
    Json(json!({
        "llm_provider": cfg.llm_provider,
        "llm_base_url": cfg.llm_base_url,
        "llm_model": cfg.llm_model,
        "has_api_key": has_key,
        "available_models": available_models,
        "orchestrator_port": state.config.orchestrator_port,
        "surreal_url": state.config.surreal_url,
        "tei_embed_url": state.config.tei_embed_url,
        "tei_rerank_url": state.config.tei_rerank_url,
        "tei_rerank_fast_url": state.config.tei_rerank_fast_url
    }))
}

/// Dynamically updates the LLM configuration at runtime (`POST /v1/llm/config`)
pub async fn update_llm_config_handler(
    State(state): State<AppState>,
    Json(req): Json<LlmConfigUpdateRequest>,
) -> impl IntoResponse {
    let updated = state.llm.update_runtime_config(
        req.llm_provider,
        req.llm_base_url,
        req.llm_api_key.clone(),
        req.llm_model,
    );
    info!(
        "LLM configuration updated live -> Provider: {}, URL: {}, Model: {}",
        updated.llm_provider, updated.llm_base_url, updated.llm_model
    );

    let _ = state
        .db
        .save_system_llm_settings(
            &updated.llm_provider,
            &updated.llm_base_url,
            req.llm_api_key.as_deref().or(Some(&updated.llm_api_key)),
            &updated.llm_model,
        )
        .await;

    let available_models = state.llm.discover_models(Some(&updated)).await;
    let has_key = !updated.llm_api_key.is_empty() && updated.llm_api_key != "ollama";
    Json(json!({
        "status": "updated",
        "llm_provider": updated.llm_provider,
        "llm_base_url": updated.llm_base_url,
        "llm_model": updated.llm_model,
        "has_api_key": has_key,
        "available_models": available_models
    }))
}

/// Probes and lists available models from an arbitrary LLM endpoint (`POST /v1/llm/models`)
pub async fn discover_llm_models_handler(
    State(state): State<AppState>,
    Json(req): Json<LlmConfigUpdateRequest>,
) -> impl IntoResponse {
    let current = state.llm.get_runtime_config();
    let effective_key = req
        .llm_api_key
        .filter(|k| !k.is_empty())
        .unwrap_or(current.llm_api_key);
    let temp_cfg = LlmRuntimeConfig {
        llm_provider: req.llm_provider,
        llm_base_url: req.llm_base_url,
        llm_api_key: effective_key,
        llm_model: req.llm_model,
    };
    let models = state.llm.discover_models(Some(&temp_cfg)).await;
    Json(json!({ "available_models": models }))
}

/// OpenAI-compatible model list (`GET /v1/models`)
pub async fn list_models_handler(State(state): State<AppState>) -> impl IntoResponse {
    let llm_cfg = state.llm.get_runtime_config();
    Json(json!({
        "object": "list",
        "data": [
            {
                "id": "rag-default",
                "object": "model",
                "created": 1730000000,
                "owned_by": "rust-rag-surrealdb"
            },
            {
                "id": llm_cfg.llm_model,
                "object": "model",
                "created": 1730000000,
                "owned_by": llm_cfg.llm_provider
            }
        ]
    }))
}

/// Shared RAG + ephemeral attachment context builder for both sync and streaming chat endpoints.
async fn prepare_augmented_chat_context(
    state: &AppState,
    req: &OpenAiChatRequest,
) -> (Vec<ChatMessage>, Vec<RetrievedChunk>, Option<RetrievalMetrics>) {
    let last_user_query = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let retrieval_query = build_conversational_retrieval_query(&req.messages);

    let mut augmented_messages = req.messages.clone();
    let mut retrieved_sources: Vec<RetrievedChunk> = Vec::new();
    let mut retrieval_metrics: Option<RetrievalMetrics> = None;
    let reranker_mode = req.reranker.as_deref().unwrap_or("fast");

    let mut attachment_rerank_ms = 0u64;
    let mut attachment_label = String::new();
    let has_attachment = req
        .attachment
        .as_ref()
        .map(|a| !a.text.trim().is_empty())
        .unwrap_or(false);

    if let Some(ref att) = req.attachment {
        if has_attachment {
            let (att_chunks, att_ms, att_lbl) =
                prepare_ephemeral_attachment_chunks(state, &last_user_query, att, reranker_mode)
                    .await;
            retrieved_sources.extend(att_chunks);
            attachment_rerank_ms = att_ms;
            attachment_label = att_lbl;
        }
    }

    let should_query_db =
        req.use_rag && !retrieval_query.is_empty() && !(has_attachment && req.attachment_only);
    if should_query_db {
        let top_n = req.top_n.unwrap_or(state.config.retrieval_final_top_n);
        match execute_retrieval_pipeline(state, &retrieval_query, top_n, reranker_mode).await {
            Ok((chunks, mut metrics)) => {
                metrics.rerank_ms += attachment_rerank_ms;
                metrics.total_ms += attachment_rerank_ms;
                if !attachment_label.is_empty() {
                    metrics.reranker_used =
                        format!("{} + {}", attachment_label, metrics.reranker_used);
                }
                retrieved_sources.extend(chunks);
                retrieval_metrics = Some(metrics);
            }
            Err(e) => warn!(
                "RAG retrieval failed, continuing without database context: {}",
                e
            ),
        }
    } else if !retrieved_sources.is_empty() {
        retrieval_metrics = Some(RetrievalMetrics {
            embed_ms: 0,
            db_ms: 0,
            rerank_ms: attachment_rerank_ms,
            total_ms: attachment_rerank_ms,
            reranker_used: attachment_label,
        });
    }

    if !retrieved_sources.is_empty() {
        let context_block = LlmClient::format_rag_context(&retrieved_sources);
        let system_instruction = format!(
            "You are an accurate AI knowledge assistant. Answer the user's question concisely and accurately based on the following sources (attached file and/or SurrealDB knowledge base). Cite sources using [Source X].\n\n{}",
            context_block
        );
        augmented_messages.insert(
            0,
            ChatMessage {
                role: "system".into(),
                content: system_instruction,
            },
        );
    }

    (augmented_messages, retrieved_sources, retrieval_metrics)
}

/// OpenAI-compatible chat completion endpoint (`POST /v1/chat/completions`)
pub async fn openai_chat_handler(
    State(state): State<AppState>,
    Json(req): Json<OpenAiChatRequest>,
) -> impl IntoResponse {
    let t_start = std::time::Instant::now();
    let (augmented_messages, retrieved_sources, retrieval_metrics) =
        prepare_augmented_chat_context(&state, &req).await;

    let t_llm = std::time::Instant::now();
    match state
        .llm
        .chat_completion(
            &augmented_messages,
            req.model.as_deref(),
            req.temperature,
            req.num_ctx,
            req.think,
        )
        .await
    {
        Ok(answer) => {
            info!(
                "LLM response generated in {:.2}s (Total: {:.2}s)",
                t_llm.elapsed().as_secs_f64(),
                t_start.elapsed().as_secs_f64()
            );
            let response_payload = json!({
                "id": format!("chatcmpl-{}", Uuid::new_v4()),
                "object": "chat.completion",
                "created": 1730000000,
                "model": req.model.unwrap_or_else(|| state.config.llm_model.clone()),
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": answer },
                    "finish_reason": "stop"
                }],
                "rag_sources": retrieved_sources,
                "rag_metrics": retrieval_metrics
            });
            (StatusCode::OK, Json(response_payload))
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": format!("LLM invocation failed: {}", e) })),
        ),
    }
}

/// Real-time streaming endpoint (`POST /v1/chat/stream`) for Leptos WASM UI
pub async fn chat_stream_handler(
    State(state): State<AppState>,
    Json(req): Json<OpenAiChatRequest>,
) -> Response {
    let t_start = std::time::Instant::now();
    let (augmented_messages, retrieved_sources, retrieval_metrics) =
        prepare_augmented_chat_context(&state, &req).await;

    let t_llm_start = std::time::Instant::now();
    let mut token_rx = state
        .llm
        .chat_completion_stream(
            augmented_messages,
            req.model,
            req.temperature,
            req.num_ctx,
            req.think,
        )
        .await;

    let (body_tx, body_rx) =
        tokio::sync::mpsc::channel::<Result<String, std::convert::Infallible>>(64);

    tokio::spawn(async move {
        let sources_line = format!(
            "{}\n",
            json!({
                "type": "sources",
                "sources": retrieved_sources,
                "metrics": retrieval_metrics
            })
        );
        if body_tx.send(Ok(sources_line)).await.is_err() {
            return;
        }

        let mut first_token_ms: Option<u64> = None;
        while let Some(res) = token_rx.recv().await {
            if first_token_ms.is_none() {
                first_token_ms = Some(t_llm_start.elapsed().as_millis() as u64);
            }
            let line = match res {
                Ok(tok) => format!("{}\n", json!({ "type": "token", "content": tok })),
                Err(e) => format!("{}\n", json!({ "type": "error", "error": e.to_string() })),
            };
            if body_tx.send(Ok(line)).await.is_err() {
                return;
            }
        }

        let done_line = format!(
            "{}\n",
            json!({
                "type": "done",
                "llm_first_token_ms": first_token_ms.unwrap_or(0),
                "llm_total_ms": t_llm_start.elapsed().as_millis() as u64,
                "total_ms": t_start.elapsed().as_millis() as u64
            })
        );
        let _ = body_tx.send(Ok(done_line)).await;
    });

    let stream = futures_util::stream::unfold(body_rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .unwrap()
}
