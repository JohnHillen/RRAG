pub mod db;
pub mod handlers;
pub mod mcp;
pub mod seed;

use crate::{
    db::RetrievedChunk,
    llm::ChatMessage,
    sql_formatter::format_sql_results_as_chunk,
    text_to_sql::extract_and_sanitize_surrealql,
    AppState,
};
use axum::{
    routing::{get, post},
    Router,
};
use handlers::{clear_webstore_handler, list_webstore_overview_handler, seed_webstore_handler};
use tracing::info;

/// Combined Multi-DB Schema Prompt covering BOTH:
/// - INTERNAL DB (`rag_namespace / knowledge_base` -> `internal_contract_ledger`)
/// - EXTERNAL DB (`webstore_external / shop_erp` -> `shop_product`, `shop_sale`)
pub const WEBSTORE_MULTI_DB_SCHEMA_PROMPT: &str = r#"
DATABASE 1 — INTERNAL CONTRACT DATABASE (`internal_contract_ledger`):
- `contract_id` (string): 'SUP-NORDICAUDIO-2026-01', 'SUP-ALPENTECH-2026-02', 'MKT-PULSEREACH-2026', 'MKT-ADVANTAGE-2026', 'LEASE-WH-HAM-2026', 'LEASE-WH-MUC-2026'
- `contract_type` (string): Exact values: `'Liefervertrag'`, `'Werbevertrag'`, `'Mietvertrag Lager'`
- `partner_name` (string): 'NordicAudio AB', 'AlpenTech GmbH', 'PulseReach Media GmbH', 'AdVantage Digital AG', 'HanseLogistik Immobilien AG', 'Bavaria GewerbePark & Fulfillment GmbH'
- `document_ref` (string): PDF filename
- `warehouse_or_campaign` (string): 'Hamburg-Billbrook (WH-HAM)', 'München-Ost (WH-MUC)', 'PulseReach-Spring (Code: SPRING26)', 'Google-Shopping-DE'
- `monthly_fixed_cost_eur` (float): Monthly fixed retainer or base rent in EUR
- `sla_threshold_desc` (string): Contractual thresholds
- `bonus_or_penalty_clause` (string): Bonus, kickback, extra rent, or penalty formula
- `current_delay_days` (int): Supplier delivery delay in days (11 days for AlpenTech GmbH)
- `compliance_status` (string): Current audit & SLA status

DATABASE 2 — EXTERNAL WEBSTORE ERP DATABASE (`shop_product` & `shop_sale`):
1) TABLE `shop_product` (12 Webstore Products, Stock, Pallets & Return Rates):
   - `sku` (string): 'ART-1001' .. 'ART-3002'
   - `product_name` (string)
   - `category` (string): 'Audio & Kopfhörer', 'Kaffee & Küche', 'Smart Home', 'Zubehör & Powerbanks', 'Home Office'
   - `supplier_name` (string): 'NordicAudio AB', 'AlpenTech GmbH', 'HanseCharge Components GmbH'
   - `purchase_price_eur` (float): Purchase price per unit in EUR
   - `retail_price_eur` (float): Selling price per unit in EUR
   - `margin_eur` (float): Margin per unit in EUR
   - `stock_units` (int): Current stock in units
   - `min_stock_sla` (int): Minimum required safety stock in units
   - `occupied_pallets` (int): Occupied Euro-pallets in warehouse
   - `warehouse_location` (string): Exact values: `'Hamburg-Billbrook (WH-HAM)'` or `'München-Ost (WH-MUC)'`
   - `contains_lithium_battery` (bool): `true` if product contains a lithium-ion battery
   - `return_rate_percent` (float): Customer return/defect rate in %

2) TABLE `shop_sale` (Webstore Sales History in March & April 2026):
   - `order_id` (string): 'ORD-2026-0301' .. 'ORD-2026-0409'
   - `order_date` (string): '2026-03-05' .. '2026-04-28'
   - `month` (string): Exact values: `'März 2026'` or `'April 2026'`
   - `sku` (string): Sold product SKU
   - `product_name` (string): Sold product name
   - `supplier_name` (string): 'NordicAudio AB', 'AlpenTech GmbH', 'HanseCharge Components GmbH'
   - `quantity` (int): Sold units
   - `unit_price_eur` (float): Unit selling price
   - `purchase_cost_eur` (float): Total purchase cost (`quantity * purchase_price_eur`)
   - `revenue_eur` (float): Net revenue in EUR
   - `profit_eur` (float): Net profit in EUR
   - `discount_code` (string): `'SPRING26'`, `'WELCOME10'`, `'NONE'`
   - `marketing_campaign` (string): Exact values: `'PulseReach-Spring'`, `'Google-Shopping-DE'`, `'Organisch / Direkt'`, `'Newsletter-VIP'`
   - `customer_region` (string): 'DE-Nord', 'DE-Süd', 'DE-West', 'DE-Ost', 'AT/CH'
   - `order_status` (string)

