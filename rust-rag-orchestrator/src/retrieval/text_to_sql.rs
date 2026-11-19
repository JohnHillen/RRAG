use crate::{
    db::{external::ExternalDbClient, RetrievedChunk, SurrealClient},
    llm::ChatMessage,
    sql_formatter::format_sql_results_as_chunk,
    AppState,
};
use serde_json::Value;
use tracing::{info, warn};

/// Executes Dynamic Text-to-SurrealQL against the primary `state.db` client.
pub async fn execute_dynamic_text_to_sql(
    state: &AppState,
    user_query: &str,
    hop1_anchor: Option<&RetrievedChunk>,
    schema_prompt: &str,
    chunk_id: &str,
    doc_id: &str,
    title_prefix: &str,
) -> Option<RetrievedChunk> {
    execute_dynamic_text_to_sql_on_client(
        state,
        &state.db,
        user_query,
        hop1_anchor,
        schema_prompt,
        chunk_id,
        doc_id,
        title_prefix,
    )
    .await
}

/// Checks if SurrealDB returned `"status": "ERR"` inside any statement response.
fn check_surreal_statement_errors(responses: &[Value]) -> Option<String> {
    let mut errs = Vec::new();
    for (idx, item) in responses.iter().enumerate() {
        if item.get("status").and_then(|s| s.as_str()) == Some("ERR") {
            let msg = item
                .get("result")
                .or_else(|| item.get("detail"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown SurrealQL statement error");
            errs.push(format!("Statement #{}: {}", idx + 1, msg));
        }
    }
    if errs.is_empty() {
        None
    } else {
        Some(errs.join(" | "))
    }
}

/// Dynamically translates a natural-language question (plus optional Hop-1 document context)
/// into 1–3 read-only SurrealQL `SELECT` queries using the configured LLM and executes them
/// deterministically against the given `SurrealClient` (either primary `state.db` or an attached `external_db`).
pub async fn execute_dynamic_text_to_sql_on_client(
    state: &AppState,
    db_client: &SurrealClient,
    user_query: &str,
    hop1_anchor: Option<&RetrievedChunk>,
    schema_prompt: &str,
    chunk_id: &str,
    doc_id: &str,
    title_prefix: &str,
) -> Option<RetrievedChunk> {
    let anchor_context = match hop1_anchor {
        Some(a) => {
            let excerpt: String = a.content.chars().take(1200).collect();
            format!(
                "\n\nADDITIONAL CONTEXT FROM RETRIEVED DOCUMENT ('{}'):\n{}",
                a.doc_title, excerpt
            )
        }
        None => String::new(),
    };

    let system_prompt = format!(
        r#"You are an expert SurrealQL (SurrealDB 4.0) query generator.
Translate the user's question (and any requirements from the retrieved document context) into 1 to 3 read-only SurrealQL `SELECT` statements using ONLY the tables and exact column names listed in DATABASE SCHEMA below.
If the user's question is completely unrelated to all tables in DATABASE SCHEMA below, output ONLY the word `NONE`.

CRITICAL SURREALQL 4.0 SYNTAX RULES:
1. Output ONLY raw SurrealQL `SELECT` statements separated by semicolons `;` (or `NONE`). Do NOT output markdown explanations or prose.
2. NEVER invent column names that are not listed in the table's schema below!
3. NEVER use `HAVING`, `CASE WHEN`, `IF(...)`, `IIF(...)`, `COALESCE(...)`, or subqueries! SurrealQL does NOT support `HAVING` or SQL `CASE WHEN`. Simply `SELECT` the columns and aggregate numbers (e.g. `math::sum(revenue_eur) AS total_revenue_eur`); threshold comparisons are done after the query runs.
4. NEVER use `COUNT(*)`, `SUM()`, `AVG()`, `MIN()`, `MAX()`. In SurrealQL you MUST use:
   - `count()` instead of `COUNT(*)`
   - `math::sum(field)` instead of `SUM(field)`
   - `math::mean(field)` instead of `AVG(field)`
   - `math::min(field)` instead of `MIN(field)`
   - `math::max(field)` instead of `MAX(field)`
5. GROUP BY / GROUP ALL rules in SurrealQL:
   - Every non-aggregated field in `SELECT` MUST appear in `GROUP BY` (e.g. `SELECT category, math::sum(amount) AS total_amount, count() AS cnt FROM table_name GROUP BY category ORDER BY total_amount DESC;`).
   - If ALL fields in `SELECT` are aggregate functions (`math::sum(...)`, `count()`, etc.) without any grouping columns, you MUST add `GROUP ALL` (e.g. `SELECT math::sum(amount) AS total_amount FROM table_name GROUP ALL;`).
   - NEVER use positional `GROUP BY 1` or `ORDER BY 1`; always use explicit field/alias names.
6. Array & value filtering in SurrealQL:
   - Array contains: `'value' IN array_field`
   - Field in list: `field_name IN ['val1', 'val2']`
7. Always add a sensible `LIMIT` (e.g. `LIMIT 30`) on ungrouped queries.

DATABASE SCHEMA:
{schema_prompt}"#
    );

    let user_msg = format!("USER QUESTION: {}{}", user_query, anchor_context);

    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: system_prompt.clone(),
        },
        ChatMessage {
            role: "user".into(),
            content: user_msg.clone(),
        },
    ];

    let raw_llm_out = match state
        .llm
        .chat_completion(&messages, None, Some(0.0), Some(4096), Some(false))
        .await
    {
        Ok(out) => out,
        Err(e) => {
            warn!("Text-to-SurrealQL LLM call failed: {}", e);
            return None;
        }
    };

    if raw_llm_out.trim().eq_ignore_ascii_case("NONE") {
        return None;
    }

    let statements = extract_and_sanitize_surrealql(&raw_llm_out);
    if statements.is_empty() {
        return None;
    }

    let combined_sql = statements.join("\n");
    info!(
        "Executing dynamic Text-to-SurrealQL ({}): {}",
        title_prefix, combined_sql
    );

    let mut exec_result = db_client.execute_sql(&combined_sql).await;
    let mut stmt_err = exec_result
        .as_ref()
        .ok()
        .and_then(|res| check_surreal_statement_errors(res));

    let mut final_statements = statements;
    if exec_result.is_err() || stmt_err.is_some() {
        let err_desc = match (&exec_result, &stmt_err) {
            (Err(e), _) => e.to_string(),
            (_, Some(se)) => se.clone(),
            _ => String::new(),
        };
        warn!(
            "Initial Text-to-SurrealQL query reported error ({}), attempting 1-step repair...",
            err_desc
        );
        let repair_messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".into(),
                content: format!(
                    "{}\n\nYour previous query failed with this SurrealDB error:\n{}\nFailed query:\n{}\n\nPlease output ONLY the corrected SurrealQL SELECT statement(s).",
                    user_msg, err_desc, combined_sql
                ),
            },
        ];
        if let Ok(repaired_out) = state
            .llm
            .chat_completion(&repair_messages, None, Some(0.0), Some(4096), Some(false))
            .await
        {
            let repaired_stmts = extract_and_sanitize_surrealql(&repaired_out);
            if !repaired_stmts.is_empty() {
                let repaired_sql = repaired_stmts.join("\n");
                info!("Executing repaired Text-to-SurrealQL: {}", repaired_sql);
                exec_result = db_client.execute_sql(&repaired_sql).await;
                stmt_err = exec_result
                    .as_ref()
                    .ok()
                    .and_then(|res| check_surreal_statement_errors(res));
                if stmt_err.is_some() {
                    warn!("Repaired query still had error: {:?}", stmt_err);
                }
                final_statements = repaired_stmts;
            }
        }
    }

    let raw_responses = match exec_result {
        Ok(res) => res,
        Err(e) => {
            warn!("Text-to-SurrealQL execution error: {}", e);
            return None;
        }
    };

    Some(format_sql_results_as_chunk(
        &final_statements,
        &raw_responses,
        chunk_id,
        doc_id,
        title_prefix,
    ))
}

