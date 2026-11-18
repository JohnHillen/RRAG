use super::SPEDITION_SCHEMA_PROMPT;
use crate::{mcp::JsonRpcResponse, text_to_sql::execute_dynamic_text_to_sql, AppState};
use serde_json::{json, Value};

pub const TOOL_NAME: &str = "query_fleet_logistics";

pub fn tool_definition() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": "Executes a dynamic Text-to-SurrealQL query against the structured Spedition / Fleet database (`fleet_vehicle`, `fleet_tour`, `fleet_warehouse`).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "Natural language question or analytics query for the fleet, tours, or warehouses (e.g. 'List all drivers sorted descending by kilometers driven in April')"
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
        .unwrap_or("SELECT * FROM fleet_vehicle LIMIT 12;");

    match execute_dynamic_text_to_sql(
        state,
        question,
        None,
        SPEDITION_SCHEMA_PROMPT,
        "fleet_sql_live_query_result",
        "fleet_logistics_tables",
        "🚛 [Speditions-DB Text-to-SQL]",
    )
    .await
    {
        Some(chunk) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(json!({
                "content": [{ "type": "text", "text": chunk.content }],
                "isError": false
            })),
            error: None,
        },
        None => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(json!({
                "content": [{ "type": "text", "text": "Text-to-SurrealQL query returned no results." }],
                "isError": true
            })),
            error: None,
        },
    }
}
