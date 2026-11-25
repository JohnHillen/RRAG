# 🔌 How-To Guide: Connecting an Existing SQL or Cloud Database (PostgreSQL, MySQL, AWS RDS, Azure, SQLite, SurrealDB)

Because the orchestrator includes a **Universal Multi-Engine External Database Connector** ([`./rust-rag-orchestrator/src/db/external.rs`](./rust-rag-orchestrator/src/db/external.rs)) and a **Dynamic Text-to-SQL Engine** ([`./rust-rag-orchestrator/src/retrieval/text_to_sql.rs`](./rust-rag-orchestrator/src/retrieval/text_to_sql.rs)), your external company database **does not need to be SurrealDB**.

Only the **internal RAG & GraphRAG knowledge base** (storing PDF chunks, 1024d HNSW vectors, BM25 full-text indexes, and Knowledge Graph relations) runs on the bundled SurrealDB container. Your **attached external database** can be any standard relational SQL database or managed cloud database on **AWS**, **Microsoft Azure**, or **Google Cloud**.

---

## 🏛️ 1. Architecture: How the Internal RAG DB and Your External SQL DB Work Together

When you attach an existing company database via `EXTERNAL_DB_URL`, the pipeline queries **both databases** automatically:

```text
User Question
   │
   ├─► 1. INTERNAL SURREALDB (Hop 1 — Unstructured Knowledge Base, Always Active)
   │      • Searches uploaded PDFs, contracts, SLAs, tax laws & manuals (HNSW Vector + BM25 + 2-Hop GraphRAG)
   │      • Produces Document Anchor (Source #1)
   │
   ├─► 2A. INTERNAL SURREALDB TABLES (Hop 2A — Optional Internal / Showcase Tables)
   │      • Generates read-only SurrealQL `SELECT` against internal tables -> Markdown Table (Source #2)
   │
   └─► 2B. ATTACHED EXTERNAL SQL / CLOUD DATABASE (Hop 2B — via `EXTERNAL_DB_URL`)
          • Automatically detects the SQL dialect from `EXTERNAL_DB_URL`:
            - `postgres://` or `postgresql://` -> Standard PostgreSQL (AWS RDS, Aurora, Azure Postgres, GCP, Supabase)
            - `mysql://` or `mariadb://`       -> Standard MySQL / MariaDB (AWS RDS, Azure MySQL)
            - `sqlite://`                      -> Standard SQLite
            - `http://` or `https://`          -> External SurrealDB instance
          • Passes Question + Hop-1 PDF Anchor + External Schema (`EXTERNAL_DB_SCHEMA_FILE`) to the LLM
          • Executes read-only `SELECT` on your external database -> Markdown Table (Source #3)
          • (Outputs `NONE` in ~0.5s if the user's question is unrelated to the external database)
```

---

## 🚀 2. Zero-Code Setup via `.env` (`EXTERNAL_DB_URL`)

To connect an existing database without writing any Rust code, you only need two things:
1. A plain-text **Schema Description File** (e.g., `./sql/external_schema.txt`) listing the tables and columns the LLM is allowed to query.
2. The connection string in **`EXTERNAL_DB_URL`** inside `.env`.

### Step 1: Create Your Schema Description File (`./sql/external_schema.txt`)
Describe your existing SQL tables and columns in plain text:

```text
1) TABLE `customer_orders` (ERP Customer Orders & Invoices):
   - `order_id` (varchar): Unique order number, e.g. 'ORD-2026-1001'
   - `customer_name` (varchar): Company name, e.g. 'Siemens AG', 'BMW AG'
   - `order_date` (date): ISO date 'YYYY-MM-DD'
   - `status` (varchar): Exact values: 'OPEN', 'SHIPPED', 'DELAYED', 'CANCELLED'
   - `total_eur` (numeric): Order value in EUR
   - `items_count` (int): Number of line items

2) TABLE `inventory_items` (Warehouse Stock Levels):
   - `sku` (varchar): Product SKU
   - `product_name` (varchar): Product description
   - `stock_qty` (int): Available quantity in warehouse
   - `reorder_threshold` (int): Minimum stock before reorder is required

EXAMPLES:
- Top customers sorted descending by order volume:
  `SELECT customer_name, SUM(total_eur) AS revenue_eur, COUNT(*) AS orders FROM customer_orders GROUP BY customer_name ORDER BY revenue_eur DESC LIMIT 20;`
