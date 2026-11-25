/// Structured XML-to-Markdown converter with native support for:
/// 1. German federal laws (`gii-norm.dtd` from *gesetze-im-internet.de*, e.g. GwG, KWG, EStG, AO)
/// 2. European Union regulations & directives (`Formex v4/v6` `.fmx.xml` from *EUR-Lex / EU Publications Office*, e.g. AMLR EU 2024/1624)
/// 3. Generic XML documents.
pub fn extract_structured_markdown_from_xml(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    if raw.contains("<dokumente") && raw.contains("<norm") {
        let parsed = parse_german_statute_xml(&raw);
        if !parsed.trim().is_empty() {
            return parsed;
        }
    }
    if (raw.contains("<ACT") || raw.contains("<ANNEX"))
        && (raw.contains("<ENACTING.TERMS") || raw.contains("<ARTICLE") || raw.contains("<TI.ART>"))
    {
        let parsed = parse_eu_formex_xml(&raw);
        if !parsed.trim().is_empty() {
            return parsed;
        }
    }
    parse_generic_xml(&raw)
}

/// Parses a `gii-norm.dtd` German statute XML (`<dokumente><norm>...</norm></dokumente>`)
/// into clean, hierarchical Markdown optimized for structure-aware legal chunking.
fn parse_german_statute_xml(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 2);
    let mut law_abbr = String::new();
    let mut law_short_title = String::new();
    let mut law_long_title = String::new();
    let mut current_section = String::new();
    let mut header_emitted = false;

    for norm_chunk in xml.split("<norm").skip(1) {
        let Some(norm_body_start) = norm_chunk.find('>') else {
            continue;
        };
        let norm_end = norm_chunk.find("</norm>").unwrap_or(norm_chunk.len());
        let norm_xml = &norm_chunk[norm_body_start + 1..norm_end];

        let meta_xml = extract_tag_inner(norm_xml, "metadaten").unwrap_or("");
        let textdaten_xml = extract_tag_inner(norm_xml, "textdaten").unwrap_or("");

        if law_abbr.is_empty() {
            if let Some(amtabk) = extract_tag_inner(meta_xml, "amtabk") {
                let clean = clean_inline_xml(amtabk);
                if !clean.is_empty() {
                    law_abbr = clean;
                }
            }
            if law_abbr.is_empty() {
                let jurabks = extract_all_tags_inner(meta_xml, "jurabk");
                if let Some(best) = jurabks
                    .into_iter()
                    .map(clean_inline_xml)
                    .filter(|s| !s.is_empty())
                    .min_by_key(|s| s.len())
                {
                    law_abbr = best;
                }
            }
        }

        if law_short_title.is_empty() {
            if let Some(kurzue) = extract_tag_inner(meta_xml, "kurzue") {
                law_short_title = clean_inline_xml(kurzue);
            }
        }
        if law_long_title.is_empty() {
            if let Some(langue) = extract_tag_inner(meta_xml, "langue") {
                law_long_title = clean_inline_xml(langue);
            }
        }

        if !header_emitted && (!law_abbr.is_empty() || !law_long_title.is_empty()) {
            let main_heading = match (
                !law_abbr.is_empty(),
                !law_short_title.is_empty(),
                !law_long_title.is_empty(),
            ) {
                (true, true, true) if !law_long_title.eq_ignore_ascii_case(&law_short_title) => {
                    format!("# {} — {} ({})", law_abbr, law_short_title, law_long_title)
                }
                (true, true, _) => format!("# {} — {}", law_abbr, law_short_title),
                (true, false, true) => format!("# {} — {}", law_abbr, law_long_title),
                (false, _, true) => format!("# {}", law_long_title),
                _ => format!("# {}", law_abbr),
            };
            out.push_str(&main_heading);
            out.push_str("\n\n");

            if let Some(ausf) = extract_tag_inner(meta_xml, "ausfertigung-datum") {
                let date_str = clean_inline_xml(ausf);
                if !date_str.is_empty() {
                    out.push_str(&format!("- **Ausfertigungsdatum:** {}\n", date_str));
                }
            }
            for stand in extract_all_tags_inner(meta_xml, "standkommentar") {
                let st_clean = clean_inline_xml(stand);
                if !st_clean.is_empty() {
                    out.push_str(&format!("- **Stand:** {}\n", st_clean));
                }
            }
            out.push('\n');
            header_emitted = true;
        }

        if let Some(glied_xml) = extract_tag_inner(meta_xml, "gliederungseinheit") {
            let g_bez = extract_tag_inner(glied_xml, "gliederungsbez")
                .map(clean_inline_xml)
                .unwrap_or_default();
            let g_tit = extract_tag_inner(glied_xml, "gliederungstitel")
                .map(clean_inline_xml)
                .unwrap_or_default();
            current_section = match (!g_bez.is_empty(), !g_tit.is_empty()) {
                (true, true) => format!("{} — {}", g_bez, g_tit),
                (true, false) => g_bez,
                (false, true) => g_tit,
                (false, false) => current_section,
            };
        }

        let enbez = extract_tag_inner(meta_xml, "enbez")
            .map(clean_inline_xml)
            .unwrap_or_default();
        let titel = extract_tag_inner(meta_xml, "titel")
            .map(clean_inline_xml)
            .unwrap_or_default();

        // Skip table-of-contents norms (`Inhaltsübersicht`) and repealed empty norms (`(weggefallen)`)
        if enbez.eq_ignore_ascii_case("Inhaltsübersicht")
            || titel.eq_ignore_ascii_case("(weggefallen)")
            || enbez.contains("(weggefallen)")
        {
            continue;
        }

        // Only extract active statutory text (`<text>...</text>`), skipping historical `<fussnoten>`
        let text_xml = extract_tag_inner(textdaten_xml, "text").unwrap_or("");
        if text_xml.contains("<TOC") && enbez.is_empty() {
            continue;
        }
        let content_xml = extract_tag_inner(text_xml, "Content").unwrap_or(text_xml);
        let body_md = convert_law_content_to_markdown(content_xml);
        let body_trimmed = body_md.trim();

        if enbez.is_empty() && titel.is_empty() {
            continue;
        }
        if body_trimmed.is_empty() || body_trimmed.eq_ignore_ascii_case("(weggefallen)") {
            continue;
        }

        let norm_heading = build_norm_heading(&law_abbr, &enbez, &titel, &current_section);
        out.push_str(&norm_heading);
        out.push_str("\n\n");
        out.push_str(body_trimmed);
        out.push_str("\n\n");
    }

    out.trim().to_string()
}

