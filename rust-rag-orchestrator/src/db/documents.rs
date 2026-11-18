use super::search::flatten_relations;
use crate::db::{extract_result_array, sanitize_key, DocumentSummary, SurrealClient};
use anyhow::Result;
use serde_json::json;

impl SurrealClient {
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

    /// Replaces older revisions of a document sharing the same `title` (but differing content hash `doc_id`).
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

        let chunk_items = extract_result_array(res.first());

        let mut doc_relations_map: std::collections::HashMap<
            String,
            std::collections::HashSet<String>,
        > = std::collections::HashMap::new();

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

        for d in extract_result_array(res.get(2)) {
            let did = d["doc_id"].as_str().unwrap_or_default().to_string();
            let rel_arr = d["relations"]
                .as_array()
                .or_else(|| d["metadata"]["relations"].as_array());
            if let Some(arr) = rel_arr {
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
        raw_pdf_b64: Option<&str>,
    ) -> Result<()> {
        let rel_strings: Vec<String> = relations
            .iter()
            .map(|(s, r, d)| format!("{} ──[{}]──► {}", s, r, d))
            .collect();

        let doc_json = json!({
            "title": title,
            "source": source,
            "mime_type": if raw_pdf_b64.is_some() { "application/pdf" } else { "text/plain" },
            "metadata": {
                "relations": rel_strings,
                "raw_b64": raw_pdf_b64,
            }
        });
        let doc_stmt = format!(
            "UPSERT type::record('document', {}) CONTENT {};\n",
            serde_json::to_string(doc_id)?,
            doc_json
        );
        self.execute_sql(&doc_stmt).await?;

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
}
