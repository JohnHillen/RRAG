use anyhow::{Context, Result};
use flate2::read::DeflateDecoder;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;

// ============================================================================
// 1. In-Memory ZIP Archive Reader (for .docx, .odt, .xlsx, .ods, .pptx, .odp)
// ============================================================================

/// Extracts all entries from an in-memory ZIP archive (`PK\x03\x04` / Central Directory `PK\x01\x02`).
/// Uses the Central Directory first so streaming/Data-Descriptor ZIPs (created by LibreOffice, Java, etc.)
/// are decoded reliably, with automatic fallback to sequential Local File Header scanning.
pub fn extract_zip_entries(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>> {
    if bytes.len() < 22 || &bytes[0..4] != b"PK\x03\x04" {
        anyhow::bail!("Not a valid ZIP container (missing PK\\x03\\x04 header).");
    }

    let mut entries = HashMap::new();

    // 1. Locate End of Central Directory (EOCD: PK\x05\x06) near end of file
    let min_search = bytes.len().saturating_sub(65_557);
    let mut eocd_pos: Option<usize> = None;
    for i in (min_search..=bytes.len().saturating_sub(22)).rev() {
        if &bytes[i..i + 4] == b"PK\x05\x06" {
            eocd_pos = Some(i);
            break;
        }
    }

    if let Some(eocd) = eocd_pos {
        let cd_offset = u32::from_le_bytes([
            bytes[eocd + 16],
            bytes[eocd + 17],
            bytes[eocd + 18],
            bytes[eocd + 19],
        ]) as usize;

        let mut pos = cd_offset;
        while pos + 46 <= bytes.len() && &bytes[pos..pos + 4] == b"PK\x01\x02" {
            let method = u16::from_le_bytes([bytes[pos + 10], bytes[pos + 11]]);
            let comp_size = u32::from_le_bytes([
                bytes[pos + 20],
                bytes[pos + 21],
                bytes[pos + 22],
                bytes[pos + 23],
            ]) as usize;
            let uncomp_size = u32::from_le_bytes([
                bytes[pos + 24],
                bytes[pos + 25],
                bytes[pos + 26],
                bytes[pos + 27],
            ]) as usize;
            let fname_len = u16::from_le_bytes([bytes[pos + 28], bytes[pos + 29]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[pos + 30], bytes[pos + 31]]) as usize;
            let comment_len = u16::from_le_bytes([bytes[pos + 32], bytes[pos + 33]]) as usize;
            let local_off = u32::from_le_bytes([
                bytes[pos + 42],
                bytes[pos + 43],
                bytes[pos + 44],
                bytes[pos + 45],
            ]) as usize;

            let name_start = pos + 46;
            let name_end = name_start + fname_len;
            if name_end > bytes.len() {
                break;
            }
            let name = String::from_utf8_lossy(&bytes[name_start..name_end]).to_string();
            pos = name_end + extra_len + comment_len;

            if name.ends_with('/') || comp_size == 0 || uncomp_size > 100 * 1024 * 1024 {
                continue;
            }
            if local_off + 30 <= bytes.len() && &bytes[local_off..local_off + 4] == b"PK\x03\x04" {
                let l_fname =
                    u16::from_le_bytes([bytes[local_off + 26], bytes[local_off + 27]]) as usize;
                let l_extra =
                    u16::from_le_bytes([bytes[local_off + 28], bytes[local_off + 29]]) as usize;
                let data_start = local_off + 30 + l_fname + l_extra;
                let data_end = data_start.saturating_add(comp_size);
                if data_end <= bytes.len() {
                    let payload = &bytes[data_start..data_end];
                    if method == 0 {
                        entries.insert(name, payload.to_vec());
                    } else if method == 8 {
                        let mut decoded = Vec::with_capacity(uncomp_size.min(16 * 1024 * 1024));
                        if DeflateDecoder::new(payload)
                            .read_to_end(&mut decoded)
                            .is_ok()
                        {
                            entries.insert(name, decoded);
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback: Sequential Local File Header scan if Central Directory was missing
    if entries.is_empty() {
        let mut pos = 0usize;
        while pos + 30 <= bytes.len() && &bytes[pos..pos + 4] == b"PK\x03\x04" {
            let method = u16::from_le_bytes([bytes[pos + 8], bytes[pos + 9]]);
            let comp_size = u32::from_le_bytes([
                bytes[pos + 18],
                bytes[pos + 19],
                bytes[pos + 20],
                bytes[pos + 21],
            ]) as usize;
            let fname_len = u16::from_le_bytes([bytes[pos + 26], bytes[pos + 27]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[pos + 28], bytes[pos + 29]]) as usize;
            let name_start = pos + 30;
            let name_end = name_start + fname_len;
            let data_start = name_end + extra_len;
            let data_end = data_start.saturating_add(comp_size);
            if data_end > bytes.len() {
                break;
            }
            let name = String::from_utf8_lossy(&bytes[name_start..name_end]).to_string();
            let payload = &bytes[data_start..data_end];
            if !name.ends_with('/') && comp_size > 0 {
                if method == 0 {
                    entries.insert(name, payload.to_vec());
                } else if method == 8 {
                    let mut decoded = Vec::new();
                    if DeflateDecoder::new(payload)
                        .read_to_end(&mut decoded)
                        .is_ok()
                    {
                        entries.insert(name, decoded);
                    }
                }
            }
            pos = data_end;
        }
    }

    Ok(entries)
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#160;", " ")
        .replace("&#xa0;", " ")
        .replace("&#10;", "\n")
        .replace("&#13;", "")
}

fn strip_all_xml_tags(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_xml_entities(&out)
}

fn extract_xml_attr<'a>(tag_header: &'a str, attr_name: &str) -> Option<&'a str> {
    let pat_dq = format!("{}=\"", attr_name);
    if let Some(pos) = tag_header.find(&pat_dq) {
        let rest = &tag_header[pos + pat_dq.len()..];
        if let Some(end) = rest.find('"') {
            return Some(&rest[..end]);
        }
    }
    let pat_sq = format!("{}='", attr_name);
    if let Some(pos) = tag_header.find(&pat_sq) {
        let rest = &tag_header[pos + pat_sq.len()..];
        if let Some(end) = rest.find('\'') {
            return Some(&rest[..end]);
        }
    }
    None
}

// ============================================================================
// 2. Microsoft Word (.docx, .doc), OpenDocument Text (.odt), RTF (.rtf),
//    HTML (.html), LaTeX (.tex) & EPUB (.epub) via ArtCraft (storytold/craft-libs)
// ============================================================================

/// Extracts structured Markdown (headings `#`..`######`, paragraphs, lists, GFM tables,
/// `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``, and hyperlinks) from a `.docx` Word document.
/// Uses ArtCraft's `craft_docx::read` + `craft_markdown::export`, with a lenient fallback for
/// minimal synthetic ZIP archives missing `[Content_Types].xml`.
pub fn extract_docx_markdown(bytes: &[u8]) -> Result<String> {
    if let Ok(doc) = craft_docx::read(bytes) {
        let md = craft_markdown::export(&doc);
        if !md.trim().is_empty() {
            return Ok(md.trim().to_string());
        }
    }

    let entries = extract_zip_entries(bytes)?;
    let doc_bytes = entries
        .get("word/document.xml")
        .context("Invalid .docx file: 'word/document.xml' not found in archive")?;
    let xml = String::from_utf8_lossy(doc_bytes);

    let mut out = String::with_capacity(xml.len() / 3);
    // Process document body sequentially, handling <w:tbl> tables and <w:p> paragraphs
    let mut cursor = 0usize;
    while cursor < xml.len() {
        let rest = &xml[cursor..];
        let next_tbl = rest.find("<w:tbl>").or_else(|| rest.find("<w:tbl "));
        let next_p = rest.find("<w:p>").or_else(|| rest.find("<w:p "));

        match (next_tbl, next_p) {
            (Some(t_idx), Some(p_idx)) if t_idx < p_idx => {
                let abs_t = cursor + t_idx;
                if let Some(end_rel) = xml[abs_t..].find("</w:tbl>") {
                    let tbl_xml = &xml[abs_t..abs_t + end_rel + 8];
                    let md_tbl = parse_docx_table(tbl_xml);
                    if !md_tbl.is_empty() {
                        out.push_str(&md_tbl);
                        out.push('\n');
                    }
                    cursor = abs_t + end_rel + 8;
                } else {
                    break;
                }
            }
            (_, Some(p_idx)) => {
                let abs_p = cursor + p_idx;
                if let Some(end_rel) = xml[abs_p..].find("</w:p>") {
                    let p_xml = &xml[abs_p..abs_p + end_rel + 6];
                    let md_p = parse_docx_paragraph(p_xml);
                    if !md_p.is_empty() {
                        out.push_str(&md_p);
                        out.push_str("\n\n");
                    }
                    cursor = abs_p + end_rel + 6;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    if out.trim().is_empty() {
        anyhow::bail!("Word document (.docx) does not contain any readable text.");
    }
    Ok(out.trim().to_string())
}

/// Extracts structured Markdown from a legacy Word 97–2003 / Word 6.0/95 binary `.doc` file
/// using ArtCraft's `craft_doc_binary::read` + `craft_markdown::export`.
pub fn extract_doc_binary_markdown(bytes: &[u8]) -> Result<String> {
    let doc = craft_doc_binary::read(bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse legacy Word .doc file: {}", e))?;
    let md = craft_markdown::export(&doc);
    if md.trim().is_empty() {
        anyhow::bail!("Legacy Word (.doc) document does not contain any readable text.");
    }
    Ok(md.trim().to_string())
}

/// Extracts structured Markdown from an HTML/XHTML file using ArtCraft's `craft_html::doc::import`
/// + `craft_markdown::export` (preserving headings, tables, lists, bold/italic, code, and links).
pub fn extract_html_markdown(bytes: &[u8]) -> Result<String> {
    let decoded = craft_html::doc::decode(bytes);
    let doc = craft_html::doc::import(&decoded);
    let md = craft_markdown::export(&doc);
    if !md.trim().is_empty() {
        return Ok(md.trim().to_string());
    }
    Ok(html2text::from_read(bytes, 100))
}

/// Extracts structured Markdown from a LaTeX (`.tex`, `.latex`) document using ArtCraft's
/// `craft_latex::import` + `craft_markdown::export`.
pub fn extract_latex_markdown(bytes: &[u8]) -> Result<String> {
    let doc = craft_latex::import(bytes);
    let md = craft_markdown::export(&doc);
    if md.trim().is_empty() {
        anyhow::bail!("LaTeX document does not contain any readable text.");
    }
    Ok(md.trim().to_string())
}

/// Extracts structured Markdown from an EPUB 2/3 e-book (`.epub`) using ArtCraft's `craft_epub::Epub`
/// + `craft_html::doc::import` + `craft_markdown::export`.
pub fn extract_epub_markdown(bytes: &[u8]) -> Result<String> {
    let mut epub = craft_epub::Epub::open(bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse EPUB archive: {}", e))?;
    let mut out = String::new();
    if !epub.package.title.trim().is_empty() {
        out.push_str(&format!("# {}\n\n", epub.package.title.trim()));
    }
    if !epub.package.creators.is_empty() {
        out.push_str(&format!(
            "- **Autor(en)**: {}\n\n",
            epub.package.creators.join(", ")
        ));
    }
    let spine_paths: Vec<String> = epub
        .package
        .spine_paths()
        .map(|s| s.to_string())
        .collect();
    for path in spine_paths {
        if let Ok(Some(chapter_bytes)) = epub.read(&path) {
            let decoded = craft_html::doc::decode(&chapter_bytes);
            let doc = craft_html::doc::import(&decoded);
            let md = craft_markdown::export(&doc);
            if !md.trim().is_empty() {
                out.push_str(md.trim());
                out.push_str("\n\n");
            }
        }
    }
    if out.trim().is_empty() {
        anyhow::bail!("EPUB e-book does not contain any readable chapters.");
    }
    Ok(out.trim().to_string())
}

fn parse_docx_paragraph(p_xml: &str) -> String {
    let mut heading_level: Option<usize> = None;
    let mut is_list_item = p_xml.contains("<w:numPr>");

    if let Some(st_pos) = p_xml.find("<w:pStyle") {
        let st_rest = &p_xml[st_pos..];
        let st_end = st_rest.find('>').unwrap_or(st_rest.len());
        if let Some(val) = extract_xml_attr(&st_rest[..st_end], "w:val") {
            let low = val.to_lowercase();
            if low == "title" || low == "titel" || low.starts_with("heading1") || low.starts_with("berschrift1") || low == "1" {
                heading_level = Some(1);
            } else if low.starts_with("heading2") || low.starts_with("berschrift2") || low == "subtitle" || low == "2" {
                heading_level = Some(2);
            } else if low.starts_with("heading3") || low.starts_with("berschrift3") || low == "3" {
                heading_level = Some(3);
            } else if low.starts_with("heading") || low.starts_with("berschrift") {
                heading_level = Some(4);
            } else if low.contains("list") {
                is_list_item = true;
            }
        }
    }

    let mut text = String::new();
    let mut cur = 0usize;
    while cur < p_xml.len() {
        let rest = &p_xml[cur..];
        let next_t = rest.find("<w:t>").or_else(|| rest.find("<w:t "));
        let next_br = rest.find("<w:br");
        let next_tab = rest.find("<w:tab");

        let min_pos = [next_t, next_br, next_tab]
            .into_iter()
            .flatten()
            .min();

        let Some(pos) = min_pos else {
            break;
        };
        let abs = cur + pos;
        if Some(pos) == next_t {
            if let Some(gt) = p_xml[abs..].find('>') {
                let start_txt = abs + gt + 1;
                if let Some(end_t) = p_xml[start_txt..].find("</w:t>") {
                    text.push_str(&decode_xml_entities(&p_xml[start_txt..start_txt + end_t]));
                    cur = start_txt + end_t + 6;
                    continue;
                }
            }
            cur = abs + 4;
        } else if Some(pos) == next_br {
            text.push('\n');
            cur = abs + 5;
        } else {
            text.push('\t');
            cur = abs + 6;
        }
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(lvl) = heading_level {
        let hashes = "#".repeat(lvl.clamp(1, 4));
        format!("{} {}", hashes, trimmed.replace('\n', " "))
    } else if is_list_item {
        format!("- {}", trimmed)
    } else {
        trimmed.to_string()
    }
}

fn parse_docx_table(tbl_xml: &str) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for tr_part in tbl_xml.split("<w:tr").skip(1) {
        let Some(tr_gt) = tr_part.find('>') else {
            continue;
        };
        let tr_end = tr_part.find("</w:tr>").unwrap_or(tr_part.len());
        let tr_body = &tr_part[tr_gt + 1..tr_end];
        let mut row_cells: Vec<String> = Vec::new();

        for tc_part in tr_body.split("<w:tc").skip(1) {
            let Some(tc_gt) = tc_part.find('>') else {
                continue;
            };
            let tc_end = tc_part.find("</w:tc>").unwrap_or(tc_part.len());
            let tc_body = &tc_part[tc_gt + 1..tc_end];

            let mut cell_paragraphs = Vec::new();
            for p_part in tc_body.split("<w:p").skip(1) {
                let full_p = format!("<w:p{}", p_part);
                let p_txt = parse_docx_paragraph(&full_p);
                let clean = p_txt
                    .trim_start_matches('#')
                    .trim_start_matches('-')
                    .trim()
                    .replace('|', "/");
                if !clean.is_empty() {
                    cell_paragraphs.push(clean);
                }
            }
            row_cells.push(cell_paragraphs.join(" "));
        }
        if row_cells.iter().any(|c| !c.is_empty()) {
            rows.push(row_cells);
        }
    }
    render_rows_as_markdown_table(&rows)
}

/// Extracts structured Markdown (headings, paragraphs, lists, bold/italic, and tables)
/// from an OpenDocument Text (`.odt`, `.ott`, `.fodt`) file via ArtCraft's `craft_odf::text` + `craft_markdown::export`.
pub fn extract_odt_markdown(bytes: &[u8]) -> Result<String> {
    if let Ok(doc) = craft_odf::text::import(bytes).or_else(|_| craft_odf::text::import_flat(bytes)) {
        let md = craft_markdown::export(&doc);
        if !md.trim().is_empty() {
            return Ok(md.trim().to_string());
        }
    }

    let entries = extract_zip_entries(bytes)?;
    let content_bytes = entries
        .get("content.xml")
        .context("Invalid .odt file: 'content.xml' not found in archive")?;
    let xml = String::from_utf8_lossy(content_bytes);

    let mut out = String::with_capacity(xml.len() / 3);
    let mut cursor = 0usize;
    while cursor < xml.len() {
        let rest = &xml[cursor..];
        let next_tbl = rest.find("<table:table ");
        let next_h = rest.find("<text:h ");
        let next_p = rest.find("<text:p").and_then(|pos| {
            let after = rest.as_bytes().get(pos + 7).copied().unwrap_or(b' ');
            if after == b'>' || after == b' ' {
                Some(pos)
            } else {
                None
            }
        });

        let min_pos = [next_tbl, next_h, next_p].into_iter().flatten().min();
        let Some(pos) = min_pos else {
            break;
        };
        let abs = cursor + pos;

        if Some(pos) == next_tbl {
            if let Some(end_rel) = xml[abs..].find("</table:table>") {
                let tbl_xml = &xml[abs..abs + end_rel + 14];
                let md_tbl = parse_odf_table(tbl_xml, false);
                if !md_tbl.is_empty() {
                    out.push_str(&md_tbl);
                    out.push('\n');
                }
                cursor = abs + end_rel + 14;
            } else {
                break;
            }
        } else if Some(pos) == next_h {
            if let Some(end_rel) = xml[abs..].find("</text:h>") {
                let h_xml = &xml[abs..abs + end_rel + 9];
                let gt = h_xml.find('>').unwrap_or(0);
                let lvl = extract_xml_attr(&h_xml[..gt], "text:outline-level")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(2)
                    .clamp(1, 4);
                let txt = strip_all_xml_tags(&h_xml[gt + 1..]).trim().to_string();
                if !txt.is_empty() {
                    out.push_str(&format!("{} {}\n\n", "#".repeat(lvl), txt));
                }
                cursor = abs + end_rel + 9;
            } else {
                break;
            }
        } else if let Some(end_rel) = xml[abs..].find("</text:p>") {
            let p_xml = &xml[abs..abs + end_rel + 9];
            let gt = p_xml.find('>').unwrap_or(0);
            let txt = strip_all_xml_tags(&p_xml[gt + 1..]).trim().to_string();
            if !txt.is_empty() {
                out.push_str(&txt);
                out.push_str("\n\n");
            }
            cursor = abs + end_rel + 9;
        } else {
            cursor = abs + 7;
        }
    }

    if out.trim().is_empty() {
        anyhow::bail!("OpenDocument Text (.odt) does not contain any readable text.");
    }
    Ok(out.trim().to_string())
}

/// Extracts structured Markdown (headings, lists, tables, bold/italic) from a Rich Text Format (`.rtf`)
/// document via ArtCraft's `craft_rtf::doc::import` + `craft_markdown::export`.
pub fn extract_rtf_markdown(bytes: &[u8]) -> Result<String> {
    if let Ok(doc) = craft_rtf::doc::import(bytes) {
        let md = craft_markdown::export(&doc);
        if !md.trim().is_empty() {
            return Ok(md.trim().to_string());
        }
    }

    let raw = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(raw.len() / 2);
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0usize;
    let mut skip_group_depth = 0i32;
    let mut group_depth = 0i32;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '{' {
            group_depth += 1;
            if i + 2 < chars.len() && chars[i + 1] == '\\' && chars[i + 2] == '*' {
                if skip_group_depth == 0 {
                    skip_group_depth = group_depth;
                }
            }
            i += 1;
            continue;
        }
        if ch == '}' {
            if skip_group_depth == group_depth {
                skip_group_depth = 0;
            }
            group_depth = (group_depth - 1).max(0);
            i += 1;
            continue;
        }
        if skip_group_depth > 0 {
            i += 1;
            continue;
        }
        if ch == '\\' {
            i += 1;
            if i >= chars.len() {
                break;
            }
            let next = chars[i];
            if next == '\\' || next == '{' || next == '}' {
                out.push(next);
                i += 1;
                continue;
            }
            if next == '\'' && i + 2 < chars.len() {
                let hex: String = chars[i + 1..=i + 2].iter().collect();
                if let Ok(b) = u8::from_str_radix(&hex, 16) {
                    out.push(b as char);
                }
                i += 3;
                continue;
            }
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                i += 1;
            }
            if i < chars.len() && chars[i] == ' ' {
                i += 1;
            }
            match word.as_str() {
                "par" | "pard" | "line" | "row" => out.push('\n'),
                "tab" | "cell" => out.push('\t'),
                "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" => {
                    skip_group_depth = group_depth;
                }
                _ => {}
            }
            continue;
        }
        if ch != '\r' && ch != '\n' {
            out.push(ch);
        }
        i += 1;
    }

    Ok(out.trim().to_string())
}

// ============================================================================
// 3. Spreadsheet & Tabular Data Pipeline (.xlsx, .ods, .csv, .tsv)
// ============================================================================

/// Formats a `craft_sheet::Value` into a clean table cell string.
fn format_craft_sheet_value(v: &craft_sheet::Value) -> String {
    match v {
        craft_sheet::Value::Empty => String::new(),
        craft_sheet::Value::Text(s) => s
            .replace('\r', "")
            .replace('\n', " ")
            .replace('|', "/")
            .trim()
            .to_string(),
        craft_sheet::Value::Number(n) => {
            if n.is_finite() {
                if (*n - n.trunc()).abs() == 0.0 && n.abs() < 1e15 {
                    format!("{:.0}", n)
                } else {
                    format!("{}", (n * 10_000.0).round() / 10_000.0)
                }
            } else {
                String::new()
            }
        }
        craft_sheet::Value::Bool(b) => {
            if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        craft_sheet::Value::Error(e) => e.to_string(),
    }
}

fn craft_grid_to_rows(grid: &BTreeMap<(u32, u32), String>) -> Vec<Vec<String>> {
    let mut by_row: BTreeMap<u32, BTreeMap<u32, String>> = BTreeMap::new();
    let mut max_col = 0usize;
    for ((r, c), v) in grid {
        if !v.is_empty() {
            max_col = max_col.max((*c as usize) + 1);
            by_row.entry(*r).or_default().insert(*c, v.clone());
        }
    }
    if max_col == 0 {
        return Vec::new();
    }
    by_row
        .into_values()
        .map(|cols| {
            let mut row = vec![String::new(); max_col];
            for (c, v) in cols {
                if (c as usize) < max_col {
                    row[c as usize] = v;
                }
            }
            row
        })
        .collect()
}

/// Extracts all worksheets from an Excel `.xlsx`, `.xlsm`, or `.xlsb` file into structured Markdown tables
/// (`## Tabellenblatt: <SheetName>` + `| Col A | Col B | ... |`) using ArtCraft's `craft_xlsx::Package`.
pub fn extract_xlsx_markdown(bytes: &[u8]) -> Result<String> {
    if let Ok(mut pkg) = craft_xlsx::Package::open(bytes, &craft_xlsx::Limits::default()) {
        if let Ok(wb) = pkg.workbook() {
            let strings = pkg.shared_strings().unwrap_or_default();
            let mut out = String::new();
            for sh in &wb.sheets {
                let mut grid: BTreeMap<(u32, u32), String> = BTreeMap::new();
                if pkg
                    .sheet(sh, |row| {
                        if (row.index as usize) < 100_000 {
                            for cell in &row.cells {
                                if (cell.at.col as usize) < 256 {
                                    let resolved = cell.value.resolve(&strings, wb.date_1904);
                                    let s = format_craft_sheet_value(&resolved);
                                    if !s.is_empty() {
                                        grid.insert((cell.at.row, cell.at.col), s);
                                    }
                                }
                            }
                        }
                        Ok(())
                    })
                    .is_ok()
                {
                    let rows = craft_grid_to_rows(&grid);
                    if !rows.is_empty() {
                        out.push_str(&format!("## Tabellenblatt: {}\n\n", sh.name));
                        out.push_str(&render_rows_as_markdown_table(&rows));
                        out.push_str("\n\n");
                    }
                }
            }
            if !out.trim().is_empty() {
                return Ok(out.trim().to_string());
            }
        }
    }

    let entries = extract_zip_entries(bytes)?;

    // 1. Parse Shared Strings (`xl/sharedStrings.xml`)
    let mut shared_strings: Vec<String> = Vec::new();
    if let Some(ss_bytes) = entries.get("xl/sharedStrings.xml") {
        let ss_xml = String::from_utf8_lossy(ss_bytes);
        for si_part in ss_xml.split("<si").skip(1) {
            let Some(gt) = si_part.find('>') else {
                continue;
            };
            let end_si = si_part.find("</si>").unwrap_or(si_part.len());
            let si_body = &si_part[gt + 1..end_si];
            let mut val = String::new();
            for t_part in si_body.split("<t").skip(1) {
                if let Some(t_gt) = t_part.find('>') {
                    if let Some(t_end) = t_part[t_gt + 1..].find("</t>") {
                        val.push_str(&decode_xml_entities(&t_part[t_gt + 1..t_gt + 1 + t_end]));
                    }
                }
            }
            shared_strings.push(val);
        }
    }

    // 2. Parse Sheet Names from `xl/workbook.xml`
    let mut sheet_names: Vec<String> = Vec::new();
    if let Some(wb_bytes) = entries.get("xl/workbook.xml") {
        let wb_xml = String::from_utf8_lossy(wb_bytes);
        for sh_part in wb_xml.split("<sheet ").skip(1) {
            let end_tag = sh_part.find('>').unwrap_or(sh_part.len());
            if let Some(name) = extract_xml_attr(&sh_part[..end_tag], "name") {
                sheet_names.push(decode_xml_entities(name));
            }
        }
    }

    // 3. Collect worksheet XML files (`xl/worksheets/sheet1.xml`, `sheet2.xml`, ...)
    let mut sheet_files: Vec<(usize, String)> = entries
        .keys()
        .filter(|k| k.starts_with("xl/worksheets/sheet") && k.ends_with(".xml"))
        .map(|k| {
            let num_str: String = k
                .trim_start_matches("xl/worksheets/sheet")
                .trim_end_matches(".xml")
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            let num = num_str.parse::<usize>().unwrap_or(999);
            (num, k.clone())
        })
        .collect();
    sheet_files.sort_by_key(|(n, _)| *n);

    let mut out = String::new();
    for (idx, (_, path)) in sheet_files.iter().enumerate() {
        let Some(sh_bytes) = entries.get(path) else {
            continue;
        };
        let sh_xml = String::from_utf8_lossy(sh_bytes);
        let sheet_title = sheet_names
            .get(idx)
            .cloned()
            .unwrap_or_else(|| format!("Sheet {}", idx + 1));

        let rows = parse_xlsx_worksheet(&sh_xml, &shared_strings);
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!("## Tabellenblatt: {}\n\n", sheet_title));
        out.push_str(&render_rows_as_markdown_table(&rows));
        out.push_str("\n\n");
    }

    if out.trim().is_empty() {
        anyhow::bail!("Excel workbook (.xlsx) does not contain any non-empty worksheet cells.");
    }
    Ok(out.trim().to_string())
}

fn parse_xlsx_worksheet(sh_xml: &str, shared_strings: &[String]) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut max_cols = 0usize;

    for row_part in sh_xml.split("<row").skip(1) {
        let Some(row_gt) = row_part.find('>') else {
            continue;
        };
        let Some(row_end) = row_part.find("</row>") else {
            continue;
        };
        let row_body = &row_part[row_gt + 1..row_end];

        let mut row_cells: Vec<String> = Vec::new();
        for c_part in row_body.split("<c ").skip(1) {
            let Some(c_gt) = c_part.find('>') else {
                continue;
            };
            let c_header = &c_part[..c_gt];
            // Check self-closing empty cell `<c r="A1" s="1"/>`
            if c_header.ends_with('/') {
                continue;
            }
            let c_end = c_part.find("</c>").unwrap_or(c_part.len());
            let c_body = &c_part[c_gt + 1..c_end];

            let col_idx = extract_xml_attr(c_header, "r")
                .map(excel_col_ref_to_index)
                .unwrap_or(row_cells.len());
            let cell_type = extract_xml_attr(c_header, "t").unwrap_or("");

            let raw_val = if cell_type == "inlineStr" {
                strip_all_xml_tags(c_body)
            } else if let Some(v_start) = c_body.find("<v>") {
                let v_rest = &c_body[v_start + 3..];
                let v_end = v_rest.find("</v>").unwrap_or(v_rest.len());
                v_rest[..v_end].to_string()
            } else {
                String::new()
            };

            let display_val = match cell_type {
                "s" => raw_val
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| shared_strings.get(i).cloned())
                    .unwrap_or_default(),
                "b" => {
                    if raw_val.trim() == "1" {
                        "TRUE".to_string()
                    } else {
                        "FALSE".to_string()
                    }
                }
                _ => clean_excel_number(&decode_xml_entities(&raw_val)),
            };

            let clean_val = display_val
                .replace('\r', "")
                .replace('\n', " ")
                .replace('|', "/")
                .trim()
                .to_string();

            if col_idx < 128 {
                if row_cells.len() <= col_idx {
                    row_cells.resize(col_idx + 1, String::new());
                }
                row_cells[col_idx] = clean_val;
            }
        }

        if row_cells.iter().any(|c| !c.is_empty()) {
            max_cols = max_cols.max(row_cells.len());
            rows.push(row_cells);
        }
    }

    for r in &mut rows {
        r.resize(max_cols, String::new());
    }
    rows
}

fn excel_col_ref_to_index(cell_ref: &str) -> usize {
    let mut col = 0usize;
    for ch in cell_ref.chars() {
        if ch.is_ascii_alphabetic() {
            col = col * 26 + ((ch.to_ascii_uppercase() as usize) - ('A' as usize) + 1);
        } else {
            break;
        }
    }
    col.saturating_sub(1)
}

fn clean_excel_number(val: &str) -> String {
    let trimmed = val.trim();
    if trimmed.contains('.') && (trimmed.contains("000000") || trimmed.contains("999999")) {
        if let Ok(f) = trimmed.parse::<f64>() {
            let rounded = (f * 10_000.0).round() / 10_000.0;
            return format!("{}", rounded);
        }
    }
    trimmed.to_string()
}

/// Extracts all worksheets from a legacy Excel 97–2003 (`.xls`, BIFF8) workbook into Markdown tables
/// using ArtCraft's `craft_xls::read`.
pub fn extract_xls_markdown(bytes: &[u8]) -> Result<String> {
    let read = craft_xls::read(bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse legacy Excel .xls file: {}", e))?;
    let mut out = String::new();
    for sh in &read.workbook.sheets {
        let mut grid: BTreeMap<(u32, u32), String> = BTreeMap::new();
        for (cell_ref, cell) in &sh.cells {
            if (cell_ref.row as usize) < 100_000 && (cell_ref.col as usize) < 256 {
                let s = format_craft_sheet_value(&cell.value);
                if !s.is_empty() {
                    grid.insert((cell_ref.row, cell_ref.col), s);
                }
            }
        }
        let rows = craft_grid_to_rows(&grid);
        if !rows.is_empty() {
            out.push_str(&format!("## Tabellenblatt: {}\n\n", sh.name));
            out.push_str(&render_rows_as_markdown_table(&rows));
            out.push_str("\n\n");
        }
    }
    if out.trim().is_empty() {
        anyhow::bail!("Legacy Excel (.xls) workbook does not contain any non-empty cells.");
    }
    Ok(out.trim().to_string())
}

/// Extracts all worksheets from an OpenDocument Spreadsheet (`.ods` / LibreOffice Calc) into Markdown tables
/// using ArtCraft's `craft_ods::read`.
pub fn extract_ods_markdown(bytes: &[u8]) -> Result<String> {
    if let Ok(book) = craft_ods::read(bytes, &craft_ods::Options::default()) {
        let mut out = String::new();
        for sh in &book.sheets {
            let mut grid: BTreeMap<(u32, u32), String> = BTreeMap::new();
            for (cell_ref, cell) in &sh.cells {
                if (cell_ref.row as usize) < 100_000 && (cell_ref.col as usize) < 256 {
                    let s = if !cell.text.trim().is_empty() {
                        cell.text
                            .replace('\r', "")
                            .replace('\n', " ")
                            .replace('|', "/")
                            .trim()
                            .to_string()
                    } else if let Some(v) = cell.value() {
                        format_craft_sheet_value(&v)
                    } else {
                        String::new()
                    };
                    if !s.is_empty() {
                        grid.insert((cell_ref.row, cell_ref.col), s);
                    }
                }
            }
            let rows = craft_grid_to_rows(&grid);
            if !rows.is_empty() {
                out.push_str(&format!("## Tabellenblatt: {}\n\n", sh.name));
                out.push_str(&render_rows_as_markdown_table(&rows));
                out.push_str("\n\n");
            }
        }
        if !out.trim().is_empty() {
            return Ok(out.trim().to_string());
        }
    }

    let entries = extract_zip_entries(bytes)?;
    let content_bytes = entries
        .get("content.xml")
        .context("Invalid .ods file: 'content.xml' not found in archive")?;
    let xml = String::from_utf8_lossy(content_bytes);

    let md = parse_odf_table(&xml, true);
    if md.trim().is_empty() {
        anyhow::bail!("OpenDocument Spreadsheet (.ods) does not contain any non-empty tables.");
    }
    Ok(md)
}

fn parse_odf_table(xml: &str, emit_sheet_headings: bool) -> String {
    let mut out = String::new();
    for (idx, tbl_part) in xml.split("<table:table ").skip(1).enumerate() {
        let Some(tbl_gt) = tbl_part.find('>') else {
            continue;
        };
        let tbl_header = &tbl_part[..tbl_gt];
        let sheet_name = extract_xml_attr(tbl_header, "table:name")
            .map(decode_xml_entities)
            .unwrap_or_else(|| format!("Sheet {}", idx + 1));
        let tbl_end = tbl_part.find("</table:table>").unwrap_or(tbl_part.len());
        let tbl_body = &tbl_part[tbl_gt + 1..tbl_end];

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut max_cols = 0usize;

        for row_part in tbl_body.split("<table:table-row").skip(1) {
            let Some(r_gt) = row_part.find('>') else {
                continue;
            };
            if row_part[..r_gt].ends_with('/') {
                continue;
            }
            let r_end = row_part
                .find("</table:table-row>")
                .unwrap_or(row_part.len());
            let r_body = &row_part[r_gt + 1..r_end];

            let mut row_cells: Vec<String> = Vec::new();
            for cell_part in r_body.split("<table:table-cell").skip(1) {
                let Some(c_gt) = cell_part.find('>') else {
                    continue;
                };
                let c_header = &cell_part[..c_gt];
                let repeat = extract_xml_attr(c_header, "table:number-columns-repeated")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .min(32);

                if c_header.ends_with('/') {
                    for _ in 0..repeat {
                        row_cells.push(String::new());
                    }
                    continue;
                }
                let c_end = cell_part
                    .find("</table:table-cell>")
                    .unwrap_or(cell_part.len());
                let c_body = &cell_part[c_gt + 1..c_end];
                let cell_txt = strip_all_xml_tags(c_body)
                    .replace('\r', "")
                    .replace('\n', " ")
                    .replace('|', "/")
                    .trim()
                    .to_string();
                let eff_repeat = if cell_txt.is_empty() { repeat } else { repeat.min(8) };
                for _ in 0..eff_repeat {
                    row_cells.push(cell_txt.clone());
                }
            }

            // Trim trailing empty cells
            while row_cells.last().map_or(false, |c| c.is_empty()) {
                row_cells.pop();
            }
            if !row_cells.is_empty() {
                max_cols = max_cols.max(row_cells.len());
                rows.push(row_cells);
            }
        }

        if rows.is_empty() {
            continue;
        }
        for r in &mut rows {
            r.resize(max_cols, String::new());
        }
        if emit_sheet_headings {
            out.push_str(&format!("## Tabellenblatt: {}\n\n", sheet_name));
        }
        out.push_str(&render_rows_as_markdown_table(&rows));
        out.push_str("\n\n");
    }
    out.trim().to_string()
}

/// Converts a `.csv` or `.tsv` file (with automatic delimiter detection for `;`, `,`, `\t`, `|`)
/// into a structured Markdown table.
pub fn extract_csv_markdown(filename: &str, bytes: &[u8]) -> Result<String> {
    let raw = String::from_utf8_lossy(bytes);
    let non_empty: Vec<&str> = raw
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    if non_empty.is_empty() {
        anyhow::bail!("CSV/TSV file '{}' is empty.", filename);
    }

    let sample = non_empty.iter().take(5).copied().collect::<Vec<_>>().join("\n");
    let delim = if filename.to_lowercase().ends_with(".tsv") || sample.contains('\t') {
        '\t'
    } else {
        let semi = sample.chars().filter(|&c| c == ';').count();
        let comma = sample.chars().filter(|&c| c == ',').count();
        let pipe = sample.chars().filter(|&c| c == '|').count();
        if semi >= comma && semi >= pipe && semi > 0 {
            ';'
        } else if pipe > comma && pipe > 0 {
            '|'
        } else {
            ','
        }
    };

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(non_empty.len());
    let mut max_cols = 0usize;
    for line in non_empty {
        let cells = parse_csv_line(line, delim);
        if cells.iter().any(|c| !c.is_empty()) {
            max_cols = max_cols.max(cells.len());
            rows.push(cells);
        }
    }
    for r in &mut rows {
        r.resize(max_cols, String::new());
    }

    let mut out = format!("## Tabelle: {}\n\n", filename);
    out.push_str(&render_rows_as_markdown_table(&rows));
    Ok(out)
}

fn parse_csv_line(line: &str, delim: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            if in_quotes && i + 1 < chars.len() && chars[i + 1] == '"' {
                cur.push('"');
                i += 2;
                continue;
            }
            in_quotes = !in_quotes;
        } else if ch == delim && !in_quotes {
            cells.push(cur.trim().replace('|', "/"));
            cur.clear();
        } else {
            cur.push(ch);
        }
        i += 1;
    }
    cells.push(cur.trim().replace('|', "/"));
    cells
}

fn render_rows_as_markdown_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return String::new();
    }

    let mut out = String::new();
    for (r_idx, row) in rows.iter().enumerate() {
        out.push('|');
        for c_idx in 0..col_count {
            let val = row.get(c_idx).map(|s| s.as_str()).unwrap_or("");
            out.push(' ');
            out.push_str(val);
            out.push_str(" |");
        }
        out.push('\n');
        if r_idx == 0 {
            out.push('|');
            for _ in 0..col_count {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out
}

/// Specialized table-aware chunker for Spreadsheets (`.xlsx`, `.ods`, `.csv`, `.tsv`).
/// When a worksheet has many rows and spans multiple chunks, it automatically repeats the
/// `## Tabellenblatt: <Name>` heading AND the Markdown table header row (`| Col A | Col B |` + `| --- | --- |`)
/// at the top of every chunk so every row retains its exact column semantics!
pub fn chunk_spreadsheet_with_headers(
    doc_title: &str,
    markdown: &str,
    max_chars: usize,
) -> Vec<String> {
    let target_max = max_chars.max(600);
    let mut chunks: Vec<String> = Vec::new();

    for section in markdown.split("\n## ") {
        let sec_text = if section.trim_start().starts_with("## ") {
            section.trim().to_string()
        } else if section.trim().is_empty() {
            continue;
        } else {
            format!("## {}", section.trim())
        };

        let lines: Vec<&str> = sec_text.lines().collect();
        let sheet_heading = lines
            .first()
            .copied()
            .unwrap_or("## Tabellenblatt")
            .trim_start_matches('#')
            .trim();

        let table_lines: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|l| l.trim().starts_with('|'))
            .collect();

        if table_lines.len() <= 2 {
            chunks.push(format!("[Dokument: {} | {}]\n{}", doc_title, sheet_heading, sec_text));
            continue;
        }

        let header_line = table_lines[0];
        let sep_line = table_lines[1];
        let data_rows = &table_lines[2..];

        let mut cur_block = String::new();
        let mut row_start_idx = 1usize;

        for (i, &row) in data_rows.iter().enumerate() {
            if !cur_block.is_empty()
                && (cur_block.len() + row.len() + header_line.len() + 120) > target_max
            {
                let chunk_txt = format!(
                    "[Dokument: {} | {} (Zeilen {}–{})]\n## {}\n\n{}\n{}\n{}",
                    doc_title,
                    sheet_heading,
                    row_start_idx,
                    i,
                    sheet_heading,
                    header_line,
                    sep_line,
                    cur_block.trim_end()
                );
                chunks.push(chunk_txt);
                cur_block.clear();
                row_start_idx = i + 1;
            }
            cur_block.push_str(row);
            cur_block.push('\n');
        }

        if !cur_block.is_empty() {
            let chunk_txt = format!(
                "[Dokument: {} | {} (Zeilen {}–{})]\n## {}\n\n{}\n{}\n{}",
                doc_title,
                sheet_heading,
                row_start_idx,
                data_rows.len(),
                sheet_heading,
                header_line,
                sep_line,
                cur_block.trim_end()
            );
            chunks.push(chunk_txt);
        }
    }

    chunks
}

// ============================================================================
// 4. Presentations Pipeline (.pptx, .ppt & .odp) via ArtCraft (craft-pptx & craft-ppt-binary)
// ============================================================================

fn render_craft_presentation_to_markdown(pres: &craft_pptx::Presentation) -> String {
    let mut out = String::new();
    for (idx, slide_arc) in pres.slides.iter().enumerate() {
        let slide = slide_arc.as_ref();
        let slide_num = pres.slide_number(idx);
        let mut title = slide.name.trim().to_string();
        let mut body_items: Vec<String> = Vec::new();
        let mut tables_md: Vec<String> = Vec::new();

        craft_pptx::walk(&slide.shapes, &mut |shape, _| {
            if shape.hidden {
                return;
            }
            let is_title_ph = matches!(
                shape.ph_type(),
                Some(craft_pptx::PhType::Title | craft_pptx::PhType::CtrTitle)
            );
            if let Some(ref tb) = shape.text {
                for para in &tb.paragraphs {
                    let runs: Vec<(&str, bool, bool)> = para
                        .runs
                        .iter()
                        .map(|r| {
                            (
                                r.content(),
                                r.props.bold.unwrap_or(false),
                                r.props.italic.unwrap_or(false),
                            )
                        })
                        .collect();
                    let formatted = craft_markdown::extracted::paragraph(&runs)
                        .trim()
                        .to_string();
                    if formatted.is_empty() {
                        continue;
                    }
                    if is_title_ph && title.is_empty() {
                        title = formatted.trim_matches('*').trim().to_string();
                    } else {
                        let indent = "  ".repeat((para.level as usize).min(4));
                        body_items.push(format!("{}- {}", indent, formatted));
                    }
                }
            }
            if let craft_pptx::ShapeKind::Table(ref tbl) = shape.kind {
                let mut rows: Vec<Vec<String>> = Vec::new();
                for r in &tbl.rows {
                    let row_cells: Vec<String> = r
                        .cells
                        .iter()
                        .map(|c| {
                            c.text
                                .text()
                                .replace('\r', "")
                                .replace('\n', " ")
                                .replace('|', "/")
                                .trim()
                                .to_string()
                        })
                        .collect();
                    if row_cells.iter().any(|c| !c.is_empty()) {
                        rows.push(row_cells);
                    }
                }
                let md_tbl = render_rows_as_markdown_table(&rows);
                if !md_tbl.is_empty() {
                    tables_md.push(md_tbl);
                }
            }
        });

        let notes_txt = slide.notes.text().trim().to_string();
        if title.is_empty() && body_items.is_empty() && tables_md.is_empty() && notes_txt.is_empty() {
            continue;
        }

        if title.is_empty() {
            out.push_str(&format!("## Folie {}\n\n", slide_num));
        } else {
            out.push_str(&format!("## Folie {}: {}\n\n", slide_num, title));
        }

        if !body_items.is_empty() {
            out.push_str(&body_items.join("\n"));
            out.push_str("\n\n");
        }
        for t in tables_md {
            out.push_str(&t);
            out.push('\n');
        }
        if !notes_txt.is_empty() {
            out.push_str(&format!("### Sprechernotizen (Folie {})\n\n{}\n\n", slide_num, notes_txt));
        }
    }
    out.trim().to_string()
}

/// Extracts slides, formatted runs (`**bold**`, `*italic*`), tables, and speaker notes from a
/// PowerPoint (`.pptx`, `.ppsx`, `.potx`) presentation into Markdown via ArtCraft's `craft_pptx`.
pub fn extract_pptx_markdown(bytes: &[u8]) -> Result<String> {
    if let Ok(pres) = craft_pptx::import(bytes, &craft_pptx::Plain) {
        let md = render_craft_presentation_to_markdown(&pres);
        if !md.is_empty() {
            return Ok(md);
        }
    }

    let entries = extract_zip_entries(bytes)?;
    let mut slide_files: Vec<(usize, String)> = entries
        .keys()
        .filter(|k| k.starts_with("ppt/slides/slide") && k.ends_with(".xml"))
        .map(|k| {
            let num_str: String = k
                .trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            let num = num_str.parse::<usize>().unwrap_or(999);
            (num, k.clone())
        })
        .collect();
    slide_files.sort_by_key(|(n, _)| *n);

    let mut out = String::new();
    for (slide_num, path) in slide_files {
        let Some(s_bytes) = entries.get(&path) else {
            continue;
        };
        let s_xml = String::from_utf8_lossy(s_bytes);
        let mut paragraphs: Vec<String> = Vec::new();
        for p_part in s_xml.split("<a:p").skip(1) {
            let mut p_txt = String::new();
            for t_part in p_part.split("<a:t>").skip(1) {
                if let Some(end_t) = t_part.find("</a:t>") {
                    p_txt.push_str(&decode_xml_entities(&t_part[..end_t]));
                }
            }
            let clean = p_txt.trim().to_string();
            if !clean.is_empty() {
                paragraphs.push(clean);
            }
        }
        if paragraphs.is_empty() {
            continue;
        }
        let title = paragraphs.first().cloned().unwrap_or_default();
        out.push_str(&format!("## Folie {}: {}\n\n", slide_num, title));
        for bullet in paragraphs.iter().skip(1) {
            out.push_str(&format!("- {}\n", bullet));
        }
        out.push('\n');
    }

    if out.trim().is_empty() {
        anyhow::bail!("PowerPoint presentation (.pptx) does not contain any readable text.");
    }
    Ok(out.trim().to_string())
}

/// Extracts slides, formatted runs, tables, and speaker notes from a legacy PowerPoint 95 / 97–2003
/// (`.ppt`, `.pps`, `.pot`) binary presentation via ArtCraft's `craft_ppt_binary`.
pub fn extract_ppt_binary_markdown(bytes: &[u8]) -> Result<String> {
    let pres = craft_ppt_binary::import(bytes, &craft_pptx::Plain)
        .map_err(|e| anyhow::anyhow!("Failed to parse legacy PowerPoint .ppt file: {}", e))?;
    let md = render_craft_presentation_to_markdown(&pres);
    if md.is_empty() {
        anyhow::bail!("Legacy PowerPoint (.ppt) presentation does not contain any readable text.");
    }
    Ok(md)
}

/// Extracts slides from an OpenDocument Presentation (`.odp` / LibreOffice Impress) into Markdown.
pub fn extract_odp_markdown(bytes: &[u8]) -> Result<String> {
    let entries = extract_zip_entries(bytes)?;
    let content_bytes = entries
        .get("content.xml")
        .context("Invalid .odp file: 'content.xml' not found in archive")?;
    let xml = String::from_utf8_lossy(content_bytes);

    let mut out = String::new();
    for (idx, page_part) in xml.split("<draw:page ").skip(1).enumerate() {
        let Some(gt) = page_part.find('>') else {
            continue;
        };
        let page_name = extract_xml_attr(&page_part[..gt], "draw:name")
            .map(decode_xml_entities)
            .unwrap_or_else(|| format!("Folie {}", idx + 1));
        let end_page = page_part.find("</draw:page>").unwrap_or(page_part.len());
        let page_body = &page_part[gt + 1..end_page];

        let mut lines = Vec::new();
        for p_part in page_body.split("<text:p").skip(1) {
            if let Some(p_gt) = p_part.find('>') {
                let p_end = p_part.find("</text:p>").unwrap_or(p_part.len());
                let txt = strip_all_xml_tags(&p_part[p_gt + 1..p_end]).trim().to_string();
                if !txt.is_empty() {
                    lines.push(txt);
                }
            }
        }
        if !lines.is_empty() {
            out.push_str(&format!("## Folie {} ({})\n\n", idx + 1, page_name));
            for l in lines {
                out.push_str(&format!("- {}\n", l));
            }
            out.push('\n');
        }
    }

    if out.trim().is_empty() {
        anyhow::bail!("OpenDocument Presentation (.odp) does not contain any readable text.");
    }
    Ok(out.trim().to_string())
}

// ============================================================================
// 5. Markdown Pipeline (.md, .markdown)
// ============================================================================

/// Normalizes a Markdown document, converting YAML frontmatter (`--- ... ---`) into a clean
/// structured header block while preserving headings, code blocks, and tables.
pub fn extract_markdown_document(filename: &str, bytes: &[u8]) -> Result<String> {
    let raw = String::from_utf8_lossy(bytes);
    let trimmed = raw.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        anyhow::bail!("Markdown file '{}' is empty.", filename);
    }

    if let Some(after_open) = trimmed.strip_prefix("---\n").or_else(|| trimmed.strip_prefix("---\r\n")) {
        if let Some(close_idx) = after_open.find("\n---") {
            let frontmatter = &after_open[..close_idx];
            let body_start = close_idx + 4;
            let body = after_open[body_start..].trim_start();
            let mut fm_lines = Vec::new();
            let mut doc_title = String::new();
            for line in frontmatter.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'');
                    if !key.is_empty() && !val.is_empty() {
                        if key.eq_ignore_ascii_case("title") {
                            doc_title = val.to_string();
                        }
                        fm_lines.push(format!("- **{}**: {}", key, val));
                    }
                }
            }
            let mut out = String::with_capacity(trimmed.len());
            if !doc_title.is_empty() && !body.starts_with("# ") {
                out.push_str(&format!("# {}\n\n", doc_title));
            }
            if !fm_lines.is_empty() {
                out.push_str(&fm_lines.join("\n"));
                out.push_str("\n\n");
            }
            out.push_str(body);
            return Ok(out);
        }
    }

    Ok(trimmed.to_string())
}

/// Extracts a deterministic syntactic Knowledge Graph from a Markdown document:
/// links document sections (`# / ##` headings) and explicit Markdown/Wiki links (`[[Target]]`, `[Label](target)`).
pub fn extract_syntactic_markdown_graph(
    doc_title: &str,
    markdown: &str,
) -> (
    Vec<(String, String, Option<String>)>,
    Vec<(String, String, String)>,
) {
    let mut entities: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    let mut relations: BTreeSet<(String, String, String)> = BTreeSet::new();

    let root_name = doc_title.trim().to_string();
    entities.insert(root_name.clone(), ("Document".to_string(), None));

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            let sec = trimmed.trim_start_matches('#').trim();
            if sec.len() >= 3 && sec.len() <= 90 {
                entities
                    .entry(sec.to_string())
                    .or_insert(("Section".to_string(), None));
                relations.insert((root_name.clone(), "has_section".to_string(), sec.to_string()));
            }
        }
        // Extract [[WikiLinks]]
        let mut rest = trimmed;
        while let Some(open) = rest.find("[[") {
            let after = &rest[open + 2..];
            if let Some(close) = after.find("]]") {
                let target = after[..close].split('|').next().unwrap_or("").trim();
                if target.len() >= 2 && target.len() <= 80 {
                    entities
                        .entry(target.to_string())
                        .or_insert(("Topic".to_string(), None));
                    relations.insert((
                        root_name.clone(),
                        "references".to_string(),
                        target.to_string(),
                    ));
                }
                rest = &after[close + 2..];
            } else {
                break;
            }
        }
    }

    let ent_vec = entities
        .into_iter()
        .map(|(k, (t, d))| (k, t, d))
        .collect();
    let rel_vec = relations.into_iter().collect();
    (ent_vec, rel_vec)
}

