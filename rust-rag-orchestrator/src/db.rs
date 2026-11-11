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

    /// Lists all documents stored in SurrealDB with chunk counts, timestamps, and graph relations.
    pub async fn list_documents(&self) -> Result<Vec<DocumentSummary>> {
        let sql = r#"
            SELECT doc_id, doc_title, count() AS chunk_count, time::max(created_at) AS created_at FROM chunk GROUP BY doc_id, doc_title;
            SELECT
                doc_id,
                ->mentions->entity->relates_to.{ from: in.name, relation: relation_type, to: out.name } AS relations_out,
                ->mentions->entity<-relates_to.{ from: in.name, relation: relation_type, to: out.name } AS relations_in
            FROM chunk;
            SELECT record::id(id) AS doc_id, relations FROM document;
        "#;
        let res = self.execute_sql(sql).await?;

        // 1. Chunks metadata
        let chunk_items = extract_result_array(res.first());

        // 2. Relations aggregated per doc_id from chunk graph traversals (matching search_hybrid)
        let mut doc_relations_map: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();

        for item in extract_result_array(res.get(1)) {
            let did = item["doc_id"].as_str().unwrap_or_default().to_string();
            if did.is_empty() {
                continue;
            }
            let mut chunk_rels = Vec::new();
            if let Some(r_out) = item.get("relations_out") {
                flatten_relations(r_out, &mut chunk_rels);
            }
            if let Some(r_in) = item.get("relations_in") {
                flatten_relations(r_in, &mut chunk_rels);
            }

            let entry = doc_relations_map.entry(did).or_default();
            for r in chunk_rels {
                if !r.from.is_empty() && !r.to.is_empty() {
                    let formatted = format!("{} ──[{}]──► {}", r.from, r.relation, r.to);
                    entry.insert(formatted);
                }
            }
        }

        // 3. Also merge any relations explicitly stored on document records
        for d in extract_result_array(res.get(2)) {
            let did = d["doc_id"].as_str().unwrap_or_default().to_string();
            if let Some(arr) = d["relations"].as_array() {
                let entry = doc_relations_map.entry(did).or_default();
                for v in arr {
                    if let Some(s) = v.as_str() {
                        if !s.is_empty() {
                            entry.insert(s.to_string());
                        }
                    }
                }
            }
        }

        let mut docs = Vec::with_capacity(chunk_items.len());
        for val in chunk_items {
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
                let mut relations: Vec<String> = doc_relations_map
                    .remove(&doc_id)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                relations.sort();

                docs.push(DocumentSummary {
                    doc_id,
                    doc_title,
                    chunk_count,
                    created_at,
                    relations,
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
        let rel_strings: Vec<String> = relations
            .iter()
            .map(|(s, r, d)| format!("{} ──[{}]──► {}", s, r, d))
            .collect();

        // 1. Parent document record via UPSERT
        let doc_json = json!({
            "title": title,
            "source": source,
            "relations": rel_strings,
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
                            "RELATE chunk:{}->mentions->entity:{};\n",
                            chunk_key, clean_key
                        ));
                    }
                }
                // If a high-level entity does not appear verbatim in text, link it to Chunk #0
                if !matched_any && !chunks.is_empty() {
                    let first_chunk_key = format!("{}_0", doc_id);
                    graph_sql.push_str(&format!(
                        "RELATE chunk:{}->mentions->entity:{};\n",
                        first_chunk_key, clean_key
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
                    "RELATE entity:{}->relates_to->entity:{} SET relation_type = {};\n",
                    src_key,
                    dst_key,
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
                ->mentions->entity->relates_to->entity.{{name, entity_type, description}} AS related_entities,
                ->mentions->entity->relates_to.{{ from: in.name, relation: relation_type, to: out.name }} AS relations_out,
                ->mentions->entity<-relates_to.{{ from: in.name, relation: relation_type, to: out.name }} AS relations_in
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
                ->mentions->entity->relates_to->entity.{{name, entity_type, description}} AS related_entities,
                ->mentions->entity->relates_to.{{ from: in.name, relation: relation_type, to: out.name }} AS relations_out,
                ->mentions->entity<-relates_to.{{ from: in.name, relation: relation_type, to: out.name }} AS relations_in
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
                finalize_chunk_relations(&mut chunk);
                let score = 1.0 / (rrf_k + (rank as f64) + 1.0);
                chunk.rrf_score = score;
                fused_map.insert(chunk.id.clone(), chunk);
            }
        }

        for (rank, val) in bm25_items.into_iter().enumerate() {
            if let Ok(mut chunk) = serde_json::from_value::<RetrievedChunk>(val) {
                finalize_chunk_relations(&mut chunk);
                let score = 1.0 / (rrf_k + (rank as f64) + 1.0);
                fused_map
                    .entry(chunk.id.clone())
                    .and_modify(|existing| {
                        existing.rrf_score += score;
                        for rel in chunk.relations.clone() {
                            if !existing.relations.contains(&rel) {
                                existing.relations.push(rel);
                            }
                        }
                    })
                    .or_insert(chunk);
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

    /// 2nd-Hop Graph Expansion: Given a list of discovered entity names (from Hop-1 top chunks),
    /// traverses SurrealDB's `mentions` and `relates_to` edges in reverse to retrieve chunks
    /// that share or connect to these entities, sorted by the number of matching entities.
    pub async fn fetch_graph_hop_chunks(
        &self,
        entity_names: &[String],
        exclude_chunk_ids: &[String],
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        if entity_names.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        // Convert entity names into deterministic SurrealDB entity record keys (`entity:<key>`)
        let entity_keys: Vec<String> = entity_names
            .iter()
            .map(|n| sanitize_key(n))
            .filter(|k| !k.is_empty())
            .collect();

        if entity_keys.is_empty() {
            return Ok(vec![]);
        }

        let sql = format!(
            r#"
            LET $target_names = {names_json};
            SELECT
                record::id(id) AS id,
                doc_id,
                doc_title,
                chunk_index,
                content,
                0.0 AS rrf_score,
                ->mentions->entity.{{name, entity_type, description}} AS entities,
                ->mentions->entity->relates_to->entity.{{name, entity_type, description}} AS related_entities,
                ->mentions->entity->relates_to.{{ from: in.name, relation: relation_type, to: out.name }} AS relations_out,
                ->mentions->entity<-relates_to.{{ from: in.name, relation: relation_type, to: out.name }} AS relations_in
            FROM chunk
            WHERE ->mentions->entity.name ANYINSIDE $target_names
               OR ->mentions->entity->relates_to->entity.name ANYINSIDE $target_names
               OR ->mentions->entity<-relates_to<-entity.name ANYINSIDE $target_names
            LIMIT 60;
            "#,
            names_json = serde_json::to_string(entity_names)?
        );

        let raw_responses = self.execute_sql(&sql).await?;
        let items = extract_result_array(raw_responses.get(1));

        let target_lower: std::collections::HashSet<String> =
            entity_names.iter().map(|s| s.to_lowercase()).collect();
        let excluded: std::collections::HashSet<&str> =
            exclude_chunk_ids.iter().map(|s| s.as_str()).collect();

        let mut scored_chunks: Vec<(usize, RetrievedChunk)> = Vec::new();
        for val in items {
            if let Ok(mut chunk) = serde_json::from_value::<RetrievedChunk>(val) {
                if excluded.contains(chunk.id.as_str()) {
                    continue;
                }
                finalize_chunk_relations(&mut chunk);

                // Count how many distinct target entities this chunk mentions or relates to
                let mut matched_entities = std::collections::HashSet::new();
                for e in &chunk.entities {
                    let low = e.name.to_lowercase();
                    if target_lower.contains(&low) {
                        matched_entities.insert(low);
                    }
                }
                for rel in &chunk.relations {
                    let f_low = rel.from.to_lowercase();
                    let t_low = rel.to.to_lowercase();
                    if target_lower.contains(&f_low) {
                        matched_entities.insert(f_low);
                    }
                    if target_lower.contains(&t_low) {
                        matched_entities.insert(t_low);
                    }
                }

                let overlap_count = matched_entities.len().max(1);
                chunk.rrf_score = (overlap_count as f64) * 0.015;
                scored_chunks.push((overlap_count, chunk));
            }
        }

        // Prioritize chunks that share the highest number of entities (e.g. Rust + SQL + Project Alpha)
        scored_chunks.sort_by(|a, b| b.0.cmp(&a.0));

        let mut out: Vec<RetrievedChunk> = scored_chunks.into_iter().map(|(_, c)| c).collect();
        out.truncate(limit);
        Ok(out)
    }

    /// Deletes all documents, chunks, graph entities and relations.
    pub async fn delete_all_data(&self) -> Result<()> {
        let sql = r#"
            DELETE chunk;
            DELETE document;
            DELETE entity;
            DELETE mentions;
            DELETE relates_to;
        "#;
        self.execute_sql(sql).await?;
        Ok(())
    }

    /// Loads persisted system LLM settings from SurrealDB (if saved previously).
    pub async fn get_system_llm_settings(&self) -> Result<Option<(String, String, Option<String>, String)>> {
        let sql = "SELECT llm_provider, llm_base_url, llm_api_key, llm_model FROM system_settings:app;";
        let res = self.execute_sql(sql).await?;
        if let Some(first_resp) = res.first() {
            if let Some(arr) = first_resp.get("result").and_then(|v| v.as_array()) {
                if let Some(item) = arr.first() {
                    let prov = item.get("llm_provider").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let url = item.get("llm_base_url").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let key = item.get("llm_api_key").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let model = item.get("llm_model").and_then(|v| v.as_str()).unwrap_or_default().to_string();
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

fn extract_result_array(statement_res: Option<&Value>) -> Vec<Value> {
    statement_res
        .and_then(|v| v.get("result"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default()
}

fn finalize_chunk_relations(chunk: &mut RetrievedChunk) {
    chunk.relations.extend(chunk.relations_out.clone());
    chunk.relations.extend(chunk.relations_in.clone());
    let mut seen = std::collections::HashSet::new();
    chunk.relations.retain(|r| {
        !r.from.is_empty()
            && !r.to.is_empty()
            && seen.insert(format!(
                "{}:{}:{}",
                r.from.to_lowercase(),
                r.relation.to_lowercase(),
                r.to.to_lowercase()
            ))
    });
}

fn deserialize_graph_entities<'de, D>(deserializer: D) -> std::result::Result<Vec<GraphEntity>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<Value> = serde::Deserialize::deserialize(deserializer)?;
    let mut results = Vec::new();
    if let Some(val) = opt {
        flatten_entities(&val, &mut results);
    }
    let mut seen = std::collections::HashSet::new();
    results.retain(|e| !e.name.is_empty() && seen.insert(e.name.to_lowercase()));
    Ok(results)
}

fn flatten_entities(val: &Value, out: &mut Vec<GraphEntity>) {
    match val {
        Value::Array(arr) => {
            for item in arr {
                flatten_entities(item, out);
            }
        }
        Value::Object(obj) => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            let entity_type = obj
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("Entity")
                .trim()
                .to_string();
            let description = obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if !name.is_empty() {
                out.push(GraphEntity {
                    name,
                    entity_type,
                    description,
                });
            }
        }
        _ => {}
    }
}

fn deserialize_graph_relations<'de, D>(deserializer: D) -> std::result::Result<Vec<GraphRelation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<Value> = serde::Deserialize::deserialize(deserializer)?;
    let mut results = Vec::new();
    if let Some(val) = opt {
        flatten_relations(&val, &mut results);
    }
    let mut seen = std::collections::HashSet::new();
    results.retain(|r| {
        !r.from.is_empty()
            && !r.to.is_empty()
            && seen.insert(format!(
                "{}:{}:{}",
                r.from.to_lowercase(),
                r.relation.to_lowercase(),
                r.to.to_lowercase()
            ))
    });
    Ok(results)
}

fn flatten_relations(val: &Value, out: &mut Vec<GraphRelation>) {
    match val {
        Value::Array(arr) => {
            for item in arr {
                flatten_relations(item, out);
            }
        }
        Value::Object(obj) => {
            let from = obj
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            let relation = obj
                .get("relation")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to")
                .trim()
                .to_string();
            let to = obj
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            if !from.is_empty() && !to.is_empty() {
                out.push(GraphRelation { from, relation, to });
            }
        }
        _ => {}
    }
}

fn sanitize_key(input: &str) -> String {
    let sanitized: String = input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if sanitized.is_empty() {
        return sanitized;
    }
    // SurrealQL record identifiers must start with an alphabetic character
    if sanitized.chars().next().map_or(false, |c| !c.is_ascii_alphabetic()) {
        format!("e_{}", sanitized)
    } else {
        sanitized
    }
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
