use crate::ingestion::{xml, InferenceClient};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

/// Detects whether a file or text payload is a structured legal statute (`gii-norm.dtd`, EU `Formex`,
/// or a text structured into `§` / `Artikel` norms).
pub fn is_statutory_xml_or_text(filename: &str, bytes: &[u8]) -> bool {
    let lower_name = filename.to_lowercase();
    let sample_len = bytes.len().min(4096);
    let head = String::from_utf8_lossy(&bytes[..sample_len]);

    if (head.contains("<dokumente") && head.contains("<norm"))
        || ((head.contains("<ACT") || head.contains("<ANNEX"))
            && (head.contains("<ENACTING.TERMS")
                || head.contains("<ARTICLE")
                || head.contains("<TI.ART>")))
    {
        return true;
    }

    if lower_name.ends_with(".xml") && (head.contains("<jurabk>") || head.contains("<TI.ART>")) {
        return true;
    }

    head.starts_with("# ")
        && (head.contains("\n## §") || head.contains("\n## Artikel ") || head.contains("\n## Art. "))
}

/// Extracts structured Markdown from a statutory XML (`gii-norm.dtd` / EU `Formex`) or legal PDF/text file.
pub fn extract_legal_document_text(filename: &str, bytes: &[u8]) -> Result<String> {
    let lower = filename.to_lowercase();
    if lower.ends_with(".xml")
        || bytes.trim_ascii_start().starts_with(b"<?xml")
        || bytes.trim_ascii_start().starts_with(b"<dokumente")
        || bytes.trim_ascii_start().starts_with(b"<ACT")
    {
        Ok(xml::extract_structured_markdown_from_xml(bytes))
    } else {
        InferenceClient::extract_text_from_bytes(filename, bytes)
    }
}

/// 100% generic, schema- and syntax-driven Knowledge Graph extractor for legal statutes.
///
/// Contains ZERO hardcoded statute names, authority names, or domain vocabulary:
/// 1. Derives the primary statute entity dynamically from the document's `# <Header>` line.
/// 2. Extracts cross-referenced laws syntactically from grammatical citation patterns in the text:
///    - `des <Wort>gesetzes` / `des <Wort>gesetzbuchs` / `der <Wort>verordnung` / `der <Wort>ordnung`
///    - `Verordnung (EU) <YYYY>/<NNNN>` and `Richtlinie (EU) <YYYY>/<NNNN>`
/// 3. Extracts public authorities and registers syntactically by institutional noun prefixes:
///    - `Bundesanstalt für ...`, `Bundesamt für ...`, `Bundesministerium ...`, `Zentralstelle für ...`,
///      `Europäische ...behörde`, `...register`
/// 4. Extracts key regulated subjects dynamically from the statute's own `## § ... — <Titel>`
///    and `## Artikel ... — <Titel>` section headings.
pub fn extract_syntactic_legal_graph(
    fallback_title: &str,
    markdown_text: &str,
) -> (
    Vec<(String, String, Option<String>)>,
    Vec<(String, String, String)>,
) {
    let mut entities: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    let mut relations: BTreeSet<(String, String, String)> = BTreeSet::new();

    // 1. Primary Statute identity from `# <Abbr> — <Title>` (or fallback_title)
    let first_heading = markdown_text
        .lines()
        .find(|l| l.trim_start().starts_with("# "))
        .map(|l| l.trim().trim_start_matches('#').trim())
        .unwrap_or(fallback_title);

    let (primary_id, primary_desc) = parse_primary_statute_header(first_heading, fallback_title);
    entities.insert(
        primary_id.clone(),
        ("Statute".to_string(), Some(primary_desc)),
    );

    // 2. Syntactic extraction of referenced German statutes (`des ...gesetzes`, `der ...verordnung`, `der ...ordnung`)
    for ref_statute in extract_german_statute_citations(markdown_text) {
        if !ref_statute.eq_ignore_ascii_case(&primary_id) {
            entities
                .entry(ref_statute.clone())
                .or_insert(("Statute".to_string(), None));
            relations.insert((
                primary_id.clone(),
                "references".to_string(),
                ref_statute,
            ));
        }
    }

    // 3. Syntactic extraction of referenced EU acts (`Verordnung (EU) YYYY/NNNN`, `Richtlinie (EU) YYYY/NNNN`)
    for eu_act in extract_eu_act_citations(markdown_text) {
        if !primary_id.contains(&eu_act) {
            entities
                .entry(eu_act.clone())
                .or_insert(("EU_Act".to_string(), None));
            relations.insert((primary_id.clone(), "references".to_string(), eu_act));
        }
    }

    // 4. Syntactic extraction of public authorities & official registers (`Bundesanstalt für ...`, `Zentralstelle für ...`, etc.)
    for authority in extract_institutional_authorities(markdown_text) {
        entities
            .entry(authority.clone())
            .or_insert(("Authority".to_string(), None));
        relations.insert((
            primary_id.clone(),
            "involves_authority".to_string(),
            authority,
        ));
    }

    // 5. Dynamic extraction of core regulated subjects from the statute's own `## <Norm> — <Title>` headings
    let mut norm_subject_count = 0usize;
    for line in markdown_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if let Some(dash_pos) = rest.find(" — ") {
                let norm_ref = rest[..dash_pos].trim();
                let after_dash = &rest[dash_pos + " — ".len()..];
                let norm_title = after_dash
                    .split(" [")
                    .next()
                    .unwrap_or(after_dash)
                    .trim();

                if is_meaningful_norm_subject(norm_title) && norm_subject_count < 250 {
                    let label = format!("{} ({})", norm_title, norm_ref);
                    entities
                        .entry(label.clone())
                        .or_insert(("LegalNorm".to_string(), Some(norm_ref.to_string())));
                    relations.insert((primary_id.clone(), "defines_norm".to_string(), label));
                    norm_subject_count += 1;
                }
            }
        }
    }

    let ent_vec = entities
        .into_iter()
        .map(|(name, (etype, desc))| (name, etype, desc))
        .collect();
    let rel_vec = relations.into_iter().collect();
    (ent_vec, rel_vec)
}

