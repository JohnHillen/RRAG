mod config;
mod db;
mod ingestion;
mod llm;
mod mcp;

use anyhow::Result;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use config::AppConfig;
use db::{RetrievedChunk, SurrealClient};
use ingestion::InferenceClient;
use llm::{ChatMessage, LlmClient, LlmRuntimeConfig};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{net::SocketAddr, time::Duration};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: SurrealClient,
    pub inference: InferenceClient,
    pub llm: LlmClient,
}

// --- API Request / Response Payloads ---

#[derive(Debug, Deserialize)]
pub struct IngestJsonRequest {
    pub title: String,
    pub text: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub replace_revision: bool,
    #[serde(default)]
    pub extract_graph: Option<bool>,
}

fn default_source() -> String {
    "api_upload".to_string()
}

#[derive(Debug, Deserialize)]
pub struct RetrieveRequest {
    pub query: String,
    pub top_n: Option<usize>,
    pub reranker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachment {
    pub filename: String,
    pub text: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMetrics {
    pub embed_ms: u64,
    pub db_ms: u64,
    pub rerank_ms: u64,
    pub total_ms: u64,
    pub reranker_used: String,
}

#[derive(Debug, Serialize)]
pub struct RetrieveResponse {
    pub query: String,
    pub count: usize,
    pub results: Vec<RetrievedChunk>,
    pub metrics: RetrievalMetrics,
    pub formatted_context: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfigUpdateRequest {
    pub llm_provider: String,
    pub llm_base_url: String,
    #[serde(default)]
    pub llm_api_key: Option<String>,
    pub llm_model: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = AppConfig::from_env();
    let http = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    let state = AppState {
        db: SurrealClient::new(http.clone(), config.clone()),
        inference: InferenceClient::new(http.clone(), config.clone()),
        llm: LlmClient::new(http, config.clone()),
        config: config.clone(),
    };

    // Load persisted system LLM settings from SurrealDB (survives restarts & setup.sh)
    let db_clone = state.db.clone();
    let llm_clone = state.llm.clone();
    tokio::spawn(async move {
        // Wait briefly for SurrealDB to finish initial container healthcheck if starting concurrently
        for _ in 0..15 {
            if let Ok(Some((prov, url, key, model))) = db_clone.get_system_llm_settings().await {
                llm_clone.update_runtime_config(prov.clone(), url.clone(), key, model.clone());
                info!(
                    "Restored persisted LLM configuration from SurrealDB -> Provider: {}, Model: {}",
                    prov, model
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/models", get(list_models_handler))
        .route(
            "/v1/llm/config",
            get(get_llm_config_handler).post(update_llm_config_handler),
        )
        .route("/v1/llm/models", post(discover_llm_models_handler))
        .route("/v1/documents", get(list_documents_handler))
        .route("/v1/documents/deduplicate", post(deduplicate_documents_handler))
        .route("/v1/documents/:doc_id", delete(delete_document_handler))
        .route("/v1/ingest", post(ingest_json_handler))
        .route("/v1/ingest/file", post(ingest_file_handler))
        .route("/v1/extract", post(extract_ephemeral_file_handler))
        .route("/v1/retrieve", post(retrieve_handler))
        .route("/v1/chat/completions", post(openai_chat_handler))
        .route("/v1/chat/stream", post(chat_stream_handler))
        .route("/mcp", post(mcp::handle_mcp_request))
        .fallback_service(ServeDir::new("./static"))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // Support up to 50 MB uploads
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.orchestrator_port));
    info!(
        "Rust RAG Orchestrator started on http://{} (LLM Provider: {}, Model: {})",
        addr, config.llm_provider, config.llm_model
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Health check endpoint (`/health`)
async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let llm_cfg = state.llm.get_runtime_config();
    Json(json!({
        "status": "ok",
        "service": "rust-rag-orchestrator",
        "surreal_url": state.config.surreal_url,
        "llm_provider": llm_cfg.llm_provider,
        "llm_base_url": llm_cfg.llm_base_url,
        "llm_model": llm_cfg.llm_model,
    }))
}

/// Returns currently active LLM configuration and discovered models (`GET /v1/llm/config`)
async fn get_llm_config_handler(State(state): State<AppState>) -> impl IntoResponse {
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
async fn update_llm_config_handler(
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

    // Persist to SurrealDB so the user's selected model survives restarts & setup.sh
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
async fn discover_llm_models_handler(
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
    Json(json!({
        "available_models": models
    }))
}

/// OpenAI-compatible model list (`GET /v1/models`)
async fn list_models_handler(State(state): State<AppState>) -> impl IntoResponse {
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

/// Lists all indexed documents in SurrealDB (`GET /v1/documents`)
async fn list_documents_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_documents().await {
        Ok(docs) => (StatusCode::OK, Json(json!({ "documents": docs }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Deletes a document and its associated chunks (`DELETE /v1/documents/:doc_id`)
async fn delete_document_handler(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_document(&doc_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "status": "deleted", "doc_id": doc_id })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Cleans up older duplicate document revisions (`POST /v1/documents/deduplicate`)
async fn deduplicate_documents_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.deduplicate_by_title().await {
        Ok(removed) => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "removed_documents": removed })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Ingests raw text/markdown payload (`POST /v1/ingest`)
async fn ingest_json_handler(
    State(state): State<AppState>,
    Json(req): Json<IngestJsonRequest>,
) -> impl IntoResponse {
    let full_title = format_title_with_category(&req.title, req.category.as_deref());
    match process_document_ingestion(
        &state,
        None,
        &full_title,
        &req.source,
        &req.text,
        req.replace_revision,
        req.category.as_deref(),
        req.extract_graph,
    )
    .await
    {
        Ok(val) => (StatusCode::OK, Json(val)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Multipart file upload handler (PDF, HTML, Markdown, TXT) (`POST /v1/ingest/file`)
async fn ingest_file_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut category: Option<String> = None;
    let mut replace_revision = true;
    let mut extract_graph: Option<bool> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name == "category" {
            if let Ok(txt) = field.text().await {
                let trimmed = txt.trim().to_string();
                if !trimmed.is_empty() {
                    category = Some(trimmed);
                }
            }
            continue;
        }
        if field_name == "replace_revision" {
            if let Ok(txt) = field.text().await {
                replace_revision = txt.trim() != "false" && txt.trim() != "0";
            }
            continue;
        }
        if field_name == "extract_graph" {
            if let Ok(txt) = field.text().await {
                let t = txt.trim();
                extract_graph = Some(t == "true" || t == "1");
            }
            continue;
        }

        let filename = field
            .file_name()
            .unwrap_or("uploaded_document.txt")
            .to_string();
        if let Ok(bytes) = field.bytes().await {
            if !bytes.is_empty() {
                uploaded_files.push((filename, bytes.to_vec()));
            }
        }
    }

    if uploaded_files.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No files found in multipart request." })),
        );
    }

    let mut total_chunks = 0usize;
    let mut total_graph_entities = 0usize;
    let mut total_graph_relations = 0usize;
    let mut skipped_duplicates = 0usize;
    let mut replaced_revisions = 0usize;
    let mut processed_docs = Vec::new();
    let mut errors = Vec::new();

    for (raw_filename, file_bytes) in uploaded_files {
        let full_title = format_title_with_category(&raw_filename, category.as_deref());

        // 1. Content hash ID of raw file
        let file_hash_id = db::content_hash_id(&file_bytes);

        // Pre-check if this exact file is already indexed in SurrealDB
        if extract_graph != Some(true) {
            if let Ok(existing_chunks) = state.db.count_chunks_for_doc(&file_hash_id).await {
                if existing_chunks > 0 {
                    info!(
                        "File '{}' (Hash: {}) already exists with {} chunks in SurrealDB -> skipping.",
                        full_title, file_hash_id, existing_chunks
                    );
                    skipped_duplicates += 1;
                    processed_docs.push(json!({
                        "filename": full_title,
                        "doc_id": file_hash_id,
                        "chunks_indexed": existing_chunks,
                        "skipped_duplicate": true,
                        "replaced_revision": false
                    }));
                    continue;
                }
            }
        }

        let fname_clone = raw_filename.clone();
        let extraction_res = tokio::task::spawn_blocking(move || {
            InferenceClient::extract_text_from_bytes(&fname_clone, &file_bytes)
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!("Text extraction task error: {}", e)));

        match extraction_res {
            Ok(extracted_text) => {
                match process_document_ingestion(
                    &state,
                    Some(file_hash_id),
                    &full_title,
                    "file_upload",
                    &extracted_text,
                    replace_revision,
                    category.as_deref(),
                    extract_graph,
                )
                .await
                {
                    Ok(val) => {
                        let is_dup = val["skipped_duplicate"].as_bool().unwrap_or(false);
                        let is_rev = val["replaced_revision"].as_bool().unwrap_or(false);
                        let chunks = val["chunks_indexed"].as_u64().unwrap_or(0) as usize;
                        let g_ent = val["graph_entities_extracted"].as_u64().unwrap_or(0) as usize;
                        let g_rel = val["graph_relations_extracted"].as_u64().unwrap_or(0) as usize;
                        total_graph_entities += g_ent;
                        total_graph_relations += g_rel;
                        if is_dup {
                            skipped_duplicates += 1;
                        } else {
                            total_chunks += chunks;
                        }
                        if is_rev {
                            replaced_revisions += 1;
                        }
                        processed_docs.push(json!({
                            "filename": full_title,
                            "chunks_indexed": chunks,
                            "graph_entities_extracted": g_ent,
                            "graph_relations_extracted": g_rel,
                            "skipped_duplicate": is_dup,
                            "replaced_revision": is_rev
                        }));
                    }
                    Err(e) => errors.push(format!("{}: {}", full_title, e)),
                }
            }
            Err(e) => errors.push(format!("{} (Text extraction): {}", full_title, e)),
        }
    }

    if processed_docs.is_empty() && !errors.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": errors.join(" | ") })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "status": "indexed",
            "files_processed": processed_docs.len(),
            "skipped_duplicates": skipped_duplicates,
            "replaced_revisions": replaced_revisions,
            "chunks_indexed": total_chunks,
            "graph_entities_extracted": total_graph_entities,
            "graph_relations_extracted": total_graph_relations,
            "documents": processed_docs,
            "errors": errors
        })),
    )
}

fn format_title_with_category(title: &str, category: Option<&str>) -> String {
    match category.map(|c| c.trim()).filter(|c| !c.is_empty()) {
        Some(cat) => format!("[{}] {}", cat, title.trim()),
        None => title.trim().to_string(),
    }
}

/// In-memory text extraction for ephemeral chat attachments without storing into SurrealDB (`POST /v1/extract`).
async fn extract_ephemeral_file_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded_file: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = field
            .file_name()
            .unwrap_or("attached_file.pdf")
            .to_string();
        if let Ok(bytes) = field.bytes().await {
            if !bytes.is_empty() {
                uploaded_file = Some((filename, bytes.to_vec()));
                break;
            }
        }
    }

    let Some((filename, file_bytes)) = uploaded_file else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No file found in multipart request." })),
        );
    };

    let t_extract = std::time::Instant::now();
    let fname_clone = filename.clone();
    let extraction_res = tokio::task::spawn_blocking(move || {
        InferenceClient::extract_text_from_bytes(&fname_clone, &file_bytes)
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("Text extraction task error: {}", e)));

    match extraction_res {
        Ok(text) => {
            let trimmed = text.trim().to_string();
            let chunks = state.inference.semantic_chunk_text(&trimmed);
            let extract_ms = t_extract.elapsed().as_millis() as u64;
            info!(
                "Ephemeral chat attachment '{}' extracted in memory ({} chars, {} chunks in {}ms).",
                filename,
                trimmed.chars().count(),
                chunks.len(),
                extract_ms
            );
            (
                StatusCode::OK,
                Json(json!({
                    "status": "extracted_in_memory",
                    "filename": filename,
                    "char_count": trimmed.chars().count(),
                    "chunk_count": chunks.len(),
                    "extract_ms": extract_ms,
                    "text": trimmed
                })),
            )
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": format!("{}: {}", filename, e) })),
        ),
    }
}

/// Prepares chunks from an in-memory chat attachment for querying.
async fn prepare_ephemeral_attachment_chunks(
    state: &AppState,
    query: &str,
    attachment: &ChatAttachment,
    reranker_mode: &str,
) -> (Vec<RetrievedChunk>, u64, String) {
    let t_rerank = std::time::Instant::now();
    let raw_chunks = state.inference.semantic_chunk_text(&attachment.text);
    let doc_label = format!("📎 {} (Ephemeral Chat Attachment)", attachment.filename);

    let all_candidates: Vec<RetrievedChunk> = raw_chunks
        .into_iter()
        .enumerate()
        .map(|(idx, c_text)| RetrievedChunk {
            id: format!("ephemeral_{}", idx),
            doc_id: "ephemeral_attachment".to_string(),
            doc_title: doc_label.clone(),
            chunk_index: idx as i64,
            content: c_text,
            rrf_score: 1.0,
            rerank_score: Some(1.0),
            entities: vec![],
            related_entities: vec![],
            relations_out: vec![],
            relations_in: vec![],
            relations: vec![],
        })
        .collect();

    if all_candidates.len() <= 4 || query.trim().is_empty() {
        return (
            all_candidates,
            0,
            "📎 Full text in RAM (No DB)".to_string(),
        );
    }

    let first_chunk = all_candidates.first().cloned();
    let candidates_for_rerank = if all_candidates.len() > 24 {
        let q_lower = query.to_lowercase();
        let q_terms: Vec<&str> = q_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 3)
            .collect();
        let mut scored: Vec<(usize, RetrievedChunk)> = all_candidates
            .into_iter()
            .map(|c| {
                let c_low = c.content.to_lowercase();
                let hits = q_terms.iter().filter(|&&t| c_low.contains(t)).count();
                let bonus = if c.chunk_index < 2 { 2 } else { 0 };
                (hits + bonus, c)
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.chunk_index.cmp(&b.1.chunk_index)));
        scored.into_iter().take(20).map(|(_, c)| c).collect()
    } else {
        all_candidates
    };

    let mode = if reranker_mode.eq_ignore_ascii_case("none") {
        "fast"
    } else {
        reranker_mode
    };

    match state
        .inference
        .rerank_chunks(query, candidates_for_rerank.clone(), 4, mode)
        .await
    {
        Ok((mut top_chunks, model_label)) => {
            if let Some(first) = first_chunk {
                if !top_chunks.iter().any(|c| c.chunk_index == 0) {
                    top_chunks.insert(0, first);
                    if top_chunks.len() > 5 {
                        top_chunks.truncate(5);
                    }
                }
            }
            let elapsed = t_rerank.elapsed().as_millis() as u64;
            (
                top_chunks,
                elapsed,
                format!("📎 In-Memory Rerank ({})", model_label),
            )
        }
        Err(_) => {
            let mut fallback = candidates_for_rerank;
            fallback.truncate(4);
            (
                fallback,
                t_rerank.elapsed().as_millis() as u64,
                "📎 In-Memory Selection".to_string(),
            )
        }
    }
}

/// Standalone Retrieval API (`POST /v1/retrieve`)
async fn retrieve_handler(
    State(state): State<AppState>,
    Json(req): Json<RetrieveRequest>,
) -> impl IntoResponse {
    let top_n = req.top_n.unwrap_or(state.config.retrieval_final_top_n);
    let reranker_mode = req.reranker.as_deref().unwrap_or("fast");
    match execute_retrieval_pipeline(&state, &req.query, top_n, reranker_mode).await {
        Ok((results, metrics)) => {
            let formatted_context = LlmClient::format_rag_context(&results);
            let resp = RetrieveResponse {
                query: req.query,
                count: results.len(),
                results,
                metrics,
                formatted_context,
            };
            (StatusCode::OK, Json(json!(resp)))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// OpenAI-compatible chat completion endpoint (`POST /v1/chat/completions`)
async fn openai_chat_handler(
    State(state): State<AppState>,
    Json(req): Json<OpenAiChatRequest>,
) -> impl IntoResponse {
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
    let t_start = std::time::Instant::now();
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
                prepare_ephemeral_attachment_chunks(&state, &last_user_query, att, reranker_mode)
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
        match execute_retrieval_pipeline(&state, &retrieval_query, top_n, reranker_mode).await {
            Ok((chunks, mut metrics)) => {
                metrics.rerank_ms += attachment_rerank_ms;
                metrics.total_ms += attachment_rerank_ms;
                if !attachment_label.is_empty() {
                    metrics.reranker_used = format!("{} + {}", attachment_label, metrics.reranker_used);
                }
                retrieved_sources.extend(chunks);
                retrieval_metrics = Some(metrics);
            }
            Err(e) => {
                warn!("RAG retrieval failed, continuing without database context: {}", e);
            }
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
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": answer
                        },
                        "finish_reason": "stop"
                    }
                ],
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
async fn chat_stream_handler(
    State(state): State<AppState>,
    Json(req): Json<OpenAiChatRequest>,
) -> Response {
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
    let t_start = std::time::Instant::now();
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
                prepare_ephemeral_attachment_chunks(&state, &last_user_query, att, reranker_mode)
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
        if let Ok((chunks, mut metrics)) =
            execute_retrieval_pipeline(&state, &retrieval_query, top_n, reranker_mode).await
        {
            info!(
                "RAG stream retrieval finished in {}ms (Embed={}ms, DB={}ms, Rerank={}ms via {})",
                metrics.total_ms,
                metrics.embed_ms,
                metrics.db_ms,
                metrics.rerank_ms,
                metrics.reranker_used
            );
            metrics.rerank_ms += attachment_rerank_ms;
            metrics.total_ms += attachment_rerank_ms;
            if !attachment_label.is_empty() {
                metrics.reranker_used = format!("{} + {}", attachment_label, metrics.reranker_used);
            }
            retrieved_sources.extend(chunks);
            retrieval_metrics = Some(metrics);
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

    let (body_tx, body_rx) = tokio::sync::mpsc::channel::<Result<String, std::convert::Infallible>>(64);

    tokio::spawn(async move {
        // 1. Send sources + metrics line
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

        // 2. Stream generated tokens
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

        // 3. Send final done metrics
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

/// Shared document ingestion logic (content-hash check -> revision replacement -> chunking -> embedding -> SurrealDB)
async fn process_document_ingestion(
    state: &AppState,
    explicit_doc_id: Option<String>,
    title: &str,
    source: &str,
    text: &str,
    replace_revision: bool,
    category: Option<&str>,
    extract_graph: Option<bool>,
) -> Result<Value> {
    let text_hash_id = db::content_hash_id(text.trim().as_bytes());
    let doc_id = explicit_doc_id.unwrap_or_else(|| text_hash_id.clone());

    if extract_graph != Some(true) {
        if let Ok(existing) = state.db.count_chunks_for_doc(&doc_id).await {
            if existing > 0 {
                return Ok(json!({
                    "status": "skipped_duplicate",
                    "skipped_duplicate": true,
                    "replaced_revision": false,
                    "doc_id": doc_id,
                    "title": title,
                    "chunks_indexed": existing,
                    "graph_entities_extracted": 0,
                    "graph_relations_extracted": 0
                }));
            }
        }
    }

    let chunks_text = state.inference.semantic_chunk_text(text);
    if chunks_text.is_empty() {
        anyhow::bail!("Document does not contain any usable text.");
    }

    // Prepend document title to each chunk for dense embedding representation
    let texts_for_embedding: Vec<String> = chunks_text
        .iter()
        .map(|c| format!("[Document: {}]\n{}", title, c))
        .collect();
    let embeddings = state.inference.embed_texts(&texts_for_embedding).await?;
    let mut indexed_chunks = Vec::with_capacity(chunks_text.len());
    for (idx, (chunk_str, emb)) in chunks_text.into_iter().zip(embeddings.into_iter()).enumerate()
    {
        indexed_chunks.push((idx, chunk_str, emb));
    }

    let (mut entities, mut relations) = state.llm.extract_graph_entities(text, extract_graph).await?;

    if let Some(cat) = category.map(|c| c.trim()).filter(|c| !c.is_empty()) {
        if extract_graph.unwrap_or(state.config.enable_graph_extraction) {
            for (ent_name, _, _) in &entities {
                if !ent_name.eq_ignore_ascii_case(cat) {
                    relations.push((
                        ent_name.clone(),
                        "belongs_to_project".to_string(),
                        cat.to_string(),
                    ));
                }
            }
            entities.push((
                cat.to_string(),
                "Project/Folder".to_string(),
                Some(format!("Parent Project / Category ({})", cat)),
            ));
        }
    }

    let mut replaced_revision = false;
    if replace_revision {
        if let Ok(replaced) = state.db.replace_older_revisions(title, &doc_id).await {
            if replaced {
                info!(
                    "Older revision of '{}' replaced by new version (Hash: {}).",
                    title, doc_id
                );
                replaced_revision = true;
            }
        }
    }

    state
        .db
        .insert_document_and_chunks(&doc_id, title, source, &indexed_chunks, &entities, &relations)
        .await?;

    Ok(json!({
        "status": "indexed",
        "skipped_duplicate": false,
        "replaced_revision": replaced_revision,
        "doc_id": doc_id,
        "title": title,
        "chunks_indexed": indexed_chunks.len(),
        "graph_entities_extracted": entities.len(),
        "graph_relations_extracted": relations.len()
    }))
}

/// Core retrieval pipeline (with automatic 2-Hop Graph & Requirement Expansion):
/// 1. Hop-1: Query embedding via TEI + SurrealDB Hybrid Search (HNSW + BM25 + RRF)
/// 2. Hop-2 (Multi-Hop Expansion):
///    a) Traverses Knowledge Graph entities & relations from Hop-1 top hits (`fetch_graph_hop_chunks`)
///       to pull in connected records (e.g. employees linked to `Project Alpha`, `Rust`, `SQL`).
///    b) Extracts discovered technical requirements/skills from Hop-1 anchor documents and runs a
///       targeted 2nd-hop hybrid search if new criteria were uncovered in the anchor documents.
/// 3. Joint Cross-Encoder reranking (`fast`, `high`, or `none`) with an enriched multi-hop query.
pub async fn execute_retrieval_pipeline(
    state: &AppState,
    query: &str,
    top_n: usize,
    reranker_mode: &str,
) -> Result<(Vec<RetrievedChunk>, RetrievalMetrics)> {
    let t_total = std::time::Instant::now();

    let t_embed = std::time::Instant::now();
    let query_vecs = state.inference.embed_texts(&[query.to_string()]).await?;
    let query_embedding = query_vecs
        .first()
        .ok_or_else(|| anyhow::anyhow!("No query embedding received from TEI server"))?;
    let mut embed_ms = t_embed.elapsed().as_millis() as u64;

    let t_db = std::time::Instant::now();
    let mut candidates = state
        .db
        .hybrid_graph_search(query, query_embedding, state.config.retrieval_top_k_candidates)
        .await?;

    // --- Hop-2: Automatic Graph-Edge & Requirement Expansion ---
    let mut hop2_entities: Vec<String> = Vec::new();
    let mut hop2_keywords: Vec<String> = Vec::new();
    let query_lower = query.to_lowercase();

    // Inspect the top 3 anchor candidates from Hop-1 (e.g. a customer contract or project spec)
    for anchor in candidates.iter().take(3) {
        for ent in &anchor.entities {
            let name = ent.name.trim();
            // Skip generic folder categories when hopping
            if !name.is_empty()
                && !matches!(name.to_lowercase().as_str(), "hr" | "sales" | "projects" | "general")
                && !hop2_entities.iter().any(|x| x.eq_ignore_ascii_case(name))
            {
                hop2_entities.push(name.to_string());
            }
        }
        for rel in &anchor.relations {
            for endpoint in [&rel.from, &rel.to] {
                let name = endpoint.trim();
                if !name.is_empty()
                    && !matches!(name.to_lowercase().as_str(), "hr" | "sales" | "projects" | "general")
                    && !hop2_entities.iter().any(|x| x.eq_ignore_ascii_case(name))
                {
                    hop2_entities.push(name.to_string());
                }
            }
        }

        // Also scan anchor content for technical skills, certifications, or project names not in the original query
        for kw in extract_bridge_terms_from_anchor(&anchor.content, &query_lower) {
            if !hop2_keywords.iter().any(|x| x.eq_ignore_ascii_case(&kw)) {
                hop2_keywords.push(kw);
            }
        }
    }

    let mut hop2_added = 0usize;
    let mut hop2_triggered = false;
    let existing_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();

    // 2a) Reverse Graph-Hop in SurrealDB: find chunks mentioning or linked to the anchor entities
    if !hop2_entities.is_empty() {
        hop2_triggered = true;
        if let Ok(graph_chunks) = state
            .db
            .fetch_graph_hop_chunks(&hop2_entities, &existing_ids, 10)
            .await
        {
            for gc in graph_chunks {
                if !candidates.iter().any(|c| c.id == gc.id) {
                    candidates.push(gc);
                    hop2_added += 1;
                }
            }
        }
    }

    // 2b) Secondary Hybrid Hop: If Hop-1 anchor documents revealed specific skills/projects
    //     that were not in the user's initial prompt, run a fast 2nd-hop retrieval for those criteria!
    let mut enriched_rerank_query = query.to_string();
    if !hop2_keywords.is_empty() || !hop2_entities.is_empty() {
        let mut expansion_terms: Vec<String> = Vec::new();

        // If the query asks about employees / candidates ("mitarbeiter", "employee", "wer", "who"),
        // explicitly inject "Mitarbeiterprofil" into Hop-2 so candidate profiles outrank unrelated contracts!
        if query_lower.contains("mitarbeiter")
            || query_lower.contains("employee")
            || query_lower.contains("kandidat")
            || query_lower.contains("candidate")
        {
            expansion_terms.push("Mitarbeiterprofil".to_string());
        }

        for k in hop2_keywords.iter().take(6) {
            if !expansion_terms.iter().any(|x| x.eq_ignore_ascii_case(k)) {
                expansion_terms.push(k.clone());
            }
        }
        for e in hop2_entities.iter().take(5) {
            if !query_lower.contains(&e.to_lowercase())
                && !expansion_terms.iter().any(|x| x.eq_ignore_ascii_case(e))
            {
                expansion_terms.push(e.clone());
            }
        }

        if !expansion_terms.is_empty() {
            hop2_triggered = true;
            // Pure Hop-2 target query (focused on the discovered skills/profile criteria)
            let hop2_target_query = expansion_terms.join(" ");
            let sub_query = format!("{} ({})", query, hop2_target_query);
            enriched_rerank_query = sub_query.clone();

            let t_emb2 = std::time::Instant::now();
            if let Ok(vecs2) = state.inference.embed_texts(&[hop2_target_query.clone()]).await {
                embed_ms += t_emb2.elapsed().as_millis() as u64;
                if let Some(emb2) = vecs2.first() {
                    if let Ok(hop2_candidates) = state
                        .db
                        .hybrid_graph_search(&hop2_target_query, emb2, 10)
                        .await
                    {
                        for hc in hop2_candidates {
                            if !candidates.iter().any(|c| c.id == hc.id) {
                                candidates.push(hc);
                                hop2_added += 1;
                            } else if let Some(existing) = candidates.iter_mut().find(|c| c.id == hc.id) {
                                // Boost candidates confirmed by Hop-2 criteria (e.g. matching Rust/SQL)
                                existing.rrf_score += hc.rrf_score * 1.1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Re-sort candidate pool by combined RRF (Hop-1 + Hop-2 boost) for `reranker: none` mode
    candidates.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let db_ms = t_db.elapsed().as_millis() as u64;

    let t_rerank = std::time::Instant::now();
    // Rerank the full candidate pool so we can compose a balanced multi-hop result set
    // (1-2 anchor documents from Hop-1 + top discovered target candidates from Hop-2)
    let pool_len = candidates.len().max(top_n);
    let (mut ranked_pool, mut reranker_used) = state
        .inference
        .rerank_chunks(&enriched_rerank_query, candidates, pool_len, reranker_mode)
        .await?;
    let rerank_ms = t_rerank.elapsed().as_millis() as u64;

    // Multi-Hop Slot Balancing:
    // When the query asks for employees/candidates ("mitarbeiter", "employee", "kandidat") and `top_n >= 4`,
    // ensure the #1 Requirement Anchor (e.g. customer contract) is preserved AND fill the remaining slots
    // with the highest-ranked employee profiles so no candidate is crowded out by secondary project docs.
    let is_employee_lookup = query_lower.contains("mitarbeiter")
        || query_lower.contains("employee")
        || query_lower.contains("kandidat")
        || query_lower.contains("candidate");

    let final_chunks = if is_employee_lookup && top_n >= 4 && ranked_pool.len() > top_n {
        let mut selected: Vec<RetrievedChunk> = Vec::with_capacity(top_n);

        // 1. Keep the #1 anchor document (preferring customer contract/SLA defining mandatory requirements)
        if let Some(first_anchor) = ranked_pool
            .iter()
            .find(|c| {
                let t = c.doc_title.to_lowercase();
                t.contains("vereinbarung") || t.contains("vertrag") || t.contains("sla") || t.contains("[sales]")
            })
            .or_else(|| {
                ranked_pool.iter().find(|c| {
                    !c.doc_title.to_lowercase().contains("mitarbeiter")
                        && !c.content.to_lowercase().contains("mitarbeiterprofil:")
                })
            })
            .or_else(|| ranked_pool.first())
        {
            selected.push(first_anchor.clone());
        }

        // 2. Fill remaining (top_n - 1) slots with the highest-ranked employee/candidate profiles
        for chunk in &ranked_pool {
            if selected.len() >= top_n {
                break;
            }
            let is_profile = chunk.doc_title.to_lowercase().contains("mitarbeiter")
                || chunk.doc_title.to_lowercase().contains("[hr]")
                || chunk.content.to_lowercase().contains("mitarbeiterprofil:");
            if is_profile && !selected.iter().any(|s| s.id == chunk.id) {
                selected.push(chunk.clone());
            }
        }

        // 3. If slots still remain, fill with the next highest ranked chunks overall
        for chunk in &ranked_pool {
            if selected.len() >= top_n {
                break;
            }
            if !selected.iter().any(|s| s.id == chunk.id) {
                selected.push(chunk.clone());
            }
        }

        selected
    } else {
        ranked_pool.truncate(top_n);
        ranked_pool
    };

    if hop2_triggered {
        if hop2_added > 0 {
            reranker_used = format!("{} + 🕸️ 2-Hop (+{} Chunks)", reranker_used, hop2_added);
        } else {
            reranker_used = format!("{} + 🕸️ 2-Hop Query-Expansion", reranker_used);
        }
    }

    let metrics = RetrievalMetrics {
        embed_ms,
        db_ms,
        rerank_ms,
        total_ms: t_total.elapsed().as_millis() as u64,
        reranker_used,
    };

    Ok((final_chunks, metrics))
}

/// Scans Hop-1 anchor chunks (e.g. a contract or project specification) for concrete technical
/// requirements, skills, language levels, or project identifiers that were not part of the user's
/// original question, enabling automatic 2nd-hop candidate discovery across hundreds of profiles.
fn extract_bridge_terms_from_anchor(anchor_text: &str, original_query_lower: &str) -> Vec<String> {
    let mut found = Vec::new();
    let text_lower = anchor_text.to_lowercase();

    // Curated domain/skill & qualification markers + capitalized technical identifiers
    let skill_markers = [
        "rust", "sql", "python", "java", "c++", "golang", "kubernetes", "docker", "terraform",
        "kafka", "postgresql", "surrealdb", "snowflake", "bigquery", "nosql", "mongodb", "redis",
        "b1", "b2", "c1", "c2", "iso/iec 27001", "bsi", "projekt alpha", "project alpha",
        "projekt titan", "project titan", "mitarbeiterprofil",
    ];

    for marker in skill_markers {
        if text_lower.contains(marker) && !original_query_lower.contains(marker) {
            // Check word boundary so short tokens like "b2" or "c1" match cleanly
            let is_clean_match = text_lower
                .split(|c: char| !c.is_alphanumeric() && c != '/' && c != '+')
                .any(|w| w == marker)
                || marker.contains(' ');
            if is_clean_match {
                found.push(marker.to_string());
            }
        }
    }

    found
}

/// Builds a context-aware retrieval query for multi-turn conversations:
/// When a follow-up question (e.g. "Welche Mitarbeiter kommen noch infrage, die nur eine Weiterbildung brauchen?")
/// refers implicitly to entities/topics from the previous turn (e.g. "BMW AG"), this function carries
/// over the subject context so Hop-1 and Hop-2 retrieval stay anchored on the right customer/project.
fn build_conversational_retrieval_query(messages: &[ChatMessage]) -> String {
    let user_msgs: Vec<&str> = messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let Some(&latest) = user_msgs.last() else {
        return String::new();
    };

    if user_msgs.len() < 2 {
        return latest.to_string();
    }

    let prev_user = user_msgs[user_msgs.len() - 2];
    let latest_lower = latest.to_lowercase();

    // If the latest question is short or uses follow-up phrasing ("noch", "weitere", "andere", "dafür", "dort", "er", "sie", "more", "other", "else")
    // or lacks an explicit organization/project name that was present in the previous question, combine them.
    let is_followup = latest.chars().count() < 110
        || [
            "noch", "weitere", "andere", "sonst", "dafür", "dort", "diesem", "dieser", "dieses",
            "weiterbildung", "other", "more", "else", "also",
        ]
        .iter()
        .any(|w| latest_lower.contains(w));

    if is_followup {
        format!("{} (Kontext: {})", latest, prev_user)
    } else {
        latest.to_string()
    }
}