/// Executes Dynamic Text-to-SQL against any attached `ExternalDbClient` (PostgreSQL, MySQL, SQLite, AWS RDS, Azure SQL, or SurrealDB).
/// Automatically switches between Standard ANSI SQL (for Postgres/MySQL/SQLite) and SurrealQL (for SurrealDB).
pub async fn execute_dynamic_text_to_sql_on_external(
    state: &AppState,
    ext_client: &ExternalDbClient,
    user_query: &str,
    hop1_anchor: Option<&RetrievedChunk>,
    schema_prompt: &str,
    chunk_id: &str,
    doc_id: &str,
    title_prefix: &str,
) -> Option<RetrievedChunk> {
    if let ExternalDbClient::Surreal(ref sur) = ext_client {
        return execute_dynamic_text_to_sql_on_client(
            state,
            sur,
            user_query,
            hop1_anchor,
            schema_prompt,
            chunk_id,
            doc_id,
            title_prefix,
        )
        .await;
    }

    let anchor_context = match hop1_anchor {
        Some(a) => {
            let excerpt: String = a.content.chars().take(1200).collect();
            format!(
                "\n\nADDITIONAL CONTEXT FROM RETRIEVED DOCUMENT ('{}'):\n{}",
                a.doc_title, excerpt
            )
        }
        None => String::new(),
    };

    let dialect = ext_client.dialect_label();
    let system_prompt = format!(
        r#"You are an expert {dialect} query generator.
Translate the user's question (and any requirements from the retrieved document context) into 1 to 3 read-only SQL `SELECT` statements using ONLY the tables and exact column names listed in DATABASE SCHEMA below.
If the user's question is completely unrelated to all tables in DATABASE SCHEMA below, output ONLY the word `NONE`.

CRITICAL SQL SYNTAX RULES:
1. Output ONLY raw `SELECT` statements separated by semicolons `;` (or `NONE`). Do NOT output markdown explanations or prose.
2. Use standard SQL functions (`COUNT(*)`, `SUM(field)`, `AVG(field)`, `MIN(field)`, `MAX(field)`), `WHERE`, `GROUP BY`, and `ORDER BY`.
3. Always add a sensible `LIMIT` (e.g. `LIMIT 30`) on ungrouped queries.

DATABASE SCHEMA:
{schema_prompt}"#
    );

    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: system_prompt,
        },
        ChatMessage {
            role: "user".into(),
            content: format!("USER QUESTION: {}{}", user_query, anchor_context),
        },
    ];

    let raw_llm_out = state
        .llm
        .chat_completion(&messages, None, Some(0.0), Some(4096), Some(false))
        .await
        .ok()?;

    if raw_llm_out.trim().eq_ignore_ascii_case("NONE") {
        return None;
    }

    let statements = extract_readonly_standard_sql(&raw_llm_out);
    if statements.is_empty() {
        return None;
    }

    info!(
        "Executing dynamic Standard Text-to-SQL ({} - {}): {}",
        title_prefix,
        dialect,
        statements.join(" ")
    );

    let raw_responses = ext_client.execute_select_statements(&statements).await.ok()?;

    Some(format_sql_results_as_chunk(
        &statements,
        &raw_responses,
        chunk_id,
        doc_id,
        title_prefix,
    ))
}

