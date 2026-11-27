pub mod formats;
pub mod invoice;
pub mod legal;

use crate::{ingestion::InferenceClient, AppState};
use anyhow::Result;

/// Pluggable Ingestion Pipeline selector.
/// Allows routing different document classes through specialized extraction, chunking,
/// and graph-building pipelines while keeping the core RAG orchestrator 100% generic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionPipelineKind {
    /// Automatically detects the right format/domain pipeline based on file extension and content header.
    Auto,
    /// PDF & Scanned Document OCR pipeline (`.pdf`, `.png`, `.jpg`, `.jpeg`, `.webp`, `.tiff`).
    PdfOcr,
    /// Word & OpenDocument Text pipeline (`.docx`, `.odt`, `.rtf`) — preserves heading hierarchy, lists & embedded tables.
    WordOdt,
    /// Spreadsheet & Tabular Data pipeline (`.xlsx`, `.ods`, `.csv`, `.tsv`) — repeats worksheet title & column header row in every chunk.
    Spreadsheet,
    /// Presentation pipeline (`.pptx`, `.odp`) — slide-by-slide extraction & chunking.
    Presentation,
    /// Markdown & Technical Docs pipeline (`.md`, `.markdown`) — YAML frontmatter extraction, heading tree & syntactic link graph.
    Markdown,
    /// Structured XML pipeline (`.xml`) — automatically promotes statutory XMLs (`gii-norm.dtd` / `Formex`) or parses generic XML trees.
    Xml,
    /// JSON & JSON Lines pipeline (`.json`, `.jsonl`) — record-aligned structured key-value or table conversion.
    Json,
    /// Legal & Statutory pipeline: norm-level XML/statute parser -> §/Art. chunker -> syntactic citation & norm graph extractor.
    LegalStatute,
    /// Generic document pipeline: text extraction -> structure-aware chunker -> optional LLM graph extraction.
    Generic,
}