// ============================================================================
// 6. JSON & JSONL Structured Data Pipeline (.json, .jsonl)
// ============================================================================

/// Parses `.json` (Objects or Arrays) and `.jsonl` (newline-delimited JSON) into record-aligned
/// structured Markdown (`## Datensatz #N: <Identifier>` + flattened `path.to.field: value` bullets)
/// so chunking never cuts a JSON record in half.
pub fn extract_json_markdown(filename: &str, bytes: &[u8]) -> Result<String> {
    let raw = String::from_utf8_lossy(bytes);
    let trimmed = raw.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        anyhow::bail!("JSON file '{}' is empty.", filename);
    }

    let mut out = format!("# JSON Datenquelle: {}\n\n", filename);

    // 1. Check standard JSON (Object or Array)
    if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
        match val {
            Value::Array(arr) => {
                out.push_str(&format!("- **Typ**: JSON-Array ({} Einträge)\n\n", arr.len()));
                for (idx, item) in arr.iter().enumerate() {
                    let label = detect_json_record_label(item, idx + 1);
                    out.push_str(&format!("## Datensatz #{}: {}\n\n", idx + 1, label));
                    let mut lines = Vec::new();
                    flatten_json_value("", item, &mut lines);
                    out.push_str(&lines.join("\n"));
                    out.push_str("\n\n");
                }
            }
            Value::Object(map) => {
                // Check if the object wraps a single main array (e.g. `{ "items": [ ... ] }`)
                for (k, v) in &map {
                    if let Some(arr) = v.as_array() {
                        if arr.len() > 1 && arr.iter().all(|x| x.is_object()) {
                            out.push_str(&format!(
                                "## Kollektion `{}` ({} Einträge)\n\n",
                                k,
                                arr.len()
                            ));
                            for (idx, item) in arr.iter().enumerate() {
                                let label = detect_json_record_label(item, idx + 1);
                                out.push_str(&format!(
                                    "## {} #{} — {}\n\n",
                                    k,
                                    idx + 1,
                                    label
                                ));
                                let mut lines = Vec::new();
                                flatten_json_value(k, item, &mut lines);
                                out.push_str(&lines.join("\n"));
                                out.push_str("\n\n");
                            }
                            continue;
                        }
                    }
                    out.push_str(&format!("## Sektion `{}`\n\n", k));
                    let mut lines = Vec::new();
                    flatten_json_value(k, v, &mut lines);
                    out.push_str(&lines.join("\n"));
                    out.push_str("\n\n");
                }
            }
            other => {
                out.push_str(&format!("- **Wert**: {}\n", other));
            }
        }
        return Ok(out.trim().to_string());
    }

    // 2. Check JSON Lines (`.jsonl` / NDJSON)
    let mut jsonl_count = 0usize;
    for line in trimmed.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if let Ok(item) = serde_json::from_str::<Value>(l) {
            jsonl_count += 1;
            let label = detect_json_record_label(&item, jsonl_count);
            out.push_str(&format!("## Datensatz #{}: {}\n\n", jsonl_count, label));
            let mut lines = Vec::new();
            flatten_json_value("", &item, &mut lines);
            out.push_str(&lines.join("\n"));
            out.push_str("\n\n");
        }
    }

    if jsonl_count > 0 {
        return Ok(out.trim().to_string());
    }

    // 3. Fallback: return raw text if invalid JSON
    Ok(trimmed.to_string())
}

