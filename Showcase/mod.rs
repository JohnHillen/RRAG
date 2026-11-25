#![allow(unused_variables, unused_mut, unused_imports)]

#[cfg(feature = "showcase-01")]
#[path = "Showcase_01_HR_Consulting/src/mod.rs"]
pub mod showcase_01_hr;

#[cfg(feature = "showcase-02")]
#[path = "Showcase_02_Spedition/src/mod.rs"]
pub mod showcase_02_spedition;

#[cfg(feature = "showcase-03")]
#[path = "Showcase_03_Webstore/src/mod.rs"]
pub mod showcase_03_webstore;

use crate::{
    db::{RetrievedChunk, SurrealClient},
    mcp::JsonRpcResponse,
    AppState,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde_json::{json, Value};
use std::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveShowcase {
    None,
    HrConsulting,
    Spedition,
    Webstore,
}

static ACTIVE_SHOWCASE: RwLock<ActiveShowcase> = RwLock::new(ActiveShowcase::None);

impl AppState {
    pub fn get_active_showcase(&self) -> ActiveShowcase {
        ACTIVE_SHOWCASE
            .read()
            .map(|g| *g)
            .unwrap_or(ActiveShowcase::None)
    }

    pub fn set_active_showcase(&self, sc: ActiveShowcase) {
        if let Ok(mut g) = ACTIVE_SHOWCASE.write() {
            *g = sc;
        }
    }

    pub fn set_active_showcase_if_matches(&self, expected: ActiveShowcase, next: ActiveShowcase) {
        if let Ok(mut g) = ACTIVE_SHOWCASE.write() {
            if *g == expected {
                *g = next;
            }
        }
    }

    /// Returns the isolated `webstore_external / shop_erp` SurrealClient for Showcase 03 (Webstore Multi-DB).
    pub fn webstore_external_db(&self) -> SurrealClient {
        if let Some(crate::db::external::ExternalDbClient::Surreal(ref ext)) = self.external_db {
            return ext.clone();
        }
        let mut ext_cfg = self.config.clone();
        ext_cfg.surreal_ns = "webstore_external".into();
        ext_cfg.surreal_db = "shop_erp".into();
        SurrealClient::new(self.db.http_client(), ext_cfg)
    }
}

/// Mounts showcase HTTP endpoints and `/v1/showcase/reset`.
pub fn router() -> Router<AppState> {
    let mut r = Router::new().route("/v1/showcase/reset", post(reset_showcase_handler));

    #[cfg(feature = "showcase-01")]
    {
        r = r.merge(showcase_01_hr::router());
    }

    #[cfg(feature = "showcase-02")]
    {
        r = r.merge(showcase_02_spedition::router());
    }

    #[cfg(feature = "showcase-03")]
    {
        r = r.merge(showcase_03_webstore::router());
    }

    r
}

/// Completely resets all showcase documents, graphs, and showcase SQL tables in both internal and external DBs (`POST /v1/showcase/reset`),
/// returning the orchestrator to `ActiveShowcase::None`.
async fn reset_showcase_handler(State(state): State<AppState>) -> impl IntoResponse {
    let ext_db = state.webstore_external_db();
    let _ = ext_db
        .execute_sql("DELETE shop_product; DELETE shop_sale;")
        .await;
    match state
        .db
        .execute_sql(
            "DELETE mentions; DELETE relates_to; DELETE entity; DELETE chunk; DELETE document; DELETE employee_record; DELETE fleet_vehicle; DELETE fleet_tour; DELETE fleet_warehouse; DELETE internal_contract_ledger;",
        )
        .await
    {
        Ok(_) => {
            state.set_active_showcase(ActiveShowcase::None);
            (StatusCode::OK, Json(json!({ "status": "reset_complete" })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[cfg(any(
    feature = "showcase-01",
    feature = "showcase-02",
    feature = "showcase-03"
))]
async fn count_table_rows_on(client: &SurrealClient, table: &str) -> usize {
    let sql = format!("SELECT count() AS cnt FROM {} GROUP ALL;", table);
    client
        .execute_sql(&sql)
        .await
        .ok()
        .and_then(|res| {
            res.first()
                .and_then(|v| v.get("result"))
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
                .and_then(|o| o.get("cnt"))
                .and_then(|c| c.as_u64())
        })
        .unwrap_or(0) as usize
}

/// Checks once at startup whether SurrealDB already holds persisted data from a previously loaded showcase
/// WITHOUT creating any showcase schemas or tables when no showcase is loaded.
pub async fn detect_active_showcase_on_startup(state: &AppState) {
    #[cfg(feature = "showcase-01")]
    {
        let cnt = count_table_rows_on(&state.db, "employee_record").await;
        if cnt > 0 {
            state.set_active_showcase(ActiveShowcase::HrConsulting);
            info!(
                "Detected persisted Showcase 01 (HR Consulting) in SurrealDB ({} employees).",
                cnt
            );
            return;
        }
    }

    #[cfg(feature = "showcase-02")]
    {
        let cnt = count_table_rows_on(&state.db, "fleet_vehicle").await;
        if cnt > 0 {
            state.set_active_showcase(ActiveShowcase::Spedition);
            info!(
                "Detected persisted Showcase 02 (Spedition) in SurrealDB ({} vehicles).",
                cnt
            );
            return;
        }
    }

    #[cfg(feature = "showcase-03")]
    {
        let ext_db = state.webstore_external_db();
        let cnt = count_table_rows_on(&ext_db, "shop_product").await;
        if cnt > 0 {
            state.set_active_showcase(ActiveShowcase::Webstore);
            info!(
                "Detected persisted Showcase 03 (Webstore Multi-DB) in SurrealDB ({} products in external DB).",
                cnt
            );
            return;
        }
    }

    state.set_active_showcase(ActiveShowcase::None);
}

/// Extracts Hop-2 bridge terms only if the active showcase provides them.
pub fn extract_bridge_terms_if_active(
    state: &AppState,
    anchor_text: &str,
    original_query_lower: &str,
) -> Vec<String> {
    match state.get_active_showcase() {
        #[cfg(feature = "showcase-01")]
        ActiveShowcase::HrConsulting => {
            showcase_01_hr::extract_bridge_terms(anchor_text, original_query_lower)
        }
        _ => Vec::new(),
    }
}

/// Injects showcase-specific Hop-2 expansion terms only if the corresponding showcase is active.
pub fn expand_hop2_terms_if_active(
    state: &AppState,
    query_lower: &str,
    expansion_terms: &mut Vec<String>,
) {
    #[cfg(feature = "showcase-01")]
    if state.get_active_showcase() == ActiveShowcase::HrConsulting
        && showcase_01_hr::is_employee_lookup(query_lower)
    {
        expansion_terms.push("Mitarbeiterprofil".to_string());
    }
}

/// Runs showcase-specific retrieval augmentation ONLY for the currently active showcase.
pub async fn augment_for_active_showcase(
    state: &AppState,
    query: &str,
    mut ranked_pool: Vec<RetrievedChunk>,
    top_n: usize,
    db_ms: &mut u64,
    reranker_used: &mut String,
) -> Vec<RetrievedChunk> {
    match state.get_active_showcase() {
        #[cfg(feature = "showcase-01")]
        ActiveShowcase::HrConsulting => {
            showcase_01_hr::augment_retrieval(
                state,
                query,
                ranked_pool,
                top_n,
                db_ms,
                reranker_used,
            )
            .await
        }
        #[cfg(feature = "showcase-02")]
        ActiveShowcase::Spedition => {
            showcase_02_spedition::augment_retrieval(
                state,
                query,
                ranked_pool,
                top_n,
                db_ms,
                reranker_used,
            )
            .await
        }
        #[cfg(feature = "showcase-03")]
        ActiveShowcase::Webstore => {
            showcase_03_webstore::augment_retrieval(
                state,
                query,
                ranked_pool,
                top_n,
                db_ms,
                reranker_used,
            )
            .await
        }
        _ => {
            ranked_pool.truncate(top_n);
            ranked_pool
        }
    }
}

/// Appends MCP tool definitions only for the currently active showcase.
pub fn append_active_mcp_tools(state: &AppState, tools: &mut Vec<Value>) {
    match state.get_active_showcase() {
        #[cfg(feature = "showcase-01")]
        ActiveShowcase::HrConsulting => {
            tools.push(showcase_01_hr::mcp::tool_definition());
        }
        #[cfg(feature = "showcase-02")]
        ActiveShowcase::Spedition => {
            tools.push(showcase_02_spedition::mcp::tool_definition());
        }
        #[cfg(feature = "showcase-03")]
        ActiveShowcase::Webstore => {
            tools.push(showcase_03_webstore::mcp::tool_definition());
        }
        _ => {}
    }
}

/// Dispatches MCP tool calls for showcase tools to their respective showcase folder modules.
pub async fn try_handle_mcp_tool_call(
    state: &AppState,
    tool_name: &str,
    id: Option<Value>,
    args: &Value,
) -> Option<JsonRpcResponse> {
    #[cfg(feature = "showcase-01")]
    if tool_name == showcase_01_hr::mcp::TOOL_NAME {
        return Some(showcase_01_hr::mcp::handle_tool_call(state, id, args).await);
    }

    #[cfg(feature = "showcase-02")]
    if tool_name == showcase_02_spedition::mcp::TOOL_NAME {
        return Some(showcase_02_spedition::mcp::handle_tool_call(state, id, args).await);
    }

    #[cfg(feature = "showcase-03")]
    if tool_name == showcase_03_webstore::mcp::TOOL_NAME {
        return Some(showcase_03_webstore::mcp::handle_tool_call(state, id, args).await);
    }

    None
}
