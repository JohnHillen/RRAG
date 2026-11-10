# 🦀 100% Rust RAG & GraphRAG Pipeline (SurrealDB + Hugging Face TEI + Axum + Leptos WASM)

A self-hosted, enterprise-grade **RAG & GraphRAG pipeline** written **100% in Rust** (`SurrealDB 4.0`, `Hugging Face TEI`, `Axum`, and a `Leptos WebAssembly` Chat & Knowledge Studio UI).

It runs via **Docker Compose** (supporting **Apple Silicon Metal GPU**, **NVIDIA CUDA GPU**, or **pure CPU**) and requires zero external cloud dependencies—while offering a live runtime configuration UI to connect **local LLMs** (`Ollama`, `vLLM`, `llama.cpp`, `LM Studio`), **remote GPU pods** (`RunPod`), or **Cloud APIs** (`OpenAI`, `Google Gemini`, `Anthropic Claude`).

---

## ✨ Key Features

- **100% Rust End-to-End Stack**:
  - **Database**: `SurrealDB 4.0` (`SurrealKV`) combining **HNSW Vector Search**, **BM25 Full-Text Search** (German & English Snowball stemming), **Reciprocal Rank Fusion (RRF)**, and native **Knowledge Graph traversal** (`->mentions->entity->relates_to->entity`) in a single query.
  - **Inference**: `Hugging Face Text Embeddings Inference (TEI)` for high-throughput dense embeddings (`BAAI/bge-m3`, 1024d) and dual Cross-Encoder Rerankers (`mmarco-mMiniLMv2-L12` Fast & `BAAI/bge-reranker-v2-m3` High).
  - **Orchestrator**: `Axum` + `Tokio` backend with real-time token streaming (`/v1/chat/stream`), OpenAI-compatible `/v1/chat/completions`, and a built-in **MCP Server** (`/mcp`).
  - **Frontend**: `Leptos` Client-Side Rendered **WebAssembly (WASM)** single-page application compiled via `Trunk`.
- **Modern Leptos WASM Chat & Knowledge Studio**:
  - **Multi-Chat Session Management**: Create, switch, clear, and delete multiple independent conversations persisted in browser `localStorage`.
  - **Bilingual UI (`🇩🇪 Deutsch` / `🇬🇧 English`)**: Instant language toggle inside the Settings dialog.
  - **Document Ingestion & Automatic OCR**: Multi-file upload for digital & scanned PDFs (3-stage extraction: `pdftotext` -> `pdf-extract` -> automatic `pdftoppm` + `Tesseract OCR`), Markdown, HTML, and plain text.
  - **Content-Hash Deduplication & Revision Control**: 128-bit content hashing prevents duplicate indexing and automatically replaces older revisions of updated documents.
  - **Ephemeral Chat Attachments (`📎`)**: Attach PDFs or text files in-memory for a single conversation without storing them in SurrealDB.
  - **Live LLM & Cloud Configuration**: Switch between local `Ollama`, `RunPod vLLM`, `OpenAI`, `Google Gemini`, and `Anthropic Claude` at runtime—including live model discovery from the target server.

---

## 1. Quick Start (Automated Setup Script)

```bash
chmod +x setup.sh

# Automatic hardware detection (Apple Silicon Metal, NVIDIA GPU, or CPU):
./setup.sh

# Or explicitly select compute mode & model preset:
./setup.sh --mode metal --preset high   # Apple Silicon M1–M6 (Native Metal GPU acceleration)
./setup.sh --mode gpu --preset high     # NVIDIA CUDA GPU (Docker with NVIDIA Runtime)
./setup.sh --mode cpu --preset light    # Pure CPU mode in Docker

# Custom port configuration (auto-fallback if ports are already in use):
./setup.sh --port 8090

# Database backup & restore (e.g., migrating from MacBook to RunPod GPU server):
./setup.sh --export backup.surql
./setup.sh --import backup.surql

# Check live service status or stop all containers & native processes:
./setup.sh --status
./setup.sh --stop
```

