use crate::{config::AppConfig, db::SurrealClient};
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Map, Value};
use sqlx::{
    mysql::{MySqlPool, MySqlPoolOptions, MySqlRow},
    postgres::{PgPool, PgPoolOptions},
    sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow},
    Column, Row,
};
use std::time::Duration;
use tracing::info;

#[derive(Clone)]
pub enum ExternalDbClient {
    Postgres(PgPool),
    MySql(MySqlPool),
    Sqlite(SqlitePool),
    Surreal(SurrealClient),
}

impl ExternalDbClient {
    /// Connects to an external database based on the URL scheme in `EXTERNAL_DB_URL` (or `EXTERNAL_SURREAL_URL`):
    /// - `postgres://` or `postgresql://` -> PostgreSQL (Local, AWS RDS/Aurora, Azure Database for PostgreSQL, GCP Cloud SQL, Supabase)
    /// - `mysql://` or `mariadb://`       -> MySQL / MariaDB (Local, AWS RDS MySQL, Azure Database for MySQL)
    /// - `sqlite://`                      -> SQLite (Local / Embedded `.db` file)
    /// - `http://` or `https://`          -> SurrealDB (External SurrealDB instance or Cloud)
    pub async fn from_config(http: Client, config: &AppConfig) -> Option<Self> {
        let raw_url = config.external_db_url.as_ref()?.trim();
        if raw_url.is_empty() {
            return None;
        }
        let lower = raw_url.to_lowercase();

        if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
            match PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(10))
                .connect_lazy(raw_url)
            {
                Ok(pool) => {
                    info!("Configured External PostgreSQL Database Client (AWS RDS / Azure / Postgres).");
                    Some(Self::Postgres(pool))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize external PostgreSQL pool: {}", e);
                    None
                }
            }
        } else if lower.starts_with("mysql://") || lower.starts_with("mariadb://") {
            let norm_url = if lower.starts_with("mariadb://") {
                format!("mysql://{}", &raw_url["mariadb://".len()..])
            } else {
                raw_url.to_string()
            };
            match MySqlPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(10))
                .connect_lazy(&norm_url)
            {
                Ok(pool) => {
                    info!("Configured External MySQL/MariaDB Database Client (AWS RDS / Azure / MySQL).");
                    Some(Self::MySql(pool))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize external MySQL pool: {}", e);
                    None
                }
            }
        } else if lower.starts_with("sqlite://") {
            match SqlitePoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(10))
                .connect_lazy(raw_url)
            {
                Ok(pool) => {
                    info!("Configured External SQLite Database Client ({}).", raw_url);
                    Some(Self::Sqlite(pool))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize external SQLite pool: {}", e);
                    None
                }
            }
        } else {
            let mut ext_cfg = config.clone();
            ext_cfg.surreal_url = raw_url.to_string();
            ext_cfg.surreal_user = config.external_surreal_user.clone();
            ext_cfg.surreal_pass = config.external_surreal_pass.clone();
            ext_cfg.surreal_ns = config.external_surreal_ns.clone();
            ext_cfg.surreal_db = config.external_surreal_db.clone();
            info!(
                "Configured External SurrealDB Client -> URL: {}, NS: {}, DB: {}",
                raw_url, ext_cfg.surreal_ns, ext_cfg.surreal_db
            );
            Some(Self::Surreal(SurrealClient::new(http, ext_cfg)))
        }
    }

    /// Returns true if this external database uses standard SQL (PostgreSQL, MySQL, SQLite) rather than SurrealQL.
    pub fn is_standard_sql(&self) -> bool {
        !matches!(self, Self::Surreal(_))
    }

    /// Returns a human-readable dialect label for the Text-to-SQL prompt.
    pub fn dialect_label(&self) -> &'static str {
        match self {
            Self::Postgres(_) => "PostgreSQL (Standard ANSI SQL)",
            Self::MySql(_) => "MySQL / MariaDB (Standard ANSI SQL)",
            Self::Sqlite(_) => "SQLite (Standard ANSI SQL)",
            Self::Surreal(_) => "SurrealQL (SurrealDB 4.0)",
        }
    }

    /// Executes 1..N read-only `SELECT` statements and returns the unified JSON response array
    /// (`[{"status": "OK", "result": [ ...rows... ]}, ...]`) compatible with `sql_formatter.rs`.
    pub async fn execute_select_statements(&self, statements: &[String]) -> Result<Vec<Value>> {
        match self {
            Self::Surreal(client) => {
                let combined = statements.join("\n");
                client.execute_sql(&combined).await
            }
            Self::Postgres(pool) => {
                let mut results = Vec::with_capacity(statements.len());
                for stmt in statements {
                    let clean = stmt.trim().trim_end_matches(';');
                    let wrapped = format!(
                        "SELECT COALESCE(json_agg(t), '[]'::json)::text FROM ({}) t",
                        clean
                    );
                    let json_str: Option<String> = sqlx::query_scalar(&wrapped)
                        .fetch_one(pool)
                        .await
                        .with_context(|| format!("PostgreSQL query failed: {}", clean))?;
                    let arr: Value = serde_json::from_str(json_str.as_deref().unwrap_or("[]"))
                        .unwrap_or_else(|_| json!([]));
                    results.push(json!({
                        "status": "OK",
                        "result": arr
                    }));
                }
                Ok(results)
            }
            Self::MySql(pool) => {
                let mut results = Vec::with_capacity(statements.len());
                for stmt in statements {
                    let clean = stmt.trim().trim_end_matches(';');
                    let rows: Vec<MySqlRow> = sqlx::query(clean)
                        .fetch_all(pool)
                        .await
                        .with_context(|| format!("MySQL query failed: {}", clean))?;
                    let json_rows: Vec<Value> = rows.iter().map(mysql_row_to_json).collect();
                    results.push(json!({
                        "status": "OK",
                        "result": json_rows
                    }));
                }
                Ok(results)
            }
            Self::Sqlite(pool) => {
                let mut results = Vec::with_capacity(statements.len());
                for stmt in statements {
                    let clean = stmt.trim().trim_end_matches(';');
                    let rows: Vec<SqliteRow> = sqlx::query(clean)
                        .fetch_all(pool)
                        .await
                        .with_context(|| format!("SQLite query failed: {}", clean))?;
                    let json_rows: Vec<Value> = rows.iter().map(sqlite_row_to_json).collect();
                    results.push(json!({
                        "status": "OK",
                        "result": json_rows
                    }));
                }
                Ok(results)
            }
        }
    }
}

