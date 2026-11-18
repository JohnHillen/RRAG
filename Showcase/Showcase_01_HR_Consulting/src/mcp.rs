use super::HR_SCHEMA_PROMPT;
use crate::{mcp::JsonRpcResponse, text_to_sql::execute_dynamic_text_to_sql, AppState};
use serde_json::{json, Value};

pub const TOOL_NAME: &str = "query_employees_crm";

pub fn tool_definition() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": "Executes a dynamic Text-to-SurrealQL query against the structured `employee_record` CRM/HR table (3,000+ employee records).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "Natural language question or filter request for the employee database (e.g. 'All employees with Rust, SQL, and English >= B2 sorted by availability')"
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
        .unwrap_or("SELECT * FROM employee_record LIMIT 25;");

    match execute_dynamic_text_to_sql(
        state,
        question,
        None,
        HR_SCHEMA_PROMPT,
        "crm_sql_live_query_result",
        "crm_employee_record_table",
        "🗄️ [CRM/HR-DB Text-to-SQL]",
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
