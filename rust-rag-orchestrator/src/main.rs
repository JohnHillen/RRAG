pub mod api;
pub mod config;
pub mod db;
pub mod ingestion;
pub mod llm;
pub mod retrieval;

pub use api::{chat, mcp, showcases};
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
use showcases::ActiveShowcase;
use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: SurrealClient,
    pub external_db: Option<db::external::ExternalDbClient>,
    pub inference: InferenceClient,
    pub llm: LlmClient,
    pub active_showcase: Arc<RwLock<ActiveShowcase>>,
}

impl AppState {
    pub fn get_active_showcase(&self) -> ActiveShowcase {
        self.active_showcase
            .read()
            .map(|g| *g)
            .unwrap_or(ActiveShowcase::None)
    }

    pub fn set_active_showcase(&self, sc: ActiveShowcase) {
        if let Ok(mut g) = self.active_showcase.write() {
            *g = sc;
        }
    }

    pub fn set_active_showcase_if_matches(&self, expected: ActiveShowcase, next: ActiveShowcase) {
        if let Ok(mut g) = self.active_showcase.write() {
            if *g == expected {
                *g = next;
            }
        }
    }

    /// Returns the attached external SurrealDB client (or an isolated `webstore_external / shop_erp` SurrealClient)
    /// for Showcase 03 (Webstore Multi-DB).
    pub fn webstore_external_db(&self) -> SurrealClient {
        if let Some(db::external::ExternalDbClient::Surreal(ref ext)) = self.external_db {
            return ext.clone();
        }
        let mut ext_cfg = self.config.clone();
        ext_cfg.surreal_ns = "webstore_external".into();
        ext_cfg.surreal_db = "shop_erp".into();
        SurrealClient::new(self.db.http_client(), ext_cfg)
    }
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
        .timeout(Duration::from_secs(120))
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
        active_showcase: Arc::new(RwLock::new(ActiveShowcase::None)),
    };

    // Restore persisted system LLM settings from SurrealDB & check if an existing showcase is already in DB
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
                break;
            }
            if state_clone.db.execute_sql("INFO FOR DB;").await.is_ok() {
                showcases::detect_active_showcase_on_startup(&state_clone).await;
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
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
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
