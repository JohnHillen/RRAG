pub mod api;
pub mod config;
pub mod db;
pub mod ingestion;
pub mod llm;
pub mod retrieval;

#[path = "../../Showcase/mod.rs"]
pub mod showcases;

pub use api::{chat, mcp, runpod};
pub use ingestion::{attachments, documents, pdf};
pub use retrieval::{execute_retrieval_pipeline, sql_formatter, text_to_sql};

use anyhow::Result;
use axum::{
    extract::{DefaultBodyLimit, State},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use config::AppConfig;
use db::SurrealClient;
use ingestion::InferenceClient;
use llm::LlmClient;
use reqwest::Client;
use serde_json::json;
use std::{net::SocketAddr, time::Duration};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: SurrealClient,
    pub external_db: Option<db::external::ExternalDbClient>,
    pub inference: InferenceClient,
    pub llm: LlmClient,
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
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()?;
    let llm_http = Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(900))
        .build()?;

    let external_db = db::external::ExternalDbClient::from_config(http.clone(), &config).await;

    let state = AppState {
        db: SurrealClient::new(http.clone(), config.clone()),
        external_db,
        inference: InferenceClient::new(http.clone(), config.clone()),
        llm: LlmClient::new(llm_http, config.clone()),
        config: config.clone(),
    };

    // Restore persisted system LLM settings from SurrealDB
    let state_clone = state.clone();
    tokio::spawn(async move {
        for _ in 0..15 {
            if let Ok(Some((prov, url, key, model))) = state_clone.db.get_system_llm_settings().await
            {
                state_clone
                    .llm
                    .update_runtime_config(prov.clone(), url.clone(), key, model.clone());
                info!(
                    "Restored persisted LLM configuration from SurrealDB -> Provider: {}, Model: {}",
                    prov, model
                );
                showcases::detect_active_showcase_on_startup(&state_clone).await;
                runpod::restore_runpod_settings_from_db(&state_clone).await;
                break;
            }
            if state_clone.db.execute_sql("INFO FOR DB;").await.is_ok() {
                showcases::detect_active_showcase_on_startup(&state_clone).await;
                runpod::restore_runpod_settings_from_db(&state_clone).await;
                break;
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/models", get(chat::list_models_handler))
        .route(
            "/v1/llm/config",
            get(chat::get_llm_config_handler).post(chat::update_llm_config_handler),
        )
        .route("/v1/llm/models", post(chat::discover_llm_models_handler))
        .route("/v1/runpod/status", get(runpod::get_runpod_status_handler))
        .route("/v1/runpod/config", post(runpod::save_runpod_config_handler))
        .route("/v1/runpod/start", post(runpod::start_runpod_handler))
        .route("/v1/runpod/stop", post(runpod::stop_runpod_handler))
        .route("/v1/db/export", get(documents::export_surreal_db_handler))
        .route("/v1/db/import", post(documents::import_surreal_db_handler))
        .route("/v1/documents", get(documents::list_documents_handler))
        .route(
            "/v1/documents/deduplicate",
            post(documents::deduplicate_documents_handler),
        )
        .route(
            "/v1/documents/:doc_id",
            delete(documents::delete_document_handler),
        )
        .route(
            "/v1/documents/:doc_id/file",
            get(pdf::get_document_file_handler),
        )
        .route("/v1/ingest", post(documents::ingest_json_handler))
        .route("/v1/ingest/file", post(documents::ingest_file_handler))
        .route(
            "/v1/extract",
            post(attachments::extract_ephemeral_file_handler),
        )
        .route("/v1/retrieve", post(retrieval::retrieve_handler))
        .route("/v1/chat/completions", post(chat::openai_chat_handler))
        .route("/v1/chat/stream", post(chat::chat_stream_handler))
        .route("/mcp", post(mcp::handle_mcp_request))
        .merge(showcases::router())
        .fallback_service(ServeDir::new("./static"))
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
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
