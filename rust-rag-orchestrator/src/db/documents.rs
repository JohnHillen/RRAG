use super::search::flatten_relations;
use crate::{
    db::{extract_result_array, sanitize_key, DocumentSummary, RetrievedChunk, SurrealClient},
    ingestion::pipelines::invoice::{
        extract_query_years, format_financial_aggregation_markdown, FinancialDocMetadata,
    },
};
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

    /// Checks whether a document already has stored graph relations in `document.metadata.relations`.
    pub async fn doc_has_graph_relations(&self, doc_id: &str) -> Result<bool> {
        let sql = format!(
            "SELECT metadata.relations AS rels FROM type::record('document', {});",
            serde_json::to_string(doc_id)?
        );
        let res = self.execute_sql(&sql).await?;
        let arr = extract_result_array(res.first());
        let has_rels = arr
            .first()
            .and_then(|v| v.get("rels"))
            .and_then(|r| r.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        Ok(has_rels)
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

        let full_doc_text: String = chunks
            .iter()
            .map(|(_, c, _)| c.as_str())
            .collect::<Vec<&str>>()
            .join("\n");
        let financial_meta = FinancialDocMetadata::extract(doc_id, title, &full_doc_text);

        let doc_json = json!({
            "title": title,
            "source": source,
            "mime_type": if raw_pdf_b64.is_some() { "application/pdf" } else { "text/plain" },
            "metadata": {
                "relations": rel_strings,
                "financial": financial_meta,
                "raw_b64": raw_pdf_b64,
            }
        });
        let doc_stmt = format!(
            "UPSERT type::record('document', {}) CONTENT {};\n",
            serde_json::to_string(doc_id)?,
            doc_json
        );
        self.execute_sql(&doc_stmt).await?;

        for batch in chunks.chunks(80) {
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
                let entity_json = match description {
                    Some(desc) if !desc.trim().is_empty() => json!({
                        "name": name,
                        "entity_type": entity_type,
                        "description": desc,
                    }),
                    _ => json!({
                        "name": name,
                        "entity_type": entity_type,
                    }),
                };
                graph_sql.push_str(&format!(
                    "UPSERT type::record('entity', {}) CONTENT {};\n",
                    serde_json::to_string(&clean_key)?,
                    entity_json
                ));

                let mut matched_count = 0usize;
                let name_lower = name.to_lowercase();
                let desc_lower = description
                    .as_deref()
                    .map(|d| d.trim().to_lowercase())
                    .filter(|d| d.len() >= 3 && d.len() <= 40);

                for (idx, content, _) in chunks {
                    // Match against chunk body (skipping the synthetic `[Dokument: ...]` metadata line if present)
                    let body = match content.find('\n') {
                        Some(pos) if content.starts_with('[') => &content[pos + 1..],
                        _ => content.as_str(),
                    };
                    let body_low = body.to_lowercase();
                    let is_match = body_low.contains(&name_lower)
                        || desc_lower
                            .as_ref()
                            .map_or(false, |d| body_low.contains(d.as_str()));
                    if is_match {
                        matched_count += 1;
                        let chunk_key = format!("{}_{}", doc_id, idx);
                        graph_sql.push_str(&format!(
                            "RELATE chunk:{}->mentions->entity:{};\n",
                            chunk_key, clean_key
                        ));
                        if matched_count >= 12 {
                            break;
                        }
                    }
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

    /// Performs a fast (<20 ms) full-collection financial & document aggregation over all documents
    /// in the active SurrealDB database when the user asks for totals, sums, or year/category expenses.
    pub async fn aggregate_financial_documents(
        &self,
        query: &str,
    ) -> Result<Option<RetrievedChunk>> {
        let sql = "SELECT doc_id, doc_title, chunk_index, content FROM chunk ORDER BY doc_title ASC, chunk_index ASC;";
        let res = self.execute_sql(sql).await?;
        let rows = extract_result_array(res.first());
        if rows.is_empty() {
            return Ok(None);
        }

        let mut docs_map: std::collections::BTreeMap<String, (String, Vec<String>)> =
            std::collections::BTreeMap::new();
        for r in rows {
            let doc_id = r["doc_id"].as_str().unwrap_or_default().to_string();
            let doc_title = r["doc_title"].as_str().unwrap_or_default().to_string();
            let content = r["content"].as_str().unwrap_or_default().to_string();
            if doc_id.is_empty() || doc_title.is_empty() {
                continue;
            }
            let entry = docs_map
                .entry(doc_title.clone())
                .or_insert_with(|| (doc_id, Vec::new()));
            entry.1.push(content);
        }

        let mut all_meta: Vec<FinancialDocMetadata> = Vec::with_capacity(docs_map.len());
        let mut positive_amount_docs = 0usize;
        for (doc_title, (doc_id, chunks)) in docs_map {
            let full_text = chunks.join("\n");
            let meta = FinancialDocMetadata::extract(&doc_id, &doc_title, &full_text);
            if meta.amount_gross.unwrap_or(0.0) > 0.0 {
                positive_amount_docs += 1;
            }
            all_meta.push(meta);
        }

        if positive_amount_docs < 2 {
            return Ok(None);
        }

        let filter_years = extract_query_years(query);
        let q_low = query.to_lowercase();

        // Dynamically detect if the query asks for a specific subfolder/category group present in the DB
        let mut distinct_cats: Vec<String> = all_meta
            .iter()
            .map(|m| m.category_group.clone())
            .filter(|c| {
                let cl = c.to_lowercase();
                c.len() >= 3
                    && !cl.contains("einzelbelege")
                    && !cl.contains("allgemein")
                    && !c.chars().all(|ch| ch.is_ascii_digit())
            })
            .collect();
        distinct_cats.sort();
        distinct_cats.dedup();
        // Sort longest category name first so specific names match before shorter prefixes
        distinct_cats.sort_by_key(|b| std::cmp::Reverse(b.len()));

        let matched_category = distinct_cats.into_iter().find(|cat| {
            let cl = cat.to_lowercase();
            q_low.contains(&cl)
        });

        let report_md = format_financial_aggregation_markdown(
            &all_meta,
            &filter_years,
            matched_category.as_deref(),
        );
        if report_md.is_empty() {
            return Ok(None);
        }

        let scope_title = if !filter_years.is_empty() {
            let yrs: Vec<String> = filter_years.iter().map(|y| y.to_string()).collect();
            format!(
                "📊 SurrealDB Exakte Rechnungs-Aggregation ({})",
                yrs.join(", ")
            )
        } else {
            "📊 SurrealDB Exakte Rechnungs-Aggregation (Alle Jahre)".to_string()
        };

        Ok(Some(RetrievedChunk {
            id: "surrealdb_financial_aggregation".to_string(),
            doc_id: "surrealdb_financial_aggregation".to_string(),
            doc_title: scope_title,
            chunk_index: 0,
            content: report_md,
            rrf_score: 1.0,
            rerank_score: Some(1.0),
            entities: vec![],
            related_entities: vec![],
            relations_out: vec![],
            relations_in: vec![],
            relations: vec![],
        }))
    }
}