EXAMPLES (Always output 1 query for `internal_contract_ledger` AND 1–2 queries for `shop_sale` / `shop_product`):
- For marketing contracts & April campaign revenue (NEVER filter to just one `marketing_campaign` — group across ALL campaigns in April!):
  `SELECT contract_id, partner_name, warehouse_or_campaign, monthly_fixed_cost_eur, sla_threshold_desc, bonus_or_penalty_clause, compliance_status FROM internal_contract_ledger WHERE contract_type = 'Werbevertrag' ORDER BY contract_id ASC;`
  `SELECT marketing_campaign, math::sum(revenue_eur) AS april_revenue_eur, math::sum(profit_eur) AS april_profit_eur, math::sum(quantity) AS sold_units, count() AS orders FROM shop_sale WHERE month = 'April 2026' GROUP BY marketing_campaign ORDER BY april_revenue_eur DESC;`
- For warehouse leases, pallets & lithium battery stock:
  `SELECT contract_id, partner_name, warehouse_or_campaign, monthly_fixed_cost_eur, sla_threshold_desc, bonus_or_penalty_clause, compliance_status FROM internal_contract_ledger WHERE contract_type = 'Mietvertrag Lager' ORDER BY contract_id ASC;`
  `SELECT warehouse_location, math::sum(occupied_pallets) AS total_pallets, math::sum(stock_units) AS total_units FROM shop_product GROUP BY warehouse_location;`
  `SELECT warehouse_location, math::sum(stock_units) AS lithium_battery_units FROM shop_product WHERE contains_lithium_battery = true GROUP BY warehouse_location;`
- For supplier contracts, kickbacks, return rates & minimum stock:
  `SELECT contract_id, partner_name, sla_threshold_desc, bonus_or_penalty_clause, current_delay_days, compliance_status FROM internal_contract_ledger WHERE contract_type = 'Liefervertrag' ORDER BY contract_id ASC;`
  `SELECT sku, product_name, supplier_name, math::sum(quantity) AS total_units_sold, math::sum(purchase_cost_eur) AS total_purchase_eur, math::sum(revenue_eur) AS total_revenue_eur FROM shop_sale GROUP BY sku, product_name, supplier_name ORDER BY total_units_sold DESC;`
  `SELECT sku, product_name, supplier_name, stock_units, min_stock_sla, return_rate_percent, occupied_pallets, warehouse_location FROM shop_product ORDER BY sku ASC;`
"#;

pub const INTERNAL_WEBSTORE_SCHEMA_PROMPT: &str = WEBSTORE_MULTI_DB_SCHEMA_PROMPT;
pub const EXTERNAL_WEBSTORE_SCHEMA_PROMPT: &str = WEBSTORE_MULTI_DB_SCHEMA_PROMPT;

/// Registers all HTTP routes belonging to Showcase 03 (Webstore Multi-DB: Internal Contracts + External Shop ERP).
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/webstore/overview",
            get(list_webstore_overview_handler).delete(clear_webstore_handler),
        )
        .route("/v1/webstore/seed", post(seed_webstore_handler))
}

