# 🦀 100% Rust RAG, GraphRAG & Multi-Project Knowledge Studio (`SurrealDB 4.0` + `Hugging Face TEI` + `Axum` + `Leptos WASM`)

A self-hosted, enterprise-grade **RAG, GraphRAG, Financial Aggregation & Dynamic Text-to-SurrealQL platform** written **100% in Rust** (`SurrealDB 4.0`, `Hugging Face TEI`, `Axum`, and a `Leptos WebAssembly` Chat & Knowledge Studio UI).

All day-to-day configuration—**LLM providers, RAG parameters, Multi-Project SurrealDB workspaces, `.surql` database backups, and On-Demand RunPod Cloud GPUs**—is managed **live inside the Chat UI (`http://localhost:8080/`)** without editing `.env` files or restarting containers.

---

## ✨ Key Capabilities

### 1. Live In-App Configuration (`⚙️ Settings` in the Chat UI)
Once the stack is running, all settings are controlled directly inside the web interface and persisted automatically:

- **🔍 RAG & Retrieval**:
  - Toggle Hybrid Search (`HNSW Vector + BM25 Full-Text + Reciprocal Rank Fusion`), select the Cross-Encoder Reranker (`⚡ Fast mmarco-mMiniLMv2`, `🎯 High Precision bge-reranker-v2-m3`, or `🚫 Off`), and adjust **Top-N Chunks** (`1`–`10`), **Context Window** (`4k`–`64k` tokens), **Temperature** (`0.0`–`1.5`), and **Reasoning Mode** (`🧠 Thinking ON` / `⚡ Fast`) at any time—even mid-conversation.
- **🤖 Language Model (LLM)**:
  - Switch live at runtime between **Local Ollama / vLLM / llama.cpp / LM Studio**, **RunPod Cloud GPUs**, **OpenAI**, **Google Gemini**, and **Anthropic Claude** with 1-click live model discovery (`POST /v1/llm/models`).
- **🚀 Built-in RunPod GPU Cloud Manager**:
  - **Live RunPod GPU Catalog (`GET /v1/runpod/gpus`)**: Queries RunPod's official GraphQL API in real time to display a full dropdown of all ~40 GPU types (`NVIDIA A40 48 GB`, `RTX A6000 48 GB`, `RTX 4090 24 GB`, `RTX 3090 24 GB`, `L40S 48 GB`, `A100 80 GB`, `H100`, etc.) with live **Community Cloud & Secure Cloud hourly prices** and real-time **stock availability (`🟢` / `⚪`)**.
  - **1-Click Single-Pod Deployment (`LLM + GPU Embedding`)**: Automatically provisions a RunPod GPU pod, installs and loads your chosen Ollama model (e.g. `qwen3:8b`, `qwen3:14b`, `qwen3:32b`, `deepseek-r1:14b`, `deepseek-r1:32b`) alongside an FP16 CUDA `BAAI/bge-m3` embedding server (`~1.2 GB VRAM`), opens an encrypted SSH port-forwarding tunnel (`:11435` & `:18081`), and switches the Chat UI to the cloud GPU.
  - **Encrypted Credential Vault**: Your RunPod API key and SSH key passphrase are encrypted at rest (`enc_v1:...` in `system_settings:runpod` + local browser fallback) so they survive page reloads and container restarts while remaining completely excluded from `.surql` database exports.
  - **1-Click Teardown**: Clicking **"Pod beenden & auf Mac zurückschalten"** terminates the cloud pod immediately (zero ongoing GPU billing) and switches the Chat UI back to your local model.
- **💾 Multi-Project SurrealDB Workspaces & `.surql` Backup / Restore**:
  - **Parallel Project Databases (`GET/POST/DELETE /v1/db/projects`)**: Maintain separate, isolated SurrealDB 4.0 databases per project or topic (e.g. `knowledge_base`, `manuals`, `invoices`) and switch between them in **0 ms** via the sidebar dropdown (`🗄️ Projekt-DB`).
  - **Isolated Project Chat Lists**: Each project database maintains its own isolated list of chat sessions in the sidebar; switching projects automatically restores your last active chat for that project.
  - **In-App SurrealDB Export & Import (`GET /v1/db/export`, `POST /v1/db/import`)**: Download a full `.surql` backup of the active project database (including all precomputed 1024-d HNSW vector embeddings and Knowledge Graph edges) or import `.surql` backups in seconds (`➕ Merge` or `🧹 Clean Restore`).