/// Extracts read-only standard SQL `SELECT` statements (for PostgreSQL, MySQL, SQLite) without rewriting to SurrealQL.
pub fn extract_readonly_standard_sql(raw: &str) -> Vec<String> {
    let without_think = if let Some(end_idx) = raw.find("</think>") {
        &raw[end_idx + "</think>".len()..]
    } else {
        raw
    };

    let cleaned = if let Some(fence_start) = without_think.find("```") {
        let after_fence = &without_think[fence_start + 3..];
        let body_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
        let body = &after_fence[body_start..];
        if let Some(fence_end) = body.find("```") {
            &body[..fence_end]
        } else {
            body
        }
    } else {
        without_think
    };

    let mut valid = Vec::new();
    for raw_stmt in cleaned.split(';') {
        let code_lines: Vec<&str> = raw_stmt
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with("--") && !l.starts_with("//"))
            .collect();
        if code_lines.is_empty() {
            continue;
        }
        let joined = code_lines.join(" ");
        let upper = joined.to_uppercase();
        if !upper.starts_with("SELECT ") {
            continue;
        }
        let forbidden = [
            "DELETE ", "UPDATE ", "UPSERT ", "CREATE ", "INSERT ", "REMOVE ", "DEFINE ",
            "RELATE ", "KILL ", "ALTER ", "DROP ", "TRUNCATE ", "GRANT ", "REVOKE ",
        ];
        if forbidden.iter().any(|kw| upper.contains(kw)) {
            continue;
        }
        valid.push(format!("{};", joined.trim_end_matches(';')));
        if valid.len() >= 3 {
            break;
        }
    }
    valid
}

