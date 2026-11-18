use crate::db::{sanitize_key, SurrealClient};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Stored in the INTERNAL database (`rag_namespace / knowledge_base` -> table `internal_contract_ledger`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalContractRecord {
    pub contract_id: String,
    pub contract_type: String,
    pub partner_name: String,
    pub document_ref: String,
    pub warehouse_or_campaign: String,
    pub monthly_fixed_cost_eur: f64,
    pub sla_threshold_desc: String,
    pub bonus_or_penalty_clause: String,
    pub current_delay_days: i64,
    pub compliance_status: String,
}

/// Stored in the EXTERNAL Webstore database (`webstore_external / shop_erp` -> table `shop_product`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopProductRecord {
    pub sku: String,
    pub product_name: String,
    pub category: String,
    pub supplier_name: String,
    pub purchase_price_eur: f64,
    pub retail_price_eur: f64,
    pub margin_eur: f64,
    pub stock_units: i64,
    pub min_stock_sla: i64,
    pub occupied_pallets: i64,
    pub warehouse_location: String,
    pub contains_lithium_battery: bool,
    pub return_rate_percent: f64,
}

/// Stored in the EXTERNAL Webstore database (`webstore_external / shop_erp` -> table `shop_sale`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopSaleRecord {
    pub order_id: String,
    pub order_date: String,
    pub month: String,
    pub sku: String,
    pub product_name: String,
    pub category: String,
    pub supplier_name: String,
    pub quantity: i64,
    pub unit_price_eur: f64,
    pub purchase_cost_eur: f64,
    pub revenue_eur: f64,
    pub profit_eur: f64,
    pub discount_code: String,
    pub marketing_campaign: String,
    pub customer_region: String,
    pub order_status: String,
}

impl SurrealClient {
    /// Ensures the `internal_contract_ledger` table exists in the INTERNAL SurrealDB.
    pub async fn ensure_internal_webstore_schema(&self) -> Result<()> {
        let sql = "
            DEFINE TABLE IF NOT EXISTS internal_contract_ledger SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS contract_id            ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS contract_type          ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS partner_name           ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS document_ref           ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS warehouse_or_campaign  ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS monthly_fixed_cost_eur ON TABLE internal_contract_ledger TYPE float;
            DEFINE FIELD IF NOT EXISTS sla_threshold_desc     ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS bonus_or_penalty_clause ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS current_delay_days     ON TABLE internal_contract_ledger TYPE int DEFAULT 0;
            DEFINE FIELD IF NOT EXISTS compliance_status      ON TABLE internal_contract_ledger TYPE string;
            DEFINE FIELD IF NOT EXISTS updated_at             ON TABLE internal_contract_ledger TYPE datetime DEFAULT time::now();
            DEFINE INDEX IF NOT EXISTS idx_internal_contract  ON TABLE internal_contract_ledger FIELDS contract_id UNIQUE;
        ";
        self.execute_sql(sql).await?;
        Ok(())
    }

    /// Upserts `internal_contract_ledger` rows into the INTERNAL SurrealDB (`state.db`).
    pub async fn upsert_internal_webstore_contracts(
        &self,
        contracts: &[InternalContractRecord],
    ) -> Result<usize> {
        self.ensure_internal_webstore_schema().await?;
        let mut sql = String::from("BEGIN TRANSACTION;\n");
        for c in contracts {
            let rk = sanitize_key(&c.contract_id);
            sql.push_str(&format!(
                "UPSERT type::record('internal_contract_ledger', {rk}) SET \
                    contract_id = {cid}, \
                    contract_type = {ctype}, \
                    partner_name = {pname}, \
                    document_ref = {dref}, \
                    warehouse_or_campaign = {woc}, \
                    monthly_fixed_cost_eur = {fcost}, \
                    sla_threshold_desc = {sla}, \
                    bonus_or_penalty_clause = {bop}, \
                    current_delay_days = {dly}, \
                    compliance_status = {st}, \
                    updated_at = time::now();\n",
                rk = serde_json::to_string(&rk)?,
                cid = serde_json::to_string(&c.contract_id)?,
                ctype = serde_json::to_string(&c.contract_type)?,
                pname = serde_json::to_string(&c.partner_name)?,
                dref = serde_json::to_string(&c.document_ref)?,
                woc = serde_json::to_string(&c.warehouse_or_campaign)?,
                fcost = c.monthly_fixed_cost_eur,
                sla = serde_json::to_string(&c.sla_threshold_desc)?,
                bop = serde_json::to_string(&c.bonus_or_penalty_clause)?,
                dly = c.current_delay_days,
                st = serde_json::to_string(&c.compliance_status)?,
            ));
        }
        sql.push_str("COMMIT TRANSACTION;\n");
        self.execute_sql(&sql).await?;
        Ok(contracts.len())
    }

