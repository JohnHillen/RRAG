use super::seed::generate_demo_webstore_data;
use crate::{db::extract_result_array, showcases::ActiveShowcase, AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use tracing::info;

/// Returns internal contracts (`state.db`) and external webstore products & sales (`state.webstore_external_db()`) (`GET /v1/webstore/overview`)
pub async fn list_webstore_overview_handler(State(state): State<AppState>) -> impl IntoResponse {
    let ext_db = state.webstore_external_db();

    let internal_res = state
        .db
        .execute_sql("SELECT * FROM internal_contract_ledger ORDER BY contract_id ASC LIMIT 200;")
        .await
        .unwrap_or_default();
    let contracts = extract_result_array(internal_res.first());

    let external_res = ext_db
        .execute_sql(
            "SELECT * FROM shop_product ORDER BY sku ASC LIMIT 1000;
             SELECT * FROM shop_sale ORDER BY order_date ASC, order_id ASC LIMIT 5000;",
        )
        .await
        .unwrap_or_default();
    let products = extract_result_array(external_res.first());
    let sales = extract_result_array(external_res.get(1));

    (
        StatusCode::OK,
        Json(json!({
            "internal_database": "rag_namespace / knowledge_base (internal_contract_ledger + PDFs)",
            "external_database": "webstore_external / shop_erp (shop_product + shop_sale)",
            "contract_count": contracts.len(),
            "product_count": products.len(),
            "sale_count": sales.len(),
            "contracts": contracts,
            "products": products,
            "sales": sales
        })),
    )
}

/// Clears Showcase 03 tables in both the internal and external databases (`DELETE /v1/webstore/overview`)
pub async fn clear_webstore_handler(State(state): State<AppState>) -> impl IntoResponse {
    let ext_db = state.webstore_external_db();
    let _ = state
        .db
        .execute_sql("DELETE internal_contract_ledger;")
        .await;
    match ext_db
        .execute_sql("DELETE shop_product; DELETE shop_sale;")
        .await
    {
        Ok(_) => {
            state.set_active_showcase_if_matches(ActiveShowcase::Webstore, ActiveShowcase::None);
            (StatusCode::OK, Json(json!({ "status": "cleared" })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Seeds Showcase 03 across BOTH databases (`POST /v1/webstore/seed`):
/// - Internal DB (`state.db`): `internal_contract_ledger`
/// - External DB (`state.webstore_external_db()`): `shop_product` & `shop_sale`
pub async fn seed_webstore_handler(State(state): State<AppState>) -> impl IntoResponse {
    let ext_db = state.webstore_external_db();
    let _ = state
        .db
        .execute_sql("DELETE internal_contract_ledger;")
        .await;
    let _ = ext_db
        .execute_sql("DELETE shop_product; DELETE shop_sale;")
        .await;

    let (contracts, products, sales) = generate_demo_webstore_data();
    let c_cnt = match state.db.upsert_internal_webstore_contracts(&contracts).await {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Internal DB seed error: {}", e) })),
            );
        }
    };

    match ext_db.upsert_external_webstore_data(&products, &sales).await {
        Ok((p_cnt, s_cnt)) => {
            state.set_active_showcase(ActiveShowcase::Webstore);
            info!(
                "Seeded Showcase 03 (Webstore Multi-DB): {} internal contracts (Internal DB) + {} products & {} sales orders (External DB).",
                c_cnt, p_cnt, s_cnt
            );
            (
                StatusCode::OK,
                Json(json!({
                    "status": "seeded",
                    "internal_contract_count": c_cnt,
                    "external_product_count": p_cnt,
                    "external_sale_count": s_cnt
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("External DB seed error: {}", e) })),
        ),
    }
}