- **🌐 General & System**:
  - Instant bilingual UI switch (`🇩🇪 Deutsch` / `🇬🇧 English`) and live backend telemetry.

---

### 2. High-Speed Multi-File Ingestion & Pluggable Domain Pipelines

- **Batched Multi-File & Folder Upload (`➕`)**:
  - Select multiple files or entire directory trees (automatically preserving subfolder hierarchies as `[Category / Subfolder]` tags).
  - Uploads are automatically grouped into **16-file batches** with **parallel multi-core PDF/OCR extraction** (`pdftotext` + automatic 200-DPI `pdftoppm` & `Tesseract OCR` for scanned PDFs and photographed receipts `.jpg`/`.png`) and **cross-file batched GPU embeddings** (`batch_size = 64`).
- **Pluggable Ingestion Pipelines (`rust-rag-orchestrator/src/ingestion/pipelines/`)**:
  - **`🔄 Auto-Detect` / `📄 Standard / Generic`**: Structure-aware document chunker (headings, tables, lists) with optional LLM Knowledge Graph extraction.
  - **`⚖️ Legal & Statutory Pipeline` (`legal.rs`)**: Specialized parser for German (`gii-norm.dtd` XML from `gesetze-im-internet.de`) and EU (`Formex` XML) statutes. Chunks strictly along `§` and `Artikel` boundaries, injects statutory breadcrumb headers (`[<Statute> | <Section>]`), and extracts a **100% deterministic syntactic Knowledge Graph** (cross-referenced laws, public authorities, registers, and legal definitions) in **0 ms** without LLM hallucination.
  - **`📊 Financial & Invoice Aggregation` (`invoice.rs`)**: Generic syntactic and mathematical VAT-triplet ($N + \text{Tax} = G$) extractor for invoices and receipts (`EUR`, `USD`, `CHF`). When you ask expense or summation questions in the chat (e.g. *"Sum up all my 2025 invoices"*), SurrealDB executes a live full-collection aggregation in **< 500 ms**, automatically deduplicates `Invoice` + `Receipt` pairs, and injects the exact per-category and line-by-line calculation table as `[Source 1]`.
- **Integrated Full-Document Viewer (`GET /v1/documents/:doc_id/file`)**:
  - Displays dynamic format buttons (`📄 PDF anzeigen`, `📜 XML anzeigen`, `📝 Markdown anzeigen`) and renders a native PDF viewer for PDFs or a searchable dark-mode full-document HTML viewer for `.xml`, `.md`, and `.txt` files.

---

### 3. 100% Rust End-to-End Architecture

