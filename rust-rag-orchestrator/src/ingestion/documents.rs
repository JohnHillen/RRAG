use crate::{db, ingestion::InferenceClient, pdf, AppState};
use anyhow::Result;
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct IngestJsonRequest {
    pub title: String,
    pub text: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub replace_revision: bool,
    #[serde(default)]
    pub extract_graph: Option<bool>,
}

fn default_source() -> String {
    "api_upload".to_string()
}

fn default_true() -> bool {
    true
}

/// Lists all indexed documents in SurrealDB (`GET /v1/documents`)
pub async fn list_documents_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_documents().await {
        Ok(docs) => (StatusCode::OK, Json(json!({ "documents": docs }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Deletes a document and its associated chunks (`DELETE /v1/documents/:doc_id`)
pub async fn delete_document_handler(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_document(&doc_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "status": "deleted", "doc_id": doc_id })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Cleans up older duplicate document revisions (`POST /v1/documents/deduplicate`)
pub async fn deduplicate_documents_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.deduplicate_by_title().await {
        Ok(removed) => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "removed_documents": removed })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Ingests raw text/markdown payload (`POST /v1/ingest`)
pub async fn ingest_json_handler(
    State(state): State<AppState>,
    Json(req): Json<IngestJsonRequest>,
) -> impl IntoResponse {
    let full_title = format_title_with_category(&req.title, req.category.as_deref());
    match process_document_ingestion(
        &state,
        None,
        &full_title,
        &req.source,
        &req.text,
        req.replace_revision,
        req.category.as_deref(),
        req.extract_graph,
        None,
    )
    .await
    {
        Ok(val) => (StatusCode::OK, Json(val)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Multipart file upload handler (PDF, HTML, Markdown, TXT) (`POST /v1/ingest/file`)
pub async fn ingest_file_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut category: Option<String> = None;
    let mut replace_revision = true;
    let mut extract_graph: Option<bool> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name == "category" {
            if let Ok(txt) = field.text().await {
                let trimmed = txt.trim().to_string();
                if !trimmed.is_empty() {
                    category = Some(trimmed);
                }
            }
            continue;
        }
        if field_name == "replace_revision" {
            if let Ok(txt) = field.text().await {
                replace_revision = txt.trim() != "false" && txt.trim() != "0";
            }
            continue;
        }
        if field_name == "extract_graph" {
            if let Ok(txt) = field.text().await {
                let t = txt.trim();
                extract_graph = Some(t == "true" || t == "1");
            }
            continue;
        }

        let filename = field
            .file_name()
            .unwrap_or("uploaded_document.txt")
            .to_string();
        if let Ok(bytes) = field.bytes().await {
            if !bytes.is_empty() {
                uploaded_files.push((filename, bytes.to_vec()));
            }
        }
    }

    if uploaded_files.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No files found in multipart request." })),
        );
    }

    let mut total_chunks = 0usize;
    let mut total_graph_entities = 0usize;
    let mut total_graph_relations = 0usize;
    let mut skipped_duplicates = 0usize;
    let mut replaced_revisions = 0usize;
    let mut processed_docs = Vec::new();
    let mut errors = Vec::new();

    for (raw_filename, file_bytes) in uploaded_files {
        let full_title = format_title_with_category(&raw_filename, category.as_deref());
        let file_hash_id = db::content_hash_id(&file_bytes);

        if extract_graph != Some(true) {
            if let Ok(existing_chunks) = state.db.count_chunks_for_doc(&file_hash_id).await {
                if existing_chunks > 0 {
                    info!(
                        "File '{}' (Hash: {}) already exists with {} chunks in SurrealDB -> skipping.",
                        full_title, file_hash_id, existing_chunks
                    );
                    skipped_duplicates += 1;
                    processed_docs.push(json!({
                        "filename": full_title,
                        "doc_id": file_hash_id,
                        "chunks_indexed": existing_chunks,
                        "skipped_duplicate": true,
                        "replaced_revision": false
                    }));
                    continue;
                }
            }
        }

        let raw_pdf_b64 = if raw_filename.to_lowercase().ends_with(".pdf")
            && file_bytes.len() <= 15 * 1024 * 1024
        {
            Some(pdf::base64_encode(&file_bytes))
        } else {
            None
        };
        let fname_clone = raw_filename.clone();
        let extraction_res = tokio::task::spawn_blocking(move || {
            InferenceClient::extract_text_from_bytes(&fname_clone, &file_bytes)
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!("Text extraction task error: {}", e)));

        match extraction_res {
            Ok(extracted_text) => {
                match process_document_ingestion(
                    &state,
                    Some(file_hash_id),
                    &full_title,
                    "file_upload",
                    &extracted_text,
                    replace_revision,
                    category.as_deref(),
                    extract_graph,
                    raw_pdf_b64.as_deref(),
                )
                .await
                {
                    Ok(val) => {
                        let is_dup = val["skipped_duplicate"].as_bool().unwrap_or(false);
                        let is_rev = val["replaced_revision"].as_bool().unwrap_or(false);
                        let chunks = val["chunks_indexed"].as_u64().unwrap_or(0) as usize;
                        let g_ent =
                            val["graph_entities_extracted"].as_u64().unwrap_or(0) as usize;
                        let g_rel =
                            val["graph_relations_extracted"].as_u64().unwrap_or(0) as usize;
                        total_graph_entities += g_ent;
                        total_graph_relations += g_rel;
                        if is_dup {
                            skipped_duplicates += 1;
                        } else {
                            total_chunks += chunks;
                        }
                        if is_rev {
                            replaced_revisions += 1;
                        }
                        processed_docs.push(json!({
                            "filename": full_title,
                            "chunks_indexed": chunks,
                            "graph_entities_extracted": g_ent,
                            "graph_relations_extracted": g_rel,
                            "skipped_duplicate": is_dup,
                            "replaced_revision": is_rev
                        }));
                    }
                    Err(e) => errors.push(format!("{}: {}", full_title, e)),
                }
            }
            Err(e) => errors.push(format!("{} (Text extraction): {}", full_title, e)),
        }
    }

    if processed_docs.is_empty() && !errors.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": errors.join(" | ") })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "status": "indexed",
            "files_processed": processed_docs.len(),
            "skipped_duplicates": skipped_duplicates,
            "replaced_revisions": replaced_revisions,
            "chunks_indexed": total_chunks,
            "graph_entities_extracted": total_graph_entities,
            "graph_relations_extracted": total_graph_relations,
            "documents": processed_docs,
            "errors": errors
        })),
    )
}

