use crate::config::AppConfig;
use crate::db::RetrievedChunk;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use text_splitter::{ChunkConfig, MarkdownSplitter};

#[derive(Clone)]
pub struct InferenceClient {
    http: Client,
    config: AppConfig,
}

#[derive(Debug, Deserialize)]
struct TeiRerankItem {
    index: usize,
    score: f64,
}

impl InferenceClient {
    pub fn new(http: Client, config: AppConfig) -> Self {
        Self { http, config }
    }

    /// Extracts text from raw bytes (PDF, HTML, Markdown, Plaintext).
    /// 1. For PDFs, first attempts fast digital text extraction via `pdftotext`.
    /// 2. Falls back to pure-Rust `pdf_extract` (safeguarded with `catch_unwind`).
    /// 3. If the PDF is a scanned image without a digital text layer,
    ///    automatically runs OCR via `pdftoppm` + `tesseract` (German + English, Apache 2.0).
    pub fn extract_text_from_bytes(filename: &str, bytes: &[u8]) -> Result<String> {
        let lower = filename.to_lowercase();
        if lower.ends_with(".pdf") {
            // 1st attempt: Fast digital text extraction via `pdftotext`
            if let Ok(mut child) = std::process::Command::new("pdftotext")
                .args(["-enc", "UTF-8", "-", "-"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(bytes);
                }
                if let Ok(output) = child.wait_with_output() {
                    if output.status.success() {
                        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if text.chars().count() >= 25 {
                            return Ok(text);
                        }
                    }
                }
            }

            // 2nd attempt: Pure Rust `pdf_extract` wrapped in `catch_unwind`
            let bytes_owned = bytes.to_vec();
            if let Ok(Ok(text)) =
                std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&bytes_owned))
            {
                if text.trim().chars().count() >= 25 {
                    return Ok(text);
                }
            }

