use crate::db::{sanitize_key, SurrealClient};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeRecord {
    pub emp_id: String,
    pub name: String,
    pub role: String,
    pub department: String,
    #[serde(default)]
    pub skills: Vec<String>,
    pub english_level: String,
    #[serde(default)]
    pub certifications: Vec<String>,
    pub current_project: String,
    #[serde(default = "default_availability")]
    pub availability_percent: i64,
    #[serde(default = "default_hourly_rate")]
    pub hourly_rate: f64,
    #[serde(default = "default_location")]
    pub location: String,
}

fn default_availability() -> i64 {
    100
}
fn default_hourly_rate() -> f64 {
    125.0
}
fn default_location() -> String {
    "Muenchen".to_string()
}

impl SurrealClient {
    /// Ensures the `employee_record` schema exists in SurrealDB (only called when Showcase 01 is seeded/imported).
    pub async fn ensure_employee_schema(&self) -> Result<()> {
        let sql = r#"
            DEFINE TABLE IF NOT EXISTS employee_record SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS emp_id               ON TABLE employee_record TYPE string;
            DEFINE FIELD IF NOT EXISTS name                 ON TABLE employee_record TYPE string;
            DEFINE FIELD IF NOT EXISTS role                 ON TABLE employee_record TYPE string;
            DEFINE FIELD IF NOT EXISTS department           ON TABLE employee_record TYPE string;
            DEFINE FIELD IF NOT EXISTS skills               ON TABLE employee_record TYPE array<string>;
            DEFINE FIELD IF NOT EXISTS english_level        ON TABLE employee_record TYPE string;
            DEFINE FIELD IF NOT EXISTS certifications       ON TABLE employee_record TYPE array<string>;
            DEFINE FIELD IF NOT EXISTS current_project      ON TABLE employee_record TYPE string;
            DEFINE FIELD IF NOT EXISTS availability_percent ON TABLE employee_record TYPE int DEFAULT 100;
            DEFINE FIELD IF NOT EXISTS hourly_rate          ON TABLE employee_record TYPE float DEFAULT 120.0;
            DEFINE FIELD IF NOT EXISTS location             ON TABLE employee_record TYPE string DEFAULT 'Muenchen';
            DEFINE FIELD IF NOT EXISTS updated_at           ON TABLE employee_record TYPE datetime DEFAULT time::now();
            DEFINE INDEX IF NOT EXISTS idx_employee_emp_id  ON TABLE employee_record FIELDS emp_id UNIQUE;
        "#;
        let _ = self.execute_sql(sql).await;
        Ok(())
    }

    /// Inserts or updates a batch of structured `EmployeeRecord` rows in SurrealDB.
    pub async fn upsert_employees(&self, employees: &[EmployeeRecord]) -> Result<usize> {
        if employees.is_empty() {
            return Ok(0);
        }
        self.ensure_employee_schema().await?;

        for batch in employees.chunks(250) {
            let mut sql = String::from("BEGIN TRANSACTION;\n");
            for emp in batch {
                let rec_key = sanitize_key(&emp.emp_id);
                if rec_key.is_empty() {
                    continue;
                }
                sql.push_str(&format!(
                    "UPSERT type::record('employee_record', {rk}) SET \
                        emp_id = {id}, \
                        name = {name}, \
                        role = {role}, \
                        department = {dept}, \
                        skills = {skills}, \
                        english_level = {eng}, \
                        certifications = {certs}, \
                        current_project = {proj}, \
                        availability_percent = {avail}, \
                        hourly_rate = {rate}, \
                        location = {loc}, \
                        updated_at = time::now();\n",
                    rk = serde_json::to_string(&rec_key)?,
                    id = serde_json::to_string(&emp.emp_id)?,
                    name = serde_json::to_string(&emp.name)?,
                    role = serde_json::to_string(&emp.role)?,
                    dept = serde_json::to_string(&emp.department)?,
                    skills = serde_json::to_string(&emp.skills)?,
                    eng = serde_json::to_string(&emp.english_level)?,
                    certs = serde_json::to_string(&emp.certifications)?,
                    proj = serde_json::to_string(&emp.current_project)?,
                    avail = emp.availability_percent,
                    rate = emp.hourly_rate,
                    loc = serde_json::to_string(&emp.location)?,
                ));
            }
            sql.push_str("COMMIT TRANSACTION;\n");
            self.execute_sql(&sql).await?;
        }
        Ok(employees.len())
    }
}