The [setup.sh](file:///Users/john/Documents/workspaces/RAG-Gemini/setup.sh) script automatically:
1. Checks Docker, CPU cores, system RAM, Apple Silicon Metal, and NVIDIA GPU (`nvidia-smi`).
2. Checks whether configured ports (`8080`, `8000`, `8081`, `8082`, `8083`) are free and automatically selects the next available port if occupied by another tool.
3. Generates/updates `.env` and synchronizes vector dimensions in [sql/init.surql](file:///Users/john/Documents/workspaces/RAG-Gemini/sql/init.surql).
4. Starts all services via [docker-compose.yml](file:///Users/john/Documents/workspaces/RAG-Gemini/docker-compose.yml) (plus native `text-embeddings-router` Metal GPU servers on Apple Silicon).
5. Initializes the SurrealDB schema (HNSW vector index, German/English BM25 full-text index, and Knowledge Graph relation tables).

---

## 2. Hardware Requirements

| Compute Mode | Tier | CPU | System RAM | GPU / VRAM | Disk (SSD) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Pure CPU** (`--mode cpu`) | **Minimum** (`--preset light`) | 4 vCPUs (AVX2 / ARM64) | **8 GB RAM** | No GPU required | 20 GB SSD |
| **Pure CPU** (`--mode cpu`) | **Recommended** (`--preset high`) | 8+ vCPUs | **16 GB RAM** | No GPU required | 50 GB NVMe/SSD |
| **Apple Silicon Metal** (`--mode metal`) | **Recommended Mac** (`--preset high`) | Apple M1 / M2 / M3 / M4 / M5 / M6 | **16+ GB Unified Memory** (min. 8 GB for `light`) | Integrated **Apple Metal GPU** (via native `text-embeddings-router`) | 30 GB SSD |
| **Hybrid CPU + GPU** (`--mode gpu`) | **Minimum GPU** *(External LLM)* | 4 vCPUs | **8 GB RAM** | NVIDIA GPU (Turing / Compute 7.5+) with **min. 6 GB VRAM** | 30 GB NVMe/SSD |
| **Hybrid CPU + GPU** (`--mode gpu`) | **Recommended GPU** *(Incl. local LLM)* | 8–16 vCPUs | **32 GB RAM** | NVIDIA GPU with **16–24 GB VRAM** (e.g., RTX 3090/4090/L4) | 150 GB NVMe/SSD |

---

## 3. Generic LLM Configuration (`.env` or Live in Chat UI)

You can configure any LLM either in `.env` (template in [.env.example](file:///Users/john/Documents/workspaces/RAG-Gemini/.env.example)) or live at runtime via the **Settings (`⚙️`)** button in the web UI:

### Option A: Local LLM (Ollama / vLLM / llama.cpp / LM Studio)
```env
LLM_PROVIDER=openai_compatible
LLM_BASE_URL=http://host.docker.internal:11434/v1
LLM_API_KEY=ollama
LLM_MODEL=qwen2.5:7b
```

### Option B: Remote GPU Pod (RunPod vLLM / Remote Ollama)
```env
LLM_PROVIDER=openai_compatible
LLM_BASE_URL=https://YOUR-POD-ID-8000.proxy.runpod.net/v1
LLM_API_KEY=rp_...
LLM_MODEL=Qwen/Qwen2.5-32B-Instruct-AWQ
```

### Option C: OpenAI API
```env
LLM_PROVIDER=openai_compatible
LLM_BASE_URL=https://api.openai.com/v1
LLM_API_KEY=sk-...
LLM_MODEL=gpt-4o
```

### Option D: Google Gemini (via OpenAI-Compatible Endpoint or Native API)
```env
LLM_PROVIDER=openai_compatible
LLM_BASE_URL=https://generativelanguage.googleapis.com/v1beta/openai
LLM_API_KEY=AIzaSy...
LLM_MODEL=gemini-2.5-flash
```

### Option E: Anthropic Claude
```env
LLM_PROVIDER=anthropic
LLM_BASE_URL=https://api.anthropic.com/v1
LLM_API_KEY=sk-ant-...
LLM_MODEL=claude-3-7-sonnet-latest
```

### Option F: Retrieval-Only Mode (No LLM Calls in Orchestrator)
```env
LLM_PROVIDER=none
```

---

## 4. REST API & MCP Endpoints

The Rust orchestrator ([rust-rag-orchestrator/src/main.rs](file:///Users/john/Documents/workspaces/RAG-Gemini/rust-rag-orchestrator/src/main.rs)) runs on port `8080` by default:

### 1. Leptos WASM Chat & Knowledge Studio (`GET /`)
Open `http://localhost:8080/` in your browser.

### 2. Ingest JSON / Markdown (`POST /v1/ingest`)
```bash
curl -X POST http://localhost:8080/v1/ingest \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Knowledge Management Manual",
    "source": "internal_wiki",
    "text": "# Hybrid Search & Reranking\nSurrealDB combines HNSW vector search with BM25 full-text search and GraphRAG."
  }'
```

### 3. Upload Files (PDF with Auto-OCR, HTML, Markdown, TXT) (`POST /v1/ingest/file`)
```bash
curl -X POST http://localhost:8080/v1/ingest/file \
  -F "files=@/path/to/document.pdf"
```

### 4. Two-Stage Retrieval: Hybrid Search + GraphRAG + Cross-Encoder Reranking (`POST /v1/retrieve`)
```bash
curl -X POST http://localhost:8080/v1/retrieve \
  -H "Content-Type: application/json" \
  -d '{
    "query": "How does SurrealDB combine vector search and full-text search?",
    "top_n": 4,
    "reranker": "fast"
  }'
```

### 5. OpenAI-Compatible Chat Endpoint (`POST /v1/chat/completions`)
Can be used directly in **Open WebUI**, **LibreChat**, **Cursor**, or custom clients as an OpenAI Base URL (`http://localhost:8080/v1`):
```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "rag-default",
    "use_rag": true,
    "messages": [
      {"role": "user", "content": "Explain our architecture based on the knowledge base."}
    ]
  }'
```

### 6. Runtime LLM Configuration & Model Discovery (`GET/POST /v1/llm/config`, `POST /v1/llm/models`)
Query or update the active LLM provider, base URL, API key, and model without restarting the container.

### 7. MCP Server (Model Context Protocol) (`POST /mcp`)
Exposes the `search_knowledge_base` tool over JSON-RPC 2.0 so external AI agents (Claude Desktop, Cursor, custom agents) can query SurrealDB natively.

---

## 5. License Overview (Commercial Use)

All components in this stack are free and allow **commercial use**:

| Component | Language | License | Commercial Use | Details |
| :--- | :--- | :--- | :--- | :--- |
| **SurrealDB** (`surrealdb/surrealdb`) | Rust | **BSL 1.1** (converts to Apache 2.0 after 4 yrs) | **Yes** (with 1 exception) | Free for commercial products, client solutions, and internal production use. Only restriction: may not be resold as a managed DBaaS offering to third parties. |
| **Hugging Face TEI** (`text-embeddings-inference`) | Rust | **Apache 2.0** | **Yes (Unrestricted)** | Official Rust inference server for embeddings & cross-encoder rerankers. |
| **Rust RAG Orchestrator & Leptos UI** (`axum`, `tokio`, `leptos`, `text-splitter`, `pdf-extract`) | Rust | **MIT / Apache 2.0** | **Yes (Unrestricted)** | Full source code in `rust-rag-orchestrator/` and `rust-rag-ui/`. |
| **Embedding Models** (`BAAI/bge-m3` / `bge-base-en-v1.5`) | Safetensors | **MIT License** | **Yes (Unrestricted)** | Multilingual dense embedding models. |
| **Reranker Models** (`BAAI/bge-reranker-v2-m3` / `mmarco-mMiniLMv2-L12`) | Safetensors | **Apache 2.0 / MIT** | **Yes (Unrestricted)** | Cross-encoder models for precision reranking. |
