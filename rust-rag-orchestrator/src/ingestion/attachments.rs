use crate::{db::RetrievedChunk, ingestion::InferenceClient, AppState};
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachment {
    pub filename: String,
    pub text: String,
}

/// In-memory text extraction for ephemeral chat attachments without storing into SurrealDB (`POST /v1/extract`).
pub async fn extract_ephemeral_file_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded_file: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = field
            .file_name()
            .unwrap_or("attached_file.pdf")
            .to_string();
        if let Ok(bytes) = field.bytes().await {
            if !bytes.is_empty() {
                uploaded_file = Some((filename, bytes.to_vec()));
                break;
            }
        }
    }

    let Some((filename, file_bytes)) = uploaded_file else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No file found in multipart request." })),
        );
    };

    let t_extract = std::time::Instant::now();
    let fname_clone = filename.clone();
    let extraction_res = tokio::task::spawn_blocking(move || {
        InferenceClient::extract_text_from_bytes(&fname_clone, &file_bytes)
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("Text extraction task error: {}", e)));

    match extraction_res {
        Ok(text) => {
            let trimmed = text.trim().to_string();
            let chunks = state
                .inference
                .semantic_chunk_text_with_title(Some(&filename), &trimmed);
            let extract_ms = t_extract.elapsed().as_millis() as u64;
            info!(
                "Ephemeral chat attachment '{}' extracted in memory ({} chars, {} chunks in {}ms).",
                filename,
                trimmed.chars().count(),
                chunks.len(),
                extract_ms
            );
            (
                StatusCode::OK,
                Json(json!({
                    "status": "extracted_in_memory",
                    "filename": filename,
                    "char_count": trimmed.chars().count(),
                    "chunk_count": chunks.len(),
                    "extract_ms": extract_ms,
                    "text": trimmed
                })),
            )
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": format!("{}: {}", filename, e) })),
        ),
    }
}

/// Prepares chunks from an in-memory chat attachment for querying.
pub async fn prepare_ephemeral_attachment_chunks(
    state: &AppState,
    query: &str,
    attachment: &ChatAttachment,
    reranker_mode: &str,
) -> (Vec<RetrievedChunk>, u64, String) {
    let t_rerank = std::time::Instant::now();
    let raw_chunks = state
        .inference
        .semantic_chunk_text_with_title(Some(&attachment.filename), &attachment.text);
    let doc_label = format!("📎 {} (Ephemeral Chat Attachment)", attachment.filename);

    let all_candidates: Vec<RetrievedChunk> = raw_chunks
        .into_iter()
        .enumerate()
        .map(|(idx, c_text)| RetrievedChunk {
            id: format!("ephemeral_{}", idx),
            doc_id: "ephemeral_attachment".to_string(),
            doc_title: doc_label.clone(),
            chunk_index: idx as i64,
            content: c_text,
            rrf_score: 1.0,
            rerank_score: Some(1.0),
            entities: vec![],
            related_entities: vec![],
            relations_out: vec![],
            relations_in: vec![],
            relations: vec![],
        })
        .collect();

    if all_candidates.len() <= 4 || query.trim().is_empty() {
        return (
            all_candidates,
            0,
            "📎 Full text in RAM (No DB)".to_string(),
        );
    }

    let first_chunk = all_candidates.first().cloned();
    let candidates_for_rerank = if all_candidates.len() > 24 {
        let q_lower = query.to_lowercase();
        let q_terms: Vec<&str> = q_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 3)
            .collect();
        let mut scored: Vec<(usize, RetrievedChunk)> = all_candidates
            .into_iter()
            .map(|c| {
                let c_low = c.content.to_lowercase();
                let hits = q_terms.iter().filter(|&&t| c_low.contains(t)).count();
                let bonus = if c.chunk_index < 2 { 2 } else { 0 };
                (hits + bonus, c)
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.chunk_index.cmp(&b.1.chunk_index)));
        scored.into_iter().take(20).map(|(_, c)| c).collect()
    } else {
        all_candidates
    };

    let mode = if reranker_mode.eq_ignore_ascii_case("none") {
        "fast"
    } else {
        reranker_mode
    };

    match state
        .inference
        .rerank_chunks(query, candidates_for_rerank.clone(), 4, mode)
        .await
    {
        Ok((mut top_chunks, model_label)) => {
            if let Some(first) = first_chunk {
                if !top_chunks.iter().any(|c| c.chunk_index == 0) {
                    top_chunks.insert(0, first);
                    if top_chunks.len() > 5 {
                        top_chunks.truncate(5);
                    }
                }
            }
            let elapsed = t_rerank.elapsed().as_millis() as u64;
            (
                top_chunks,
                elapsed,
                format!("📎 In-Memory Rerank ({})", model_label),
            )
        }
        Err(_) => {
            let mut fallback = candidates_for_rerank;
            fallback.truncate(4);
            (
                fallback,
                t_rerank.elapsed().as_millis() as u64,
                "📎 In-Memory Selection".to_string(),
            )
        }
    }
}
