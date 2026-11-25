use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Generic, zero-hardcoding financial & invoice metadata extracted syntactically and mathematically
/// from unstructured document text and folder/category titles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialDocMetadata {
    pub doc_id: String,
    pub doc_title: String,
    pub year: Option<i64>,
    pub category_group: String,
    pub amount_gross: Option<f64>,
    pub currency: String,
    /// `"invoice"` | `"receipt"` | `"non_invoice"`
    pub doc_kind: String,
    pub extraction_method: String,
}

impl FinancialDocMetadata {
    /// Extracts structured financial metadata (`year`, `category_group`, `amount_gross`, `currency`, `doc_kind`)
    /// from any document's title and full text using purely generic syntactic and mathematical rules.
    pub fn extract(doc_id: &str, doc_title: &str, full_text: &str) -> Self {
        let (year, category_group) = parse_year_and_group(doc_title, full_text);
        let currency = detect_currency(full_text);
        let (amount_opt, method) = extract_gross_amount(full_text);
        let lower_title = doc_title.to_lowercase();
        let lower_text = full_text.to_lowercase();

        let is_non_invoice_doc = lower_title.contains("manual")
            || lower_title.contains("bedienungsanleitung")
            || lower_title.contains("licensecertificate")
            || lower_title.contains("garantiezertifikat")
            || lower_title.contains("kündigungsbestätigung")
            || lower_title.contains("kuendigungsbestaetigung")
            || (lower_text.contains("lieferschein")
                && !lower_text.contains("rechnungsbetrag")
                && !lower_text.contains("gesamtbetrag"));

        let (final_amount, doc_kind) = if is_non_invoice_doc && method == "fallback_max" {
            (None, "non_invoice".to_string())
        } else if let Some(amt) = amount_opt {
            // Distinguish between primary Invoice and supplemental Payment Receipt
            let base_filename = doc_title
                .rsplit(']')
                .next()
                .unwrap_or(doc_title)
                .trim()
                .to_lowercase();
            let kind = if base_filename.starts_with("receipt")
                || base_filename.contains("-receipt")
                || base_filename.contains("_receipt_")
            {
                "receipt".to_string()
            } else {
                "invoice".to_string()
            };
            (Some(amt), kind)
        } else {
            (None, "non_invoice".to_string())
        };

        Self {
            doc_id: doc_id.to_string(),
            doc_title: doc_title.to_string(),
            year,
            category_group,
            amount_gross: final_amount,
            currency,
            doc_kind,
            extraction_method: method,
        }
    }
}

/// Parses optional `[YYYY / Subfolder]` or `[Category]` prefix and detects a 4-digit year (`2000..=2099`).
fn parse_year_and_group(doc_title: &str, full_text: &str) -> (Option<i64>, String) {
    let trimmed = doc_title.trim();
    let mut detected_year: Option<i64> = None;
    let mut category_group = "Allgemein".to_string();

    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some(end_bracket) = rest.find(']') {
            let bracket_content = rest[..end_bracket].trim();
            let parts: Vec<&str> = bracket_content
                .split('/')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            for p in &parts {
                if detected_year.is_none() {
                    if let Some(yr) = find_four_digit_year(p) {
                        detected_year = Some(yr);
                    }
                }
            }

            if parts.len() >= 2 {
                category_group = parts[1..].join(" / ");
            } else if parts.len() == 1 {
                if find_four_digit_year(parts[0]).is_some() {
                    category_group = format!("{} (Einzelbelege)", parts[0]);
                } else {
                    category_group = parts[0].to_string();
                }
            }
        }
    }

    if detected_year.is_none() {
        detected_year = find_four_digit_year(doc_title).or_else(|| find_four_digit_year(full_text));
    }

    (detected_year, category_group)
}

