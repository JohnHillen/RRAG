use crate::config::AppConfig;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    #[serde(default)]
    pub entities: Vec<GraphEntity>,
    #[serde(default)]
    pub related_entities: Vec<GraphEntity>,
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
}

impl SurrealClient {
    pub fn new(http: Client, config: AppConfig) -> Self {
        Self { http, config }
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

    /// Checks by deterministic content hash (`doc_id`) whether a document's chunks exist in SurrealDB.
    pub async fn count_chunks_for_doc(&self, doc_id: &str) -> Result<usize> {
        let sql = format!(
            "SELECT count() AS cnt FROM chunk WHERE doc_id = {} GROUP ALL;",
            serde_json::to_string(doc_id)?
        );
        let res = self.execute_sql(&sql).await?;
        let arr = extract_result_array(res.first());
        let cnt = arr
            .first()
            .and_then(|v| v.get("cnt"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize;
        Ok(cnt)
    }

    /// Replaces older revisions of a document sharing the same `title` (but differing content hash `doc_id`),
    /// ensuring stale chunks are pruned from future retrieval.
    pub async fn replace_older_revisions(&self, title: &str, new_doc_id: &str) -> Result<bool> {
        let check_sql = format!(
            "SELECT count() AS cnt FROM chunk WHERE doc_title = {} AND doc_id != {} GROUP ALL;",
            serde_json::to_string(title)?,
            serde_json::to_string(new_doc_id)?
        );
        let res = self.execute_sql(&check_sql).await?;
        let old_chunks = extract_result_array(res.first())
            .first()
            .and_then(|v| v.get("cnt"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);

        if old_chunks > 0 {
            let del_sql = format!(
                "DELETE chunk WHERE doc_title = {t} AND doc_id != {d};\nDELETE document WHERE title = {t} AND record::id(id) != {d};",
                t = serde_json::to_string(title)?,
                d = serde_json::to_string(new_doc_id)?
            );
            self.execute_sql(&del_sql).await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Lists all documents stored in SurrealDB with chunk counts and timestamps.
    pub async fn list_documents(&self) -> Result<Vec<DocumentSummary>> {
        let sql = "SELECT doc_id, doc_title, count() AS chunk_count, time::max(created_at) AS created_at FROM chunk GROUP BY doc_id, doc_title;";
        let res = self.execute_sql(sql).await?;
        let items = extract_result_array(res.first());

        let mut docs = Vec::with_capacity(items.len());
        for val in items {
            let doc_id = val["doc_id"].as_str().unwrap_or_default().to_string();
            let doc_title = val["doc_title"].as_str().unwrap_or("Untitled").to_string();
            let chunk_count = val["chunk_count"].as_u64().unwrap_or(0) as usize;
            let created_at = val["created_at"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(19)
                .collect::<String>()
                .replace('T', " ");
            if !doc_id.is_empty() {
                docs.push(DocumentSummary {
                    doc_id,
                    doc_title,
                    chunk_count,
                    created_at,
                });
            }
        }

        docs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(docs)
    }

    /// Deletes a document and its associated chunks from SurrealDB.
    pub async fn delete_document(&self, doc_id: &str) -> Result<()> {
        let sql = format!(
            "DELETE chunk WHERE doc_id = {d};\nDELETE type::record('document', {d});",
            d = serde_json::to_string(doc_id)?
        );
        self.execute_sql(&sql).await?;
        Ok(())
    }

    /// Prunes older duplicate revisions (identical `doc_title`, differing `doc_id`),
    /// keeping only the most recently indexed version.
    pub async fn deduplicate_by_title(&self) -> Result<usize> {
        let docs = self.list_documents().await?;
        let mut seen_titles = std::collections::HashSet::new();
        let mut removed = 0usize;

        // Since `list_documents` is sorted descending by `created_at`, the first occurrence is latest
        for d in docs {
            if !seen_titles.insert(d.doc_title.clone()) {
                self.delete_document(&d.doc_id).await?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Inserts document metadata, chunks, and optional knowledge graph relations via UPSERT.
    pub async fn insert_document_and_chunks(
        &self,
        doc_id: &str,
        title: &str,
        source: &str,
        chunks: &[(usize, String, Vec<f32>)],
        entities: &[(String, String, Option<String>)],
        relations: &[(String, String, String)],
    ) -> Result<()> {
        // 1. Parent document record via UPSERT
        let doc_json = json!({
            "title": title,
            "source": source,
        });
        let doc_stmt = format!(
            "UPSERT type::record('document', {}) CONTENT {};\n",
            serde_json::to_string(doc_id)?,
            doc_json
        );
        self.execute_sql(&doc_stmt).await?;

        // 2. Insert chunks and embeddings in batches of 20
        for batch in chunks.chunks(20) {
            let mut batch_sql = String::new();
            for (idx, content, embedding) in batch {
                let chunk_key = format!("{}_{}", doc_id, idx);
                let chunk_json = json!({
                    "doc_id": doc_id,
                    "doc_title": title,
                    "chunk_index": idx,
                    "content": content,
                    "embedding": embedding,
                });
                batch_sql.push_str(&format!(
                    "UPSERT type::record('chunk', {}) CONTENT {};\n",
                    serde_json::to_string(&chunk_key)?,
                    chunk_json
                ));
            }
            self.execute_sql(&batch_sql).await?;
        }

        // 3. Optional entity nodes and relation edges for GraphRAG
        if !entities.is_empty() || !relations.is_empty() {
            let mut graph_sql = String::new();
            for (name, entity_type, description) in entities {
                let clean_key = sanitize_key(name);
                if clean_key.is_empty() {
                    continue;
                }
                let entity_json = json!({
                    "name": name,
                    "entity_type": entity_type,
                    "description": description,
                });
                graph_sql.push_str(&format!(
                    "UPSERT type::record('entity', {}) CONTENT {};\n",
                    serde_json::to_string(&clean_key)?,
                    entity_json
                ));

                let mut matched_any = false;
                for (idx, content, _) in chunks {
                    if content.to_lowercase().contains(&name.to_lowercase()) {
                        matched_any = true;
                        let chunk_key = format!("{}_{}", doc_id, idx);
                        graph_sql.push_str(&format!(
                            "RELATE type::record('chunk', {})->mentions->type::record('entity', {});\n",
                            serde_json::to_string(&chunk_key)?,
                            serde_json::to_string(&clean_key)?
                        ));
                    }
                }
                // If a high-level entity does not appear verbatim in text, link it to Chunk #0
                if !matched_any && !chunks.is_empty() {
                    let first_chunk_key = format!("{}_0", doc_id);
                    graph_sql.push_str(&format!(
                        "RELATE type::record('chunk', {})->mentions->type::record('entity', {});\n",
                        serde_json::to_string(&first_chunk_key)?,
                        serde_json::to_string(&clean_key)?
                    ));
                }
            }

            for (src_entity, rel_type, dst_entity) in relations {
                let src_key = sanitize_key(src_entity);
                let dst_key = sanitize_key(dst_entity);
                if src_key.is_empty() || dst_key.is_empty() {
                    continue;
                }
                graph_sql.push_str(&format!(
                    "RELATE type::record('entity', {})->relates_to->type::record('entity', {}) SET relation_type = {};\n",
                    serde_json::to_string(&src_key)?,
                    serde_json::to_string(&dst_key)?,
                    serde_json::to_string(rel_type)?
                ));
            }

            if !graph_sql.is_empty() {
                self.execute_sql(&graph_sql).await?;
            }
        }

        Ok(())
    }

    /// Performs a hybrid search (HNSW vector + BM25 full-text), merges ranks via
    /// Reciprocal Rank Fusion (RRF), and expands matches with GraphRAG relations.
    pub async fn hybrid_graph_search(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let pool_k = (top_k * 2).max(25);
        let bm25_query = expand_query_for_bm25(query_text);
        let sql = format!(
            r#"
            LET $q_vec = {vec_json};
            LET $q_txt = {txt_json};

            -- 1. HNSW Semantic Vector Search
            SELECT
                record::id(id) AS id,
                doc_id,
                doc_title,
                chunk_index,
                content,
                vector::distance::knn() AS dist,
                ->mentions->entity.{{name, entity_type, description}} AS entities,
                ->mentions->entity->relates_to->entity.{{name, entity_type, description}} AS related_entities
            FROM chunk
            WHERE embedding <|{k}, 100|> $q_vec
            ORDER BY dist ASC;

            -- 2. BM25 Lexical Full-Text Search
            SELECT
                record::id(id) AS id,
                doc_id,
                doc_title,
                chunk_index,
                content,
                search::score(1) AS bm25_score,
                ->mentions->entity.{{name, entity_type, description}} AS entities,
                ->mentions->entity->relates_to->entity.{{name, entity_type, description}} AS related_entities
            FROM chunk
            WHERE content @1@ $q_txt
            ORDER BY bm25_score DESC
            LIMIT {k};
            "#,
            vec_json = serde_json::to_string(query_embedding)?,
            txt_json = serde_json::to_string(&bm25_query)?,
            k = pool_k
        );

        let raw_responses = self.execute_sql(&sql).await?;

        // Statements: [0] = LET $q_vec, [1] = LET $q_txt, [2] = Vector Results, [3] = BM25 Results
        let vec_items = extract_result_array(raw_responses.get(2));
        let bm25_items = extract_result_array(raw_responses.get(3));

        // Reciprocal Rank Fusion (RRF with k = 60.0 smoothing constant)
        use std::collections::HashMap;
        let mut fused_map: HashMap<String, RetrievedChunk> = HashMap::new();
        let rrf_k = 60.0;

        for (rank, val) in vec_items.into_iter().enumerate() {
            if let Ok(mut chunk) = serde_json::from_value::<RetrievedChunk>(val) {
                let score = 1.0 / (rrf_k + (rank as f64) + 1.0);
                chunk.rrf_score = score;
                fused_map.insert(chunk.id.clone(), chunk);
            }
        }

        for (rank, val) in bm25_items.into_iter().enumerate() {
            if let Ok(chunk) = serde_json::from_value::<RetrievedChunk>(val) {
                let score = 1.0 / (rrf_k + (rank as f64) + 1.0);
                fused_map
                    .entry(chunk.id.clone())
                    .and_modify(|existing| existing.rrf_score += score)
                    .or_insert_with(|| {
                        let mut c = chunk;
                        c.rrf_score = score;
                        c
                    });
            }
        }

        // Filename/Title RRF Boost: When significant query tokens appear in doc_title,
        // candidate chunks from that document receive an RRF boost.
        let query_tokens = extract_query_keywords(query_text);
        if !query_tokens.is_empty() {
            for chunk in fused_map.values_mut() {
                let title_lower = chunk.doc_title.to_lowercase();
                let title_norm: String = title_lower
                    .chars()
                    .map(|c| if c.is_alphanumeric() { c } else { ' ' })
                    .collect();
                let mut matched_tokens = 0usize;
                for tok in &query_tokens {
                    if title_lower.contains(tok.as_str()) || title_norm.contains(tok.as_str()) {
                        matched_tokens += 1;
                    }
                }
                if matched_tokens > 0 {
                    chunk.rrf_score += (matched_tokens as f64) * (0.6 / (rrf_k + 1.0));
                }
            }
        }

        let mut results: Vec<RetrievedChunk> = fused_map.into_values().collect();
        results.sort_by(|a, b| {
            b.rrf_score
                .partial_cmp(&a.rrf_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate identical chunk content
        let mut seen_content_hashes = std::collections::HashSet::new();
        results.retain(|c| seen_content_hashes.insert(content_hash_id(c.content.trim().as_bytes())));

        results.truncate(top_k);
        Ok(results)
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

fn extract_result_array(statement_res: Option<&Value>) -> Vec<Value> {
    statement_res
        .and_then(|v| v.get("result"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default()
}

fn sanitize_key(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Expands hyphenated model identifiers (e.g. "TR-1" -> "TR-1 TR1", "E-Stream" -> "E-Stream EStream")
/// for BM25 full-text matching against differing manual conventions.
fn expand_query_for_bm25(query: &str) -> String {
    let mut extras = Vec::new();
    for raw_word in query.split_whitespace() {
        let trimmed = raw_word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        if trimmed.contains('-') {
            let joined: String = trimmed.chars().filter(|c| c.is_alphanumeric()).collect();
            if joined.len() >= 2 {
                extras.push(joined);
            }
        }
    }
    if extras.is_empty() {
        query.to_string()
    } else {
        format!("{} {}", query, extras.join(" "))
    }
}

/// Extracts relevant keywords (stripping German and English stopwords) for title boosting.
fn extract_query_keywords(query: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "der", "die", "das", "den", "dem", "des", "ein", "eine", "einer", "einem", "einen",
        "und", "oder", "aber", "von", "vom", "zum", "zur", "mit", "bei", "für", "auf", "aus",
        "nach", "über", "unter", "vor", "hinter", "wie", "was", "wer", "wo", "wann", "warum",
        "welche", "welcher", "welches", "welchem", "welchen", "hat", "haben", "ist", "sind",
        "war", "waren", "wird", "werden", "kann", "können", "muss", "müssen", "soll", "sollen",
        "bitte", "mir", "mich", "ich", "du", "wir", "ihr", "sie", "the", "and", "for", "with",
    ];

    let mut keywords = Vec::new();
    for raw in query.split_whitespace() {
        let cleaned = raw
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
            .to_lowercase();
        if cleaned.is_empty() {
            continue;
        }
        if cleaned.contains('-') {
            let compact: String = cleaned.chars().filter(|c| c.is_alphanumeric()).collect();
            if compact.len() >= 3 && !STOPWORDS.contains(&compact.as_str()) && !keywords.contains(&compact) {
                keywords.push(compact);
            }
        }
        for part in cleaned.split('-') {
            if part.len() >= 3 && !STOPWORDS.contains(&part) {
                let s = part.to_string();
                if !keywords.contains(&s) {
                    keywords.push(s);
                }
            }
        }
    }
    keywords
}