impl IngestionPipelineKind {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_lowercase()).as_deref() {
            Some("pdf" | "pdf_ocr" | "ocr") => Self::PdfOcr,
            Some("word" | "docx" | "odt" | "rtf" | "word_odt") => Self::WordOdt,
            Some("spreadsheet" | "excel" | "xlsx" | "ods" | "csv" | "tsv" | "table") => {
                Self::Spreadsheet
            }
            Some("presentation" | "pptx" | "odp" | "slides") => Self::Presentation,
            Some("markdown" | "md") => Self::Markdown,
            Some("xml") => Self::Xml,
            Some("json" | "jsonl") => Self::Json,
            Some("legal" | "statute" | "law" | "gesetze") => Self::LegalStatute,
            Some("generic" | "standard" | "default") => Self::Generic,
            _ => Self::Auto,
        }
    }

    pub fn resolve(self, filename: &str, raw_bytes_or_text: &[u8]) -> Self {
        match self {
            Self::Xml => {
                if legal::is_statutory_xml_or_text(filename, raw_bytes_or_text) {
                    Self::LegalStatute
                } else {
                    Self::Xml
                }
            }
            Self::Auto => {
                if legal::is_statutory_xml_or_text(filename, raw_bytes_or_text) {
                    return Self::LegalStatute;
                }
                let lower = filename.trim().to_lowercase();
                if lower.ends_with(".pdf")
                    || lower.ends_with(".jpg")
                    || lower.ends_with(".jpeg")
                    || lower.ends_with(".png")
                    || lower.ends_with(".webp")
                    || lower.ends_with(".tif")
                    || lower.ends_with(".tiff")
                {
                    Self::PdfOcr
                } else if lower.ends_with(".docx")
                    || lower.ends_with(".docm")
                    || lower.ends_with(".dotx")
                    || lower.ends_with(".dotm")
                    || lower.ends_with(".doc")
                    || lower.ends_with(".dot")
                    || lower.ends_with(".odt")
                    || lower.ends_with(".ott")
                    || lower.ends_with(".fodt")
                    || lower.ends_with(".rtf")
                    || lower.ends_with(".tex")
                    || lower.ends_with(".latex")
                    || lower.ends_with(".ltx")
                    || lower.ends_with(".epub")
                {
                    Self::WordOdt
                } else if lower.ends_with(".xlsx")
                    || lower.ends_with(".xlsm")
                    || lower.ends_with(".xlsb")
                    || lower.ends_with(".xls")
                    || lower.ends_with(".ods")
                    || lower.ends_with(".ots")
                    || lower.ends_with(".csv")
                    || lower.ends_with(".tsv")
                {
                    Self::Spreadsheet
                } else if lower.ends_with(".pptx")
                    || lower.ends_with(".ppsx")
                    || lower.ends_with(".potx")
                    || lower.ends_with(".ppt")
                    || lower.ends_with(".pps")
                    || lower.ends_with(".pot")
                    || lower.ends_with(".odp")
                    || lower.ends_with(".otp")
                {
                    Self::Presentation
                } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
                    Self::Markdown
                } else if lower.ends_with(".json") || lower.ends_with(".jsonl") {
                    Self::Json
                } else if lower.ends_with(".xml")
                    || raw_bytes_or_text.trim_ascii_start().starts_with(b"<?xml")
                {
                    Self::Xml
                } else {
                    Self::Generic
                }
            }
            other => other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::PdfOcr => "pdf",
            Self::WordOdt => "word",
            Self::Spreadsheet => "spreadsheet",
            Self::Presentation => "presentation",
            Self::Markdown => "markdown",
            Self::Xml => "xml",
            Self::Json => "json",
            Self::LegalStatute => "legal",
            Self::Generic => "generic",
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
        IngestionPipelineKind::WordOdt => {
            let lower = fname_owned.to_lowercase();
            if lower.ends_with(".odt") || lower.ends_with(".ott") || lower.ends_with(".fodt") {
                formats::extract_odt_markdown(&bytes_owned)
            } else if lower.ends_with(".rtf") {
                formats::extract_rtf_markdown(&bytes_owned)
            } else if lower.ends_with(".doc") || lower.ends_with(".dot") {
                formats::extract_doc_binary_markdown(&bytes_owned)
            } else if lower.ends_with(".tex") || lower.ends_with(".latex") || lower.ends_with(".ltx") {
                formats::extract_latex_markdown(&bytes_owned)
            } else if lower.ends_with(".epub") {
                formats::extract_epub_markdown(&bytes_owned)
            } else if lower.ends_with(".docx")
                || lower.ends_with(".docm")
                || lower.ends_with(".dotx")
                || lower.ends_with(".dotm")
            {
                formats::extract_docx_markdown(&bytes_owned)
            } else {
                InferenceClient::extract_text_from_bytes(&fname_owned, &bytes_owned)
            }
        }
        IngestionPipelineKind::Spreadsheet => {
            let lower = fname_owned.to_lowercase();
            if lower.ends_with(".xlsx") || lower.ends_with(".xlsm") || lower.ends_with(".xlsb") {
                formats::extract_xlsx_markdown(&bytes_owned)
            } else if lower.ends_with(".xls") {
                formats::extract_xls_markdown(&bytes_owned)
            } else if lower.ends_with(".ods") || lower.ends_with(".ots") {
                formats::extract_ods_markdown(&bytes_owned)
            } else if lower.ends_with(".csv") || lower.ends_with(".tsv") {
                formats::extract_csv_markdown(&fname_owned, &bytes_owned)
            } else {
                InferenceClient::extract_text_from_bytes(&fname_owned, &bytes_owned)
            }
        }
        IngestionPipelineKind::Presentation => {
            let lower = fname_owned.to_lowercase();
            if lower.ends_with(".pptx") || lower.ends_with(".ppsx") || lower.ends_with(".potx") {
                formats::extract_pptx_markdown(&bytes_owned)
            } else if lower.ends_with(".ppt") || lower.ends_with(".pps") || lower.ends_with(".pot") {
                formats::extract_ppt_binary_markdown(&bytes_owned)
            } else if lower.ends_with(".odp") || lower.ends_with(".otp") {
                formats::extract_odp_markdown(&bytes_owned)
            } else {
                InferenceClient::extract_text_from_bytes(&fname_owned, &bytes_owned)
            }
        }
        IngestionPipelineKind::Markdown => {
            formats::extract_markdown_document(&fname_owned, &bytes_owned)
        }
        IngestionPipelineKind::Json => {
            formats::extract_json_markdown(&fname_owned, &bytes_owned)
        }
        IngestionPipelineKind::Xml => {
            Ok(crate::ingestion::xml::extract_structured_markdown_from_xml(&bytes_owned))
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
        IngestionPipelineKind::Spreadsheet => {
            // Specialized tabular chunker: repeats worksheet title and Markdown table header row in every chunk
            let mut chunks = formats::chunk_spreadsheet_with_headers(
                full_title,
                text,
                state.config.chunk_max_chars,
            );
            if chunks.is_empty() {
                chunks = state
                    .inference
                    .semantic_chunk_text_with_title(Some(full_title), text);
            }
            if chunks.is_empty() {
                anyhow::bail!("Spreadsheet does not contain any usable rows or text.");
            }
            let (entities, relations) = state
                .llm
                .extract_graph_entities(text, Some(should_extract_graph))
                .await?;
            Ok(PreparedDocumentPipeline {
                pipeline_used: resolved.label(),
                chunks,
                entities,
                relations,
            })
        }
        IngestionPipelineKind::Markdown => {
            let chunks = state
                .inference
                .semantic_chunk_text_with_title(Some(full_title), text);
            if chunks.is_empty() {
                anyhow::bail!("Markdown document does not contain any usable text.");
            }
            let (mut entities, mut relations) = if should_extract_graph {
                formats::extract_syntactic_markdown_graph(full_title, text)
            } else {
                (Vec::new(), Vec::new())
            };
            if should_extract_graph {
                if let Ok((llm_ent, llm_rel)) =
                    state.llm.extract_graph_entities(text, Some(true)).await
                {
                    entities.extend(llm_ent);
                    relations.extend(llm_rel);
                }
            }
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
                pipeline_used: resolved.label(),
                chunks,
                entities,
                relations,
            })
        }
    }
}