/// Extracts `SELECT` statements from LLM output, strips `<think>` tags and markdown code fences,
/// enforces read-only safety, and normalizes standard SQL functions (`SUM`, `AVG`, `COUNT(*)`) into SurrealQL.
pub fn extract_and_sanitize_surrealql(raw: &str) -> Vec<String> {
    let without_think = if let Some(end_idx) = raw.find("</think>") {
        &raw[end_idx + "</think>".len()..]
    } else {
        raw
    };

    let cleaned = if let Some(fence_start) = without_think.find("```") {
        let after_fence = &without_think[fence_start + 3..];
        let body_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
        let body = &after_fence[body_start..];
        if let Some(fence_end) = body.find("```") {
            &body[..fence_end]
        } else {
            body
        }
    } else {
        without_think
    };

    let mut valid_statements = Vec::new();
    for raw_stmt in cleaned.split(';') {
        let stmt = raw_stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        let code_lines: Vec<&str> = stmt
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with("--") && !l.starts_with("//"))
            .collect();
        if code_lines.is_empty() {
            continue;
        }
        let joined = code_lines.join(" ");
        let upper = joined.to_uppercase();

        if !upper.starts_with("SELECT ") {
            continue;
        }

        let forbidden = [
            "DELETE ", "UPDATE ", "UPSERT ", "CREATE ", "INSERT ", "REMOVE ", "DEFINE ",
            "RELATE ", "KILL ", "ALTER ", "DROP ",
        ];
        if forbidden.iter().any(|kw| upper.contains(kw)) {
            warn!("Rejected non-read-only SQL statement: {}", joined);
            continue;
        }

        let normalized = normalize_sql_to_surrealql(&joined);
        valid_statements.push(format!("{};", normalized.trim_end_matches(';')));
        if valid_statements.len() >= 3 {
            break;
        }
    }

    valid_statements
}

fn normalize_sql_to_surrealql(sql: &str) -> String {
    let mut out = sql
        .replace("COUNT(*)", "count()")
        .replace("count(*)", "count()")
        .replace("COUNT(1)", "count()")
        .replace("count(1)", "count()");

    for (std_fn, sur_fn) in [
        ("SUM(", "math::sum("),
        ("sum(", "math::sum("),
        ("AVG(", "math::mean("),
        ("avg(", "math::mean("),
        ("MIN(", "math::min("),
        ("min(", "math::min("),
        ("MAX(", "math::max("),
        ("max(", "math::max("),
    ] {
        let target_already = format!("math::{}", &sur_fn[6..]);
        if out.contains(std_fn) && !out.contains(&target_already) {
            out = out.replace(std_fn, sur_fn);
        }
    }

    // Convert standard SQL `IN ('a', 'b')` parentheses to SurrealQL `IN ['a', 'b']` array brackets
    out = convert_sql_in_parens_to_brackets(&out);

    // Strip unsupported SQL `HAVING ...` clauses before `ORDER BY` / `LIMIT` / end of query
    let lower_having = out.to_lowercase();
    if let Some(hav_idx) = lower_having.find(" having ") {
        let after_hav = &lower_having[hav_idx + 8..];
        let keep_tail = after_hav
            .find(" order by ")
            .or_else(|| after_hav.find(" limit "))
            .map(|rel| &out[hav_idx + 8 + rel..])
            .unwrap_or("");
        out = format!("{}{}", &out[..hav_idx], keep_tail);
    }

    // If the query uses aggregate functions (`math::sum(`, `math::mean(`, `math::min(`, `math::max(`, `count()`)
    // without `GROUP BY` or `GROUP ALL`, automatically insert `GROUP ALL` (removing any trailing `LIMIT` since GROUP ALL returns 1 row).
    let low_check = out.to_lowercase();
    let has_agg = low_check.contains("math::sum(")
        || low_check.contains("math::mean(")
        || low_check.contains("math::min(")
        || low_check.contains("math::max(")
        || low_check.contains("count()");
    if has_agg && !low_check.contains("group by") && !low_check.contains("group all") {
        // Strip trailing `LIMIT <n>` if present and append `GROUP ALL`
        if let Some(lim_idx) = low_check.rfind(" limit ") {
            out = format!("{} GROUP ALL", out[..lim_idx].trim());
        } else {
            out = format!("{} GROUP ALL", out.trim_end_matches(';').trim());
        }
    }

    out
}

/// Replaces `IN ('val1', 'val2')` with `IN ['val1', 'val2']` for SurrealQL compatibility.
fn convert_sql_in_parens_to_brackets(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Check case-insensitively for ` IN (`
        if i + 5 <= bytes.len() && sql[i..i + 5].eq_ignore_ascii_case(" in (") {
            result.push_str(&sql[i..i + 4]); // " IN "
            result.push('[');
            i += 5;
            let mut in_quotes = false;
            while i < bytes.len() {
                let ch = sql[i..].chars().next().unwrap();
                if ch == '\'' {
                    in_quotes = !in_quotes;
                    result.push(ch);
                } else if ch == ')' && !in_quotes {
                    result.push(']');
                    i += 1;
                    break;
                } else {
                    result.push(ch);
                }
                i += ch.len_utf8();
            }
        } else {
            let ch = sql[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}
