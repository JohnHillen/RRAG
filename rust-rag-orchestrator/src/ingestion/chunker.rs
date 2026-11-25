use super::InferenceClient;

#[derive(Debug, Clone)]
enum StructuralBlock {
    Heading(String),
    Table(String),
    List(String),
    Paragraph(String),
}

impl InferenceClient {
    /// Structure-aware document chunker without external filename context.
    pub fn semantic_chunk_text(&self, text: &str) -> Vec<String> {
        self.semantic_chunk_text_with_title(None, text)
    }

    /// Structure-aware document chunker for PDFs, tax/legal statutes (`§ 1 Abs. 1`), contracts, and Markdown:
    /// 1. Extracts the document/statute title from the beginning of the PDF (e.g. `"Einkommensteuergesetz (EStG)"`
    ///    or `"Werbevertrag: AdVantage Digital AG"`) and combines it with `doc_title`.
    /// 2. Keeps paragraphs, bullet/numbered lists, and tables intact as atomic blocks.
    /// 3. Never orphans a chapter/section/paragraph (`§`) heading at the bottom of a chunk.
    /// 4. Prepends a hierarchical context header (`[Dokument: <Titel> | Abschnitt: <§ / Kapitel>]`) to every chunk
    ///    so that identical section numbers (like `§ 1 Abs. 1` across different laws or `3. Bonus/Malus` across contracts)
    ///    are always uniquely identifiable in BM25 full-text search, HNSW vector search, and LLM synthesis.
    pub fn semantic_chunk_text_with_title(
        &self,
        doc_title: Option<&str>,
        text: &str,
    ) -> Vec<String> {
        let target_max = self.config.chunk_max_chars.max(2200);
        let hard_max = target_max + 1000;

        let extracted_pdf_title = extract_document_main_title(text);
        let doc_context_prefix =
            build_document_context_prefix(doc_title, extracted_pdf_title.as_deref());

        let blocks = parse_structural_blocks(text);
        if blocks.is_empty() {
            return Vec::new();
        }

        let mut raw_chunks: Vec<(Option<String>, String)> = Vec::new();
        let mut current_chunk = String::new();
        let mut current_has_body = false;
        let mut active_heading: Option<String> = None;
        let mut chunk_heading: Option<String> = None;

        for block in blocks {
            match block {
                StructuralBlock::Heading(h) => {
                    if current_has_body && current_chunk.len() >= 1200 {
                        raw_chunks.push((chunk_heading.clone(), current_chunk.trim().to_string()));
                        current_chunk.clear();
                        current_has_body = false;
                    }
                    let clean_h = h.trim_start_matches('#').trim().to_string();
                    active_heading = Some(clean_h.clone());
                    if !current_has_body {
                        chunk_heading = Some(clean_h);
                    }
                    if !current_chunk.is_empty() {
                        current_chunk.push_str("\n\n");
                    }
                    current_chunk.push_str(&h);
                }
                StructuralBlock::Table(tbl) => {
                    if chunk_heading.is_none() {
                        chunk_heading = active_heading.clone();
                    }
                    append_atomic_block(
                        &tbl,
                        true,
                        target_max,
                        hard_max,
                        &active_heading,
                        &mut chunk_heading,
                        &mut current_chunk,
                        &mut current_has_body,
                        &mut raw_chunks,
                    );
                }
                StructuralBlock::List(lst) | StructuralBlock::Paragraph(lst) => {
                    if chunk_heading.is_none() {
                        chunk_heading = active_heading.clone();
                    }
                    append_atomic_block(
                        &lst,
                        false,
                        target_max,
                        hard_max,
                        &active_heading,
                        &mut chunk_heading,
                        &mut current_chunk,
                        &mut current_has_body,
                        &mut raw_chunks,
                    );
                }
            }
        }

        if !current_chunk.trim().is_empty() {
            raw_chunks.push((chunk_heading, current_chunk.trim().to_string()));
        }

        raw_chunks
            .into_iter()
            .map(|(sec_hdr, body)| {
                let header_line = match (&doc_context_prefix, &sec_hdr) {
                    (Some(doc_pfx), Some(sec)) if !body.starts_with(sec) => {
                        format!("[{} | Abschnitt: {}]\n", doc_pfx, sec)
                    }
                    (Some(doc_pfx), Some(sec)) => {
                        format!("[{} | {}]\n", doc_pfx, sec)
                    }
                    (Some(doc_pfx), None) => format!("[{}]\n", doc_pfx),
                    (None, Some(sec)) if !body.starts_with(sec) => {
                        format!("[Abschnitt: {}]\n", sec)
                    }
                    _ => String::new(),
                };
                format!("{}{}", header_line, body)
            })
            .collect()
    }
}

