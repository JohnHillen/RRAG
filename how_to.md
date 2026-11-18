# 🔌 How-To Guide: Connecting an Existing Database to the Rust RAG & Text-to-SQL Pipeline

Because the orchestrator uses a **Dynamic Text-to-SQL / Text-to-SurrealQL Engine** ([`./rust-rag-orchestrator/src/retrieval/text_to_sql.rs`](./rust-rag-orchestrator/src/retrieval/text_to_sql.rs) and [`./rust-rag-orchestrator/src/retrieval/sql_formatter.rs`](./rust-rag-orchestrator/src/retrieval/sql_formatter.rs)), you do **not** need to write custom Rust filter functions or static table formatters for every possible user question.

---

## 🏛️ Architecture: How Two Databases Work Together (Internal + Attached Database)

When you attach an existing company database alongside the RAG pipeline, the system operates across **both databases** automatically:

```text
User Question
   │
   ├─► 1. INTERNAL SURREALDB (Hop 1 — Always Active)
   │      • Searches uploaded PDFs, contracts, SLAs & manuals via HNSW Vector + BM25 + 2-Hop GraphRAG
   │      • Produces Document Anchor (Source #1)
   │
   ├─► 2A. INTERNAL SURREALDB TABLES (Hop 2 — Active Showcase / Internal SQL)
   │      • Passes Question + Hop-1 Anchor + Internal Schema Prompt to LLM
   │      • Executes read-only SELECT on internal SurrealDB -> Markdown Table (Source #2)
   │      • (Outputs `NONE` if the question is unrelated to internal tables)
   │
   └─► 2B. ATTACHED EXTERNAL DATABASE (Hop 2 — External Live DB via `EXTERNAL_SURREAL_URL`)
          • Passes Question + Hop-1 Anchor + External Schema Prompt (`EXTERNAL_DB_SCHEMA_FILE`) to LLM
          • Executes read-only SELECT on the attached database -> Markdown Table (Source #3)
          • (Outputs `NONE` if the question is unrelated to external tables)
```

- **Are both databases considered on a Text-to-SQL query?**
  **Yes.** When an external database schema is configured (`EXTERNAL_DB_SCHEMA_FILE` or `EXTERNAL_DB_SCHEMA_PROMPT`), [`./rust-rag-orchestrator/src/api/showcases.rs`](./rust-rag-orchestrator/src/api/showcases.rs) evaluates **both** the internal database schema and the external database schema:
  - If a question only concerns internal tables, the external check returns `NONE` in ~0.5s.
  - If a question only concerns the attached external database, only the external database returns a table.
  - If a question spans **both** databases (plus an unstructured PDF contract from Hop 1), all three results (`Source #1` PDF + `Source #2` Internal DB + `Source #3` External DB) are merged into the LLM context window so the assistant can cross-reference them in a single answer.

---

## Scenario A: Attaching a Second SurrealDB Instance via `.env` (Zero Code Changes)

If you have an existing **SurrealDB** instance (e.g., an ERP/CRM/Telemetry database on another port, server, or cloud instance) and want to keep the bundled `rag-surrealdb` container for your PDF embeddings and Knowledge Graph:

### 1. Create a Schema Description File (`./sql/external_schema.txt`)
Describe the tables and columns of your existing database in plain English/German:

```text
1) TABLE `customer_order` (Customer Orders & Invoices):
   - `order_id` (string): Unique order number, e.g. 'ORD-2026-1001'
   - `customer_name` (string): Company name, e.g. 'Siemens AG', 'BMW AG', 'BioPharma Berlin GmbH'
   - `order_date` (string): ISO date 'YYYY-MM-DD'
   - `status` (string): Exact values: 'OPEN', 'SHIPPED', 'DELAYED', 'CANCELLED'
   - `total_eur` (float): Order value in EUR
   - `items_count` (int): Number of line items

2) TABLE `inventory_item` (Warehouse Stock Levels):
   - `sku` (string): Product SKU
   - `product_name` (string): Product description
   - `stock_qty` (int): Available quantity in warehouse
   - `reorder_threshold` (int): Minimum stock before reorder is required

EXAMPLES:
- Top customers sorted descending by order volume:
  `SELECT customer_name, math::sum(total_eur) AS revenue_eur, count() AS orders FROM customer_order GROUP BY customer_name ORDER BY revenue_eur DESC LIMIT 20;`
- Delayed orders above 10,000 EUR:
  `SELECT order_id, customer_name, order_date, total_eur, status FROM customer_order WHERE status = 'DELAYED' AND total_eur > 10000 ORDER BY total_eur DESC;`
```

