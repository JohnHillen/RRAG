pub mod db;
pub mod handlers;
pub mod mcp;
pub mod seed;

use crate::{db::RetrievedChunk, text_to_sql::execute_dynamic_text_to_sql, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use handlers::{
    clear_crm_employees_handler, import_crm_employees_handler, list_crm_employees_handler,
    seed_crm_employees_handler,
};

pub const HR_SCHEMA_PROMPT: &str = r#"
TABLE `employee_record` (3,000 Consulting & IT Employees):
- `emp_id` (string): Unique Employee ID, e.g. 'EMP-1042', 'EMP-1019', 'EMP-1174', 'EMP-1231', 'EMP-1105', 'EMP-1088'
- `name` (string): Full name, e.g. 'Thomas Schmidt', 'Dr. Elena Rostova', 'Lukas Bergmann', 'Hannah Lindner', 'Michael Kurz', 'Sarah Weber'
- `role` (string): Job title, e.g. 'Senior Systems & Cloud Engineer', 'Principal Rust & Database Engineer'
- `department` (string): Department name
- `skills` (array<string>): Technical skills (Case-sensitive! Canonical values: 'Rust', 'SQL', 'PostgreSQL', 'SurrealDB', 'Kubernetes', 'Docker', 'Terraform', 'Kafka', 'Python', 'Java', 'C++', 'Go', 'Snowflake', 'BigQuery', 'NoSQL', 'MongoDB', 'Redis', 'SAP', 'TypeScript', 'React').
  -> Example filter for mandatory skills: `'Rust' IN skills AND 'SQL' IN skills`
- `english_level` (string): CEFR level ('A1', 'A2', 'B1', 'B2', 'C1', 'C2').
  -> Note: `>= B2` means `english_level IN ['B2', 'C1', 'C2']`; `>= C1` means `english_level IN ['C1', 'C2']`.
- `certifications` (array<string>): Certifications, e.g. 'ISO/IEC 27001 Foundation', 'BSI IT-Grundschutz', 'CKA'
- `current_project` (string): Current project assignment or 'Frei / Bench'
- `availability_percent` (int): Availability 0..100 (e.g. 100 = immediately 100% available)
- `hourly_rate` (float): Hourly consulting rate in EUR (e.g. 145.0)
- `location` (string): Office city ('München', 'Stuttgart', 'Frankfurt', 'Berlin', 'Hamburg', 'Ingolstadt', 'Köln', 'Düsseldorf', 'Nürnberg')

EXAMPLES:
- Full match query (e.g. Rust + SQL + English >= B2, sorted by availability DESC):
  `SELECT emp_id, name, role, skills, english_level, availability_percent, hourly_rate, location, current_project FROM employee_record WHERE 'Rust' IN skills AND 'SQL' IN skills AND english_level IN ['B2', 'C1', 'C2'] ORDER BY availability_percent DESC, emp_id ASC LIMIT 35;`
- Aggregate count & average rate of all full matches:
  `SELECT count() AS total_full_matches, math::mean(hourly_rate) AS avg_hourly_rate FROM employee_record WHERE 'Rust' IN skills AND 'SQL' IN skills AND english_level IN ['B2', 'C1', 'C2'] GROUP ALL;`
- 1-skill training gap candidates (missing exactly 1 of Rust, SQL, or B2 English):
  `SELECT emp_id, name, role, skills, english_level, availability_percent, hourly_rate, location FROM employee_record WHERE (('Rust' IN skills AND 'SQL' IN skills AND english_level = 'B1') OR ('Rust' IN skills AND 'SQL' NOT IN skills AND english_level IN ['B2', 'C1', 'C2']) OR ('Rust' NOT IN skills AND 'SQL' IN skills AND english_level IN ['B2', 'C1', 'C2'])) ORDER BY availability_percent DESC LIMIT 20;`
"#;

/// Registers all HTTP routes belonging to Showcase 01 (HR & IT Consulting).
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/crm/employees",
            get(list_crm_employees_handler).delete(clear_crm_employees_handler),
        )
        .route("/v1/crm/seed", post(seed_crm_employees_handler))
        .route("/v1/crm/import", post(import_crm_employees_handler))
}

/// Scans Hop-1 anchor chunks (e.g. a customer SLA or project specification) for technical
/// qualifications, skills, or CEFR levels when Showcase 01 is active.
pub fn extract_bridge_terms(anchor_text: &str, original_query_lower: &str) -> Vec<String> {
    let mut found = Vec::new();
    let text_lower = anchor_text.to_lowercase();

    let skill_markers = [
        "rust",
        "sql",
        "python",
        "java",
        "c++",
        "golang",
        "kubernetes",
        "docker",
        "terraform",
        "kafka",
        "postgresql",
        "surrealdb",
        "snowflake",
        "bigquery",
        "nosql",
        "mongodb",
        "redis",
        "b1",
        "b2",
        "c1",
        "c2",
        "iso/iec 27001",
        "bsi",
        "projekt alpha",
        "project alpha",
        "projekt titan",
        "project titan",
        "mitarbeiterprofil",
    ];

    for marker in skill_markers {
        if text_lower.contains(marker) && !original_query_lower.contains(marker) {
            let is_clean_match = text_lower
                .split(|c: char| !c.is_alphanumeric() && c != '/' && c != '+')
                .any(|w| w == marker)
                || marker.contains(' ');
            if is_clean_match {
                found.push(marker.to_string());
            }
        }
    }

    found
}

/// Returns whether a query asks for employees/candidates/training in Showcase 01.
pub fn is_employee_lookup(query_lower: &str) -> bool {
    query_lower.contains("mitarbeiter")
        || query_lower.contains("employee")
        || query_lower.contains("kandidat")
        || query_lower.contains("candidate")
        || query_lower.contains("weiterbildung")
        || query_lower.contains("schulung")
        || query_lower.contains("besetzung")
        || query_lower.contains("stundensatz")
        || query_lower.contains("verfügbar")
        || query_lower.contains("standort")
        || query_lower.contains("skill")
}

/// Dynamically generates and executes SurrealQL via the LLM (`execute_dynamic_text_to_sql`) when Showcase 01 is active.
pub async fn augment_retrieval(
    state: &AppState,
    query: &str,
    mut ranked_pool: Vec<RetrievedChunk>,
    top_n: usize,
    db_ms: &mut u64,
    reranker_used: &mut String,
) -> Vec<RetrievedChunk> {
    let primary_anchor = ranked_pool
        .iter()
        .find(|c| {
            let t = c.doc_title.to_lowercase();
            t.contains("vereinbarung")
                || t.contains("vertrag")
                || t.contains("sla")
                || t.contains("[sales]")
        })
        .or_else(|| ranked_pool.first())
        .cloned();

    ranked_pool.truncate(top_n);
    let mut final_chunks = ranked_pool;

    let t_sql = std::time::Instant::now();
    if let Some(sql_chunk) = execute_dynamic_text_to_sql(
        state,
        query,
        primary_anchor.as_ref(),
        HR_SCHEMA_PROMPT,
        "crm_sql_live_query_result",
        "crm_employee_record_table",
        "🗄️ [CRM/HR-DB Text-to-SQL]",
    )
    .await
    {
        *db_ms += t_sql.elapsed().as_millis() as u64;
        let insert_idx = if final_chunks.is_empty() { 0 } else { 1 };
        final_chunks.insert(insert_idx, sql_chunk);
        if final_chunks.len() > top_n.max(2) {
            final_chunks.truncate(top_n.max(2));
        }
        *reranker_used = format!("{} + 🧠 Text-to-SurrealQL (HR-DB)", reranker_used);
    }

    final_chunks
}