    /// Ensures `shop_product` and `shop_sale` exist in the EXTERNAL Webstore database (`webstore_external / shop_erp`).
    pub async fn ensure_external_webstore_schema(&self) -> Result<()> {
        let sql = "
            DEFINE TABLE IF NOT EXISTS shop_product SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS sku                      ON TABLE shop_product TYPE string;
            DEFINE FIELD IF NOT EXISTS product_name             ON TABLE shop_product TYPE string;
            DEFINE FIELD IF NOT EXISTS category                 ON TABLE shop_product TYPE string;
            DEFINE FIELD IF NOT EXISTS supplier_name            ON TABLE shop_product TYPE string;
            DEFINE FIELD IF NOT EXISTS purchase_price_eur       ON TABLE shop_product TYPE float;
            DEFINE FIELD IF NOT EXISTS retail_price_eur         ON TABLE shop_product TYPE float;
            DEFINE FIELD IF NOT EXISTS margin_eur               ON TABLE shop_product TYPE float;
            DEFINE FIELD IF NOT EXISTS stock_units              ON TABLE shop_product TYPE int;
            DEFINE FIELD IF NOT EXISTS min_stock_sla            ON TABLE shop_product TYPE int;
            DEFINE FIELD IF NOT EXISTS occupied_pallets         ON TABLE shop_product TYPE int;
            DEFINE FIELD IF NOT EXISTS warehouse_location       ON TABLE shop_product TYPE string;
            DEFINE FIELD IF NOT EXISTS contains_lithium_battery ON TABLE shop_product TYPE bool DEFAULT false;
            DEFINE FIELD IF NOT EXISTS return_rate_percent      ON TABLE shop_product TYPE float;
            DEFINE FIELD IF NOT EXISTS updated_at               ON TABLE shop_product TYPE datetime DEFAULT time::now();
            DEFINE INDEX IF NOT EXISTS idx_shop_sku             ON TABLE shop_product FIELDS sku UNIQUE;

            DEFINE TABLE IF NOT EXISTS shop_sale SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS order_id           ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS order_date         ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS month              ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS sku                ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS product_name       ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS category           ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS supplier_name      ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS quantity           ON TABLE shop_sale TYPE int;
            DEFINE FIELD IF NOT EXISTS unit_price_eur     ON TABLE shop_sale TYPE float;
            DEFINE FIELD IF NOT EXISTS purchase_cost_eur  ON TABLE shop_sale TYPE float;
            DEFINE FIELD IF NOT EXISTS revenue_eur        ON TABLE shop_sale TYPE float;
            DEFINE FIELD IF NOT EXISTS profit_eur         ON TABLE shop_sale TYPE float;
            DEFINE FIELD IF NOT EXISTS discount_code      ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS marketing_campaign ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS customer_region    ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS order_status       ON TABLE shop_sale TYPE string;
            DEFINE FIELD IF NOT EXISTS updated_at         ON TABLE shop_sale TYPE datetime DEFAULT time::now();
            DEFINE INDEX IF NOT EXISTS idx_shop_order_id  ON TABLE shop_sale FIELDS order_id UNIQUE;
        ";
        self.execute_sql(sql).await?;
        Ok(())
    }