```text
├── rust-rag-orchestrator/src/
│   ├── main.rs                  # Axum server startup, router assembly & AppState
│   ├── config.rs                # Default startup configuration (overridable live in UI)
│   ├── db/                      # [Package: SurrealDB 4.0 Multi-Project & GraphRAG]
│   │   ├── mod.rs               # SurrealDB HTTP client, multi-DB switching & .surql export/import
│   │   ├── documents.rs         # Document/chunk CRUD, Graph persistence & live financial aggregation
│   │   ├── search.rs            # Hybrid Search (HNSW + BM25 + RRF) & 2-Hop Graph traversal
│   │   └── external.rs          # Universal external SQL connector (PostgreSQL, MySQL, SQLite, SurrealDB)
│   ├── ingestion/               # [Package: Batched Ingestion, PDF/Image OCR & Domain Pipelines]
│   │   ├── mod.rs               # TEI batch embedding (16 local / 64 RunPod GPU) & Cross-Encoder reranking
│   │   ├── chunker.rs           # Structure-aware document chunker (headings, tables, norms)
│   │   ├── pdf.rs               # Fast pdftotext, PDF/Image Tesseract OCR & full-document HTML/PDF viewer
│   │   ├── xml.rs               # Statutory XML (gii-norm.dtd & EU Formex) to Markdown parser
│   │   ├── documents.rs         # Multi-file batch upload, deduplication & DB backup HTTP handlers
│   │   ├── attachments.rs       # Ephemeral in-memory chat file extraction (/v1/extract)
│   │   └── pipelines/           # Pluggable domain ingestion pipelines
│   │       ├── mod.rs           # Pipeline router (Auto, Generic, LegalStatute)
│   │       ├── legal.rs         # 0-ms syntactic legal citation & authority Knowledge Graph extractor
│   │       └── invoice.rs       # Syntactic & VAT-triplet financial metadata extractor & aggregator
│   ├── retrieval/               # [Package: Multi-Hop Retrieval & Dynamic Text-to-SQL]
│   │   ├── mod.rs               # 2-Hop Hybrid Retrieval + live financial & SQL augmentation (/v1/retrieve)
│   │   ├── text_to_sql.rs       # Dynamic Text-to-SurrealQL Engine (read-only guardrails)
│   │   └── sql_formatter.rs     # Generic SQL-to-Markdown table renderer
│   ├── llm/                     # [Package: Multi-Provider LLM Runtime & SSE Streaming]
│   │   ├── mod.rs               # Live LLM runtime config, model discovery & graph entity extraction
│   │   ├── providers.rs         # Synchronous completions (Ollama native, OpenAI, Anthropic, Gemini)
│   │   └── stream.rs            # Real-time SSE token & reasoning (<think>) streaming
│   └── api/                     # [Package: Chat API, RunPod Cloud Manager, MCP & Showcases]
│       ├── chat.rs              # Streaming RAG chat & OpenAI-compatible /v1/chat/completions
│       ├── runpod.rs            # RunPod GraphQL GPU catalog, REST v2 Pod deployment & SSH tunnel manager
│       ├── mcp.rs               # Model Context Protocol (MCP) JSON-RPC 2.0 server (/mcp)
│       └── showcases.rs         # Optional Showcase dispatcher (ActiveShowcase::None by default)
│
├── rust-rag-ui/src/
│   └── main.rs                  # Leptos CSR WebAssembly Chat & Knowledge Studio UI
│
└── Showcase/                    # Optional self-contained industry showcases (HR, Spedition GPS, Webstore)
```

---

## 🚀 Initial Bootstrap & Container Startup

You only need [`./setup.sh`](./setup.sh) (or `docker compose up -d`) once to start the local containers and native Apple Silicon Metal embedding servers. Everything else is configured inside the Chat UI at **`http://localhost:8080/`**.

```bash
chmod +x setup.sh

# Start stack with automatic hardware detection (Apple Silicon Metal, NVIDIA GPU, or CPU):
./setup.sh

# Check live service status or stop all containers:
./setup.sh --status
./setup.sh --stop
```

> 💡 **Why does `setup.sh` recommend Colima on macOS?**
> On macOS, `./setup.sh` works with any Docker runtime (**Colima**, **Docker Desktop**, or **OrbStack**). If Docker is not yet installed, it recommends **Colima + Docker CLI** for three reasons:
> 1. **100% Open Source (MIT / Apache 2.0)**: Unlike Docker Desktop, Colima requires no commercial license subscription in larger organizations.
> 2. **Minimal Unified Memory Overhead**: Colima runs a lightweight headless Linux VM via Apple's native `Virtualization.framework` (`--vm-type vz`), leaving maximum Apple Silicon Unified Memory available for local Metal GPU embeddings and local Ollama LLMs.
> 3. **Zero-GUI CLI Automation**: Fully scriptable setup without manual GUI installers.

### Optional Industry Showcases
By default, the stack starts with a clean knowledge base. If you want to explore the built-in **Hybrid RAG + Dynamic Text-to-SurrealQL** industry demos (or load them via the UI), you can run:

```bash
./setup.sh --showcase list   # List available Showcases
./setup.sh --showcase 1      # Showcase 01: HR & IT Consulting (7 SLA PDFs + 3,000 Employee SQL records)
./setup.sh --showcase 2      # Showcase 02: Spedition & 1-Hz GPS Telemetry (6 PDFs + 120 Trucks + Interactive Map)
./setup.sh --showcase 3      # Showcase 03: E-Commerce Webstore Multi-DB (Internal Contracts + External ERP DB)
./setup.sh --showcase none   # Unload Showcase data
```

> 📖 **Connecting External SQL Databases**: See **[`./how_to.md`](./how_to.md)** for connecting external **PostgreSQL, MySQL/MariaDB, SQLite, AWS RDS, Google Cloud SQL, Azure Database, or SurrealDB** instances to the Dynamic Text-to-SQL engine.