fn extract_document_main_title(text: &str) -> Option<String> {
    for line in text.lines().take(8) {
        let trimmed = line.trim().trim_start_matches('#').trim();
        if trimmed.len() >= 6
            && trimmed.len() <= 130
            && !is_list_item(trimmed)
            && !is_table_line(trimmed)
        {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn build_document_context_prefix(doc_title: Option<&str>, pdf_title: Option<&str>) -> Option<String> {
    match (doc_title.map(str::trim).filter(|s| !s.is_empty()), pdf_title) {
        (Some(dt), Some(pt)) => {
            if dt.eq_ignore_ascii_case(pt) || pt.to_lowercase().contains(&dt.to_lowercase()) {
                Some(format!("Dokument: {}", pt))
            } else {
                Some(format!("Dokument: {} — {}", dt, pt))
            }
        }
        (Some(dt), None) => Some(format!("Dokument: {}", dt)),
        (None, Some(pt)) => Some(format!("Dokument: {}", pt)),
        (None, None) => None,
    }
}

fn parse_structural_blocks(text: &str) -> Vec<StructuralBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut prev_blank = true;

    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            prev_blank = true;
            i += 1;
            continue;
        }

        if is_section_heading(trimmed, prev_blank) {
            blocks.push(StructuralBlock::Heading(trimmed.to_string()));
            prev_blank = false;
            i += 1;
            continue;
        }

        if is_table_line(trimmed) {
            let mut tbl_lines = vec![raw];
            i += 1;
            while i < lines.len() {
                let next_trim = lines[i].trim();
                if next_trim.is_empty() {
                    break;
                }
                if is_table_line(next_trim) {
                    tbl_lines.push(lines[i]);
                    i += 1;
                } else {
                    break;
                }
            }
            blocks.push(StructuralBlock::Table(tbl_lines.join("\n")));
            prev_blank = false;
            continue;
        }

        if is_list_item(trimmed) {
            let mut list_lines = vec![raw.trim_end()];
            i += 1;
            while i < lines.len() {
                let next_raw = lines[i];
                let next_trim = next_raw.trim();
                if next_trim.is_empty() {
                    break;
                }
                if is_section_heading(next_trim, false) || is_table_line(next_trim) {
                    break;
                }
                list_lines.push(next_raw.trim_end());
                i += 1;
            }
            blocks.push(StructuralBlock::List(list_lines.join("\n")));
            prev_blank = false;
            continue;
        }

        let mut para_lines = vec![raw.trim_end()];
        i += 1;
        while i < lines.len() {
            let next_raw = lines[i];
            let next_trim = next_raw.trim();
            if next_trim.is_empty()
                || is_section_heading(next_trim, false)
                || is_table_line(next_trim)
                || is_list_item(next_trim)
            {
                break;
            }
            para_lines.push(next_raw.trim_end());
            i += 1;
        }
        blocks.push(StructuralBlock::Paragraph(para_lines.join("\n")));
        prev_blank = false;
    }

    blocks
}

fn is_section_heading(line: &str, prev_blank: bool) -> bool {
    if line.starts_with('#') {
        return true;
    }
    if line.len() > 110 {
        return false;
    }
    let upper = line.to_uppercase();
    if upper.starts_with("§ ")
        || upper.starts_with("§§ ")
        || upper.starts_with("ART. ")
        || upper.starts_with("ARTIKEL ")
        || upper.starts_with("KAPITEL ")
        || upper.starts_with("ABSCHNITT ")
        || upper.starts_with("TEIL ")
        || upper.starts_with("ANLAGE ")
    {
        return true;
    }
    if prev_blank {
        let mut chars = line.chars().peekable();
        let mut saw_digit = false;
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '.' {
                if c.is_ascii_digit() {
                    saw_digit = true;
                }
                chars.next();
            } else {
                break;
            }
        }
        if saw_digit && line.contains('.') {
            let rest: String = chars.collect();
            if let Some(first_after_space) = rest.strip_prefix(' ').and_then(|s| s.chars().next()) {
                if first_after_space.is_uppercase() {
                    return true;
                }
            }
        }
    }
    false
}

fn is_table_line(line: &str) -> bool {
    let pipe_count = line.chars().filter(|&c| c == '|').count();
    if pipe_count >= 2 {
        return true;
    }
    if (line.starts_with("+---") || line.starts_with("|---")) && line.len() >= 6 {
        return true;
    }
    false
}

fn is_list_item(line: &str) -> bool {
    if line.starts_with("* ")
        || line.starts_with("- ")
        || line.starts_with("• ")
        || line.starts_with("· ")
    {
        return true;
    }
    let bytes = line.as_bytes();
    if bytes.len() >= 3
        && (bytes[0].is_ascii_digit() || bytes[0].is_ascii_lowercase())
        && bytes[1] == b')'
        && bytes[2] == b' '
    {
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn append_atomic_block(
    block_text: &str,
    is_table: bool,
    target_max: usize,
    hard_max: usize,
    active_heading: &Option<String>,
    chunk_heading: &mut Option<String>,
    current_chunk: &mut String,
    current_has_body: &mut bool,
    raw_chunks: &mut Vec<(Option<String>, String)>,
) {
    let projected = if current_chunk.is_empty() {
        block_text.len()
    } else {
        current_chunk.len() + 2 + block_text.len()
    };

    if projected <= target_max || (!*current_has_body && projected <= hard_max) {
        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(block_text);
        *current_has_body = true;
        return;
    }

    if *current_has_body && projected <= hard_max {
        current_chunk.push_str("\n\n");
        current_chunk.push_str(block_text);
        return;
    }

    if !current_chunk.trim().is_empty() && *current_has_body {
        raw_chunks.push((chunk_heading.clone(), current_chunk.trim().to_string()));
        current_chunk.clear();
        *current_has_body = false;
        *chunk_heading = active_heading.clone();
    }

    if block_text.len() > hard_max {
        let lines: Vec<&str> = block_text.lines().collect();
        let table_header = if is_table && lines.len() >= 2 {
            Some(format!("{}\n{}", lines[0], lines[1]))
        } else {
            None
        };

        for line in lines {
            if current_chunk.len() + line.len() + 2 > target_max && *current_has_body {
                raw_chunks.push((chunk_heading.clone(), current_chunk.trim().to_string()));
                current_chunk.clear();
                *current_has_body = false;
                *chunk_heading = active_heading.clone();
            }
            if current_chunk.is_empty() {
                if let Some(ref th) = table_header {
                    if !th.contains(line) {
                        current_chunk.push_str(th);
                        current_chunk.push('\n');
                    }
                }
            } else {
                current_chunk.push('\n');
            }
            current_chunk.push_str(line);
            *current_has_body = true;
        }
    } else {
        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(block_text);
        *current_has_body = true;
    }
}
