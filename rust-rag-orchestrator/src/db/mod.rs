pub mod documents;
pub mod external;
pub mod search;

use self::search::{deserialize_graph_entities, deserialize_graph_relations};
use crate::config::AppConfig;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct SurrealClient {
    http: Client,
    config: AppConfig,
    active_db: Arc<RwLock<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDatabaseInfo {
    pub db_name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub document_count: usize,
    pub chunk_count: usize,
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
        let initial_db = config.surreal_db.clone();
        Self {
            http,
            config,
            active_db: Arc::new(RwLock::new(initial_db)),
        }
    }

    pub fn http_client(&self) -> Client {
        self.http.clone()
    }

    /// Returns the currently active SurrealDB project database name.
    pub fn get_active_db(&self) -> String {
        self.active_db
            .read()
            .map(|d| d.clone())
            .unwrap_or_else(|_| self.config.surreal_db.clone())
    }

    /// Sets the active SurrealDB project database name in memory.
    pub fn set_active_db(&self, db_name: &str) {
        let clean = sanitize_db_name(db_name);
        if !clean.is_empty() {
            if let Ok(mut guard) = self.active_db.write() {
                *guard = clean;
            }
        }
    }

    /// Executes raw SurrealQL against the `/sql` endpoint on the currently active project database.
    pub async fn execute_sql(&self, sql: &str) -> Result<Vec<Value>> {
        let current_db = self.get_active_db();
        self.execute_sql_on_db(&current_db, sql).await
    }

    /// Executes raw SurrealQL against the `/sql` endpoint on a specific database within the configured namespace.
    pub async fn execute_sql_on_db(&self, db_name: &str, sql: &str) -> Result<Vec<Value>> {
        let url = format!("{}/sql", self.config.surreal_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .basic_auth(&self.config.surreal_user, Some(&self.config.surreal_pass))
            .header("Accept", "application/json")
            .header("surreal-ns", &self.config.surreal_ns)
            .header("surreal-db", db_name)
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

    /// Ensures the full Hybrid RAG (HNSW + BM25) & Knowledge Graph schema exists on `db_name`.
    pub async fn ensure_rag_schema_on_db(&self, db_name: &str) -> Result<()> {
        let schema_sql = r#"
DEFINE ANALYZER IF NOT EXISTS rag_analyzer TOKENIZERS class, punct FILTERS lowercase, ascii, snowball(german), snowball(english);
DEFINE TABLE IF NOT EXISTS document SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS title      ON TABLE document TYPE string;
DEFINE FIELD IF NOT EXISTS source     ON TABLE document TYPE string;
DEFINE FIELD IF NOT EXISTS mime_type  ON TABLE document TYPE option<string>;
DEFINE FIELD IF NOT EXISTS metadata   ON TABLE document TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE document TYPE datetime DEFAULT time::now();
DEFINE TABLE IF NOT EXISTS chunk SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS doc_id      ON TABLE chunk TYPE string;
DEFINE FIELD IF NOT EXISTS doc_title   ON TABLE chunk TYPE string;
DEFINE FIELD IF NOT EXISTS chunk_index ON TABLE chunk TYPE int;
DEFINE FIELD IF NOT EXISTS content     ON TABLE chunk TYPE string;
DEFINE FIELD IF NOT EXISTS embedding   ON TABLE chunk TYPE array<float>;
DEFINE FIELD IF NOT EXISTS metadata    ON TABLE chunk TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS created_at  ON TABLE chunk TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS idx_chunk_doc_id    ON TABLE chunk FIELDS doc_id;
DEFINE INDEX IF NOT EXISTS idx_chunk_doc_title ON TABLE chunk FIELDS doc_title;
DEFINE INDEX IF NOT EXISTS idx_chunk_bm25 ON TABLE chunk FIELDS content FULLTEXT ANALYZER rag_analyzer BM25(1.2, 0.75) HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS idx_chunk_embedding ON TABLE chunk FIELDS embedding HNSW DIMENSION 1024 DIST COSINE EFC 150 M 16;
DEFINE TABLE IF NOT EXISTS entity SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name        ON TABLE entity TYPE string;
DEFINE FIELD IF NOT EXISTS entity_type ON TABLE entity TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE entity TYPE option<string>;
DEFINE INDEX IF NOT EXISTS idx_entity_name ON TABLE entity FIELDS name UNIQUE;
DEFINE TABLE IF NOT EXISTS mentions TYPE RELATION IN chunk OUT entity;
DEFINE TABLE IF NOT EXISTS relates_to TYPE RELATION IN entity OUT entity;
DEFINE FIELD IF NOT EXISTS relation_type ON TABLE relates_to TYPE string DEFAULT 'related_to';
DEFINE FIELD IF NOT EXISTS weight        ON TABLE relates_to TYPE float DEFAULT 1.0;
"#;
        self.execute_sql_on_db(db_name, schema_sql).await?;
        Ok(())
    }

    /// Lists all project databases in the namespace along with their document and chunk counts.
    pub async fn list_project_databases(&self) -> Result<Vec<ProjectDatabaseInfo>> {
        let active = self.get_active_db();
        let default_db = self.config.surreal_db.clone();
        let info_res = self
            .execute_sql_on_db(&default_db, "INFO FOR NS;")
            .await
            .unwrap_or_default();

        let mut db_names: Vec<String> = Vec::new();
        if let Some(dbs_obj) = info_res
            .first()
            .and_then(|v| v.get("result"))
            .and_then(|r| r.get("databases"))
            .and_then(|d| d.as_object())
        {
            for key in dbs_obj.keys() {
                let clean = key.trim().to_string();
                if !clean.is_empty() && !db_names.contains(&clean) {
                    db_names.push(clean);
                }
            }
        }

        if !db_names.contains(&default_db) {
            db_names.insert(0, default_db.clone());
        }
        if !db_names.contains(&active) {
            db_names.push(active.clone());
        }
        db_names.sort();
        // Always put default_db first
        if let Some(pos) = db_names.iter().position(|x| x == &default_db) {
            if pos > 0 {
                let d = db_names.remove(pos);
                db_names.insert(0, d);
            }
        }

        let mut out = Vec::with_capacity(db_names.len());
        for name in db_names {
            let counts = self
                .execute_sql_on_db(
                    &name,
                    "SELECT count() AS c FROM document GROUP ALL; SELECT count() AS c FROM chunk GROUP ALL;",
                )
                .await
                .unwrap_or_default();
            let doc_cnt = extract_result_array(counts.first())
                .first()
                .and_then(|v| v["c"].as_u64())
                .unwrap_or(0) as usize;
            let chunk_cnt = extract_result_array(counts.get(1))
                .first()
                .and_then(|v| v["c"].as_u64())
                .unwrap_or(0) as usize;
            out.push(ProjectDatabaseInfo {
                is_active: name == active,
                is_default: name == default_db,
                db_name: name,
                document_count: doc_cnt,
                chunk_count: chunk_cnt,
            });
        }
        Ok(out)
    }

    /// Creates (if not exists) and switches to a project database, initializing its HNSW + BM25 + Graph schema.
    pub async fn create_or_switch_project_database(&self, raw_name: &str) -> Result<String> {
        let clean = sanitize_db_name(raw_name);
        if clean.is_empty() {
            anyhow::bail!("Ungültiger Datenbank-/Projektname.");
        }
        let define_sql = format!("DEFINE DATABASE IF NOT EXISTS {};", clean);
        let _ = self
            .execute_sql_on_db(&self.config.surreal_db, &define_sql)
            .await;
        self.ensure_rag_schema_on_db(&clean).await?;
        self.set_active_db(&clean);
        let _ = self.save_active_project_db_setting(&clean).await;
        Ok(clean)
    }

    /// Deletes a project database (`REMOVE DATABASE <name>`) and switches back to the default database if needed.
    pub async fn delete_project_database(&self, raw_name: &str) -> Result<String> {
        let clean = sanitize_db_name(raw_name);
        if clean.is_empty() {
            anyhow::bail!("Ungültiger Datenbankname.");
        }
        if clean == self.config.surreal_db {
            anyhow::bail!(
                "Die Standard-Datenbank '{}' kann nicht gelöscht werden (nutze stattdessen Clean Restore oder Dokumenten-Löschung).",
                self.config.surreal_db
            );
        }
        if self.get_active_db() == clean {
            self.set_active_db(&self.config.surreal_db);
            let _ = self
                .save_active_project_db_setting(&self.config.surreal_db)
                .await;
        }
        let remove_sql = format!("REMOVE DATABASE IF EXISTS {};", clean);
        self.execute_sql_on_db(&self.config.surreal_db, &remove_sql)
            .await?;
        Ok(self.get_active_db())
    }

    pub async fn save_active_project_db_setting(&self, db_name: &str) -> Result<()> {
        let sql = format!(
            "UPSERT system_settings:app SET active_project_db = {}, updated_at = time::now();",
            serde_json::to_string(db_name)?
        );
        self.execute_sql_on_db(&self.config.surreal_db, &sql)
            .await?;
        Ok(())
    }

    /// Loads persisted system LLM settings (and active project database) from the default SurrealDB database.
    pub async fn get_system_llm_settings(
        &self,
    ) -> Result<Option<(String, String, Option<String>, String)>> {
        let sql =
            "SELECT llm_provider, llm_base_url, llm_api_key, llm_model, active_project_db FROM system_settings:app;";
        let res = self
            .execute_sql_on_db(&self.config.surreal_db, sql)
            .await?;
        if let Some(first_resp) = res.first() {
            if let Some(arr) = first_resp.get("result").and_then(|v| v.as_array()) {
                if let Some(item) = arr.first() {
                    if let Some(proj_db) = item
                        .get("active_project_db")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        self.set_active_db(proj_db);
                    }
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

    /// Persists system LLM settings into the default SurrealDB database so they survive restarts and project switches.
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
            "UPSERT system_settings:app SET llm_provider = {}, llm_base_url = {}, llm_model = {}, llm_api_key = {}, active_project_db = {}, updated_at = time::now();",
            serde_json::to_string(provider)?,
            serde_json::to_string(base_url)?,
            serde_json::to_string(model)?,
            key_json,
            serde_json::to_string(&self.get_active_db())?
        );
        self.execute_sql_on_db(&self.config.surreal_db, &sql).await?;
        Ok(())
    }

    /// Exports the currently active SurrealDB project database (`GET /export`) as a `.surql` dump,
    /// stripping internal `system_settings` records so API keys are never included in backups.
    pub async fn export_database_surql(&self) -> Result<String> {
        let active_db = self.get_active_db();
        let url = format!("{}/export", self.config.surreal_url.trim_end_matches('/'));
        let resp = self
            .http
            .get(&url)
            .basic_auth(&self.config.surreal_user, Some(&self.config.surreal_pass))
            .header("Accept", "application/octet-stream")
            .header("surreal-ns", &self.config.surreal_ns)
            .header("surreal-db", &active_db)
            .send()
            .await
            .context("Failed to request SurrealDB /export")?;

        let status = resp.status();
        let raw_text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("SurrealDB /export HTTP {}: {}", status, raw_text);
        }

        let mut filtered = String::with_capacity(raw_text.len());
        for line in raw_text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("system_settings") {
                continue;
            }
            filtered.push_str(line);
            filtered.push('\n');
        }
        Ok(filtered)
    }

    /// Imports a `.surql` backup dump into SurrealDB (`POST /import`) in safe ~4 MB statement batches
    /// so backups of any size (e.g. 100+ MB with vector embeddings) succeed without hitting HTTP body limits.
    pub async fn import_database_surql(
        &self,
        surql_text: &str,
        clean_before_import: bool,
    ) -> Result<(usize, usize, usize)> {
        if clean_before_import {
            let _ = self
                .execute_sql(
                    "DELETE mentions; DELETE relates_to; DELETE chunk; DELETE document; DELETE entity;",
                )
                .await;
        }

        let url = format!("{}/import", self.config.surreal_url.trim_end_matches('/'));
        const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;
        let mut batch = String::from("OPTION IMPORT;\n");
        let mut batches_sent = 0usize;

        for raw_line in surql_text.lines() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("--")
                || trimmed.eq_ignore_ascii_case("OPTION IMPORT;")
                || trimmed.contains("system_settings")
            {
                continue;
            }

            let normalized_line: std::borrow::Cow<'_, str> =
                if let Some(rest) = trimmed.strip_prefix("INSERT [") {
                    std::borrow::Cow::Owned(format!("INSERT IGNORE [{}", rest))
                } else if let Some(rest) = trimmed.strip_prefix("INSERT RELATION [") {
                    std::borrow::Cow::Owned(format!("INSERT RELATION IGNORE [{}", rest))
                } else {
                    std::borrow::Cow::Borrowed(trimmed)
                };

            if batch.len() + normalized_line.len() + 2 > MAX_BATCH_BYTES
                && batch.len() > "OPTION IMPORT;\n".len()
            {
                self.send_surreal_import_batch(&url, &batch).await?;
                batches_sent += 1;
                batch.clear();
                batch.push_str("OPTION IMPORT;\n");
            }

            batch.push_str(&normalized_line);
            batch.push('\n');
        }

        if batch.len() > "OPTION IMPORT;\n".len() {
            self.send_surreal_import_batch(&url, &batch).await?;
            batches_sent += 1;
        }

        let count_res = self
            .execute_sql(
                "SELECT count() AS c FROM document GROUP ALL; SELECT count() AS c FROM chunk GROUP ALL;",
            )
            .await
            .unwrap_or_default();
        let total_docs = extract_result_array(count_res.first())
            .first()
            .and_then(|v| v["c"].as_u64())
            .unwrap_or(0) as usize;
        let total_chunks = extract_result_array(count_res.get(1))
            .first()
            .and_then(|v| v["c"].as_u64())
            .unwrap_or(0) as usize;

        Ok((batches_sent, total_docs, total_chunks))
    }

    async fn send_surreal_import_batch(&self, url: &str, batch_surql: &str) -> Result<()> {
        let active_db = self.get_active_db();
        let resp = self
            .http
            .post(url)
            .basic_auth(&self.config.surreal_user, Some(&self.config.surreal_pass))
            .header("Accept", "application/json")
            .header("Content-Type", "application/octet-stream")
            .header("surreal-ns", &self.config.surreal_ns)
            .header("surreal-db", &active_db)
            .body(batch_surql.to_string())
            .send()
            .await
            .context("Failed to send SurrealDB /import batch")?;

        let status = resp.status();
        let body_txt = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("SurrealDB /import HTTP {}: {}", status, body_txt);
        }
        Ok(())
    }
}

pub fn sanitize_db_name(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.trim().to_lowercase().chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            c if c.is_ascii_alphanumeric() => out.push(c),
            '_' | '-' | ' ' => {
                if !out.ends_with('_') && !out.is_empty() {
                    out.push('_');
                }
            }
            _ => {}
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed
        .chars()
        .next()
        .map_or(false, |c| !c.is_ascii_alphabetic())
    {
        format!("db_{}", trimmed)
    } else {
        trimmed
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
    let mut sanitized = String::with_capacity(input.len() + 4);
    for c in input.trim().to_lowercase().chars() {
        match c {
            'ä' => sanitized.push_str("ae"),
            'ö' => sanitized.push_str("oe"),
            'ü' => sanitized.push_str("ue"),
            'ß' => sanitized.push_str("ss"),
            c if c.is_ascii_alphanumeric() => sanitized.push(c),
            _ => sanitized.push('_'),
        }
    }
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
