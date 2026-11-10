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
