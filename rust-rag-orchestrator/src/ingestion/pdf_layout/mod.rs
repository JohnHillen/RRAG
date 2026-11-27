//! ArtCraft (`storytold/pdfcraft`, MIT/Apache-2.0) PDF layout, heading, reading-order,
//! inline emphasis (`**bold**`, `*italic*`), and multi-column table reconstruction engine.
//!
//! Feeds `pdftohtml -xml -noroundcoord -fontfullname` bounding boxes and font metrics directly
//! into `pdfcraft-export`'s `Page` / `Block` model and exports structured GFM Markdown via
//! `craft_markdown::extracted::Writer`.

pub mod as_markdown;
pub mod flow;
pub mod layout;

use std::collections::HashMap;
use layout::{Block, Page};

#[derive(Clone, Debug)]
struct FontSpec {
    size: f64,
    bold: bool,
    italic: bool,
}

/// Extracts structured Markdown (with `#` / `##` headings inferred from font size & weight,
/// `**bold**` / `*italic*` runs, multi-column reading order, and GFM pipe tables `| ... |`)
/// from a PDF byte slice using Poppler's `pdftohtml -xml` + ArtCraft's `pdfcraft-export` layout engine.
pub fn extract_pdf_markdown_via_artcraft_layout(bytes: &[u8]) -> Option<String> {
    let job_id = uuid::Uuid::new_v4().simple().to_string();
    let tmp_pdf = std::env::temp_dir().join(format!("rag_pdfcraft_{}.pdf", job_id));
    std::fs::write(&tmp_pdf, bytes).ok()?;

    let output = std::process::Command::new("pdftohtml")
        .args([
            "-q",
            "-i",
            "-xml",
            "-noroundcoord",
            "-fontfullname",
            "-stdout",
            "-enc",
            "UTF-8",
            tmp_pdf.to_str()?,
        ])
        .stderr(std::process::Stdio::null())
        .output();
    let _ = std::fs::remove_file(&tmp_pdf);

    let output = output.ok()?;
    if !output.status.success() {
        return None;
    }

    let xml = String::from_utf8_lossy(&output.stdout);
    let pages = parse_pdftohtml_xml_to_pages(&xml);
    if pages.is_empty() || pages.iter().all(|p| p.blocks.is_empty()) {
        return None;
    }

    let md = as_markdown::markdown(&pages);
    let trimmed = md.trim();
    if trimmed.chars().count() >= 25 {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Parses Poppler `pdftohtml -xml` output into `pdfcraft-export`'s `Vec<Page>` (with y-up coordinates in points).
pub fn parse_pdftohtml_xml_to_pages(xml: &str) -> Vec<Page> {
    let mut pages = Vec::new();
    let mut fonts: HashMap<String, FontSpec> = HashMap::new();
    let mut current_page: Option<Page> = None;

    for raw_line in xml.lines() {
        let line = raw_line.trim();
        if line.starts_with("<page ") {
            if let Some(p) = current_page.take() {
                pages.push(p);
            }
            let width = extract_xml_attr_f64(line, "width").unwrap_or(595.0);
            let height = extract_xml_attr_f64(line, "height").unwrap_or(842.0);
            current_page = Some(Page {
                width,
                height,
                ..Default::default()
            });
        } else if line.starts_with("</page>") {
            if let Some(p) = current_page.take() {
                pages.push(p);
            }
        } else if line.starts_with("<fontspec ") {
            if let Some(id) = extract_xml_attr_str(line, "id") {
                let size = extract_xml_attr_f64(line, "size").unwrap_or(11.0).max(1.0);
                let family = extract_xml_attr_str(line, "family")
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let bold = family.contains("bold")
                    || family.contains("black")
                    || family.contains("heavy")
                    || family.contains("demi")
                    || family.contains("semibold");
                let italic = family.contains("italic")
                    || family.contains("oblique")
                    || family.contains("slanted");
                fonts.insert(id, FontSpec { size, bold, italic });
            }
        } else if line.starts_with("<text ") {
            let Some(ref mut page) = current_page else {
                continue;
            };
            let top = extract_xml_attr_f64(line, "top").unwrap_or(0.0);
            let left = extract_xml_attr_f64(line, "left").unwrap_or(0.0);
            let width = extract_xml_attr_f64(line, "width").unwrap_or(0.0).max(1.0);
            let height = extract_xml_attr_f64(line, "height").unwrap_or(11.0).max(1.0);
            let font_id = extract_xml_attr_str(line, "font").unwrap_or_default();

            let (font_size, font_bold, font_italic) = fonts
                .get(&font_id)
                .map(|f| (f.size, f.bold, f.italic))
                .unwrap_or((height * 0.85, false, false));

            let Some(tag_close) = line.find('>') else {
                continue;
            };
            let inner = match line.rfind("</text>") {
                Some(end_idx) if end_idx > tag_close => &line[tag_close + 1..end_idx],
                _ => &line[tag_close + 1..],
            };

            let (plain_text, has_b, has_i) = strip_inline_html_tags(inner);
            if plain_text.trim().is_empty() {
                continue;
            }

            let space_before = plain_text.starts_with(char::is_whitespace);
            let space_after = plain_text.ends_with(char::is_whitespace);
            let cleaned = plain_text.trim().to_string();

            // Convert Poppler top-down coordinates to pdfcraft-export y-up coordinates:
            let y1 = (page.height - top).max(0.0);
            let y0 = (page.height - top - height).max(0.0).min(y1);
            let x0 = left.max(0.0);
            let x1 = (left + width).max(x0 + 0.5);

            page.blocks.push(Block {
                text: cleaned,
                rect: [x0, y0, x1, y1],
                size: font_size.max(1.0),
                bold: font_bold || has_b,
                italic: font_italic || has_i,
                color: [0.0, 0.0, 0.0],
                rotation: 0.0,
                space_around: [space_before, space_after],
            });
        }
    }

    if let Some(p) = current_page.take() {
        pages.push(p);
    }

    pages
}

fn extract_xml_attr_str(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = tag.find(&pattern)? + pattern.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_xml_attr_f64(tag: &str, attr: &str) -> Option<f64> {
    extract_xml_attr_str(tag, attr)?.parse::<f64>().ok()
}

fn strip_inline_html_tags(s: &str) -> (String, bool, bool) {
    let lower = s.to_ascii_lowercase();
    let has_b = lower.contains("<b>") || lower.contains("<b ");
    let has_i = lower.contains("<i>") || lower.contains("<i ");

    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#160;", " ");

    (decoded, has_b, has_i)
}