/// Generates SurrealQL statements in a single LLM call and routes each statement automatically to:
/// - `state.db` (Internal DB) if the statement queries `internal_contract_ledger`
/// - `state.webstore_external_db()` (External DB) if the statement queries `shop_product` or `shop_sale`
pub async fn execute_federated_webstore_sql(
    state: &AppState,
    user_query: &str,
    hop1_anchors: &[RetrievedChunk],
) -> (Option<RetrievedChunk>, Option<RetrievedChunk>) {
    let mut anchor_context = String::new();
    for a in hop1_anchors.iter().take(3) {
        let excerpt: String = a.content.chars().take(600).collect();
        anchor_context.push_str(&format!(
            "\n\nRETRIEVED CONTRACT PDF ('{}'):\n{}",
            a.doc_title, excerpt
        ));
    }

    let system_prompt = format!(
        r#"You are an expert SurrealQL (SurrealDB 4.0) Multi-Database query generator.
Translate the user's question into 2 to 3 read-only SurrealQL `SELECT` statements separated by semicolons `;`:
- Statement 1 MUST query `internal_contract_ledger` (Internal Database) for the relevant contract terms, fixed fees, delays, and compliance status.
- Statement 2 (and optional Statement 3) MUST query `shop_product` and/or `shop_sale` (External Webstore Database) for product inventory, pallets, lithium batteries, return rates, or sales revenue.

CRITICAL SURREALQL 4.0 SYNTAX RULES:
1. Output ONLY raw SurrealQL `SELECT` statements separated by semicolons `;`. Do NOT output markdown prose.
2. NEVER use `HAVING`, `CASE WHEN`, `IF(...)`, `IIF(...)`, or subqueries! Simply `SELECT` the columns and aggregate numbers (`math::sum(...)`, `count()`); comparisons against thresholds happen afterward.
3. When querying `shop_sale` for campaign revenues, do NOT filter to a single `marketing_campaign` — use `WHERE month = 'April 2026' GROUP BY marketing_campaign` so all campaigns are included!
4. Every non-aggregated column in `SELECT` MUST appear in `GROUP BY`. If all columns are aggregates without grouping columns, add `GROUP ALL`.

{WEBSTORE_MULTI_DB_SCHEMA_PROMPT}"#
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

    let Ok(raw_out) = state
        .llm
        .chat_completion(&messages, None, Some(0.0), Some(4096), Some(false))
        .await
    else {
        return (None, None);
    };

    let all_stmts = extract_and_sanitize_surrealql(&raw_out);
    let mut internal_stmts = Vec::new();
    let mut external_stmts = Vec::new();

    for st in all_stmts {
        let low = st.to_lowercase();
        if low.contains("internal_contract_ledger") {
            internal_stmts.push(st);
        } else if low.contains("shop_product") || low.contains("shop_sale") {
            external_stmts.push(st);
        }
    }

    let internal_chunk = if !internal_stmts.is_empty() {
        let mut ok_stmts = Vec::new();
        let mut ok_res = Vec::new();
        for st in &internal_stmts {
            info!("Executing Showcase 03 Internal DB SQL: {}", st);
            if let Ok(mut res) = state.db.execute_sql(st).await {
                if let Some(first) = res.pop() {
                    if first.get("status").and_then(|s| s.as_str()) != Some("ERR") {
                        ok_stmts.push(st.clone());
                        ok_res.push(first);
                    }
                }
            }
        }
        if !ok_stmts.is_empty() {
            Some(format_sql_results_as_chunk(
                &ok_stmts,
                &ok_res,
                "webstore_internal_sql_live_result",
                "internal_contract_ledger_table",
                "🏛️ [Interne Vertrags-DB Text-to-SQL]",
            ))
        } else {
            None
        }
    } else {
        None
    };

    let ext_db = state.webstore_external_db();
    let external_chunk = if !external_stmts.is_empty() {
        let mut ok_stmts = Vec::new();
        let mut ok_res = Vec::new();
        for st in &external_stmts {
            info!("Executing Showcase 03 External Webstore DB SQL: {}", st);
            if let Ok(mut res) = ext_db.execute_sql(st).await {
                if let Some(first) = res.pop() {
                    if first.get("status").and_then(|s| s.as_str()) != Some("ERR") {
                        ok_stmts.push(st.clone());
                        ok_res.push(first);
                    }
                }
            }
        }
        if !ok_stmts.is_empty() {
            Some(format_sql_results_as_chunk(
                &ok_stmts,
                &ok_res,
                "webstore_external_sql_live_result",
                "external_webstore_erp_tables",
                "🛒 [Externe Webstore-DB Text-to-SQL]",
            ))
        } else {
            None
        }
    } else {
        None
    };

    (internal_chunk, external_chunk)
}

/// Runs Multi-DB Text-to-SurrealQL across BOTH the Internal Contract DB (`state.db`)
/// and the External Webstore DB (`state.webstore_external_db()`) when Showcase 03 is active!
pub async fn augment_retrieval(
    state: &AppState,
    query: &str,
    mut ranked_pool: Vec<RetrievedChunk>,
    top_n: usize,
    db_ms: &mut u64,
    reranker_used: &mut String,
) -> Vec<RetrievedChunk> {
    let top_anchors: Vec<RetrievedChunk> = ranked_pool.iter().take(3).cloned().collect();
    ranked_pool.truncate(top_n);
    let mut final_chunks = ranked_pool;

    let t_sql = std::time::Instant::now();
    let (int_chunk_opt, ext_chunk_opt) =
        execute_federated_webstore_sql(state, query, &top_anchors).await;
    *db_ms += t_sql.elapsed().as_millis() as u64;

    let mut sql_added = 0usize;
    if let Some(ext_chunk) = ext_chunk_opt {
        let idx = if final_chunks.is_empty() { 0 } else { 1 };
        final_chunks.insert(idx, ext_chunk);
        sql_added += 1;
    }
    if let Some(int_chunk) = int_chunk_opt {
        let idx = final_chunks.len().min(1 + sql_added);
        final_chunks.insert(idx, int_chunk);
        sql_added += 1;
    }

    // Preserve at least `top_n` document chunks PLUS the generated SQL chunks so no second contract PDF gets cut off!
    let max_keep = (top_n + sql_added).max(4);
    if final_chunks.len() > max_keep {
        final_chunks.truncate(max_keep);
    }
    *reranker_used = format!(
        "{} + 🧠 Multi-DB Text-to-SurrealQL (Interne Vertrags-DB + Externe Webstore-DB)",
        reranker_used
    );

    final_chunks
}
