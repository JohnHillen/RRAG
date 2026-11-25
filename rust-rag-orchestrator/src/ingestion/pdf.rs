use crate::{
    db::{extract_result_array, SurrealClient},
    ingestion::InferenceClient,
    AppState,
};
use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Serves the document for inline viewing inside the Chat UI (`GET /v1/documents/:doc_id/file`).
/// - Real or synthetic `.pdf` documents are served as `application/pdf`.
/// - Structured text documents (`.xml`, `.md`, `.txt`, `.html`, `.json`, `.csv`) are rendered in a
///   full-length dark-mode HTML document viewer showing all indexed chunks.
pub async fn get_document_file_handler(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_document_viewer_payload(&doc_id).await {
        Ok((title, content_type, body_bytes)) => {
            let safe_name: String = title
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let disp = format!("inline; filename=\"{}\"", safe_name);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_DISPOSITION, disp)
                .body(Body::from(body_bytes))
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!("Document not found: {}", e)))
            .unwrap(),
    }
}

impl SurrealClient {
    /// Retrieves the viewer payload `(title, content_type, bytes)` for `doc_id` from SurrealDB:
    /// - Returns `application/pdf` if `document.metadata.raw_b64` is stored or the document title ends with `.pdf`.
    /// - Otherwise renders a complete dark-mode HTML viewer containing all chunks in sequential order.
    pub async fn get_document_viewer_payload(
        &self,
        doc_id: &str,
    ) -> Result<(String, &'static str, Vec<u8>)> {
        let sql = format!(
            "SELECT title, metadata FROM type::record('document', {d});\nSELECT doc_title, chunk_index, content FROM chunk WHERE doc_id = {d} ORDER BY chunk_index ASC;",
            d = serde_json::to_string(doc_id)?
        );
        let res = self.execute_sql(&sql).await?;
        let doc_arr = extract_result_array(res.first());
        let chunk_arr = extract_result_array(res.get(1));

        let mut title = doc_arr
            .first()
            .and_then(|v| v["title"].as_str())
            .unwrap_or_default()
            .to_string();

        if let Some(b64_str) = doc_arr
            .first()
            .and_then(|v| v["metadata"]["raw_b64"].as_str())
            .filter(|s| !s.is_empty())
        {
            let bytes = base64_decode(b64_str);
            if !bytes.is_empty() {
                if title.is_empty() {
                    title = format!("{}.pdf", doc_id);
                }
                return Ok((title, "application/pdf", bytes));
            }
        }

        let mut chunks_list: Vec<(i64, String)> = Vec::with_capacity(chunk_arr.len());
        let mut combined_text = String::new();
        for c in &chunk_arr {
            if title.is_empty() {
                if let Some(t) = c["doc_title"].as_str() {
                    title = t.to_string();
                }
            }
            let idx = c["chunk_index"].as_i64().unwrap_or(chunks_list.len() as i64);
            if let Some(txt) = c["content"].as_str() {
                if !combined_text.is_empty() {
                    combined_text.push_str("\n\n");
                }
                combined_text.push_str(txt);
                chunks_list.push((idx, txt.to_string()));
            }
        }

        if combined_text.is_empty() && title.is_empty() {
            anyhow::bail!("Document '{}' not found in SurrealDB", doc_id);
        }
        if title.is_empty() {
            title = doc_id.to_string();
        }

        if title.trim().to_lowercase().ends_with(".pdf") {
            let pdf_bytes = synthesize_pdf_from_text(&title, &combined_text);
            Ok((title, "application/pdf", pdf_bytes))
        } else {
            let html = render_html_document_viewer(&title, doc_id, &chunks_list);
            Ok((title, "text/html; charset=utf-8", html.into_bytes()))
        }
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 32);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn render_html_document_viewer(title: &str, doc_id: &str, chunks: &[(i64, String)]) -> String {
    let mut body = String::with_capacity(chunks.len() * 512 + 2048);
    body.push_str(
        r#"<!DOCTYPE html>
<html lang="de">
<head>
<meta charset="UTF-8">
<title>"#,
    );
    body.push_str(&html_escape(title));
    body.push_str(
        r#"</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    background: #0b0f19;
    color: #e2e8f0;
    padding: 1.5rem 2rem;
    line-height: 1.65;
  }
  .doc-container {
    max-width: 980px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .doc-meta-bar {
    background: #111827;
    border: 1px solid #334155;
    border-radius: 0.6rem;
    padding: 0.85rem 1.15rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.75rem;
    position: sticky;
    top: 0.75rem;
    z-index: 10;
    box-shadow: 0 8px 20px rgba(0,0,0,0.45);
  }
  .doc-meta-left {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.85rem;
    color: #94a3b8;
  }
  .badge {
    background: #312e81;
    color: #c7d2fe;
    padding: 0.18rem 0.55rem;
    border-radius: 0.3rem;
    font-size: 0.75rem;
    font-weight: 600;
  }
  .search-input {
    background: #0f172a;
    border: 1px solid #475569;
    color: #f8fafc;
    border-radius: 0.4rem;
    padding: 0.4rem 0.75rem;
    font-size: 0.82rem;
    width: 280px;
    outline: none;
  }
  .search-input:focus {
    border-color: #6366f1;
    box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.25);
  }
  .chunk-card {
    background: #111827;
    border: 1px solid #1e293b;
    border-radius: 0.6rem;
    padding: 1rem 1.25rem;
    transition: border-color 0.15s ease;
  }
  .chunk-card:hover {
    border-color: #334155;
  }
  .chunk-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.55rem;
    padding-bottom: 0.4rem;
    border-bottom: 1px dashed #1e293b;
    font-size: 0.75rem;
    color: #64748b;
  }
  .chunk-content {
    white-space: pre-wrap;
    word-break: break-word;
    font-size: 0.9rem;
    color: #e2e8f0;
  }
</style>
</head>
<body>
<div class="doc-container">
  <div class="doc-meta-bar">
    <div class="doc-meta-left">
      <span class="badge">🧩 "#,
    );
    body.push_str(&chunks.len().to_string());
    body.push_str(
        r#" Chunks</span>
      <span>ID: <code>"#,
    );
    body.push_str(&html_escape(doc_id));
    body.push_str(
        r#"</code></span>
    </div>
    <input type="text" id="chunkFilter" class="search-input" placeholder="🔍 Im Dokument filtern (§, Begriff...)" oninput="filterChunks(this.value)" />
  </div>
"#,
    );

    for (idx, content) in chunks {
        body.push_str(r#"  <div class="chunk-card" data-chunk="1">"#);
        body.push_str(&format!(
            r#"<div class="chunk-header"><span class="badge">Chunk #{}</span><span>{} Zeichen</span></div>"#,
            idx,
            content.chars().count()
        ));
        body.push_str(r#"<div class="chunk-content">"#);
        body.push_str(&html_escape(content));
        body.push_str("</div></div>\n");
    }

    body.push_str(
        r#"</div>
<script>
function filterChunks(q) {
  const term = q.trim().toLowerCase();
  const cards = document.querySelectorAll('.chunk-card');
  cards.forEach(c => {
    if (!term || c.textContent.toLowerCase().includes(term)) {
      c.style.display = '';
    } else {
      c.style.display = 'none';
    }
  });
}
</script>
</body>
</html>"#,
    );
    body
}

impl InferenceClient {
    /// Extracts text from raw bytes (PDF, HTML, Markdown, Plaintext).
    /// 1. For PDFs, first attempts fast digital text extraction via `pdftotext`.
    /// 2. Falls back to pure-Rust `pdf_extract` (safeguarded with `catch_unwind`).
    /// 3. If the PDF is a scanned image without a digital text layer,
    ///    automatically runs OCR via `pdftoppm` + `tesseract` (German + English).
    pub fn extract_text_from_bytes(filename: &str, bytes: &[u8]) -> Result<String> {
        let lower = filename.to_lowercase();
        if lower.ends_with(".pdf") {
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

            let bytes_owned = bytes.to_vec();
            if let Ok(Ok(text)) =
                std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&bytes_owned))
            {
                if text.trim().chars().count() >= 25 {
                    return Ok(text);
                }
            }

            tracing::info!(
                "PDF '{}' has no digital text layer -> starting automatic OCR extraction (Tesseract deu+eng)...",
                filename
            );
            Self::extract_pdf_via_ocr(bytes)
        } else if lower.ends_with(".xml")
            || bytes.trim_ascii_start().starts_with(b"<?xml")
            || bytes.trim_ascii_start().starts_with(b"<dokumente")
        {
            Ok(super::xml::extract_structured_markdown_from_xml(bytes))
        } else if lower.ends_with(".html") || lower.ends_with(".htm") {
            let text = html2text::from_read(bytes, 100);
            Ok(text)
        } else {
            let text = String::from_utf8_lossy(bytes).to_string();
            Ok(text)
        }
    }

    /// Renders a scanned PDF page-by-page using `pdftoppm` (200 DPI) and extracts text
    /// using `tesseract` (languages: German + English).
    fn extract_pdf_via_ocr(bytes: &[u8]) -> Result<String> {
        let job_id = uuid::Uuid::new_v4().simple().to_string();
        let tmp_dir = std::env::temp_dir().join(format!("rag_ocr_{}", job_id));
        std::fs::create_dir_all(&tmp_dir)
            .context("Failed to create temporary OCR working directory")?;

        let pdf_path = tmp_dir.join("input.pdf");
        let page_prefix = tmp_dir.join("page");

        let result = (|| -> Result<String> {
            std::fs::write(&pdf_path, bytes).context("Failed to write temporary PDF file")?;

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
                        full_ocr_text
                            .push_str(&format!("\n--- Page {} ---\n{}\n", idx + 1, page_txt));
                    }
                }
            }

            if full_ocr_text.trim().is_empty() {
                anyhow::bail!("OCR (Tesseract) was unable to detect any readable text in the PDF.");
            }

            Ok(full_ocr_text)
        })();

        let _ = std::fs::remove_dir_all(&tmp_dir);
        result
    }
}

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn base64_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for b in s.bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    out
}