fn mysql_row_to_json(row: &MySqlRow) -> Value {
    let mut map = Map::new();
    for col in row.columns() {
        let name = col.name().to_string();
        let val = if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(col.ordinal()) {
            json!(v)
        } else if let Ok(Some(v)) = row.try_get::<Option<f64>, _>(col.ordinal()) {
            json!(v)
        } else if let Ok(Some(v)) = row.try_get::<Option<bool>, _>(col.ordinal()) {
            json!(v)
        } else if let Ok(Some(v)) = row.try_get::<Option<String>, _>(col.ordinal()) {
            json!(v)
        } else if let Ok(Some(v)) = row.try_get::<Option<Value>, _>(col.ordinal()) {
            v
        } else {
            Value::Null
        };
        map.insert(name, val);
    }
    Value::Object(map)
}

fn sqlite_row_to_json(row: &SqliteRow) -> Value {
    let mut map = Map::new();
    for col in row.columns() {
        let name = col.name().to_string();
        let val = if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(col.ordinal()) {
            json!(v)
        } else if let Ok(Some(v)) = row.try_get::<Option<f64>, _>(col.ordinal()) {
            json!(v)
        } else if let Ok(Some(v)) = row.try_get::<Option<bool>, _>(col.ordinal()) {
            json!(v)
        } else if let Ok(Some(v)) = row.try_get::<Option<String>, _>(col.ordinal()) {
            json!(v)
        } else {
            Value::Null
        };
        map.insert(name, val);
    }
    Value::Object(map)
}