fn format_title_with_category(title: &str, category: Option<&str>) -> String {
    match category.map(|c| c.trim()).filter(|c| !c.is_empty()) {
        Some(cat) => format!("[{}] {}", cat, title.trim()),
        None => title.trim().to_string(),
    }
}

/// Shared document ingestion logic (content-hash check -> revision replacement -> chunking -> embedding -> SurrealDB)
async fn process_document_ingestion(
    state: &AppState,
    explicit_doc_id: Option<String>,
    title: &str,
    source: &str,
    text: &str,
    replace_revision: bool,
    category: Option<&str>,
    extract_graph: Option<bool>,
    raw_pdf_b64: Option<&str>,
) -> Result<Value> {
    let text_hash_id = db::content_hash_id(text.trim().as_bytes());
    let doc_id = explicit_doc_id.unwrap_or_else(|| text_hash_id.clone());

    if extract_graph != Some(true) {
        if let Ok(existing) = state.db.count_chunks_for_doc(&doc_id).await {
            if existing > 0 {
                return Ok(json!({
                    "status": "skipped_duplicate",
                    "skipped_duplicate": true,
                    "replaced_revision": false,
                    "doc_id": doc_id,
                    "title": title,
                    "chunks_indexed": existing,
                    "graph_entities_extracted": 0,
                    "graph_relations_extracted": 0
                }));
            }
        }
    }

    let chunks_text = state
        .inference
        .semantic_chunk_text_with_title(Some(title), text);
    if chunks_text.is_empty() {
        anyhow::bail!("Document does not contain any usable text.");
    }

    let embeddings = state.inference.embed_texts(&chunks_text).await?;
    let mut indexed_chunks = Vec::with_capacity(chunks_text.len());
    for (idx, (chunk_str, emb)) in chunks_text.into_iter().zip(embeddings.into_iter()).enumerate() {
        indexed_chunks.push((idx, chunk_str, emb));
    }

    let (mut entities, mut relations) = state.llm.extract_graph_entities(text, extract_graph).await?;

    if let Some(cat) = category.map(|c| c.trim()).filter(|c| !c.is_empty()) {
        if extract_graph.unwrap_or(state.config.enable_graph_extraction) {
            for (ent_name, _, _) in &entities {
                if !ent_name.eq_ignore_ascii_case(cat) {
                    relations.push((
                        ent_name.clone(),
                        "belongs_to_project".to_string(),
                        cat.to_string(),
                    ));
                }
            }
            entities.push((
                cat.to_string(),
                "Project/Folder".to_string(),
                Some(format!("Parent Project / Category ({})", cat)),
            ));
        }
    }

    let mut replaced_revision = false;
    if replace_revision {
        if let Ok(replaced) = state.db.replace_older_revisions(title, &doc_id).await {
            if replaced {
                info!(
                    "Older revision of '{}' replaced by new version (Hash: {}).",
                    title, doc_id
                );
                replaced_revision = true;
            }
        }
    }

    state
        .db
        .insert_document_and_chunks(
            &doc_id,
            title,
            source,
            &indexed_chunks,
            &entities,
            &relations,
            raw_pdf_b64,
        )
        .await?;

    Ok(json!({
        "status": "indexed",
        "skipped_duplicate": false,
        "replaced_revision": replaced_revision,
        "doc_id": doc_id,
        "title": title,
        "chunks_indexed": indexed_chunks.len(),
        "graph_entities_extracted": entities.len(),
        "graph_relations_extracted": relations.len()
    }))
}