fn find_four_digit_year(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    if bytes.len() < 4 {
        return None;
    }
    for i in 0..=(bytes.len() - 4) {
        if bytes[i] == b'2' && bytes[i + 1] == b'0' && bytes[i + 2].is_ascii_digit() && bytes[i + 3].is_ascii_digit()
        {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_digit();
            let after_ok = (i + 4 == bytes.len()) || !bytes[i + 4].is_ascii_digit();
            if before_ok && after_ok {
                if let Ok(yr_str) = std::str::from_utf8(&bytes[i..i + 4]) {
                    if let Ok(yr) = yr_str.parse::<i64>() {
                        if (2010..=2035).contains(&yr) {
                            return Some(yr);
                        }
                    }
                }
            }
        }
    }
    None
}

fn detect_currency(full_text: &str) -> String {
    let low = full_text.to_lowercase();
    let has_usd = low.contains("us$") || low.contains(" usd") || low.contains("usd*");
    let has_eur = low.contains(" eur") || full_text.contains('€') || low.contains("[euro]");
    if has_usd && !has_eur {
        "USD".to_string()
    } else if full_text.contains('$') && !has_eur {
        "USD".to_string()
    } else if low.contains(" chf") {
        "CHF".to_string()
    } else {
        "EUR".to_string()
    }
}

/// Extracts all candidate monetary floating-point values from text, filtering out dates (`DD.MM.YYYY`),
/// percentages (`19%`), and physical units (`GB`, `kB`, `L`, `km`, `Stck`, `ml`, `AW`).
pub fn parse_money_tokens(text: &str) -> Vec<f64> {
    let mut vals = Vec::new();
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for (idx, raw_tok) in tokens.iter().enumerate() {
            let next_tok = tokens.get(idx + 1).copied().unwrap_or("");
            if is_physical_unit(next_tok) {
                continue;
            }
            if raw_tok.contains('%') {
                continue;
            }
            // Strip surrounding currency symbols or punctuation
            let cleaned = raw_tok
                .trim_matches(|c: char| {
                    !c.is_ascii_digit() && c != '.' && c != ',' && c != '-'
                });
            if cleaned.is_empty() || cleaned.starts_with('-') {
                continue;
            }
            // Filter out dates like DD.MM.YYYY or DD.MM.YY (2 dots)
            if cleaned.chars().filter(|&c| c == '.').count() >= 2 {
                continue;
            }
            // Must contain a decimal separator (`.` or `,`) followed by 2 to 4 digits
            let sep_pos = cleaned.rfind([',', '.']);
            let Some(pos) = sep_pos else {
                continue;
            };
            let int_part = &cleaned[..pos];
            let dec_part = &cleaned[pos + 1..];
            if int_part.is_empty() || !(2..=4).contains(&dec_part.len()) {
                continue;
            }
            if !dec_part.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            // Filter out 2-digit dates like `01.05` when followed by `-` or year
            if raw_tok.contains('-') && int_part.len() == 2 && dec_part.len() == 2 {
                continue;
            }

            let normalized_int: String = int_part
                .chars()
                .filter(|&c| c.is_ascii_digit())
                .collect();
            if normalized_int.is_empty() {
                continue;
            }
            let num_str = format!("{}.{}", normalized_int, dec_part);
            if let Ok(v) = num_str.parse::<f64>() {
                let rounded = (v * 100.0).round() / 100.0;
                // Exclude fuel price-per-liter with 3 decimals (e.g. 1.489, 1.519, 1.609)
                if dec_part.len() == 3 && (1.0..=3.0).contains(&v) {
                    continue;
                }
                if (0.01..=100_000.0).contains(&rounded) {
                    vals.push(rounded);
                }
            }
        }
    }
    vals
}

fn is_physical_unit(tok: &str) -> bool {
    let clean = tok
        .trim_matches(|c: char| !c.is_ascii_alphabetic() && c != '%')
        .to_lowercase();
    matches!(
        clean.as_str(),
        "%" | "gb" | "kb" | "mb" | "tb" | "l" | "km" | "stck" | "stück" | "st" | "ml" | "aw" | "kg" | "mm" | "cm"
    )
}