---

## 🔌 REST API & MCP Endpoints (`http://localhost:8080`)

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `/` | `GET` | **Leptos WASM Chat & Knowledge Studio UI** |
| `/v1/chat/stream` | `POST` | Real-time SSE token & reasoning streaming with Hybrid RAG & GraphRAG sources |
| `/v1/chat/completions` | `POST` | **OpenAI-compatible Chat API** (usable in Cursor, Open WebUI, LibreChat, custom agents) |
| `/v1/retrieve` | `POST` | Standalone Multi-Stage Retrieval (HNSW + BM25 + 2-Hop Graph + Reranker + Aggregations) |
| `/v1/ingest/file` | `POST` | Batched multi-file upload (PDF, XML, Image OCR, Markdown, HTML, TXT) |
| `/v1/ingest` | `POST` | Raw JSON / Markdown document ingestion |
| `/v1/extract` | `POST` | Ephemeral in-memory file text extraction for temporary chat attachments (`📎`) |
| `/v1/documents` | `GET` | List all indexed documents, chunk counts, and Knowledge Graph relations in active DB |
| `/v1/documents/:doc_id` | `DELETE` | Delete a document and its chunks from the active project database |
| `/v1/documents/:doc_id/file` | `GET` | Serve original PDF or searchable dark-mode full-document HTML viewer (`.xml`/`.md`/`.txt`) |
| `/v1/db/projects` | `GET` / `POST` | List all SurrealDB project databases or create/switch the active project database |
| `/v1/db/projects/:db_name` | `DELETE` | Delete a secondary project database |
| `/v1/db/export` | `GET` | Download a `.surql` snapshot of the currently active project database |
| `/v1/db/import` | `POST` | Import a `.surql` backup into the active project database (`merge` or `replace`) |
| `/v1/llm/config` | `GET` / `POST` | Read or update the active LLM provider, base URL, API key, and model at runtime |
| `/v1/llm/models` | `POST` | Discover available models from the configured LLM endpoint |
| `/v1/runpod/status` | `GET` | Live RunPod GPU pod status, SSH tunnel health, and active endpoints |
| `/v1/runpod/gpus` | `GET` | Live RunPod GPU catalog with real-time Secure/Community Cloud pricing & stock status |
| `/v1/runpod/config` | `POST` | Verify and store encrypted RunPod API key & SSH private key / passphrase |
| `/v1/runpod/start` | `POST` | Deploy RunPod GPU pod (or switch model on active pod) and connect SSH tunnel |
| `/v1/runpod/stop` | `POST` | Terminate active RunPod pod, close SSH tunnel, and restore local Mac configuration |
| `/mcp` | `POST` | **Model Context Protocol (MCP)** JSON-RPC 2.0 server (`search_knowledge_base`, etc.) |

---

## 📜 License Overview (Commercial Use)

All components in this stack are free and allow **commercial use**:

| Component | Language | License | Commercial Use | Details |
| :--- | :--- | :--- | :--- | :--- |
| **SurrealDB 4.0** (`surrealdb/surrealdb`) | Rust | **BSL 1.1** (converts to Apache 2.0 after 4 yrs) | **Yes** | Free for commercial applications, client solutions, and internal production use (may not be resold as a managed third-party DBaaS). |
| **Hugging Face TEI** (`text-embeddings-inference`) | Rust | **Apache 2.0** | **Yes (Unrestricted)** | Official Rust inference server for dense embeddings & cross-encoder rerankers. |
| **Rust RAG Orchestrator & Leptos UI** (`axum`, `tokio`, `leptos`) | Rust | **MIT / Apache 2.0** | **Yes (Unrestricted)** | Full source code in `./rust-rag-orchestrator/` and `./rust-rag-ui/`. |
| **Embedding & Reranker Models** (`BAAI/bge-m3`, `bge-reranker-v2-m3`, `mmarco-mMiniLMv2`) | Safetensors | **MIT / Apache 2.0** | **Yes (Unrestricted)** | Multilingual dense embedding and cross-encoder reranking models. |
| **Colima** (optional macOS container runtime) | Go / Lima | **MIT License** | **Yes (Unrestricted)** | Lightweight open-source container runtime for macOS. |
