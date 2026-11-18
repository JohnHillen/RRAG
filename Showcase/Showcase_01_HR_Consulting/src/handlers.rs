use super::{db::EmployeeRecord, seed::generate_demo_employees};
use crate::{db::extract_result_array, showcases::ActiveShowcase, AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct CrmSeedRequest {
    #[serde(default = "default_seed_count")]
    pub count: usize,
}

fn default_seed_count() -> usize {
    3000
}

#[derive(Debug, Deserialize)]
pub struct CrmImportRequest {
    pub employees: Vec<EmployeeRecord>,
}

/// Lists all structured employee records in SurrealDB (`GET /v1/crm/employees`)
pub async fn list_crm_employees_handler(State(state): State<AppState>) -> impl IntoResponse {
    let sql = "SELECT emp_id, name, role, department, skills, english_level, certifications, current_project, availability_percent, hourly_rate, location FROM employee_record ORDER BY emp_id ASC LIMIT 10000;";
    match state.db.execute_sql(sql).await {
        Ok(res) => {
            let employees = extract_result_array(res.first());
            (
                StatusCode::OK,
                Json(json!({
                    "count": employees.len(),
                    "employees": employees
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Clears all structured employee records in SurrealDB (`DELETE /v1/crm/employees`)
pub async fn clear_crm_employees_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.execute_sql("DELETE employee_record;").await {
        Ok(_) => {
            state.set_active_showcase_if_matches(
                ActiveShowcase::HrConsulting,
                ActiveShowcase::None,
            );
            (StatusCode::OK, Json(json!({ "status": "cleared" })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Imports a batch of structured employee records (`POST /v1/crm/import`)
pub async fn import_crm_employees_handler(
    State(state): State<AppState>,
    Json(req): Json<CrmImportRequest>,
) -> impl IntoResponse {
    match state.db.upsert_employees(&req.employees).await {
        Ok(n) => {
            if n > 0 {
                state.set_active_showcase(ActiveShowcase::HrConsulting);
            }
            (
                StatusCode::OK,
                Json(json!({
                    "status": "imported",
                    "imported_count": n
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Seeds a realistic 300- or 3,000-employee enterprise dataset into SurrealDB (`POST /v1/crm/seed`)
pub async fn seed_crm_employees_handler(
    State(state): State<AppState>,
    Json(req): Json<CrmSeedRequest>,
) -> impl IntoResponse {
    let target_count = req.count.clamp(10, 10_000);
    let _ = state.db.execute_sql("DELETE employee_record;").await;
    let demo_employees = generate_demo_employees(target_count);
    match state.db.upsert_employees(&demo_employees).await {
        Ok(inserted) => {
            state.set_active_showcase(ActiveShowcase::HrConsulting);
            info!(
                "Seeded {} structured employee records into SurrealDB (Showcase 01: employee_record).",
                inserted
            );
            (
                StatusCode::OK,
                Json(json!({
                    "status": "seeded",
                    "count": inserted
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