fn pdf_escape_winansi(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 16);
    for ch in s.chars() {
        let b: u8 = match ch {
            '\\' => {
                out.extend_from_slice(b"\\\\");
                continue;
            }
            '(' => {
                out.extend_from_slice(b"\\(");
                continue;
            }
            ')' => {
                out.extend_from_slice(b"\\)");
                continue;
            }
            'ä' => 0xE4,
            'ö' => 0xF6,
            'ü' => 0xFC,
            'Ä' => 0xC4,
            'Ö' => 0xD6,
            'Ü' => 0xDC,
            'ß' => 0xDF,
            '€' => 0x80,
            '•' | '·' => 0x95,
            '–' | '—' => b'-',
            c if (c as u32) >= 32 && (c as u32) <= 126 => c as u8,
            c if (c as u32) >= 160 && (c as u32) <= 255 => c as u8,
            _ => b' ',
        };
        out.push(b);
    }
    out
}

pub fn synthesize_pdf_from_text(title: &str, text: &str) -> Vec<u8> {
    let mut stream = Vec::new();
    stream.extend_from_slice(b"BT\n/F1 14 Tf\n45 785 Td\n(");
    stream.extend_from_slice(&pdf_escape_winansi(title));
    stream.extend_from_slice(b") Tj\n/F2 10 Tf\n0 -24 Td\n");

    let mut line_count = 0usize;
    for raw_line in text.lines() {
        let trimmed = raw_line.trim_end();
        if trimmed.is_empty() {
            stream.extend_from_slice(b"0 -10 Td\n");
            line_count += 1;
            if line_count >= 52 {
                break;
            }
            continue;
        }
        let chars: Vec<char> = trimmed.chars().collect();
        for sub in chars.chunks(92) {
            if line_count >= 52 {
                break;
            }
            let sub_str: String = sub.iter().collect();
            stream.extend_from_slice(b"(");
            stream.extend_from_slice(&pdf_escape_winansi(&sub_str));
            stream.extend_from_slice(b") Tj\n0 -13 Td\n");
            line_count += 1;
        }
        if line_count >= 52 {
            break;
        }
    }
    stream.extend_from_slice(b"ET\n");

    let mut objs: Vec<Vec<u8>> = Vec::new();
    let mut obj1 = format!("1 0 obj\n<< /Length {} >>\nstream\n", stream.len()).into_bytes();
    obj1.extend_from_slice(&stream);
    obj1.extend_from_slice(b"endstream\nendobj\n");
    objs.push(obj1);
    objs.push(b"2 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>\nendobj\n".to_vec());
    objs.push(b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n".to_vec());
    objs.push(b"4 0 obj\n<< /Type /Page /Parent 5 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 2 0 R /F2 3 0 R >> >> /Contents 1 0 R >>\nendobj\n".to_vec());
    objs.push(b"5 0 obj\n<< /Type /Pages /Kids [4 0 R] /Count 1 >>\nendobj\n".to_vec());
    objs.push(b"6 0 obj\n<< /Type /Catalog /Pages 5 0 R >>\nendobj\n".to_vec());

    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for o in &objs {
        offsets.push(out.len());
        out.extend_from_slice(o);
    }
    let xref_pos = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 6 0 R >>\nstartxref\n{}\n%%EOF\n",
            objs.len() + 1,
            xref_pos
        )
        .as_bytes(),
    );
    out
}