- Delayed orders above 10,000 EUR:
  `SELECT order_id, customer_name, order_date, total_eur, status FROM customer_orders WHERE status = 'DELAYED' AND total_eur > 10000 ORDER BY total_eur DESC LIMIT 30;`
```

### Step 2: Set `EXTERNAL_DB_URL` in `.env` (Choose Your Database / Cloud Provider)

#### Option A: On-Premise / Self-Hosted PostgreSQL
```env
EXTERNAL_DB_URL=postgres://readonly_user:password@host.docker.internal:5432/erp_production
EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt
```

#### Option B: AWS (Amazon RDS for PostgreSQL / Amazon Aurora / Amazon Redshift)
```env
EXTERNAL_DB_URL=postgres://readonly_user:password@mydb.c9akciq32.eu-central-1.rds.amazonaws.com:5432/erp_production?sslmode=require
EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt
```

#### Option C: Microsoft Azure (Azure Database for PostgreSQL Flexible Server)
```env
EXTERNAL_DB_URL=postgres://readonly_user:password@my-company-db.postgres.database.azure.com:5432/erp_production?sslmode=require
EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt
```

#### Option D: MySQL / MariaDB (On-Premise, AWS RDS MySQL, or Azure Database for MySQL)
```env
EXTERNAL_DB_URL=mysql://readonly_user:password@my-mysql-db.mysql.database.azure.com:3306/shop_production
EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt
```

#### Option E: Local / Embedded SQLite Database
```env
EXTERNAL_DB_URL=sqlite:///data/company_erp.db
EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt
```

#### Option F: External SurrealDB Instance
```env
EXTERNAL_DB_URL=http://host.docker.internal:8001
EXTERNAL_SURREAL_USER=readonly_user
EXTERNAL_SURREAL_PASS=readonly_secret
EXTERNAL_SURREAL_NS=erp_namespace
EXTERNAL_SURREAL_DB=production_erp
EXTERNAL_DB_SCHEMA_FILE=./sql/external_schema.txt
```

---

## 🔒 3. Enterprise Security Best Practices (Defense-in-Depth)

When connecting a production company database (on-premise, AWS RDS, or Azure):

1. **Orchestrator Read-Only Guardrail ([`./rust-rag-orchestrator/src/retrieval/text_to_sql.rs`](./rust-rag-orchestrator/src/retrieval/text_to_sql.rs))**:
   - The Rust orchestrator strictly validates every LLM-generated statement before execution.
   - Only statements starting with `SELECT ` are allowed; any statement containing `DELETE`, `UPDATE`, `INSERT`, `CREATE`, `UPSERT`, `ALTER`, `DROP`, `TRUNCATE`, `GRANT`, or `REVOKE` is rejected immediately.
2. **Database-Level Read-Only User (Recommended)**:
   Always create a dedicated read-only database user in PostgreSQL / MySQL / AWS RDS / Azure for `EXTERNAL_DB_URL`:
   ```sql
   -- PostgreSQL / AWS RDS / Azure Database for PostgreSQL:
   CREATE USER rag_readonly WITH PASSWORD 'strong_password';
   GRANT CONNECT ON DATABASE erp_production TO rag_readonly;
   GRANT USAGE ON SCHEMA public TO rag_readonly;
   GRANT SELECT ON ALL TABLES IN SCHEMA public TO rag_readonly;
   ```

---

## 🎯 4. Built-in Multi-DB Reference Implementation: Showcase 03 (`Showcase_03_Webstore`)

To see an end-to-end working example of the **Internal Database + External Database** architecture in action, inspect **[`./Showcase/Showcase_03_Webstore/`](./Showcase/Showcase_03_Webstore/SHOWCASE.md)**:

- **Internal Database (`rag_namespace / knowledge_base`)**:
  Holds 6 unstructured PDF contracts (supplier SLAs, marketing agency agreements, warehouse leases) + the internal contract table `internal_contract_ledger`.
- **External Database (`webstore_external / shop_erp` or any `EXTERNAL_DB_URL`)**:
  Holds the webstore product catalog (`shop_product`) and sales history (`shop_sale`).
- Run it anytime with:
  ```bash
  ./setup.sh --showcase 3
  ```

---

## 💡 5. Tips for Writing High-Accuracy Schema Prompts

To get 100% deterministic SQL queries from both local 7B–14B models (`qwen2.5:7b`, `deepseek-r1:14b`, `llama3.1:8b`) and cloud models (`GPT-4o`, `Gemini 2.5`, `Claude 3.7`):

1. **List Exact Enum / Status Values**:
   Instead of writing `- status (varchar): Order status`, write:
   `- status (varchar): Exact values are 'OPEN', 'SHIPPED', 'DELAYED', 'CANCELLED'`.
2. **Clarify Ambiguous Columns**:
   If a table has both a lifetime counter and a monthly counter, add a short note:
   `- mileage_km (int): Total lifetime odometer reading (NOT monthly tour distance!)`.
3. **Include 2–3 Representative Query Examples**:
   Adding 2–3 one-line example `SELECT` queries at the bottom of your schema prompt (covering `GROUP BY`, `ORDER BY ... DESC`, and `WHERE`) anchors the LLM to your exact table and column names on the first try.

---

## ⚡ 6. High-Speed Batch Embedding via RunPod GPU over Encrypted SSH Tunnel (No Port 80 / No DB Export Needed)

When indexing large document collections (such as dozens of multi-megabyte XML/PDF statutes), you can offload the `BAAI/bge-m3` vector embedding calculation to a cloud GPU (e.g. **NVIDIA A40** or **RTX 4090** on RunPod) over an **encrypted SSH tunnel** while storing all chunks, vectors, and Knowledge Graph relations **directly into your local SurrealDB instance on your Mac**.

- **Zero public HTTP/HTTPS port exposure:** Port 80 / 8081 does not need to be exposed publicly on RunPod; all traffic flows through SSH (`-L 0.0.0.0:18081:127.0.0.1:8081`).
- **Zero database export/import:** Your local `rag-orchestrator` sends text batches through the local SSH tunnel to the RunPod GPU and writes the returned 1024-dimensional `BAAI/bge-m3` vectors directly into your local `rag-surrealdb` container.
- **No port conflict on macOS:** Using local tunnel port `18081` leaves your local Metal TEI server on port `8081` untouched.

### Architecture Overview

```text
Local Mac (Browser UI :8080)
   │
   ├─► rag-orchestrator (Docker Container)
   │      │
   │      ├─► 1. Sends text chunks to http://host.docker.internal:18081/embed
   │      │         │
   │      │         ▼  (Encrypted SSH Tunnel: 0.0.0.0:18081 -> RunPod 127.0.0.1:8081)
   │      │      RunPod GPU (NVIDIA A40 / RTX 4090)
   │      │      text-embeddings-router (BAAI/bge-m3, FlashAttention-2, CUDA FP16)
   │      │
   │      └─► 2. Saves returned 1024d vectors + Graph edges directly into local SurrealDB (:8000)
