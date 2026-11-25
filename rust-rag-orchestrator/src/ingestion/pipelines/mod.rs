pub mod invoice;
pub mod legal;

use crate::{ingestion::InferenceClient, AppState};
use anyhow::Result;

/// Pluggable Ingestion Pipeline selector.
/// Allows routing different document classes through specialized extraction, chunking,
/// and graph-building pipelines while keeping the core RAG orchestrator 100% generic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionPipelineKind {
    /// Automatically selects `LegalStatute` for statutory XMLs (`gii-norm.dtd` / EU `Formex`)
    /// and `Generic` for general documents (PDF, Markdown, HTML, TXT, CSV, JSON).
    Auto,
    /// Generic document pipeline: text extraction -> structure-aware chunker -> optional LLM graph extraction.
    Generic,
    /// Legal & Statutory pipeline: norm-level XML/statute parser -> §/Art. chunker -> syntactic citation & norm graph extractor.
    LegalStatute,
}

impl IngestionPipelineKind {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_lowercase()).as_deref() {
            Some("legal" | "statute" | "law" | "gesetze") => Self::LegalStatute,
            Some("generic" | "standard" | "default") => Self::Generic,
            _ => Self::Auto,
        }
    }

    pub fn resolve(self, filename: &str, raw_bytes_or_text: &[u8]) -> Self {
        match self {
            Self::Generic => Self::Generic,
            Self::LegalStatute => Self::LegalStatute,
            Self::Auto => {
                if legal::is_statutory_xml_or_text(filename, raw_bytes_or_text) {
                    Self::LegalStatute
                } else {
                    Self::Generic
                }
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Generic => "generic",
            Self::LegalStatute => "legal",
        }
    }
}

/// Output of an ingestion pipeline stage (before vector embedding and SurrealDB persistence).
pub struct PreparedDocumentPipeline {
    pub pipeline_used: &'static str,
    pub chunks: Vec<String>,
    pub entities: Vec<(String, String, Option<String>)>,
    pub relations: Vec<(String, String, String)>,
}

/// Executes the selected ingestion pipeline on raw file bytes (`POST /v1/ingest/file`).
pub async fn run_file_ingestion_pipeline(
    state: &AppState,
    pipeline_kind: IngestionPipelineKind,
    filename: &str,
    full_title: &str,
    file_bytes: &[u8],
    extract_graph: Option<bool>,
) -> Result<PreparedDocumentPipeline> {
    let resolved = pipeline_kind.resolve(filename, file_bytes);
    let fname_owned = filename.to_string();
    let bytes_owned = file_bytes.to_vec();

    let extracted_text = tokio::task::spawn_blocking(move || match resolved {
        IngestionPipelineKind::LegalStatute => {
            legal::extract_legal_document_text(&fname_owned, &bytes_owned)
        }
        _ => InferenceClient::extract_text_from_bytes(&fname_owned, &bytes_owned),
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("Text extraction task error: {}", e)))?;

    run_text_ingestion_pipeline(state, resolved, full_title, &extracted_text, extract_graph).await
}

/// Executes the selected ingestion pipeline on extracted or raw text (`POST /v1/ingest`).
pub async fn run_text_ingestion_pipeline(
    state: &AppState,
    pipeline_kind: IngestionPipelineKind,
    full_title: &str,
    text: &str,
    extract_graph: Option<bool>,
) -> Result<PreparedDocumentPipeline> {
    let resolved = pipeline_kind.resolve(full_title, text.as_bytes());
    let should_extract_graph = extract_graph.unwrap_or(state.config.enable_graph_extraction);

    match resolved {
        IngestionPipelineKind::LegalStatute => {
            let chunks = state
                .inference
                .semantic_chunk_text_with_title(Some(full_title), text);
            if chunks.is_empty() {
                anyhow::bail!("Document does not contain any usable statutory text.");
            }
            let (entities, relations) = if should_extract_graph {
                legal::extract_syntactic_legal_graph(full_title, text)
            } else {
                (Vec::new(), Vec::new())
            };
            Ok(PreparedDocumentPipeline {
                pipeline_used: resolved.label(),
                chunks,
                entities,
                relations,
            })
        }
        _ => {
            let chunks = state
                .inference
                .semantic_chunk_text_with_title(Some(full_title), text);
            if chunks.is_empty() {
                anyhow::bail!("Document does not contain any usable text.");
            }
            let (entities, relations) = state
                .llm
                .extract_graph_entities(text, Some(should_extract_graph))
                .await?;
            Ok(PreparedDocumentPipeline {
                pipeline_used: IngestionPipelineKind::Generic.label(),
                chunks,
                entities,
                relations,
            })
        }
    }
}
