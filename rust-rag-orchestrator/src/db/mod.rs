pub mod documents;
pub mod external;
pub mod search;

use self::search::{deserialize_graph_entities, deserialize_graph_relations};
use crate::config::AppConfig;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone)]
pub struct SurrealClient {
    http: Client,
    config: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub id: String,
    pub doc_id: String,
    pub doc_title: String,
    pub chunk_index: i64,
    pub content: String,
    #[serde(default)]
    pub rrf_score: f64,
    #[serde(default)]
    pub rerank_score: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_graph_entities")]
    pub entities: Vec<GraphEntity>,
    #[serde(default, deserialize_with = "deserialize_graph_entities")]
    pub related_entities: Vec<GraphEntity>,
    #[serde(default, deserialize_with = "deserialize_graph_relations")]
    pub relations_out: Vec<GraphRelation>,
    #[serde(default, deserialize_with = "deserialize_graph_relations")]
    pub relations_in: Vec<GraphRelation>,
    #[serde(default)]
    pub relations: Vec<GraphRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GraphRelation {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntity {
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub doc_id: String,
    pub doc_title: String,
    pub chunk_count: usize,
    pub created_at: String,
    #[serde(default)]
    pub relations: Vec<String>,
}

impl SurrealClient {
    pub fn new(http: Client, config: AppConfig) -> Self {
        Self { http, config }
    }

    pub fn http_client(&self) -> Client {
        self.http.clone()
    }

    /// Executes raw SurrealQL against the `/sql` endpoint.
    pub async fn execute_sql(&self, sql: &str) -> Result<Vec<Value>> {
        let url = format!("{}/sql", self.config.surreal_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .basic_auth(&self.config.surreal_user, Some(&self.config.surreal_pass))
            .header("Accept", "application/json")
            .header("surreal-ns", &self.config.surreal_ns)
            .header("surreal-db", &self.config.surreal_db)
            .body(sql.to_string())
            .send()
            .await
            .context("Failed to send SurrealQL request")?;

        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("SurrealDB HTTP {}: {}", status, body_text);
        }

        let results: Vec<Value> = serde_json::from_str(&body_text)
            .with_context(|| format!("Invalid SurrealDB JSON response: {}", body_text))?;
        Ok(results)
    }

    /// Loads persisted system LLM settings from SurrealDB (if saved previously).
    pub async fn get_system_llm_settings(
        &self,
    ) -> Result<Option<(String, String, Option<String>, String)>> {
        let sql =
            "SELECT llm_provider, llm_base_url, llm_api_key, llm_model FROM system_settings:app;";
        let res = self.execute_sql(sql).await?;
        if let Some(first_resp) = res.first() {
            if let Some(arr) = first_resp.get("result").and_then(|v| v.as_array()) {
                if let Some(item) = arr.first() {
                    let prov = item
                        .get("llm_provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let url = item
                        .get("llm_base_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let key = item
                        .get("llm_api_key")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let model = item
                        .get("llm_model")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if !prov.is_empty() && !model.is_empty() {
                        return Ok(Some((prov, url, key, model)));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Persists system LLM settings into SurrealDB so they survive restarts and setup.sh.
    pub async fn save_system_llm_settings(
        &self,
        provider: &str,
        base_url: &str,
        api_key: Option<&str>,
        model: &str,
    ) -> Result<()> {
        let key_json = match api_key {
            Some(k) if !k.is_empty() => serde_json::to_string(k)?,
            _ => "NONE".to_string(),
        };
        let sql = format!(
            "UPSERT system_settings:app SET llm_provider = {}, llm_base_url = {}, llm_model = {}, llm_api_key = {}, updated_at = time::now();",
            serde_json::to_string(provider)?,
            serde_json::to_string(base_url)?,
            serde_json::to_string(model)?,
            key_json
        );
        self.execute_sql(&sql).await?;
        Ok(())
    }
}

/// Generates a deterministic 128-bit content hash ID (`h_<32-hex>`) from raw bytes.
pub fn content_hash_id(bytes: &[u8]) -> String {
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x84222325cbf29ce4;
    for (i, &b) in bytes.iter().enumerate() {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(0x100000001b3);
        h2 ^= (b as u64).wrapping_add(i as u64);
        h2 = h2.wrapping_mul(0x100000001b3).rotate_left(5);
    }
    h1 ^= bytes.len() as u64;
    format!("h_{:016x}{:016x}", h1, h2)
}

pub(crate) fn extract_result_array(statement_res: Option<&Value>) -> Vec<Value> {
    statement_res
        .and_then(|v| v.get("result"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn sanitize_key(input: &str) -> String {
    let sanitized: String = input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if sanitized.is_empty() {
        return sanitized;
    }
    if sanitized
        .chars()
        .next()
        .map_or(false, |c| !c.is_ascii_alphabetic())
    {
        format!("e_{}", sanitized)
    } else {
        sanitized
    }
}