fn parse_primary_statute_header(first_heading: &str, fallback_title: &str) -> (String, String) {
    if let Some((abbr, rest)) = first_heading.split_once(" — ") {
        let clean_abbr = abbr.trim();
        if !clean_abbr.is_empty() && clean_abbr.len() <= 40 {
            return (clean_abbr.to_string(), rest.trim().to_string());
        }
    }
    // Check if the heading begins with `Verordnung (EU) YYYY/NNNN` or `Richtlinie (EU) YYYY/NNNN`
    let tokens: Vec<&str> = first_heading.split_whitespace().collect();
    if tokens.len() >= 3
        && (tokens[0].eq_ignore_ascii_case("Verordnung")
            || tokens[0].eq_ignore_ascii_case("Richtlinie"))
        && tokens[1].starts_with('(')
    {
        let short_id = format!("{} {} {}", tokens[0], tokens[1], tokens[2]);
        return (short_id, first_heading.to_string());
    }

    let clean_fallback = fallback_title
        .rsplit(']')
        .next()
        .unwrap_or(fallback_title)
        .trim()
        .trim_end_matches(".xml")
        .trim_end_matches(".XML")
        .to_string();
    (
        if clean_fallback.is_empty() {
            "Statute".to_string()
        } else {
            clean_fallback
        },
        first_heading.to_string(),
    )
}

/// Generically finds German compound statute nouns (`...gesetz`, `...gesetzbuch`, `...verordnung`, `...ordnung`)
/// following genitive citation markers (`des `, `der `, `dem `) and normalizes them to their nominative base form.
fn extract_german_statute_citations(text: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    let words: Vec<&str> = text.split_whitespace().collect();

    for window in words.windows(2) {
        let article = window[0];
        if !matches!(article, "des" | "der" | "dem" | "zum" | "zur") {
            continue;
        }
        let raw_word = window[1].trim_matches(|c: char| !c.is_alphabetic() && c != '-');
        if raw_word.len() < 9 {
            continue;
        }
        let first_char = raw_word.chars().next().unwrap_or('a');
        if !first_char.is_uppercase() {
            continue;
        }

        if let Some(norm) = normalize_german_statute_noun(raw_word) {
            if found.len() < 18 {
                found.insert(norm);
            }
        }
    }

    found.into_iter().collect()
}