/// Core 4-stage generic invoice total extractor:
/// 1. Explicit Payment Prose Match (`betrag von X €`, `$X.XX paid on`, `summe € X`, etc.)
/// 2. Mathematical VAT Triplet Validator ($N + T_1 (+ T_2) = G$ where $T_1 / N \approx 19\%, 7\%, \text{or } 16\%$)
/// 3. Explicit Summary Label Window (`gesamt brutto`, `bruttorechnungs-betrag`, `rechnungsbetrag`, `gesamtbetrag`, `total`, etc.)
/// 4. Integer Currency Pattern (`Gesamtsumme ... 477 €`) & Fallback
fn extract_gross_amount(full_text: &str) -> (Option<f64>, String) {
    let low = full_text.to_lowercase();

    // Stage 1: Explicit payment prose patterns
    for line in full_text.lines() {
        let ll = line.to_lowercase();
        if let Some(v) = extract_prose_amount(&ll) {
            return (Some(v), "prose".to_string());
        }
    }

    // Stage 2: Mathematical VAT Triplet Validator (Net + Tax = Gross, or Net + Tax1 + Tax2 = Gross)
    let vals = parse_money_tokens(full_text);
    let mut uniq: Vec<f64> = vals.clone();
    uniq.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    uniq.dedup_by(|a, b| (*a - *b).abs() < 0.005);

    let mut best_triplet_gross: Option<f64> = None;
    for &n in &uniq {
        if n <= 0.0 {
            continue;
        }
        for &t1 in &uniq {
            if t1 <= 0.0 || t1 >= n {
                continue;
            }
            let ratio = t1 / n;
            let is_valid_vat_rate = (0.185..=0.195).contains(&ratio)
                || (0.065..=0.075).contains(&ratio)
                || (0.155..=0.165).contains(&ratio);
            if !is_valid_vat_rate {
                continue;
            }
            for &g in &uniq {
                if (g - (n + t1)).abs() <= 0.02 {
                    if best_triplet_gross.map_or(true, |cur| g > cur) {
                        best_triplet_gross = Some(g);
                    }
                }
                for &t2 in &uniq {
                    if t2 > 0.0 && t2 < t1 && (g - (n + t1 + t2)).abs() <= 0.02 {
                        if best_triplet_gross.map_or(true, |cur| g > cur) {
                            best_triplet_gross = Some(g);
                        }
                    }
                }
            }
        }
    }
    if let Some(g) = best_triplet_gross {
        return (Some(g), "vat_triplet".to_string());
    }

    // Stage 3: Line-by-line Summary Label Window
    let non_empty_lines: Vec<&str> = full_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    let summary_labels = [
        "gesamt brutto",
        "bruttorechnungs-betrag",
        "total inkl. mwst",
        "rechnungsbetrag (brutto)",
        "endsumme",
        "endbetrag",
        "rechnungsbetrag",
        "gesamtbetrag",
        "gesamtsumme",
        "amount paid",
        "amount due",
        "invoice total",
        "paid:",
        "total",
    ];

    for lbl in summary_labels {
        for (i, &line) in non_empty_lines.iter().enumerate() {
            let ll = line.to_lowercase();
            if ll.contains(lbl)
                && !ll.contains("netto")
                && !ll.contains("ohne mwst")
                && !ll.contains("excluding")
                && !ll.contains("überweisen")
                && !ll.contains("beglichen")
                && !ll.contains("bezahlt")
                && !ll.contains("abgebucht")
            {
                let start = i.saturating_sub(1);
                let end = (i + 5).min(non_empty_lines.len());
                let window = non_empty_lines[start..end].join(" ");
                let w_vals = parse_money_tokens(&window);
                if let Some(&max_v) = w_vals
                    .iter()
                    .take(4)
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                {
                    return (Some(max_v), format!("label:{}", lbl));
                }
                // Check integer amount followed by € in window (e.g. "Gesamtsumme ... 477 €")
                if let Some(int_eur) = extract_integer_euro(&window) {
                    return (Some(int_eur), format!("label_int:{}", lbl));
                }
            }
        }
    }

    // Stage 4: Check integer Euro with explicit total word anywhere in text
    if low.contains("gesamtsumme") || low.contains("gesamtbetrag") || low.contains("rechnungsbetrag") {
        if let Some(int_eur) = extract_integer_euro(full_text) {
            if vals.is_empty() {
                return (Some(int_eur), "integer_euro".to_string());
            }
        }
    }

    if let Some(&max_v) = vals
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    {
        return (Some(max_v), "fallback_max".to_string());
    }

    (None, "none".to_string())
}

