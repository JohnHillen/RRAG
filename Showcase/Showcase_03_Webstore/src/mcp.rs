use super::{EXTERNAL_WEBSTORE_SCHEMA_PROMPT, INTERNAL_WEBSTORE_SCHEMA_PROMPT};
use crate::{
    mcp::JsonRpcResponse, text_to_sql::execute_dynamic_text_to_sql_on_client, AppState,
};
use serde_json::{json, Value};

pub const TOOL_NAME: &str = "query_webstore_multi_db";

pub fn tool_definition() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": "Executes dynamic Text-to-SurrealQL queries across BOTH the Internal Contract Database (`internal_contract_ledger`) and the External Webstore Database (`shop_product`, `shop_sale`).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "Natural language question about webstore products, sales history, marketing campaigns, warehouse pallet/battery limits, or supplier contracts"
                }
            },
            "required": ["question"]
        }
    })
}

pub async fn handle_tool_call(
    state: &AppState,
    id: Option<Value>,
    args: &Value,
) -> JsonRpcResponse {
    let question = args["question"]
        .as_str()
        .or_else(|| args["query"].as_str())
        .unwrap_or("SELECT * FROM shop_product LIMIT 20;");

    let mut combined_out = String::new();

    if let Some(int_chunk) = execute_dynamic_text_to_sql_on_client(
        state,
        &state.db,
        question,
        None,
        INTERNAL_WEBSTORE_SCHEMA_PROMPT,
        "webstore_internal_sql_result",
        "internal_contract_ledger_table",
        "🏛️ [Interne Vertrags-DB Text-to-SQL]",
    )
    .await
    {
        combined_out.push_str(&int_chunk.content);
        combined_out.push_str("\n\n");
    }

    let ext_db = state.webstore_external_db();
    if let Some(ext_chunk) = execute_dynamic_text_to_sql_on_client(
        state,
        &ext_db,
        question,
        None,
        EXTERNAL_WEBSTORE_SCHEMA_PROMPT,
        "webstore_external_sql_result",
        "external_webstore_erp_tables",
        "🛒 [Externe Webstore-DB Text-to-SQL]",
    )
    .await
    {
        combined_out.push_str(&ext_chunk.content);
    }

    if combined_out.is_empty() {
        JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(json!({
                "content": [{ "type": "text", "text": "No matching records in Internal or External Webstore DB." }],
                "isError": true
            })),
            error: None,
        }
    } else {
        JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(json!({
                "content": [{ "type": "text", "text": combined_out }],
                "isError": false
            })),
            error: None,
        }
    }
}
