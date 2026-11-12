use crate::AppState;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

/// Model Context Protocol (MCP) JSON-RPC 2.0 endpoint (`/mcp`).
/// Allows external agents and LLMs (e.g., Claude Desktop, Cursor, Open WebUI, custom agents)
/// to query the SurrealDB Hybrid- & GraphRAG pipeline natively as a tool.
pub async fn handle_mcp_request(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => Json(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "surreal-rust-rag-mcp",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "tools": {}
                }
            })),
            error: None,
        }),
        "tools/list" => Json(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(json!({
                "tools": [
                    {
                        "name": "search_knowledge_base",
                        "description": "Performs a hybrid search (HNSW vector + BM25 full-text + cross-encoder reranking + knowledge graph traversal) in the SurrealDB knowledge base.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "Search query or natural language question for the knowledge base"
                                },
                                "top_n": {
                                    "type": "integer",
                                    "description": "Number of top results to return after reranking (default: 5)"
                                }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "query_employees_crm",
                        "description": "Executes a deterministic SQL/SurrealQL query against the structured `employee_record` CRM/HR table (300+ employee records) by skills, CEFR English level, certifications, and availability, including 1-skill training gap analysis.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "required_skills": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Mandatory technical skills (e.g. [\"Rust\", \"SQL\"])"
                                },
                                "min_english_level": {
                                    "type": "string",
                                    "description": "Minimum CEFR English level (e.g. \"B1\", \"B2\", \"C1\", \"C2\")"
                                },
                                "required_certification": {
                                    "type": "string",
                                    "description": "Optional certification filter"
                                },
                                "min_availability": {
                                    "type": "integer",
                                    "description": "Minimum availability percentage (0-100)"
                                },
                                "department": {
                                    "type": "string",
                                    "description": "Optional department filter"
                                }
                            }
                        }
                    }
                ]
            })),
            error: None,
        }),
        "tools/call" => {
            let tool_name = req.params["name"].as_str().unwrap_or_default();
            let args = &req.params["arguments"];

            if tool_name == "search_knowledge_base" {
                let query = args["query"].as_str().unwrap_or_default().to_string();
                let top_n = args["top_n"]
                    .as_u64()
                    .map(|n| n as usize)
                    .unwrap_or(state.config.retrieval_final_top_n);

                match crate::execute_retrieval_pipeline(&state, &query, top_n, "fast").await {
                    Ok((chunks, _metrics)) => {
                        let formatted = crate::llm::LlmClient::format_rag_context(&chunks);
                        Json(JsonRpcResponse {
                            jsonrpc: "2.0".into(),
                            id,
                            result: Some(json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": formatted
                                    }
                                ],
                                "isError": false
                            })),
                            error: None,
                        })
                    }
                    Err(e) => Json(JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id,
                        result: Some(json!({
                            "content": [{ "type": "text", "text": format!("Retrieval error: {}", e) }],
                            "isError": true
                        })),
                        error: None,
                    }),
                }
            } else if tool_name == "query_employees_crm" {
                let required_skills: Vec<String> = args["required_skills"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let min_english = args["min_english_level"]
                    .as_str()
                    .or_else(|| args["min_english"].as_str());
                let project = args["project"]
                    .as_str()
                    .or_else(|| args["department"].as_str());
                let min_avail = args["min_availability"].as_i64();

                match state
                    .db
                    .query_employees_by_criteria(&required_skills, min_english, project, min_avail)
                    .await
                {
                    Ok((full_matches, near_matches, total_scanned)) => {
                        let criteria_summary = format!(
                            "Skills: {:?}, Min. Englisch: {}",
                            required_skills,
                            min_english.unwrap_or("Beliebig")
                        );
                        let chunk = crate::format_crm_query_as_chunk(
                            &full_matches,
                            &near_matches,
                            total_scanned,
                            &criteria_summary,
                            None,
                        );
                        Json(JsonRpcResponse {
                            jsonrpc: "2.0".into(),
                            id,
                            result: Some(json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": chunk.content
                                    }
                                ],
                                "isError": false
                            })),
                            error: None,
                        })
                    }
                    Err(e) => Json(JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id,
                        result: Some(json!({
                            "content": [{ "type": "text", "text": format!("CRM SQL query error: {}", e) }],
                            "isError": true
                        })),
                        error: None,
                    }),
                }
            } else {
                Json(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id,
                    result: None,
                    error: Some(json!({
                        "code": -32601,
                        "message": format!("Unknown MCP tool: {}", tool_name)
                    })),
                })
            }
        }
        _ => Json(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(json!({
                "code": -32601,
                "message": format!("Method not supported: {}", req.method)
            })),
        }),
    }
}
