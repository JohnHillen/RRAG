pub mod sql_formatter;
pub mod text_to_sql;

use crate::{
    db::RetrievedChunk,
    llm::{ChatMessage, LlmClient},
    showcases, AppState,
};
use anyhow::Result;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct RetrieveRequest {
    pub query: String,
    pub top_n: Option<usize>,
    pub reranker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMetrics {
    pub embed_ms: u64,
    pub db_ms: u64,
    pub rerank_ms: u64,
    pub total_ms: u64,
    pub reranker_used: String,
}

#[derive(Debug, Serialize)]
pub struct RetrieveResponse {
    pub query: String,
    pub count: usize,
    pub results: Vec<RetrievedChunk>,
    pub metrics: RetrievalMetrics,
    pub formatted_context: String,
}

/// Standalone Retrieval API (`POST /v1/retrieve`)
pub async fn retrieve_handler(
    State(state): State<AppState>,
    Json(req): Json<RetrieveRequest>,
) -> impl IntoResponse {
    let top_n = req.top_n.unwrap_or(state.config.retrieval_final_top_n);
    let reranker_mode = req.reranker.as_deref().unwrap_or("fast");
    match execute_retrieval_pipeline(&state, &req.query, top_n, reranker_mode).await {
        Ok((results, metrics)) => {
            let formatted_context = LlmClient::format_rag_context(&results);
            let resp = RetrieveResponse {
                query: req.query,
                count: results.len(),
                results,
                metrics,
                formatted_context,
            };
            (StatusCode::OK, Json(json!(resp)))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Core retrieval pipeline (with automatic 2-Hop Graph Expansion):
/// 1. Hop-1: Query embedding via TEI + SurrealDB Hybrid Search (HNSW + BM25 + RRF)
/// 2. Hop-2 (Multi-Hop Expansion): Traverses Knowledge Graph entities & relations from Hop-1 top hits
/// 3. Joint Cross-Encoder reranking (`fast`, `high`, or `none`)
/// 4. Optional active-showcase SQL augmentation (only executed if a showcase is currently loaded).
pub async fn execute_retrieval_pipeline(
    state: &AppState,
    query: &str,
    top_n: usize,
    reranker_mode: &str,
) -> Result<(Vec<RetrievedChunk>, RetrievalMetrics)> {
    let t_total = std::time::Instant::now();

    let t_embed = std::time::Instant::now();
    let query_vecs = state.inference.embed_texts(&[query.to_string()]).await?;
    let query_embedding = query_vecs
        .first()
        .ok_or_else(|| anyhow::anyhow!("No query embedding received from TEI server"))?;
    let mut embed_ms = t_embed.elapsed().as_millis() as u64;

    let t_db = std::time::Instant::now();
    let mut candidates = state
        .db
        .hybrid_graph_search(query, query_embedding, state.config.retrieval_top_k_candidates)
        .await?;

    // --- Hop-2: Automatic Graph-Edge Expansion ---
    let mut hop2_entities: Vec<String> = Vec::new();
    let mut hop2_keywords: Vec<String> = Vec::new();
    let query_lower = query.to_lowercase();

    for anchor in candidates.iter().take(3) {
        for ent in &anchor.entities {
            let name = ent.name.trim();
            if !name.is_empty()
                && !hop2_entities.iter().any(|x| x.eq_ignore_ascii_case(name))
                && hop2_entities.len() < 10
            {
                hop2_entities.push(name.to_string());
            }
        }
        for rel in anchor.relations.iter().take(6) {
            for endpoint in [&rel.from, &rel.to] {
                let name = endpoint.trim();
                if !name.is_empty()
                    && name.len() >= 5
                    && !hop2_entities.iter().any(|x| x.eq_ignore_ascii_case(name))
                    && hop2_entities.len() < 10
                {
                    hop2_entities.push(name.to_string());
                }
            }
        }

        for kw in showcases::extract_bridge_terms_if_active(state, &anchor.content, &query_lower) {
            if !hop2_keywords.iter().any(|x| x.eq_ignore_ascii_case(&kw)) {
                hop2_keywords.push(kw);
            }
        }
    }

    let mut hop2_added = 0usize;
    let mut hop2_triggered = false;
    let existing_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();

    if !hop2_entities.is_empty() {
        hop2_triggered = true;
        if let Ok(graph_chunks) = state
            .db
            .fetch_graph_hop_chunks(&hop2_entities, &existing_ids, 8)
            .await
        {
            for gc in graph_chunks {
                if !candidates.iter().any(|c| c.id == gc.id) {
                    candidates.push(gc);
                    hop2_added += 1;
                }
            }
        }
    }

    let mut enriched_rerank_query = query.to_string();
    if !hop2_keywords.is_empty() {
        let mut expansion_terms: Vec<String> = Vec::new();
        showcases::expand_hop2_terms_if_active(state, &query_lower, &mut expansion_terms);

        for k in hop2_keywords.iter().take(6) {
            if !expansion_terms.iter().any(|x| x.eq_ignore_ascii_case(k)) {
                expansion_terms.push(k.clone());
            }
        }

        if !expansion_terms.is_empty() {
            hop2_triggered = true;
            let hop2_target_query = expansion_terms.join(" ");
            let sub_query = format!("{} ({})", query, hop2_target_query);
            enriched_rerank_query = sub_query;

            let t_emb2 = std::time::Instant::now();
            if let Ok(vecs2) = state.inference.embed_texts(&[hop2_target_query.clone()]).await {
                embed_ms += t_emb2.elapsed().as_millis() as u64;
                if let Some(emb2) = vecs2.first() {
                    if let Ok(hop2_candidates) = state
                        .db
                        .hybrid_graph_search(&hop2_target_query, emb2, 10)
                        .await
                    {
                        for hc in hop2_candidates {
                            if !candidates.iter().any(|c| c.id == hc.id) {
                                candidates.push(hc);
                                hop2_added += 1;
                            } else if let Some(existing) =
                                candidates.iter_mut().find(|c| c.id == hc.id)
                            {
                                existing.rrf_score += hc.rrf_score * 1.1;
                            }
                        }
                    }
                }
            }
        }
    }

    candidates.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut db_ms = t_db.elapsed().as_millis() as u64;

    let t_rerank = std::time::Instant::now();
    let pool_len = candidates.len().max(top_n);
    let (ranked_pool, mut reranker_used) = state
        .inference
        .rerank_chunks(&enriched_rerank_query, candidates, pool_len, reranker_mode)
        .await?;
    let rerank_ms = t_rerank.elapsed().as_millis() as u64;

    if hop2_triggered {
        if hop2_added > 0 {
            reranker_used = format!("{} + 🕸️ 2-Hop (+{} Chunks)", reranker_used, hop2_added);
        } else {
            reranker_used = format!("{} + 🕸️ 2-Hop Query-Expansion", reranker_used);
        }
    }

    let primary_anchor = ranked_pool.first().cloned();
    let mut final_chunks = showcases::augment_for_active_showcase(
        state,
        query,
        ranked_pool,
        top_n,
        &mut db_ms,
        &mut reranker_used,
    )
    .await;

    if let Some(ref ext_schema) = state.config.external_db_schema_prompt {
        let t_ext = std::time::Instant::now();
        let ext_chunk_opt = if let Some(ref ext_client) = state.external_db {
            text_to_sql::execute_dynamic_text_to_sql_on_external(
                state,
                ext_client,
                query,
                primary_anchor.as_ref(),
                ext_schema,
                "external_db_live_query_result",
                "external_attached_database",
                "🔌 [Externe SQL-DB Text-to-SQL]",
            )
            .await
        } else {
            text_to_sql::execute_dynamic_text_to_sql_on_client(
                state,
                &state.db,
                query,
                primary_anchor.as_ref(),
                ext_schema,
                "external_db_live_query_result",
                "external_attached_database",
                "🔌 [Externe DB Text-to-SQL]",
            )
            .await
        };

        if let Some(ext_chunk) = ext_chunk_opt {
            db_ms += t_ext.elapsed().as_millis() as u64;
            let insert_idx = final_chunks.len().min(2);
            final_chunks.insert(insert_idx, ext_chunk);
            reranker_used = format!("{} + 🔌 Text-to-SQL (Externe DB)", reranker_used);
        }
    }

    let metrics = RetrievalMetrics {
        embed_ms,
        db_ms,
        rerank_ms,
        total_ms: t_total.elapsed().as_millis() as u64,
        reranker_used,
    };

    Ok((final_chunks, metrics))
}

/// Builds a context-aware retrieval query for multi-turn conversations.
pub fn build_conversational_retrieval_query(messages: &[ChatMessage]) -> String {
    let user_msgs: Vec<&str> = messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let Some(&latest) = user_msgs.last() else {
        return String::new();
    };

    if user_msgs.len() < 2 {
        return latest.to_string();
    }

    let prev_user = user_msgs[user_msgs.len() - 2];
    let latest_lower = latest.to_lowercase();

    let is_followup = latest.chars().count() < 110
        || [
            "noch",
            "weitere",
            "andere",
            "sonst",
            "dafür",
            "dort",
            "diesem",
            "dieser",
            "dieses",
            "weiterbildung",
            "other",
            "more",
            "else",
            "also",
        ]
        .iter()
        .any(|w| latest_lower.contains(w));

    if is_followup {
        format!("{} (Kontext: {})", latest, prev_user)
    } else {
        latest.to_string()
    }
}