```

### Option A (Recommended): 1-Command Automation via RunPod API (`./runpod_embed.sh`)

Instead of clicking through the RunPod web console manually, you can use the bundled **[`./runpod_embed.sh`](./runpod_embed.sh)** script, which talks directly to the **RunPod GraphQL API**:

1. Create a RunPod API key in your RunPod settings (**Settings -> API Keys**) and make sure your SSH public key (`~/.ssh/id_ed25519.pub` or `~/.ssh/id_rsa.pub`) is added under **Settings -> SSH Public Keys**.
2. Set `RUNPOD_API_KEY` in your local `.env`:
   ```env
   RUNPOD_API_KEY=rpa_your_api_key_here
   RUNPOD_GPU_TYPE="NVIDIA A40"
   ```
3. Start the GPU Pod + encrypted SSH tunnel + orchestrator switch with a single command:
   ```bash
   ./runpod_embed.sh start
   ```
   What `./runpod_embed.sh start` does automatically:
   - Calls the RunPod API to deploy an on-demand **NVIDIA A40** (or fallback **RTX 4090**) Pod with SSH (`22/tcp`) and launches `BAAI/bge-m3` in FP16 on `127.0.0.1:8081` inside the Pod.
   - Polls the RunPod API until the Pod's public SSH IP and TCP port are ready.
   - Opens the background SSH tunnel (`0.0.0.0:18081 -> 127.0.0.1:8081`) and waits until `http://localhost:18081/info` reports `"BAAI/bge-m3"` online.
   - Updates `TEI_EMBED_URL=http://host.docker.internal:18081` in `.env` and restarts `rag-orchestrator`.
4. Upload your files in `http://localhost:8080`.
5. When finished, terminate the RunPod Pod, close the SSH tunnel, and switch back to your local Mac with one command:
   ```bash
   ./runpod_embed.sh stop
   ```

---

### Option B: Manual Setup via RunPod Web UI