            // 3rd attempt: Automatic OCR fallback (Tesseract OCR German + English) for scanned documents
            tracing::info!(
                "PDF '{}' has no digital text layer -> starting automatic OCR extraction (Tesseract deu+eng)...",
                filename
            );
            Self::extract_pdf_via_ocr(bytes)
        } else if lower.ends_with(".html") || lower.ends_with(".htm") {
            let text = html2text::from_read(bytes, 100);
            Ok(text)
        } else {
            let text = String::from_utf8_lossy(bytes).to_string();
            Ok(text)
        }
    }

    /// Renders a scanned PDF page-by-page using `pdftoppm` (200 DPI) and extracts text
    /// using `tesseract` (languages: German + English, Apache 2.0).
    fn extract_pdf_via_ocr(bytes: &[u8]) -> Result<String> {
        let job_id = uuid::Uuid::new_v4().simple().to_string();
        let tmp_dir = std::env::temp_dir().join(format!("rag_ocr_{}", job_id));
        std::fs::create_dir_all(&tmp_dir)
            .context("Failed to create temporary OCR working directory")?;

        let pdf_path = tmp_dir.join("input.pdf");
        let page_prefix = tmp_dir.join("page");

        let result = (|| -> Result<String> {
            std::fs::write(&pdf_path, bytes).context("Failed to write temporary PDF file")?;

            // Render pages to PNG at 200 DPI
            let status = std::process::Command::new("pdftoppm")
                .args([
                    "-png",
                    "-r",
                    "200",
                    pdf_path.to_str().unwrap_or_default(),
                    page_prefix.to_str().unwrap_or_default(),
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .context("Failed to spawn 'pdftoppm' for OCR processing")?;

            if !status.success() {
                anyhow::bail!("'pdftoppm' failed to render PDF pages as images.");
            }

            // Collect and sort all generated page-*.png files
            let mut page_files: Vec<std::path::PathBuf> = std::fs::read_dir(&tmp_dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("png"))
                        .unwrap_or(false)
                })
                .collect();
            page_files.sort();

            if page_files.is_empty() {
                anyhow::bail!("No image pages found in PDF for OCR processing.");
            }

            let mut full_ocr_text = String::new();
            for (idx, img_path) in page_files.iter().enumerate() {
                let output = std::process::Command::new("tesseract")
                    .args([
                        img_path.to_str().unwrap_or_default(),
                        "stdout",
                        "-l",
                        "deu+eng",
                    ])
                    .stderr(std::process::Stdio::null())
                    .output()
                    .context("Failed to execute 'tesseract' OCR")?;

                if output.status.success() {
                    let page_txt = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !page_txt.is_empty() {
                        full_ocr_text.push_str(&format!("\n--- Page {} ---\n{}\n", idx + 1, page_txt));
                    }
                }
            }

            if full_ocr_text.trim().is_empty() {
                anyhow::bail!("OCR (Tesseract) was unable to detect any readable text in the PDF.");
            }

            Ok(full_ocr_text)
        })();

        // Always clean up temporary directory
        let _ = std::fs::remove_dir_all(&tmp_dir);
        result
    }

    /// Splits long text semantically along Markdown structure and paragraph boundaries.
    pub fn semantic_chunk_text(&self, text: &str) -> Vec<String> {
        let max_chars = self.config.chunk_max_chars;
        let splitter = MarkdownSplitter::new(ChunkConfig::new(max_chars));
        splitter
            .chunks(text)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Generates dense vector embeddings for a slice of texts via Hugging Face TEI (`/embed`).
    pub async fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/embed", self.config.tei_embed_url.trim_end_matches('/'));
        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Batch size of 16 for high stability across CPU and GPU configurations
        for batch in texts.chunks(16) {
            let payload = json!({
                "inputs": batch,
                "truncate": true
            });

            let resp = self
                .http
                .post(&url)
                .json(&payload)
                .send()
                .await
                .context("Failed to communicate with TEI embedding server")?;

            let status = resp.status();
            if !status.is_success() {
                let err_txt = resp.text().await.unwrap_or_default();
                anyhow::bail!("TEI Embed HTTP {}: {}", status, err_txt);
            }

            let batch_vecs: Vec<Vec<f32>> = resp
                .json()
                .await
                .context("Failed to parse TEI embedding response")?;
            all_embeddings.extend(batch_vecs);
        }

        Ok(all_embeddings)
    }

    /// Reranks retrieved candidate chunks using the selected Cross-Encoder model
    /// (`fast` = mmarco-mMiniLMv2-L12, `high` = bge-reranker-v2-m3, `none` = SurrealDB RRF only)
    /// based on semantic relevance to the query.
    pub async fn rerank_chunks(
        &self,
        query: &str,
        mut candidates: Vec<RetrievedChunk>,
        top_n: usize,
        reranker_mode: &str,
    ) -> Result<(Vec<RetrievedChunk>, String)> {
        if candidates.is_empty() {
            return Ok((vec![], "No candidates".to_string()));
        }

        if reranker_mode.eq_ignore_ascii_case("none") {
            candidates.truncate(top_n);
            return Ok((candidates, "SurrealDB RRF (No Reranker)".to_string()));
        }

        let (primary_url, primary_label, fallback_url, fallback_label) =
            if reranker_mode.eq_ignore_ascii_case("fast") {
                (
                    format!("{}/rerank", self.config.tei_rerank_fast_url.trim_end_matches('/')),
                    self.config.reranker_fast_model.clone(),
                    format!("{}/rerank", self.config.tei_rerank_url.trim_end_matches('/')),
                    self.config.reranker_model.clone(),
                )
            } else {
                (
                    format!("{}/rerank", self.config.tei_rerank_url.trim_end_matches('/')),
                    self.config.reranker_model.clone(),
                    format!("{}/rerank", self.config.tei_rerank_fast_url.trim_end_matches('/')),
                    self.config.reranker_fast_model.clone(),
                )
            };

        // We prepend the document title `[Document: ...]` to each candidate text,
        // allowing the Cross-Encoder to immediately disambiguate technical terms
        // across different manuals (e.g. "Brose Motor" vs. other devices).
        let texts: Vec<String> = candidates
            .iter()
            .map(|c| format!("[Document: {}]\n{}", c.doc_title, c.content))
            .collect();
        let payload = json!({
            "query": query,
            "texts": texts,
            "truncate": true
        });

        let mut active_label = primary_label;
        let mut resp = self.http.post(&primary_url).json(&payload).send().await;

        // If the selected reranker port is unavailable, fall back to the alternate reranker endpoint
        if resp.as_ref().map(|r| !r.status().is_success()).unwrap_or(true) {
            if let Ok(fallback_resp) = self.http.post(&fallback_url).json(&payload).send().await {
                if fallback_resp.status().is_success() {
                    resp = Ok(fallback_resp);
                    active_label = format!("{} (Fallback)", fallback_label);
                }
            }
        }

        match resp {
            Ok(r) if r.status().is_success() => {
                let mut scores: Vec<TeiRerankItem> = r.json().await.unwrap_or_default();

                scores.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let mut reranked = Vec::with_capacity(top_n.min(scores.len()));
                for item in scores.into_iter().take(top_n) {
                    if let Some(chunk) = candidates.get_mut(item.index) {
                        let mut c = chunk.clone();
                        c.rerank_score = Some(item.score);
                        reranked.push(c);
                    }
                }
                Ok((reranked, active_label))
            }
            _ => {
                // Fall back to SurrealDB RRF ordering if no reranker server responds
                candidates.truncate(top_n);
                Ok((candidates, "SurrealDB RRF (Reranker offline)".to_string()))
            }
        }
    }
}