fn detect_json_record_label(val: &Value, fallback_idx: usize) -> String {
    if let Some(obj) = val.as_object() {
        for key in ["title", "name", "label", "id", "uuid", "key", "filename", "email"] {
            if let Some(v) = obj.get(key) {
                if let Some(s) = v.as_str() {
                    if !s.trim().is_empty() {
                        return format!("{} ({}={})", s.trim(), key, s.trim());
                    }
                } else if v.is_number() {
                    return format!("{}={}", key, v);
                }
            }
        }
    }
    format!("Eintrag {}", fallback_idx)
}

fn flatten_json_value(prefix: &str, val: &Value, out: &mut Vec<String>) {
    match val {
        Value::Object(map) => {
            for (k, v) in map {
                let next_prefix = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json_value(&next_prefix, v, out);
            }
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                return;
            }
            if arr.iter().all(|x| x.is_string() || x.is_number() || x.is_boolean()) {
                let joined: Vec<String> = arr
                    .iter()
                    .map(|x| match x {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect();
                out.push(format!("- **{}**: {}", prefix, joined.join(", ")));
            } else {
                for (i, item) in arr.iter().enumerate() {
                    let next_prefix = format!("{}[{}]", prefix, i);
                    flatten_json_value(&next_prefix, item, out);
                }
            }
        }
        Value::String(s) => {
            let clean = s.trim();
            if !clean.is_empty() {
                out.push(format!("- **{}**: {}", prefix, clean));
            }
        }
        Value::Null => {}
        other => {
            out.push(format!("- **{}**: {}", prefix, other));
        }
    }
}