fn build_norm_heading(law_abbr: &str, enbez: &str, titel: &str, section: &str) -> String {
    let mut heading = String::from("## ");
    if !enbez.is_empty() {
        heading.push_str(enbez);
        if !law_abbr.is_empty() && !enbez.contains(law_abbr) {
            heading.push(' ');
            heading.push_str(law_abbr);
        }
    } else if !law_abbr.is_empty() {
        heading.push_str(law_abbr);
    }

    if !titel.is_empty() {
        if heading.len() > 3 {
            heading.push_str(" — ");
        }
        heading.push_str(titel);
    }

    if !section.is_empty() {
        heading.push_str(&format!(" [{}]", section));
    }
    heading
}

/// Converts `<Content>` inner XML (`<P>`, `<DL>`, `<DT>`, `<DD>`, `<LA>`, `<SUP class="Rec">`, `<table>`, `<row>`, `<entry>`, `<BR/>`)
/// into clean Markdown paragraphs, lists, sentence numbers, and tables.
fn convert_law_content_to_markdown(content_xml: &str) -> String {
    let mut s = content_xml.to_string();

    // Remove `<TOC>...</TOC>` blocks if embedded inside a content block
    while let Some(start) = s.find("<TOC") {
        if let Some(end_rel) = s[start..].find("</TOC>") {
            s.replace_range(start..start + end_rel + 6, "");
        } else {
            break;
        }
    }

    // Convert `<SUP class="Rec">1</SUP>` sentence numbers into explicit `[Satz 1] ` markers
    let mut processed = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(sup_pos) = rest.find("<SUP") {
        processed.push_str(&rest[..sup_pos]);
        let after_sup = &rest[sup_pos..];
        if let Some(tag_close) = after_sup.find('>') {
            let tag_header = &after_sup[..tag_close];
            let after_open = &after_sup[tag_close + 1..];
            if let Some(end_sup) = after_open.find("</SUP>") {
                let inner = after_open[..end_sup].trim();
                if tag_header.contains("Rec") && inner.chars().all(|c| c.is_ascii_digit()) && !inner.is_empty() {
                    processed.push_str(&format!(" [Satz {}] ", inner));
                } else if !inner.is_empty() {
                    processed.push_str(&format!("^{}", inner));
                }
                rest = &after_open[end_sup + 6..];
                continue;
            }
        }
        processed.push_str("<SUP");
        rest = &rest[sup_pos + 4..];
    }
    processed.push_str(rest);
    s = processed;

    // Convert XML tables (`<row><entry>...</entry></row>`) into Markdown tables
    s = convert_xml_tables_to_markdown(&s);

    // Tokenize remaining structural tags (`<P>`, `<BR>`, `<DL>`, `<DT>`, `<DD>`, `<LA>`)
    let mut out = String::with_capacity(s.len());
    let mut dl_depth: usize = 0;
    let mut pending_dt: Option<String> = None;
    let mut in_dt = false;
    let mut current_dt_buf = String::new();

    let mut idx = 0;
    let bytes = s.as_bytes();
    while idx < bytes.len() {
        if bytes[idx] == b'<' {
            if let Some(close_rel) = s[idx..].find('>') {
                let tag_full = &s[idx + 1..idx + close_rel];
                let tag_trim = tag_full.trim();
                let is_closing = tag_trim.starts_with('/');
                let is_self_closing = tag_trim.ends_with('/');
                let tag_name = tag_trim
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");

                match (tag_name, is_closing, is_self_closing) {
                    ("P", false, false) => {
                        if !out.ends_with("\n\n") && !out.is_empty() {
                            out.push_str("\n\n");
                        }
                    }
                    ("P", true, _) | ("P", _, true) => {
                        if !out.ends_with("\n\n") && !out.is_empty() {
                            out.push_str("\n\n");
                        }
                    }
                    ("BR" | "br", _, _) => {
                        out.push('\n');
                    }
                    ("DL", false, false) => {
                        dl_depth = dl_depth.saturating_add(1);
                        if !out.ends_with('\n') && !out.is_empty() {
                            out.push('\n');
                        }
                    }
                    ("DL", true, _) => {
                        dl_depth = dl_depth.saturating_sub(1);
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    ("DT", false, false) => {
                        in_dt = true;
                        current_dt_buf.clear();
                    }
                    ("DT", true, _) => {
                        in_dt = false;
                        let bullet = decode_xml_entities(current_dt_buf.trim());
                        pending_dt = Some(if bullet.is_empty() {
                            "-".to_string()
                        } else {
                            bullet
                        });
                    }
                    ("DD", false, false) => {
                        if !out.ends_with('\n') && !out.is_empty() {
                            out.push('\n');
                        }
                        let indent = "  ".repeat(dl_depth.saturating_sub(1));
                        let marker = pending_dt.take().unwrap_or_else(|| "-".to_string());
                        if marker == "-" || marker == "•" {
                            out.push_str(&format!("{}- ", indent));
                        } else {
                            out.push_str(&format!("{}- {} ", indent, marker));
                        }
                    }
                    ("DD", true, _) => {
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    ("LA", true, _) => {
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    _ => {}
                }

                idx += close_rel + 1;
                continue;
            }
        }

        let next_tag = s[idx..].find('<').map(|rel| idx + rel).unwrap_or(bytes.len());
        let text_slice = &s[idx..next_tag];
        if in_dt {
            current_dt_buf.push_str(text_slice);
        } else {
            let cleaned = normalize_inline_whitespace(&decode_xml_entities(text_slice));
            if !cleaned.is_empty() {
                if out.ends_with('\n') || out.ends_with(' ') || out.is_empty() {
                    out.push_str(cleaned.trim_start());
                } else {
                    out.push_str(&cleaned);
                }
            }
        }
        idx = next_tag;
    }

    collapse_blank_lines(&out)
}

fn convert_xml_tables_to_markdown(xml: &str) -> String {
    if !xml.contains("<table") && !xml.contains("<row") {
        return xml.to_string();
    }
    let mut result = String::with_capacity(xml.len());
    let mut rest = xml;

    while let Some(tbl_start) = rest.find("<table") {
        result.push_str(&rest[..tbl_start]);
        let after_start = &rest[tbl_start..];
        let Some(tbl_end_rel) = after_start.find("</table>") else {
            result.push_str(after_start);
            return result;
        };
        let tbl_xml = &after_start[..tbl_end_rel + 8];
        rest = &after_start[tbl_end_rel + 8..];

        let mut rows_md: Vec<Vec<String>> = Vec::new();
        for row_chunk in tbl_xml.split("<row").skip(1) {
            let Some(r_open) = row_chunk.find('>') else {
                continue;
            };
            let r_end = row_chunk.find("</row>").unwrap_or(row_chunk.len());
            let row_inner = &row_chunk[r_open + 1..r_end];

            let mut cells = Vec::new();
            for entry_chunk in row_inner.split("<entry").skip(1) {
                let Some(e_open) = entry_chunk.find('>') else {
                    continue;
                };
                let e_end = entry_chunk.find("</entry>").unwrap_or(entry_chunk.len());
                let cell_raw = &entry_chunk[e_open + 1..e_end];
                let cell_clean = clean_inline_xml(cell_raw).replace('|', "/");
                cells.push(cell_clean);
            }
            if cells.iter().any(|c| !c.is_empty()) {
                rows_md.push(cells);
            }
        }

        if !rows_md.is_empty() {
            let col_count = rows_md.iter().map(|r| r.len()).max().unwrap_or(1);
            result.push_str("\n\n");
            for (r_idx, r) in rows_md.iter().enumerate() {
                let mut padded = r.clone();
                while padded.len() < col_count {
                    padded.push(String::new());
                }
                result.push_str("| ");
                result.push_str(&padded.join(" | "));
                result.push_str(" |\n");
                if r_idx == 0 {
                    result.push_str("| ");
                    result.push_str(&vec!["---"; col_count].join(" | "));
                    result.push_str(" |\n");
                }
            }
            result.push('\n');
        }
    }
    result.push_str(rest);
    result
}

/// Fallback parser for generic XML documents (e.g. Akoma Ntoso / LegalDocML or custom XML files).
fn parse_eu_formex_xml(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 2);

    // 1. Extract top-level EU Act Title (before <PREAMBLE> or <ENACTING.TERMS>)
    let header_boundary = xml
        .find("<PREAMBLE")
        .or_else(|| xml.find("<ENACTING.TERMS"))
        .unwrap_or(xml.len().min(4000));
    let header_slice = &xml[..header_boundary];
    let act_title = extract_tag_inner(header_slice, "TITLE")
        .map(clean_formex_inline)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "EU-Rechtsakt (Formex XML)".to_string());

    out.push_str("# ");
    out.push_str(&act_title);
    out.push_str("\n\n");

    // 2. Parse <ENACTING.TERMS> (Divisions: KAPITEL / ABSCHNITT + Articles: <ARTICLE>)
    if let Some(enacting_start) = xml.find("<ENACTING.TERMS") {
        let enacting_end = xml
            .find("</ENACTING.TERMS>")
            .unwrap_or(xml.len());
        let enacting_xml = &xml[enacting_start..enacting_end];

        let mut current_chapter = String::new();
        let mut current_section = String::new();
        let mut cursor = 0;

        while cursor < enacting_xml.len() {
            let next_div = enacting_xml[cursor..]
                .find("<DIVISION>")
                .map(|r| cursor + r);
            let next_art = enacting_xml[cursor..]
                .find("<ARTICLE")
                .map(|r| cursor + r);

            match (next_div, next_art) {
                (Some(d_pos), Some(a_pos)) if d_pos < a_pos => {
                    let after_div = &enacting_xml[d_pos + 10..];
                    if let Some(title_xml) = extract_tag_inner(after_div, "TITLE") {
                        let ti = extract_tag_inner(title_xml, "TI")
                            .map(clean_formex_inline)
                            .unwrap_or_default();
                        let sti = extract_tag_inner(title_xml, "STI")
                            .map(clean_formex_inline)
                            .unwrap_or_default();
                        let combined = if !ti.is_empty() && !sti.is_empty() {
                            format!("{}: {}", ti, sti)
                        } else {
                            format!("{}{}", ti, sti)
                        };
                        if ti.to_uppercase().starts_with("KAPITEL")
                            || ti.to_uppercase().starts_with("TITEL")
                        {
                            current_chapter = combined;
                            current_section.clear();
                        } else if !combined.is_empty() {
                            current_section = combined;
                        }
                    }
                    cursor = d_pos + 10;
                }
                (_, Some(a_pos)) => {
                    let after_art = &enacting_xml[a_pos..];
                    let Some(open_end) = after_art.find('>') else {
                        break;
                    };
                    let Some(close_rel) = after_art.find("</ARTICLE>") else {
                        break;
                    };
                    let art_inner = &after_art[open_end + 1..close_rel];
                    let ti_art = extract_tag_inner(art_inner, "TI.ART")
                        .map(clean_formex_inline)
                        .unwrap_or_default();
                    let sti_art = extract_tag_inner(art_inner, "STI.ART")
                        .map(clean_formex_inline)
                        .unwrap_or_default();

                    let body_start = art_inner
                        .find("</STI.ART>")
                        .map(|p| p + 10)
                        .or_else(|| art_inner.find("</TI.ART>").map(|p| p + 9))
                        .unwrap_or(0);
                    let body_md = convert_formex_body_to_markdown(&art_inner[body_start..]);

                    if !ti_art.is_empty() && !body_md.is_empty() {
                        let ctx = match (!current_chapter.is_empty(), !current_section.is_empty())
                        {
                            (true, true) => format!("{} — {}", current_chapter, current_section),
                            (true, false) => current_chapter.clone(),
                            (false, true) => current_section.clone(),
                            (false, false) => String::new(),
                        };
                        out.push_str("## ");
                        out.push_str(&ti_art);
                        if !sti_art.is_empty() {
                            out.push_str(" — ");
                            out.push_str(&sti_art);
                        }
                        if !ctx.is_empty() {
                            out.push_str(&format!(" [{}]", ctx));
                        }
                        out.push_str("\n\n");
                        out.push_str(&body_md);
                        out.push_str("\n\n");
                    }
                    cursor = a_pos + close_rel + 10;
                }
                _ => break,
            }
        }
    }

    // 3. Parse any <ANNEX> blocks
    for annex_chunk in xml.split("<ANNEX").skip(1) {
        let Some(open_end) = annex_chunk.find('>') else {
            continue;
        };
        let close_rel = annex_chunk.find("</ANNEX>").unwrap_or(annex_chunk.len());
        let annex_inner = &annex_chunk[open_end + 1..close_rel];

        let title_xml = extract_tag_inner(annex_inner, "TITLE").unwrap_or("");
        let ti = extract_tag_inner(title_xml, "TI")
            .map(clean_formex_inline)
            .unwrap_or_else(|| "ANHANG".to_string());
        let sti = extract_tag_inner(title_xml, "STI")
            .map(clean_formex_inline)
            .unwrap_or_default();
        let contents_xml = extract_tag_inner(annex_inner, "CONTENTS").unwrap_or(annex_inner);
        let body_md = convert_formex_body_to_markdown(contents_xml);

        if !body_md.is_empty() {
            out.push_str("## ");
            out.push_str(&ti);
            if !sti.is_empty() {
                out.push_str(" — ");
                out.push_str(&sti);
            }
            out.push_str("\n\n");
            out.push_str(&body_md);
            out.push_str("\n\n");
        }
    }

    out.trim().to_string()
}

fn convert_formex_body_to_markdown(xml: &str) -> String {
    let mut s = xml.to_string();

    // Remove `<NOTE ...>...</NOTE>` footnotes and `<?PAGE ...?>` page-break processing instructions
    while let Some(start) = s.find("<NOTE") {
        if let Some(end_rel) = s[start..].find("</NOTE>") {
            s.replace_range(start..start + end_rel + 7, "");
        } else {
            break;
        }
    }
    while let Some(start) = s.find("<?") {
        if let Some(end_rel) = s[start..].find("?>") {
            s.replace_range(start..start + end_rel + 2, "");
        } else {
            break;
        }
    }

    let mut out = String::with_capacity(s.len());
    let mut list_depth: usize = 0;
    let mut in_no_p = false;
    let mut no_p_buf = String::new();
    let mut idx = 0;
    let bytes = s.as_bytes();

    while idx < bytes.len() {
        if bytes[idx] == b'<' {
            if let Some(close_rel) = s[idx..].find('>') {
                let tag_full = s[idx + 1..idx + close_rel].trim();
                let is_closing = tag_full.starts_with('/');
                let tag_name = tag_full
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");

                match (tag_name, is_closing) {
                    ("QUOT.START", false) => out.push('„'),
                    ("QUOT.END", false) => out.push('“'),
                    ("PARAG" | "ALINEA" | "P", false) => {
                        if list_depth == 0 && !out.is_empty() && !out.ends_with("\n\n") {
                            out.push_str("\n\n");
                        }
                    }
                    ("PARAG" | "ALINEA" | "P", true) => {
                        if list_depth == 0 && !out.is_empty() && !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    ("NO.PARAG", true) => {
                        out.push(' ');
                    }
                    ("LIST", false) => {
                        list_depth = list_depth.saturating_add(1);
                        if !out.ends_with('\n') && !out.is_empty() {
                            out.push('\n');
                        }
                    }
                    ("LIST", true) => {
                        list_depth = list_depth.saturating_sub(1);
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    ("NO.P", false) => {
                        in_no_p = true;
                        no_p_buf.clear();
                    }
                    ("NO.P", true) => {
                        in_no_p = false;
                        if !out.ends_with('\n') && !out.is_empty() {
                            out.push('\n');
                        }
                        let indent = "  ".repeat(list_depth.saturating_sub(1));
                        let marker = clean_formex_inline(&no_p_buf);
                        if marker.is_empty() {
                            out.push_str(&format!("{}- ", indent));
                        } else {
                            out.push_str(&format!("{}- {} ", indent, marker));
                        }
                    }
                    ("ITEM" | "NP" | "TXT" | "ROW", true) => {
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    ("CELL", true) => {
                        out.push_str(" | ");
                    }
                    _ => {}
                }
                idx += close_rel + 1;
                continue;
            }
        }

        let next_tag = s[idx..].find('<').map(|r| idx + r).unwrap_or(bytes.len());
        let slice = &s[idx..next_tag];
        if in_no_p {
            no_p_buf.push_str(slice);
        } else {
            let cleaned = normalize_inline_whitespace(&decode_xml_entities(slice));
            if !cleaned.is_empty() {
                if out.ends_with('\n') || out.ends_with(' ') || out.ends_with('„') || out.is_empty() {
                    out.push_str(cleaned.trim_start());
                } else {
                    out.push_str(&cleaned);
                }
            }
        }
        idx = next_tag;
    }

    collapse_blank_lines(&out)
}

fn clean_formex_inline(fragment: &str) -> String {
    let replaced = fragment
        .replace("<QUOT.START", "„<")
        .replace("<QUOT.END", "“<");
    clean_inline_xml(&replaced)
}

/// Fallback parser for generic XML documents (e.g. Akoma Ntoso / LegalDocML or custom XML files).
fn parse_generic_xml(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 2);
    let mut idx = 0;
    let bytes = xml.as_bytes();

    while idx < bytes.len() {
        if bytes[idx] == b'<' {
            if let Some(close_rel) = xml[idx..].find('>') {
                let tag_inner = xml[idx + 1..idx + close_rel].trim();
                let tag_name = tag_inner
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                if matches!(
                    tag_name.as_str(),
                    "p" | "para" | "paragraph" | "section" | "article" | "norm" | "item" | "row" | "tr" | "div" | "br" | "heading" | "title" | "titel" | "num" | "enbez"
                ) && !out.ends_with('\n')
                    && !out.is_empty()
                {
                    out.push('\n');
                }
                idx += close_rel + 1;
                continue;
            }
        }
        let next_tag = xml[idx..].find('<').map(|r| idx + r).unwrap_or(bytes.len());
        let txt = clean_inline_xml(&xml[idx..next_tag]);
        if !txt.is_empty() {
            if !out.is_empty() && !out.ends_with('\n') && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push_str(&txt);
        }
        idx = next_tag;
    }

    collapse_blank_lines(&out)
}

fn extract_tag_inner<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open_pat = format!("<{}", tag);
    let close_pat = format!("</{}>", tag);
    let mut search_from = 0;
    while let Some(rel_pos) = xml[search_from..].find(&open_pat) {
        let pos = search_from + rel_pos;
        let after_name = &xml[pos + open_pat.len()..];
        if after_name.starts_with('>') || after_name.starts_with(' ') || after_name.starts_with('/') {
            if let Some(open_end_rel) = xml[pos..].find('>') {
                let tag_header = &xml[pos..pos + open_end_rel + 1];
                if tag_header.ends_with("/>") {
                    return Some("");
                }
                let content_start = pos + open_end_rel + 1;
                if let Some(close_rel) = xml[content_start..].find(&close_pat) {
                    return Some(&xml[content_start..content_start + close_rel]);
                }
            }
        }
        search_from = pos + open_pat.len();
    }
    None
}

fn extract_all_tags_inner<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let mut results = Vec::new();
    let open_pat = format!("<{}", tag);
    let close_pat = format!("</{}>", tag);
    let mut search_from = 0;
    while let Some(rel_pos) = xml[search_from..].find(&open_pat) {
        let pos = search_from + rel_pos;
        let after_name = &xml[pos + open_pat.len()..];
        if after_name.starts_with('>') || after_name.starts_with(' ') {
            if let Some(open_end_rel) = xml[pos..].find('>') {
                let content_start = pos + open_end_rel + 1;
                if let Some(close_rel) = xml[content_start..].find(&close_pat) {
                    results.push(&xml[content_start..content_start + close_rel]);
                    search_from = content_start + close_rel + close_pat.len();
                    continue;
                }
            }
        }
        search_from = pos + open_pat.len();
    }
    results
}

fn clean_inline_xml(fragment: &str) -> String {
    let mut stripped = String::with_capacity(fragment.len());
    let mut in_tag = false;
    for ch in fragment.chars() {
        match ch {
            '<' => {
                in_tag = true;
                stripped.push(' ');
            }
            '>' => {
                in_tag = false;
            }
            _ if !in_tag => stripped.push(ch),
            _ => {}
        }
    }
    normalize_inline_whitespace(&decode_xml_entities(&stripped))
        .trim()
        .to_string()
}

fn decode_xml_entities(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&#8239;", " ")
        .replace('\u{a0}', " ")
        .replace('\u{202f}', " ")
        .replace("&amp;", "&")
}

fn normalize_inline_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

fn collapse_blank_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut blank_count = 0usize;
    for line in input.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 && !out.is_empty() {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_real_german_statutes() {
        for path in [
            "../Gesetze/1-dm-goldm_nzg.xml",
            "../Gesetze/GwG.xml",
            "../Gesetze/Estg.xml",
        ] {
            if let Ok(bytes) = std::fs::read(path) {
                let md = extract_structured_markdown_from_xml(&bytes);
                assert!(md.starts_with("# "), "Should start with main law title");
                assert!(md.contains("## § 1 "), "Should contain § 1 norm heading");
                assert!(!md.contains("<norm"), "Should not contain raw <norm tags");
                assert!(!md.contains("<metadaten>"), "Should not contain raw <metadaten> tags");
            }
        }

        if let Ok(amlr_bytes) = std::fs::read("../Gesetze/AMLR_EU_2024_1624.xml") {
            let md = extract_structured_markdown_from_xml(&amlr_bytes);
            assert!(md.starts_with("# Verordnung (EU) 2024/1624"), "Should start with EU AMLR title");
            assert!(md.contains("## Artikel 1 — Gegenstand"), "Should contain Artikel 1 heading");
            assert!(md.contains("## Artikel 11 — Compliance-Funktionen"), "Should contain Artikel 11 heading");
            assert!(md.contains("## ANHANG I — Indikative Liste der Risikovariablen"), "Should contain ANHANG I heading");
            assert!(!md.contains("<ARTICLE"), "Should not contain raw <ARTICLE tags");
        }
    }
}
