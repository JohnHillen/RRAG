#![allow(unused_variables, unused_mut, unused_imports)]

#[cfg(feature = "showcase-01")]
#[path = "../../../Showcase/Showcase_01_HR_Consulting/src/mod.rs"]
pub mod showcase_01_hr;

#[cfg(feature = "showcase-02")]
#[path = "../../../Showcase/Showcase_02_Spedition/src/mod.rs"]
pub mod showcase_02_spedition;

#[cfg(feature = "showcase-03")]
#[path = "../../../Showcase/Showcase_03_Webstore/src/mod.rs"]
pub mod showcase_03_webstore;

use crate::{db::RetrievedChunk, mcp::JsonRpcResponse, AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde_json::{json, Value};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveShowcase {
    None,
    HrConsulting,
    Spedition,
    Webstore,
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
async fn count_table_rows_on(client: &crate::db::SurrealClient, table: &str) -> usize {
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

/// Runs showcase-specific retrieval augmentation ONLY for the currently active showcase,
/// AND additionally queries any attached external database (`EXTERNAL_DB_SCHEMA_PROMPT` / `EXTERNAL_DB_SCHEMA_FILE`).
pub async fn finalize_retrieval_for_active_showcase(
    state: &AppState,
    query: &str,
    mut ranked_pool: Vec<RetrievedChunk>,
    top_n: usize,
    db_ms: &mut u64,
    reranker_used: &mut String,
) -> Vec<RetrievedChunk> {
    let primary_anchor = ranked_pool.first().cloned();

    let mut chunks = match state.get_active_showcase() {
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
    };

    if let Some(ref ext_schema) = state.config.external_db_schema_prompt {
        let target_client = state.external_db.as_ref().unwrap_or(&state.db);
        let t_ext = std::time::Instant::now();
        if let Some(ext_chunk) = crate::text_to_sql::execute_dynamic_text_to_sql_on_client(
            state,
            target_client,
            query,
            primary_anchor.as_ref(),
            ext_schema,
            "external_db_live_query_result",
            "external_attached_database",
            "🔌 [Externe DB Text-to-SQL]",
        )
        .await
        {
            *db_ms += t_ext.elapsed().as_millis() as u64;
            let insert_idx = chunks.len().min(2);
            chunks.insert(insert_idx, ext_chunk);
            *reranker_used = format!("{} + 🔌 Text-to-SQL (Externe DB)", reranker_used);
        }
    }

    chunks
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