fn normalize_german_statute_noun(word: &str) -> Option<String> {
    let lower = word.to_lowercase();
    // Skip generic non-title words like "Gesetzes", "Bundesgesetzes", "Verordnung", "Anordnung"
    if matches!(
        lower.as_str(),
        "gesetzes"
            | "gesetz"
            | "bundesgesetzes"
            | "landesgesetzes"
            | "änderungsgesetzes"
            | "verordnung"
            | "rechtsverordnung"
            | "ordnung"
            | "anordnung"
            | "zuordnung"
            | "einordnung"
            | "rangordnung"
            | "geschäftsordnung"
            | "tagesordnung"
            | "satzung"
    ) {
        return None;
    }

    if lower.ends_with("gesetzbuches") {
        return Some(word[..word.len() - 2].to_string());
    }
    if lower.ends_with("gesetzbuchs") {
        return Some(word[..word.len() - 1].to_string());
    }
    if lower.ends_with("gesetzbuch") {
        return Some(word.to_string());
    }
    if lower.ends_with("gesetzes") {
        return Some(word[..word.len() - 2].to_string());
    }
    if lower.ends_with("gesetz") {
        return Some(word.to_string());
    }
    if lower.ends_with("verordnung") || lower.ends_with("ordnung") {
        return Some(word.to_string());
    }
    None
}

/// Generically extracts EU regulation and directive citations (`Verordnung (EU) YYYY/NNNN`, `Richtlinie (EU) YYYY/NNNN`).
fn extract_eu_act_citations(text: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();

    for i in 0..tokens.len().saturating_sub(2) {
        let t0 = tokens[i].trim_matches(|c: char| !c.is_alphabetic());
        if !matches!(t0, "Verordnung" | "Richtlinie") {
            continue;
        }
        let t1 = tokens[i + 1];
        if !(t1.starts_with("(EU") || t1.starts_with("(EG")) {
            continue;
        }
        // Handle either `Verordnung (EU) 2024/1624` or `Verordnung (EU) Nr. 910/2014`
        let candidate_num = if tokens[i + 2] == "Nr." && i + 3 < tokens.len() {
            tokens[i + 3]
        } else {
            tokens[i + 2]
        };
        let clean_num = candidate_num
            .trim_matches(|c: char| !c.is_ascii_digit() && c != '/');
        if clean_num.contains('/')
            && clean_num.len() >= 6
            && clean_num.chars().all(|c| c.is_ascii_digit() || c == '/')
        {
            if found.len() < 12 {
                found.insert(format!("{} {} {}", t0, t1, clean_num));
            }
        }
    }

    found.into_iter().collect()
}

/// Generically extracts institutional authorities (`Bundesanstalt für ...`, `Bundesamt für ...`, `Zentralstelle für ...`, `Bundesministerium ...`)
/// by scanning capitalized multi-word noun phrases after institutional prefixes.
fn extract_institutional_authorities(text: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    let prefixes = [
        "Bundesanstalt für ",
        "Bundesamt für ",
        "Zentralstelle für ",
        "Bundesministerium der ",
        "Bundesministerium für ",
    ];

    for prefix in prefixes {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(prefix) {
            let start = search_from + rel;
            let after = &text[start + prefix.len()..];
            let mut phrase = String::from(prefix);
            let mut word_count = 0usize;

            for raw_tok in after.split_whitespace() {
                let clean_tok = raw_tok.trim_matches(|c: char| {
                    !c.is_alphabetic() && c != '-'
                });
                if clean_tok.is_empty() {
                    break;
                }
                let first = clean_tok.chars().next().unwrap_or('a');
                let is_connector = matches!(clean_tok, "und" | "für" | "der" | "des");
                if first.is_uppercase() || (is_connector && word_count > 0 && word_count < 5) {
                    if word_count > 0 {
                        phrase.push(' ');
                    }
                    phrase.push_str(clean_tok);
                    word_count += 1;
                    if raw_tok.ends_with(',')
                        || raw_tok.ends_with('.')
                        || raw_tok.ends_with(';')
                        || raw_tok.ends_with(')')
                        || word_count >= 5
                    {
                        break;
                    }
                } else {
                    break;
                }
            }

            let trimmed_phrase = phrase
                .trim_end_matches(" und")
                .trim_end_matches(" für")
                .trim_end_matches(" der")
                .trim_end_matches(" des")
                .trim();
            if trimmed_phrase.len() > prefix.len() + 4 && found.len() < 10 {
                found.insert(trimmed_phrase.to_string());
            }
            search_from = start + prefix.len();
        }
    }

    found.into_iter().collect()
}

fn is_meaningful_norm_subject(title: &str) -> bool {
    if title.len() < 5 || title.len() > 75 {
        return false;
    }
    let lower = title.to_lowercase();
    !matches!(
        lower.as_str(),
        "gegenstand"
            | "geltungsbereich"
            | "anwendungsbereich"
            | "inkrafttreten"
            | "außerkrafttreten"
            | "übergangsvorschrift"
            | "übergangsvorschriften"
            | "schlussvorschriften"
            | "(weggefallen)"
    )
}