### 2. Configure `EXTERNAL_SURREAL_*` in `.env`
Add the connection details for your attached database to `.env`:

```env
# Primary RAG & GraphRAG Database (Bundled Docker SurrealDB)
SURREAL_URL=http://surrealdb:8000
SURREAL_USER=root
SURREAL_PASS=root_secret_password_change_me
SURREAL_NS=rag_namespace
SURREAL_DB=knowledge_base

# Attached External Database (Queried automatically via Dynamic Text-to-SurrealQL)
EXTERNAL_SURREAL_URL=http://host.docker.internal:8001
EXTERNAL_SURREAL_USER=readonly_user
EXTERNAL_SURREAL_PASS=readonly_secret
EXTERNAL_SURREAL_NS=erp_namespace
EXTERNAL_SURREAL_DB=production_erp
EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt
```

*(Note: If your external tables live inside the **same** SurrealDB instance as the RAG database, simply omit `EXTERNAL_SURREAL_URL` and only set `EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt`.)*

---

## Scenario B: Adding a Custom Domain Module in Rust

If you want to bundle a custom database schema together with custom REST endpoints, seed generators, or MCP tools (just like `Showcase_01_HR_Consulting` or `Showcase_02_Spedition`):

### Step 1: Define Your Schema Prompt
Create `./Showcase/Custom_ERP/src/mod.rs` and call `execute_dynamic_text_to_sql_on_client`:

```rust
use crate::{
    db::RetrievedChunk,
    text_to_sql::execute_dynamic_text_to_sql_on_client,
    AppState,
};

pub const CUSTOM_SCHEMA_PROMPT: &str = r#"
TABLE `customer_order` (Customer Orders):
- `order_id` (string)
- `customer_name` (string)
- `status` (string): 'OPEN', 'SHIPPED', 'DELAYED'
- `total_eur` (float)
"#;

pub async fn augment_retrieval(
    state: &AppState,
    query: &str,
    mut ranked_pool: Vec<RetrievedChunk>,
    top_n: usize,
    db_ms: &mut u64,
    reranker_used: &mut String,
) -> Vec<RetrievedChunk> {
    let primary_anchor = ranked_pool.first().cloned();
    ranked_pool.truncate(top_n);
    let mut final_chunks = ranked_pool;

    let target_client = state.external_db.as_ref().unwrap_or(&state.db);
    let t_sql = std::time::Instant::now();
    if let Some(sql_chunk) = execute_dynamic_text_to_sql_on_client(
        state,
        target_client,
        query,
        primary_anchor.as_ref(),
        CUSTOM_SCHEMA_PROMPT,
        "erp_sql_live_query_result",
        "erp_database_tables",
        "🗄️ [ERP-DB Text-to-SQL]",
    )
    .await
    {
        *db_ms += t_sql.elapsed().as_millis() as u64;
        let insert_idx = if final_chunks.is_empty() { 0 } else { 1 };
        final_chunks.insert(insert_idx, sql_chunk);
        *reranker_used = format!("{} + 🧠 Text-to-SurrealQL (ERP-DB)", reranker_used);
    }

    final_chunks
}
```

### What `execute_dynamic_text_to_sql_on_client` Handles Automatically:
- **Multi-Hop Context Bridging**: If a user asks *"Which orders violate the delivery SLA agreed with Siemens?"*, Hop 1 retrieves the Siemens PDF contract from the primary RAG database, and `execute_dynamic_text_to_sql_on_client` receives that contract excerpt so it can translate the SLA threshold directly into a `WHERE` clause against the external database.
- **Read-Only Security Guardrail**: Rejects any statement containing `DELETE`, `UPDATE`, `INSERT`, `CREATE`, `UPSERT`, `REMOVE`, `DEFINE`, `RELATE`, `ALTER`, or `DROP`.
- **SQL-to-SurrealQL Normalization**: Automatically converts standard SQL aggregates (`SUM(x)` -> `math::sum(x)`, `AVG(x)` -> `math::mean(x)`, `MIN(x)` -> `math::min(x)`, `MAX(x)` -> `math::max(x)`, `COUNT(*)` -> `count()`).
- **Automatic Self-Correction**: If the database reports a syntax error on the first attempt, the error message is fed back to the LLM to repair the query automatically.
- **Generic Markdown Table Rendering**: Formats any returned rows into clean Markdown tables via [`./rust-rag-orchestrator/src/retrieval/sql_formatter.rs`](./rust-rag-orchestrator/src/retrieval/sql_formatter.rs).

