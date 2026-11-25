use crate::db::{extract_result_array, RetrievedChunk};
use serde_json::Value;

/// Generic SQL-result-to-Markdown-table formatter.
/// 1. Orders columns in the exact order they appear in the `SELECT` statement.
/// 2. For `SELECT *` queries, uses a generic naming heuristic (`*_id` first, `*name*`/`*title*` second).
pub fn format_sql_results_as_chunk(
    statements: &[String],
    raw_responses: &[Value],
    chunk_id: &str,
    doc_id: &str,
    title_prefix: &str,
) -> RetrievedChunk {
    let mut md = String::new();
    let mut total_rows = 0usize;

    md.push_str(&format!(
        "### {} Dynamische Text-to-SurrealQL-Abfrage (Deterministisches Datenbank-Ergebnis)\n\n",
        title_prefix
    ));

    for (idx, stmt) in statements.iter().enumerate() {
        let rows = extract_result_array(raw_responses.get(idx));
        total_rows += rows.len();

        md.push_str(&format!(
            "#### 🔎 Ausgeführte SurrealQL-Abfrage #{idx_num} ({row_cnt} Ergebnis-Zeilen):\n```sql\n{stmt}\n```\n\n",
            idx_num = idx + 1,
            row_cnt = rows.len(),
            stmt = stmt
        ));

        if rows.is_empty() {
            md.push_str("- *Keine Datensätze für diese Abfrage gefunden (`0 Zeilen`).*\n\n");
            continue;
        }

        if let Some(Value::Object(first_obj)) = rows.first() {
            let mut cols: Vec<String> = first_obj
                .keys()
                .filter(|k| *k != "id" && *k != "updated_at")
                .cloned()
                .collect();

            let stmt_lower = stmt.to_lowercase();
            let from_pos = stmt_lower.find(" from ").unwrap_or(stmt_lower.len());
            let select_head = &stmt_lower[..from_pos];

            let generic_column_rank = |c: &str| -> (usize, usize) {
                let c_low = c.to_lowercase();
                // 1. If the column is explicitly listed in the `SELECT ... FROM` clause, preserve its exact position
                if let Some(pos) = select_head.find(&c_low) {
                    return (0, pos);
                }
                // 2. Generic fallback for `SELECT *`: IDs first, names/titles second, types/status third
                if c_low == "id" || c_low.ends_with("_id") || c_low.ends_with("_code") {
                    (1, 0)
                } else if c_low.contains("name") || c_low.contains("title") {
                    (1, 1)
                } else if c_low.contains("type") || c_low.contains("category") || c_low.contains("status") {
                    (1, 2)
                } else {
                    (1, 10)
                }
            };
            cols.sort_by(|a, b| {
                generic_column_rank(a)
                    .cmp(&generic_column_rank(b))
                    .then_with(|| a.cmp(b))
            });

            md.push_str("| # | ");
            for col in &cols {
                md.push_str(&format!("{} | ", col));
            }
            md.push('\n');

            md.push_str("| :--- | ");
            for _ in &cols {
                md.push_str(":--- | ");
            }
            md.push('\n');

            let max_rows = 40usize;
            for (r_idx, row_val) in rows.iter().take(max_rows).enumerate() {
                md.push_str(&format!("| **{}** | ", r_idx + 1));
                for col in &cols {
                    let cell = format_json_cell(row_val.get(col));
                    md.push_str(&format!("{} | ", cell));
                }
                md.push('\n');
            }

            if rows.len() > max_rows {
                md.push_str(&format!(
                    "\n*(➕ Weitere {} Ergebnis-Zeilen in der Datenbank vorhanden; Tabelle zeigt die ersten {} Zeilen.)*\n",
                    rows.len() - max_rows,
                    max_rows
                ));
            }
            md.push('\n');
        } else {
            md.push_str(&format!(
                "**Ergebnis**: `{}`\n\n",
                serde_json::to_string(&rows).unwrap_or_default()
            ));
        }
    }

    RetrievedChunk {
        id: chunk_id.to_string(),
        doc_id: doc_id.to_string(),
        doc_title: format!(
            "{} Dynamische SurrealQL-Abfrage ({} SQL-Query(s), {} Ergebnis-Zeilen)",
            title_prefix,
            statements.len(),
            total_rows
        ),
        chunk_index: 0,
        content: md,
        rrf_score: 0.99,
        rerank_score: Some(0.995),
        entities: Vec::new(),
        relations: Vec::new(),
        related_entities: Vec::new(),
        relations_out: Vec::new(),
        relations_in: Vec::new(),
    }
}

fn format_json_cell(val: Option<&Value>) -> String {
    let Some(v) = val else {
        return "-".to_string();
    };
    match v {
        Value::Null => "-".to_string(),
        Value::Bool(b) => {
            if *b {
                "Ja".into()
            } else {
                "Nein".into()
            }
        }
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if n.is_i64() || n.is_u64() {
                    format!("{}", n)
                } else {
                    format!("{:.1}", f)
                }
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.replace('|', "/").replace('\n', " "),
        Value::Array(arr) => {
            // Generic safeguard for large arrays of nested objects (e.g. time-series or telemetry arrays)
            if arr.len() > 15 && arr.first().map(|x| x.is_object()).unwrap_or(false) {
                return format!("[{} Einträge]", arr.len());
            }
            let items: Vec<String> = arr
                .iter()
                .take(12)
                .map(|item| match item {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect();
            if arr.len() > 12 {
                format!("{} (+{} weitere)", items.join(", "), arr.len() - 12)
            } else {
                items.join(", ")
            }
        }
        Value::Object(_) => v.to_string().replace('|', "/"),
    }
}