fn extract_prose_amount(lower_line: &str) -> Option<f64> {
    let triggers = [
        "betrag von ",
        "betrag in höhe von ",
        "rechnungsbetrag in höhe von ",
        "rechnungsbetrag von ",
        "gesamtbetrag von ",
        "umsatz von ",
        "reparaturkostenversicherung für ",
        "summe €",
        "summe eur",
    ];
    for trig in triggers {
        if let Some(pos) = lower_line.find(trig) {
            let after = &lower_line[pos + trig.len()..];
            let tokens = parse_money_tokens(after);
            if let Some(&first) = tokens.first() {
                return Some(first);
            }
        }
    }

    // Stripe / SaaS prose: "$25.00 paid on", "US$49.33 due", "€53.10 paid on"
    if lower_line.contains(" paid on ") || lower_line.contains(" due ") {
        if lower_line.contains('$') || lower_line.contains('€') || lower_line.contains("eur") || lower_line.contains("usd") {
            let tokens = parse_money_tokens(lower_line);
            if let Some(&first) = tokens.first() {
                return Some(first);
            }
        }
    }

    None
}

fn extract_integer_euro(text: &str) -> Option<f64> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for i in 0..tokens.len().saturating_sub(1) {
        let next = tokens[i + 1];
        if next == "€" || next.eq_ignore_ascii_case("eur") {
            let clean = tokens[i].trim_matches(|c: char| !c.is_ascii_digit());
            if let Ok(v) = clean.parse::<f64>() {
                if (1.0..=50_000.0).contains(&v) {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Checks if a user query asks for financial/document aggregation, totals, sums, or year/category overviews.
pub fn is_financial_aggregation_query(query: &str) -> bool {
    let low = query.to_lowercase();
    let agg_keywords = [
        "aufsummier",
        "summier",
        "summe",
        "wieviel geld",
        "wie viel geld",
        "wieviel hab",
        "wie viel hab",
        "ausgegeben",
        "ausgaben",
        "gesamtausgaben",
        "gesamtkosten",
        "gesamtbetrag",
        "alle rechnungen",
        "rechnungen aus",
        "rechnungen 20",
        "rechnungen von",
        "kosten 20",
        "umsatz",
        "total expenses",
        "sum up",
        "how much did i spend",
    ];
    agg_keywords.iter().any(|k| low.contains(k))
}

/// Extracts any 4-digit years (`2015..=2030`) mentioned in the user's query.
pub fn extract_query_years(query: &str) -> Vec<i64> {
    let mut years = Vec::new();
    for tok in query.split(|c: char| !c.is_ascii_digit()) {
        if tok.len() == 4 {
            if let Ok(yr) = tok.parse::<i64>() {
                if (2015..=2030).contains(&yr) && !years.contains(&yr) {
                    years.push(yr);
                }
            }
        }
    }
    years.sort();
    years
}

/// Synthesizes a deterministic Markdown aggregation report from a list of `FinancialDocMetadata` items.
/// Automatically detects duplicate Invoice/Receipt pairs in the same category and reports both:
/// 1. Deduplicated net expenses (excluding duplicate Receipts/files for the same invoice)
/// 2. Raw sum across all files
/// 3. Category-by-category subtotals and complete line-by-line invoice table
pub fn format_financial_aggregation_markdown(
    items: &[FinancialDocMetadata],
    filter_years: &[i64],
    filter_category: Option<&str>,
) -> String {
    let filtered: Vec<&FinancialDocMetadata> = items
        .iter()
        .filter(|m| {
            let yr_ok = if filter_years.is_empty() {
                true
            } else {
                m.year.map_or(false, |y| filter_years.contains(&y))
            };
            let cat_ok = match filter_category {
                Some(c) if !c.is_empty() => {
                    m.category_group.to_lowercase().contains(&c.to_lowercase())
                        || m.doc_title.to_lowercase().contains(&c.to_lowercase())
                }
                _ => true,
            };
            yr_ok && cat_ok
        })
        .collect();

    if filtered.is_empty() {
        return String::new();
    }

    // Group by category_group
    let mut by_cat: BTreeMap<String, Vec<&FinancialDocMetadata>> = BTreeMap::new();
    for item in &filtered {
        by_cat
            .entry(item.category_group.clone())
            .or_default()
            .push(*item);
    }

    let mut raw_totals_by_cur: BTreeMap<String, f64> = BTreeMap::new();
    let mut dedup_totals_by_cur: BTreeMap<String, f64> = BTreeMap::new();
    let mut total_invoice_count = 0usize;
    let mut duplicate_receipt_count = 0usize;

    struct ProcessedLine<'a> {
        meta: &'a FinancialDocMetadata,
        is_dup_receipt: bool,
    }

    let mut cat_sections: Vec<(String, BTreeMap<String, f64>, Vec<ProcessedLine>)> = Vec::new();

    for (cat_name, group_items) in by_cat {
        // Check if this category contains both "invoice" and "receipt" documents for the same amounts
        let invoice_amounts: Vec<(f64, &str)> = group_items
            .iter()
            .filter(|x| x.doc_kind == "invoice")
            .filter_map(|x| x.amount_gross.map(|a| (a, x.currency.as_str())))
            .collect();

        let mut remaining_invoice_matches = invoice_amounts.clone();
        let mut cat_dedup_totals: BTreeMap<String, f64> = BTreeMap::new();
        let mut lines = Vec::with_capacity(group_items.len());

        for item in group_items {
            if let Some(amt) = item.amount_gross {
                *raw_totals_by_cur.entry(item.currency.clone()).or_insert(0.0) += amt;

                let mut is_dup = false;
                if item.doc_kind == "receipt" {
                    if let Some(pos) = remaining_invoice_matches
                        .iter()
                        .position(|(ia, ic)| (*ia - amt).abs() < 0.01 && *ic == item.currency)
                    {
                        remaining_invoice_matches.remove(pos);
                        is_dup = true;
                        duplicate_receipt_count += 1;
                    }
                }

                if !is_dup {
                    *dedup_totals_by_cur
                        .entry(item.currency.clone())
                        .or_insert(0.0) += amt;
                    *cat_dedup_totals
                        .entry(item.currency.clone())
                        .or_insert(0.0) += amt;
                    total_invoice_count += 1;
                }
                lines.push(ProcessedLine {
                    meta: item,
                    is_dup_receipt: is_dup,
                });
            } else {
                lines.push(ProcessedLine {
                    meta: item,
                    is_dup_receipt: false,
                });
            }
        }

        cat_sections.push((cat_name, cat_dedup_totals, lines));
    }

    let scope_label = if !filter_years.is_empty() {
        let yrs: Vec<String> = filter_years.iter().map(|y| y.to_string()).collect();
        format!("Jahr {}", yrs.join(", "))
    } else {
        "Alle indexierten Jahre".to_string()
    };

    let mut out = String::with_capacity(4096);
    out.push_str(&format!(
        "### 📊 Exakte SurrealDB Rechnungs- & Ausgaben-Aggregation ({})\n",
        scope_label
    ));
    out.push_str(&format!(
        "- **Analysierte Dokumente im Zeitraum**: {} Dateien ({} eindeutige Rechnungen",
        filtered.len(),
        total_invoice_count
    ));
    if duplicate_receipt_count > 0 {
        out.push_str(&format!(
            ", {} zusätzliche Zahlungsquittungen/Receipts zu bereits vorhandenen Invoices",
            duplicate_receipt_count
        ));
    }
    out.push_str(")\n");

    // 1. Primary Totals
    out.push_str("- **Bereinigte Gesamtausgaben (ohne doppelte Receipt-Quittungen)**:\n");
    for (cur, sum_val) in &dedup_totals_by_cur {
        out.push_str(&format!("  - **{}**: **{:.2} {}**\n", cur, sum_val, cur));
    }
    if duplicate_receipt_count > 0 {
        out.push_str("- **Roh-Summe über alle Dateien (inkl. doppelter Receipts)**:\n");
        for (cur, sum_val) in &raw_totals_by_cur {
            out.push_str(&format!("  - **{}**: {:.2} {}\n", cur, sum_val, cur));
        }
    }

    // 1b. Per-Year Breakdown if multiple years are covered
    let mut by_year_totals: BTreeMap<i64, (usize, BTreeMap<String, f64>)> = BTreeMap::new();
    for (_, _, lines) in &cat_sections {
        for pl in lines {
            if !pl.is_dup_receipt {
                if let Some(amt) = pl.meta.amount_gross {
                    let yr = pl.meta.year.unwrap_or(0);
                    let entry = by_year_totals
                        .entry(yr)
                        .or_insert_with(|| (0, BTreeMap::new()));
                    entry.0 += 1;
                    *entry.1.entry(pl.meta.currency.clone()).or_insert(0.0) += amt;
                }
            }
        }
    }
    if by_year_totals.len() > 1 {
        out.push_str("\n#### 📅 Jahres-Übersicht (Bereinigte Gesamtausgaben pro Jahr):\n");
        for (yr, (cnt, cur_map)) in &by_year_totals {
            let yr_lbl = if *yr > 0 {
                yr.to_string()
            } else {
                "Ohne Jahr".to_string()
            };
            let sums: Vec<String> = cur_map
                .iter()
                .map(|(c, v)| format!("**{:.2} {}**", v, c))
                .collect();
            out.push_str(&format!(
                "- **{}** ({} Rechnungen): {}\n",
                yr_lbl,
                cnt,
                sums.join(" + ")
            ));
        }
    }

    // 2. Subtotals by Category / Folder
    out.push_str("\n#### 1. Zwischensummen nach Kategorie / Anbieter:\n");
    for (cat_name, cat_totals, lines) in &cat_sections {
        let sums_str: Vec<String> = cat_totals
            .iter()
            .map(|(cur, v)| format!("{:.2} {}", v, cur))
            .collect();
        let active_cnt = lines
            .iter()
            .filter(|l| !l.is_dup_receipt && l.meta.amount_gross.is_some())
            .count();
        let dup_cnt = lines.iter().filter(|l| l.is_dup_receipt).count();
        let dup_note = if dup_cnt > 0 {
            format!(" (+{} Receipt-Belege)", dup_cnt)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "- **{}** ({} Rechnungen{}): **{}**\n",
            cat_name,
            active_cnt,
            dup_note,
            if sums_str.is_empty() {
                "0.00 EUR".to_string()
            } else {
                sums_str.join(" + ")
            }
        ));
    }

    // 3. Complete Line-by-Line List (shown in full when <= 120 docs in scope)
    if filtered.len() <= 120 {
        out.push_str("\n#### 2. Vollständige Einzelaufstellung aller Dokumente im Zeitraum:\n");
        for (_, _, lines) in &cat_sections {
            for pl in lines {
                match pl.meta.amount_gross {
                    Some(amt) if pl.is_dup_receipt => {
                        out.push_str(&format!(
                            "- `{}`: {:.2} {} *(Receipt/Zahlungsquittung zu vorhandener Invoice — in bereinigter Summe nicht doppelt gezählt)*\n",
                            pl.meta.doc_title, amt, pl.meta.currency
                        ));
                    }
                    Some(amt) => {
                        out.push_str(&format!(
                            "- `{}`: **{:.2} {}**\n",
                            pl.meta.doc_title, amt, pl.meta.currency
                        ));
                    }
                    None => {
                        out.push_str(&format!(
                            "- `{}`: *Kein Rechnungsbetrag (Lieferschein/Zertifikat/Handbuch)*\n",
                            pl.meta.doc_title
                        ));
                    }
                }
            }
        }
    }

    out
}