    /// Upserts `shop_product` and `shop_sale` into the EXTERNAL Webstore database.
    pub async fn upsert_external_webstore_data(
        &self,
        products: &[ShopProductRecord],
        sales: &[ShopSaleRecord],
    ) -> Result<(usize, usize)> {
        self.ensure_external_webstore_schema().await?;

        let mut prod_sql = String::from("BEGIN TRANSACTION;\n");
        for p in products {
            let rk = sanitize_key(&p.sku);
            prod_sql.push_str(&format!(
                "UPSERT type::record('shop_product', {rk}) SET \
                    sku = {sku}, \
                    product_name = {pname}, \
                    category = {cat}, \
                    supplier_name = {sup}, \
                    purchase_price_eur = {pp}, \
                    retail_price_eur = {rp}, \
                    margin_eur = {mrg}, \
                    stock_units = {stk}, \
                    min_stock_sla = {msla}, \
                    occupied_pallets = {pal}, \
                    warehouse_location = {wh}, \
                    contains_lithium_battery = {bat}, \
                    return_rate_percent = {ret}, \
                    updated_at = time::now();\n",
                rk = serde_json::to_string(&rk)?,
                sku = serde_json::to_string(&p.sku)?,
                pname = serde_json::to_string(&p.product_name)?,
                cat = serde_json::to_string(&p.category)?,
                sup = serde_json::to_string(&p.supplier_name)?,
                pp = p.purchase_price_eur,
                rp = p.retail_price_eur,
                mrg = p.margin_eur,
                stk = p.stock_units,
                msla = p.min_stock_sla,
                pal = p.occupied_pallets,
                wh = serde_json::to_string(&p.warehouse_location)?,
                bat = p.contains_lithium_battery,
                ret = p.return_rate_percent,
            ));
        }
        prod_sql.push_str("COMMIT TRANSACTION;\n");
        self.execute_sql(&prod_sql).await?;

        for batch in sales.chunks(100) {
            let mut sale_sql = String::from("BEGIN TRANSACTION;\n");
            for s in batch {
                let rk = sanitize_key(&s.order_id);
                sale_sql.push_str(&format!(
                    "UPSERT type::record('shop_sale', {rk}) SET \
                        order_id = {oid}, \
                        order_date = {odt}, \
                        month = {mon}, \
                        sku = {sku}, \
                        product_name = {pname}, \
                        category = {cat}, \
                        supplier_name = {sup}, \
                        quantity = {qty}, \
                        unit_price_eur = {up}, \
                        purchase_cost_eur = {pc}, \
                        revenue_eur = {rev}, \
                        profit_eur = {prf}, \
                        discount_code = {dc}, \
                        marketing_campaign = {mc}, \
                        customer_region = {reg}, \
                        order_status = {st}, \
                        updated_at = time::now();\n",
                    rk = serde_json::to_string(&rk)?,
                    oid = serde_json::to_string(&s.order_id)?,
                    odt = serde_json::to_string(&s.order_date)?,
                    mon = serde_json::to_string(&s.month)?,
                    sku = serde_json::to_string(&s.sku)?,
                    pname = serde_json::to_string(&s.product_name)?,
                    cat = serde_json::to_string(&s.category)?,
                    sup = serde_json::to_string(&s.supplier_name)?,
                    qty = s.quantity,
                    up = s.unit_price_eur,
                    pc = s.purchase_cost_eur,
                    rev = s.revenue_eur,
                    prf = s.profit_eur,
                    dc = serde_json::to_string(&s.discount_code)?,
                    mc = serde_json::to_string(&s.marketing_campaign)?,
                    reg = serde_json::to_string(&s.customer_region)?,
                    st = serde_json::to_string(&s.order_status)?,
                ));
            }
            sale_sql.push_str("COMMIT TRANSACTION;\n");
            self.execute_sql(&sale_sql).await?;
        }

        Ok((products.len(), sales.len()))
    }
}
