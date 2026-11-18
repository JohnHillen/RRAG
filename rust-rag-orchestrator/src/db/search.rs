use crate::db::{
    content_hash_id, extract_result_array, sanitize_key, GraphEntity, GraphRelation,
    RetrievedChunk, SurrealClient,
};
use anyhow::Result;
use serde_json::Value;

impl SurrealClient {
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

        let vec_items = extract_result_array(raw_responses.get(2));
        let bm25_items = extract_result_array(raw_responses.get(3));

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

        let mut seen_content_hashes = std::collections::HashSet::new();
        results.retain(|c| seen_content_hashes.insert(content_hash_id(c.content.trim().as_bytes())));

        results.truncate(top_k);
        Ok(results)
    }

    /// 2nd-Hop Graph Expansion: Given a list of discovered entity names (from Hop-1 top chunks),
    /// traverses SurrealDB's `mentions` and `relates_to` edges in reverse to retrieve chunks
    /// that share or connect to these entities.
    pub async fn fetch_graph_hop_chunks(
        &self,
        entity_names: &[String],
        exclude_chunk_ids: &[String],
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        if entity_names.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

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

        scored_chunks.sort_by(|a, b| b.0.cmp(&a.0));

        let mut out: Vec<RetrievedChunk> = scored_chunks.into_iter().map(|(_, c)| c).collect();
        out.truncate(limit);
        Ok(out)
    }
}

pub(crate) fn finalize_chunk_relations(chunk: &mut RetrievedChunk) {
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

pub(crate) fn deserialize_graph_entities<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<GraphEntity>, D::Error>
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

pub(crate) fn deserialize_graph_relations<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<GraphRelation>, D::Error>
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

pub(crate) fn flatten_relations(val: &Value, out: &mut Vec<GraphRelation>) {
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
        "der", "die", "das", "den", "dem", "des", "ein", "eine", "einer", "einem", "einen", "und",
        "oder", "aber", "von", "vom", "zum", "zur", "mit", "bei", "für", "auf", "aus", "nach",
        "über", "unter", "vor", "hinter", "wie", "was", "wer", "wo", "wann", "warum", "welche",
        "welcher", "welches", "welchem", "welchen", "hat", "haben", "ist", "sind", "war", "waren",
        "wird", "werden", "kann", "können", "muss", "müssen", "soll", "sollen", "bitte", "mir",
        "mich", "ich", "du", "wir", "ihr", "sie", "the", "and", "for", "with",
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
            if compact.len() >= 3
                && !STOPWORDS.contains(&compact.as_str())
                && !keywords.contains(&compact)
            {
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