#### Step 1: Launch a GPU Pod on RunPod (Keep the Default `PyTorch - 2.8.0` Template!)

In the RunPod **"Deploy a Pod"** screen:
1. **Workload:** Leave the default **`PyTorch - 2.8.0`** template selected as-is (do **not** replace the container image via `✏️ Set overrides`, because `PyTorch - 2.8.0` includes the `openssh-server` daemon required for SSH port forwarding plus CUDA 12.8 & PyTorch 2.8 pre-installed).
2. **Compute:** Select an **NVIDIA A40**, **RTX 4090**, **L40S**, or **RTX A5000** and click **Deploy**.

Once the Pod is running, connect to it via **SSH over exposed TCP** (`ssh root@<RUNPOD_PUBLIC_IP> -p <RUNPOD_TCP_PORT> -i ~/.ssh/id_ed25519`) and run this single command inside the RunPod terminal to start the `BAAI/bge-m3` FP16 CUDA Embedding Server on `127.0.0.1:8081`:

```bash
pip install -q sentence-transformers fastapi uvicorn && python3 - << 'EOF'
import torch, uvicorn
from fastapi import FastAPI, Request
from sentence_transformers import SentenceTransformer

print("Loading BAAI/bge-m3 on CUDA GPU (FP16)...")
model = SentenceTransformer("BAAI/bge-m3", device="cuda", model_kwargs={"torch_dtype": torch.float16})
model.max_seq_length = 8192
app = FastAPI()

@app.get("/info")
async def info():
    return {"model_id": "BAAI/bge-m3", "device": torch.cuda.get_device_name(0), "dtype": "float16"}

@app.post("/embed")
async def embed(req: Request):
    data = await req.json()
    inputs = data.get("inputs", [])
    if isinstance(inputs, str):
        inputs = [inputs]
    vecs = model.encode(inputs, batch_size=64, normalize_embeddings=True, show_progress_bar=False)
    return vecs.tolist()

uvicorn.run(app, host="127.0.0.1", port=8081, log_level="info")
EOF
```

### Step 2: Open the SSH Tunnel on Your Mac

Copy the **Direct SSH command** from your RunPod dashboard (e.g. `ssh root@<RUNPOD_PUBLIC_IP> -p <RUNPOD_TCP_PORT> -i ~/.ssh/id_ed25519` — note: use the **Direct TCP IP & Port** shown in RunPod under *"SSH over exposed TCP"*, not the `ssh.runpod.io` proxy, because `ssh.runpod.io` disables `-L` port forwarding!).

Run this in a terminal tab on your Mac:

```bash
# Bind 0.0.0.0:18081 on your Mac so Docker (host.docker.internal:18081) can reach the tunnel:
ssh -N -L 0.0.0.0:18081:127.0.0.1:8081 root@<RUNPOD_PUBLIC_IP> -p <RUNPOD_TCP_PORT> -i ~/.ssh/id_ed25519
```

Test the tunnel from a second terminal tab on your Mac:
```bash
curl -s http://localhost:18081/info
```

### Step 3: Switch `TEI_EMBED_URL` in `.env` to Port `18081`

In your local `.env` file on the Mac, comment out port `8081` and enable port `18081`:

```env
# TEI_EMBED_URL=http://host.docker.internal:8081
TEI_EMBED_URL=http://host.docker.internal:18081
```

Apply the new environment variable to the running orchestrator container (takes ~1 second, SurrealDB data is preserved):

```bash
docker compose up -d rag-orchestrator
```

### Step 4: Upload Your Files in the Local Browser UI (`http://localhost:8080`)

1. Open `http://localhost:8080`, click **`➕ Upload`**, and select your folder (e.g. `Gesetze`).
2. Every file already indexed earlier is automatically skipped in `1 ms` (`⏭️ skipped_duplicate`), and all remaining files are embedded on the RunPod A40/4090 GPU in a few seconds and stored directly in your local SurrealDB on your Mac.

### Step 5: Switch Back to Local Mac Embeddings & Terminate the RunPod Pod

Once the upload finishes:
1. Change `TEI_EMBED_URL` in `.env` back to your local Mac Metal server:
   ```env
   TEI_EMBED_URL=http://host.docker.internal:8081
   ```
2. Run:
   ```bash
   docker compose up -d rag-orchestrator
   ```
3. Close the SSH tunnel (`Ctrl + C`) and terminate the RunPod GPU Pod. All vectors and Knowledge Graph relations remain permanently stored in your local SurrealDB on your Mac!