---

## Scenario C: Connecting an External PostgreSQL, MySQL, or SQLite Database

If your existing structured data lives in **PostgreSQL**, **MySQL / MariaDB**, **SQLite**, or **MS SQL Server**, you can choose between two patterns:

### Pattern C1: Sync / Import Tables into SurrealDB
If you want a single unified query syntax (SurrealQL) and easy 1-file snapshots (`./setup.sh --export`), periodically sync rows from PostgreSQL/MySQL/CSV into SurrealDB via SurrealDB's HTTP `/sql` endpoint:

```bash
curl -X POST http://localhost:8000/sql \
  -u "root:root_secret_password_change_me" \
  -H "Accept: application/json" \
  -H "surreal-ns: rag_namespace" \
  -H "surreal-db: knowledge_base" \
  -d "
    BEGIN TRANSACTION;
    UPSERT customer_order:ord_1001 SET
      order_id = 'ORD-2026-1001',
      customer_name = 'Siemens AG',
      order_date = '2026-04-12',
      status = 'DELAYED',
      total_eur = 48500.0,
      items_count = 14;
    COMMIT TRANSACTION;
  "
```

### Pattern C2: Querying PostgreSQL / MySQL Live via `sqlx`
If your data must remain exclusively in **PostgreSQL** or **MySQL** and be queried live on every chat question:

1. **Add `sqlx` to [`./rust-rag-orchestrator/Cargo.toml`](./rust-rag-orchestrator/Cargo.toml)**:
   ```toml
   sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "json"] }
   ```
2. **Add `EXTERNAL_POSTGRES_URL` to `.env` using a Read-Only Database Role**:
   ```env
   EXTERNAL_POSTGRES_URL=postgres://readonly_user:password@host.docker.internal:5432/production_db
   ```
3. **Execute the Generated `SELECT` Query and Pass JSON Rows to `format_sql_results_as_chunk`**:
   Wrap the validated `SELECT` query in `SELECT COALESCE(json_agg(t), '[]'::json) AS result FROM (<select_query>) t;` using `sqlx::query_scalar` and pass the resulting JSON array directly into `format_sql_results_as_chunk` in [`./rust-rag-orchestrator/src/sql_formatter.rs`](./rust-rag-orchestrator/src/sql_formatter.rs). Both the primary SurrealDB (Hop 1 PDFs + Hop 2 SurrealDB tables) and PostgreSQL (Hop 2 external tables) will then be queried and formatted identically.

---

## 4. Tips for Writing High-Accuracy Schema Prompts

To get 100% deterministic SQL queries from local 7B–14B models (`qwen2.5:7b`, `deepseek-r1:14b`, `llama3.1:8b`) as well as cloud models:

1. **List Exact Enum / Status Values**:
   Instead of writing `- status (string): Order status`, write:
   `- status (string): Exact values are 'OPEN', 'SHIPPED', 'DELAYED', 'CANCELLED'`.
2. **Clarify Ambiguous Columns**:
   If a table has both a lifetime counter and a monthly counter (like `fleet_vehicle.mileage_km` vs. `fleet_tour.distance_km`), add a short note:
   `- mileage_km (int): Total lifetime odometer reading (NOT monthly tour distance!)`.
3. **Include 2–3 Representative Query Examples**:
   Adding 2–3 one-line example `SELECT` queries at the bottom of your schema prompt (covering `GROUP BY`, `ORDER BY ... DESC`, and `WHERE`) anchors the LLM to your exact table and column names on the first try.

---

## 5. Testing Your Database Integration

Once your schema prompt is active, test the generated SQL and retrieval output directly via `POST /v1/retrieve` before testing in the Chat UI:

```bash
curl -s -X POST http://localhost:8080/v1/retrieve \
  -H "Content-Type: application/json" \
  -d '{
    "query": "List all customers sorted descending by their total order volume.",
    "top_n": 4,
    "reranker": "fast"
  }' | jq .
```

Look for the chunks with `"doc_id": "..._sql_live_query_result"` or `"doc_id": "external_attached_database"` in the JSON response:
- Their headers display the exact `SELECT` statements generated by the LLM.
- Their bodies contain the deterministic Markdown tables returned by each database.
