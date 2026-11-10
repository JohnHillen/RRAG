#!/usr/bin/env bash
# ==============================================================================
# setup.sh - Automated Setup & Deployment Script
# 100% Rust RAG & GraphRAG Stack (SurrealDB + Hugging Face TEI + Axum Orchestrator)
# Supports:
#   - Apple Silicon M1–M6 Metal GPU (--mode metal)
#   - NVIDIA CUDA GPU (--mode gpu)
#   - Pure CPU mode in Docker (--mode cpu)
# License: MIT / Apache 2.0
# ==============================================================================

set -euo pipefail

# ANSI color codes for terminal output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

MODE=""
PRESET=""
LLM_MODEL_ARG=""
LLM_PROVIDER_ARG=""
LLM_URL_ARG=""
ORCHESTRATOR_PORT_ARG=""
SURREAL_PORT_ARG=""
EMBED_PORT_ARG=""
RERANK_PORT_ARG=""
RERANK_FAST_PORT_ARG=""
STOP_ALL=false
SHOW_STATUS=false
RESET_DB=false
EXPORT_FILE=""
IMPORT_FILE=""

usage() {
    cat <<EOF
================================================================================
  🦀 100% Rust RAG & GraphRAG Pipeline - Setup & Management CLI
  Stack: SurrealDB 4.0 (SurrealKV) + Hugging Face TEI + Axum + Leptos WASM UI
================================================================================

USAGE:
  ./setup.sh [OPTIONS]

COMPUTE MODES (--mode):
  --mode metal           Apple Silicon (M1–M6): Runs SurrealDB & Orchestrator
                         in Docker (arm64) and native TEI Embedding/Reranker
                         servers on macOS with FP16 Metal GPU acceleration.
  --mode gpu             NVIDIA GPU (CUDA): Runs all components in Docker
                         with NVIDIA Container Runtime passthrough.
  --mode cpu             Pure CPU mode: Runs all components in Docker
                         (AVX2 / ARM64 optimized, no GPU required).
                         (Auto-detected if omitted).

MODEL PRESETS (--preset):
  --preset high          Multilingual high-end setup (recommended for 12+ GB RAM):
                         • Embedding: BAAI/bge-m3 (1024 dimensions, MIT license)
                         • Reranker : BAAI/bge-reranker-v2-m3 (Apache 2.0)
  --preset light         Lightweight setup (recommended for 8 GB RAM):
                         • Embedding: BAAI/bge-base-en-v1.5 (768 dimensions, MIT)
                         • Reranker : BAAI/bge-reranker-base (MIT)

LLM CONFIGURATION:
  --llm-model <name>     Sets model name in .env (e.g. qwen2.5:7b,
                         llama3.1:8b, gpt-4o, gemini-2.5-flash, claude-3-7-sonnet-latest).
  --llm-provider <type>  Sets LLM provider:
                         • openai_compatible (Ollama, vLLM, llama.cpp, OpenAI, Gemini)
                         • ollama            (Native Ollama API)
                         • anthropic         (Anthropic Claude API)
                         • gemini_native     (Google Gemini REST API)
                         • none              (Retrieval-only mode without LLM)
  --llm-url <url>        Sets LLM base URL (e.g. http://host.docker.internal:11434/v1).

PORT CONFIGURATION (Auto-fallback to next free port if occupied):
  --port <num>           Sets main port for Chat UI, REST API & MCP (Default: 8080).
  --surreal-port <num>   Sets host port for SurrealDB (Default: 8000).
  --embed-port <num>     Sets port for TEI Embedding server (Default: 8081).
  --rerank-port <num>    Sets port for TEI Reranker High (Default: 8082).
  --rerank-fast-port <num> Sets port for TEI Reranker Fast (Default: 8083).

MANAGEMENT, BACKUP & MAINTENANCE:
  --export [file.surql]  Exports complete SurrealDB (documents, chunks,
                         1024d vectors & graph relations) into a backup file
                         (Default: backup_surrealdb.surql).
  --import <file.surql>  Imports a SurrealDB backup (e.g. after moving from
                         MacBook to RunPod GPU server) without re-embedding.
  --status               Displays live status of all services, ports & models.
  --stop                 Stops all Docker containers, terminates native Metal
                         inference processes, and frees Ollama memory.
                         (SurrealDB volume data remains intact!).
  --reset-db             Stops stack and completely deletes the SurrealDB
                         volume (fresh factory reset after confirmation).
  --help, -h             Shows this help overview.

EXAMPLES:
  ./setup.sh                                      # Automatic start (detects Apple Silicon / GPU / CPU)
  ./setup.sh --export backup.surql                # Save complete DB backup for server migration
  ./setup.sh --import backup.surql                # Restore DB backup on new server (e.g. RunPod)
  ./setup.sh --llm-model qwen2.5:7b               # Start with local Ollama model
  ./setup.sh --mode gpu                           # Start on RunPod / Linux with NVIDIA CUDA GPU
  ./setup.sh --status                             # Check if all ports & models are online
  ./setup.sh --stop                               # Clean shutdown of all services
================================================================================
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mode)
            MODE="$2"
            shift 2
            ;;
        --preset)
            PRESET="$2"
            shift 2
            ;;
        --llm-model)
            LLM_MODEL_ARG="$2"
            shift 2
            ;;
        --llm-provider)
            LLM_PROVIDER_ARG="$2"
            shift 2
            ;;
        --llm-url)
            LLM_URL_ARG="$2"
            shift 2
            ;;
        --port)
            ORCHESTRATOR_PORT_ARG="$2"
            shift 2
            ;;
        --surreal-port)
            SURREAL_PORT_ARG="$2"
            shift 2
            ;;
        --embed-port)
            EMBED_PORT_ARG="$2"
            shift 2
            ;;
        --rerank-port)
            RERANK_PORT_ARG="$2"
            shift 2
            ;;
        --rerank-fast-port)
            RERANK_FAST_PORT_ARG="$2"
            shift 2
            ;;
        --status)
            SHOW_STATUS=true
            shift
            ;;
        --stop)
            STOP_ALL=true
            shift
            ;;
        --reset-db)
            RESET_DB=true
            shift
            ;;
        --export)
            if [[ $# -ge 2 && ! "$2" =~ ^-- ]]; then
                EXPORT_FILE="$2"
                shift 2
            else
                EXPORT_FILE="backup_surrealdb.surql"
                shift
            fi
            ;;
        --import)
            if [[ $# -ge 2 && ! "$2" =~ ^-- ]]; then
                IMPORT_FILE="$2"
                shift 2
            else
                IMPORT_FILE="backup_surrealdb.surql"
                shift
            fi
            ;;
        --help|-h|help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

stop_metal_processes() {
    if [[ -d .pids ]]; then
        for pidfile in .pids/*.pid; do
            [[ -e "$pidfile" ]] || continue
            PID=$(cat "$pidfile")
            if kill -0 "$PID" 2>/dev/null; then
                echo -e "${YELLOW}Stopping native Metal TEI process (PID ${PID})...${NC}"
                kill "$PID" 2>/dev/null || true
            fi
            rm -f "$pidfile"
        done
    fi
}

if [[ "$STOP_ALL" == "true" ]]; then
    echo -e "${BLUE}Stopping all services...${NC}"
    stop_metal_processes
    if [[ -f .env ]]; then
        source .env
        if [[ "${LLM_PROVIDER:-}" == "openai_compatible" ]] && [[ "${LLM_BASE_URL:-}" == *"11434"* ]]; then
            echo -e "${YELLOW}Releasing Ollama model memory (${LLM_MODEL})...${NC}"
            curl -sf http://localhost:11434/api/generate -d "{\"model\": \"${LLM_MODEL}\", \"keep_alive\": 0}" > /dev/null 2>&1 || true
        fi
    fi
    docker compose --profile cpu --profile gpu down
    echo -e "${GREEN}[OK] All Docker containers and native Metal processes stopped.${NC}"
    exit 0
fi

if [[ "$RESET_DB" == "true" ]]; then
    echo -e "${RED}[WARNING] This will permanently delete all stored documents, vectors, and graphs in SurrealDB!${NC}"
    read -r -p "Are you sure you want to reset the database? [y/N]: " confirm_reset
    if [[ "$confirm_reset" =~ ^[yYjJ]$ ]]; then
        stop_metal_processes
        docker compose --profile cpu --profile gpu down -v
        echo -e "${GREEN}[OK] SurrealDB volume deleted. Run './setup.sh' to reinitialize a fresh database.${NC}"
    else
        echo -e "${BLUE}Aborted. Data preserved.${NC}"
    fi
    exit 0
fi

if [[ -n "$EXPORT_FILE" ]]; then
    [[ -f .env ]] && source .env
    echo -e "${BLUE}Exporting SurrealDB (${SURREAL_NS:-rag_namespace}/${SURREAL_DB:-knowledge_base}) to '${EXPORT_FILE}'...${NC}"
    if curl -sf -X GET "http://localhost:${SURREAL_PORT:-8000}/export" \
        -u "${SURREAL_USER:-root}:${SURREAL_PASS:-root_secret_password_change_me}" \
        -H "Accept: application/octet-stream" \
        -H "surreal-ns: ${SURREAL_NS:-rag_namespace}" \
        -H "surreal-db: ${SURREAL_DB:-knowledge_base}" > "${EXPORT_FILE}"; then
        FILE_SIZE=$(du -h "${EXPORT_FILE}" | awk '{print $1}')
        echo -e "${GREEN}[OK] Database backup successfully exported: ${EXPORT_FILE} (${FILE_SIZE})${NC}"
        echo -e "Restore on destination server (e.g. RunPod) with: ${BLUE}./setup.sh --import ${EXPORT_FILE}${NC}"
    else
        echo -e "${RED}[ERROR] Export failed. Is SurrealDB running on port ${SURREAL_PORT:-8000}?${NC}"
        exit 1
    fi
    exit 0
fi

if [[ -n "$IMPORT_FILE" ]]; then
    if [[ ! -f "$IMPORT_FILE" ]]; then
        echo -e "${RED}[ERROR] Backup file '${IMPORT_FILE}' not found!${NC}"
        exit 1
    fi
    [[ -f .env ]] && source .env
    echo -e "${BLUE}Importing SurrealDB backup '${IMPORT_FILE}' into ${SURREAL_NS:-rag_namespace}/${SURREAL_DB:-knowledge_base}...${NC}"
    HTTP_CODE=$(curl -s -o /tmp/surql_import_out.json -w "%{http_code}" \
        -X POST "http://localhost:${SURREAL_PORT:-8000}/import" \
        -u "${SURREAL_USER:-root}:${SURREAL_PASS:-root_secret_password_change_me}" \
        -H "Accept: application/json" \
        -H "Content-Type: application/octet-stream" \
        -H "surreal-ns: ${SURREAL_NS:-rag_namespace}" \
        -H "surreal-db: ${SURREAL_DB:-knowledge_base}" \
        --data-binary @"${IMPORT_FILE}")
    if [[ "$HTTP_CODE" == "200" ]]; then
        echo -e "${GREEN}[OK] Backup '${IMPORT_FILE}' successfully imported into SurrealDB! Documents & vectors are immediately available.${NC}"
    else
        echo -e "${RED}[ERROR] Import failed (HTTP ${HTTP_CODE}):${NC}"
        cat /tmp/surql_import_out.json
        exit 1
    fi
    exit 0
fi

if [[ "$SHOW_STATUS" == "true" ]]; then
    [[ -f .env ]] && source .env
    echo -e "${BLUE}=== System Status (Rust RAG & GraphRAG Stack) ===${NC}"
    curl -sf "http://localhost:${SURREAL_PORT:-8000}/health" > /dev/null 2>&1 \
        && echo -e "  • SurrealDB (Port ${SURREAL_PORT:-8000})       : ${GREEN}ONLINE${NC}" \
        || echo -e "  • SurrealDB (Port ${SURREAL_PORT:-8000})       : ${RED}OFFLINE${NC}"
    curl -sf "http://localhost:${TEI_EMBED_PORT:-8081}/health" > /dev/null 2>&1 \
        && echo -e "  • TEI Embedding (Port ${TEI_EMBED_PORT:-8081})   : ${GREEN}ONLINE (${EMBEDDING_MODEL:-bge-m3})${NC}" \
        || echo -e "  • TEI Embedding (Port ${TEI_EMBED_PORT:-8081})   : ${RED}OFFLINE${NC}"
    curl -sf "http://localhost:${TEI_RERANK_PORT:-8082}/health" > /dev/null 2>&1 \
        && echo -e "  • TEI Reranker High (${TEI_RERANK_PORT:-8082})   : ${GREEN}ONLINE (${RERANKER_MODEL:-bge-reranker-v2-m3})${NC}" \
        || echo -e "  • TEI Reranker High (${TEI_RERANK_PORT:-8082})   : ${RED}OFFLINE${NC}"
    curl -sf "http://localhost:${TEI_RERANK_FAST_PORT:-8083}/health" > /dev/null 2>&1 \
        && echo -e "  • TEI Reranker Fast (${TEI_RERANK_FAST_PORT:-8083})   : ${GREEN}ONLINE (${RERANKER_FAST_MODEL:-mmarco-mMiniLMv2-L12})${NC}" \
        || echo -e "  • TEI Reranker Fast (${TEI_RERANK_FAST_PORT:-8083})   : ${RED}OFFLINE${NC}"
    curl -sf "http://localhost:${ORCHESTRATOR_PORT:-8080}/health" > /dev/null 2>&1 \
        && echo -e "  • Rust Orchestrator & UI (${ORCHESTRATOR_PORT:-8080}): ${GREEN}ONLINE (http://localhost:${ORCHESTRATOR_PORT:-8080}/)${NC}" \
        || echo -e "  • Rust Orchestrator & UI (${ORCHESTRATOR_PORT:-8080}): ${RED}OFFLINE${NC}"
    if curl -sf "http://localhost:11434/api/tags" > /dev/null 2>&1; then
        echo -e "  • Local Ollama (Port 11434)  : ${GREEN}ONLINE (Target model: ${LLM_MODEL:-unknown})${NC}"
    else
        echo -e "  • Local Ollama (Port 11434)  : ${YELLOW}UNREACHABLE${NC}"
    fi
    exit 0
fi

echo -e "${BLUE}==============================================================================${NC}"
echo -e "${BLUE}   100% Rust RAG & GraphRAG Pipeline Setup (SurrealDB + TEI + Axum)          ${NC}"
echo -e "${BLUE}==============================================================================${NC}"

# 1. Check prerequisites (Docker & Docker Compose)
install_docker() {
    local os_name
    os_name="$(uname -s)"

    if [[ "$os_name" == "Darwin" ]]; then
        if ! command -v brew &> /dev/null; then
            echo -e "${RED}[ERROR] Homebrew ('brew') is required for automatic installation on macOS.${NC}"
            echo -e "Install Homebrew first with: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
            exit 1
        fi

        echo -e "\n${BLUE}Which Docker variant would you like to install on your Mac?${NC}"
        echo -e "  1) Colima + Docker CLI (Recommended: 100% Open Source, MIT/Apache 2.0)"
        echo -e "  2) Docker Desktop for Mac (GUI App)"
        read -r -p "Selection [1/2] (Default: 1): " mac_docker_choice
        mac_docker_choice="${mac_docker_choice:-1}"

        if [[ "$mac_docker_choice" == "2" ]]; then
            echo -e "${BLUE}Installing Docker Desktop via Homebrew Cask...${NC}"
            brew install --cask docker
            echo -e "${YELLOW}Starting Docker Desktop... Please wait until the engine is ready.${NC}"
            open -a Docker
        else
            echo -e "${BLUE}Installing Docker CLI, Docker Compose, Buildx & Colima...${NC}"
            brew install docker docker-compose docker-buildx colima
            mkdir -p ~/.docker/cli-plugins
            ln -sfn "$(brew --prefix)/opt/docker-compose/bin/docker-compose" ~/.docker/cli-plugins/docker-compose
            ln -sfn "$(brew --prefix)/opt/docker-buildx/bin/docker-buildx" ~/.docker/cli-plugins/docker-buildx
            echo -e "${BLUE}Starting Colima VM (4 CPUs, 8 GB RAM)...${NC}"
            colima start --cpu 4 --memory 8 --vm-type vz --vz-rosetta || colima start --cpu 4 --memory 8
        fi
    else
        echo -e "${BLUE}Installing Docker Engine & Docker Compose Plugin for Linux...${NC}"
        curl -fsSL https://get.docker.com | sh
        if [[ "$EUID" -ne 0 ]]; then
            sudo usermod -aG docker "$USER" || true
            sudo systemctl enable --now docker || true
        fi
    fi

    echo -e "${BLUE}Waiting for Docker daemon to become responsive...${NC}"
    for _ in {1..30}; do
        if docker info &> /dev/null; then
            echo -e "${GREEN}[OK] Docker successfully installed and started!${NC}"
            return 0
        fi
        sleep 2
    done

    echo -e "${RED}[ERROR] Docker daemon not responding. Please check status manually.${NC}"
    exit 1
}

if ! command -v docker &> /dev/null || ! docker compose version &> /dev/null; then
    echo -e "${YELLOW}[NOTICE] Docker or 'docker compose' is not yet installed on this system.${NC}"
    echo -e "What would you like to do?"
    echo -e "  1) Install Docker"
    echo -e "  2) Abort"
    read -r -p "Please select [1/2]: " docker_choice

    case "$docker_choice" in
        1)
            install_docker
            ;;
        *)
            echo -e "${RED}Setup aborted.${NC}"
            exit 1
            ;;
    esac
fi

if ! docker info &> /dev/null; then
    echo -e "${YELLOW}[NOTICE] Docker is installed, but the daemon is not currently running.${NC}"
    if [[ "$(uname -s)" == "Darwin" ]] && command -v colima &> /dev/null; then
        echo -e "${BLUE}Starting Colima...${NC}"
        colima start --cpu 4 --memory 8
    elif [[ "$(uname -s)" == "Darwin" ]] && [[ -d "/Applications/Docker.app" ]]; then
        echo -e "${BLUE}Starting Docker Desktop...${NC}"
        open -a Docker
        for _ in {1..30}; do
            docker info &> /dev/null && break
            sleep 2
        done
    else
        echo -e "${RED}[ERROR] Please start the Docker daemon and run ./setup.sh again.${NC}"
        exit 1
    fi
fi

echo -e "${GREEN}[OK] Docker & Docker Compose are ready.${NC}"

# 2. Check hardware resources (OS, architecture, RAM, CPU cores, Metal / NVIDIA GPU)
OS_TYPE="$(uname -s)"
ARCH_TYPE="$(uname -m)"
IS_APPLE_SILICON=false
APPLE_CHIP_NAME=""

if [[ "$OS_TYPE" == "Darwin" ]]; then
    TOTAL_RAM_GB=$(( $(sysctl -n hw.memsize) / 1024 / 1024 / 1024 ))
    CPU_CORES=$(sysctl -n hw.ncpu)
    if [[ "$ARCH_TYPE" == "arm64" ]]; then
        IS_APPLE_SILICON=true
        APPLE_CHIP_NAME=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "Apple Silicon M-Series")
    fi
else
    TOTAL_RAM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
    TOTAL_RAM_GB=$(( TOTAL_RAM_KB / 1024 / 1024 ))
    CPU_CORES=$(nproc)
fi

HAS_NVIDIA_GPU=false
if command -v nvidia-smi &> /dev/null && nvidia-smi &> /dev/null; then
    HAS_NVIDIA_GPU=true
fi

if [[ "$IS_APPLE_SILICON" == "true" ]]; then
    echo -e "${BLUE}[INFO] Detected hardware:${NC} ${APPLE_CHIP_NAME} (${CPU_CORES} cores) | ${TOTAL_RAM_GB} GB Unified Memory | Metal GPU: Yes"
else
    echo -e "${BLUE}[INFO] Detected hardware:${NC} ${CPU_CORES} CPU cores (${ARCH_TYPE}) | ${TOTAL_RAM_GB} GB RAM | NVIDIA GPU: ${HAS_NVIDIA_GPU}"
fi

# Determine compute profile
if [[ -z "$MODE" ]]; then
    if [[ "$IS_APPLE_SILICON" == "true" ]]; then
        MODE="metal"
        echo -e "${GREEN}[AUTO] Apple Silicon (${APPLE_CHIP_NAME}) detected -> selecting profile 'metal' (Native Metal GPU acceleration).${NC}"
    elif [[ "$HAS_NVIDIA_GPU" == "true" ]]; then
        MODE="gpu"
        echo -e "${GREEN}[AUTO] NVIDIA GPU detected -> selecting profile 'gpu' (CUDA).${NC}"
    else
        MODE="cpu"
        echo -e "${YELLOW}[AUTO] No dedicated GPU detected -> selecting profile 'cpu'.${NC}"
    fi
fi

# Determine model preset
if [[ -z "$PRESET" ]]; then
    if [[ "$TOTAL_RAM_GB" -lt 12 && "$MODE" == "cpu" ]]; then
        PRESET="light"
        echo -e "${YELLOW}[NOTICE] Less than 12 GB RAM detected in CPU mode (${TOTAL_RAM_GB} GB). Selecting 'light' preset.${NC}"
    else
        PRESET="high"
        echo -e "${GREEN}[AUTO] Sufficient memory (${TOTAL_RAM_GB} GB) detected -> selecting 'high' preset (BAAI/bge-m3).${NC}"
    fi
fi

if [[ "$TOTAL_RAM_GB" -lt 8 ]]; then
    echo -e "${YELLOW}[WARNING] Minimum RAM requirement for the complete stack is 8 GB (currently: ${TOTAL_RAM_GB} GB).${NC}"
fi

# 3. Create or update .env file
if [[ ! -f .env ]]; then
    cp .env.example .env
    echo -e "${GREEN}[OK] Created .env from .env.example.${NC}"
fi

if [[ "$PRESET" == "light" ]]; then
    EMBED_MODEL="BAAI/bge-base-en-v1.5"
    EMBED_DIM="768"
    RERANK_MODEL="BAAI/bge-reranker-base"
else
    EMBED_MODEL="BAAI/bge-m3"
    EMBED_DIM="1024"
    RERANK_MODEL="BAAI/bge-reranker-v2-m3"
fi

update_env_var() {
    local key="$1"
    local val="$2"
    if grep -q "^${key}=" .env; then
        if [[ "$OS_TYPE" == "Darwin" ]]; then
            sed -i '' "s|^${key}=.*|${key}=${val}|" .env
        else
            sed -i "s|^${key}=.*|${key}=${val}|" .env
        fi
    else
        echo "${key}=${val}" >> .env
    fi
}

update_env_var "COMPUTE_MODE" "$MODE"
update_env_var "MODEL_PRESET" "$PRESET"
update_env_var "EMBEDDING_MODEL" "$EMBED_MODEL"
update_env_var "EMBEDDING_DIM" "$EMBED_DIM"
update_env_var "RERANKER_MODEL" "$RERANK_MODEL"
update_env_var "RERANKER_FAST_MODEL" "cross-encoder/mmarco-mMiniLMv2-L12-H384-v1"
update_env_var "RETRIEVAL_TOP_K_CANDIDATES" "15"
update_env_var "RETRIEVAL_FINAL_TOP_N" "4"
update_env_var "CHUNK_MAX_CHARS" "1000"

if [[ -n "$ORCHESTRATOR_PORT_ARG" ]]; then
    update_env_var "ORCHESTRATOR_PORT" "$ORCHESTRATOR_PORT_ARG"
fi
if [[ -n "$SURREAL_PORT_ARG" ]]; then
    update_env_var "SURREAL_PORT" "$SURREAL_PORT_ARG"
fi
if [[ -n "$EMBED_PORT_ARG" ]]; then
    update_env_var "TEI_EMBED_PORT" "$EMBED_PORT_ARG"
else
    update_env_var "TEI_EMBED_PORT" "${TEI_EMBED_PORT:-8081}"
fi
if [[ -n "$RERANK_PORT_ARG" ]]; then
    update_env_var "TEI_RERANK_PORT" "$RERANK_PORT_ARG"
else
    update_env_var "TEI_RERANK_PORT" "${TEI_RERANK_PORT:-8082}"
fi
if [[ -n "$RERANK_FAST_PORT_ARG" ]]; then
    update_env_var "TEI_RERANK_FAST_PORT" "$RERANK_FAST_PORT_ARG"
else
    update_env_var "TEI_RERANK_FAST_PORT" "${TEI_RERANK_FAST_PORT:-8083}"
fi

if [[ -n "$LLM_MODEL_ARG" ]]; then
    update_env_var "LLM_MODEL" "$LLM_MODEL_ARG"
fi
if [[ -n "$LLM_PROVIDER_ARG" ]]; then
    update_env_var "LLM_PROVIDER" "$LLM_PROVIDER_ARG"
fi
if [[ -n "$LLM_URL_ARG" ]]; then
    update_env_var "LLM_BASE_URL" "$LLM_URL_ARG"
fi

set -a
source .env
set +a

# Guard against duplicate ports
if [[ "${TEI_EMBED_PORT}" == "${TEI_RERANK_PORT}" || "${TEI_EMBED_PORT}" == "${TEI_RERANK_FAST_PORT}" || "${TEI_RERANK_PORT}" == "${TEI_RERANK_FAST_PORT}" ]]; then
    update_env_var "TEI_EMBED_PORT" "${EMBED_PORT_ARG:-8081}"
    update_env_var "TEI_RERANK_PORT" "${RERANK_PORT_ARG:-8082}"
    update_env_var "TEI_RERANK_FAST_PORT" "${RERANK_FAST_PORT_ARG:-8083}"
    set -a
    source .env
    set +a
fi

# Verify port availability and auto-fallback if blocked by third-party processes
ensure_free_port() {
    local env_key="$1"
    local desired_port="$2"
    local own_pattern="$3"
    local p="$desired_port"

    while lsof -iTCP:"$p" -sTCP:LISTEN -P -n >/dev/null 2>&1; do
        local proc_info
        proc_info=$(lsof -iTCP:"$p" -sTCP:LISTEN -P -n 2>/dev/null | tail -n +2)
        # Note: on macOS lsof truncates 'text-embeddings-router' to 'text-embe'
        if echo "$proc_info" | grep -Eqi "$own_pattern|text-embe"; then
            break
        fi
        if docker ps --format '{{.Names}} {{.Ports}}' 2>/dev/null | grep -E "^rag-" | grep -q ":${p}->"; then
            break
        fi
        echo -e "${YELLOW}[NOTICE] Port ${p} (${env_key}) is occupied by another process -> selecting free port...${NC}" >&2
        p=$((p + 1))
        while [[ "$p" == "${SURREAL_PORT:-8000}" || "$p" == "${ORCHESTRATOR_PORT:-8080}" || "$p" == "${TEI_EMBED_PORT:-8081}" || "$p" == "${TEI_RERANK_PORT:-8082}" || "$p" == "${TEI_RERANK_FAST_PORT:-8083}" ]] || lsof -iTCP:"$p" -sTCP:LISTEN -P -n >/dev/null 2>&1; do
            p=$((p + 1))
        done
    done

    if [[ "$p" != "$desired_port" ]]; then
        echo -e "${GREEN}[AUTO-PORT] ${env_key} automatically switched to free port ${p}!${NC}" >&2
        update_env_var "$env_key" "$p"
        set -a
        source .env
        set +a
    fi
}

ensure_free_port "ORCHESTRATOR_PORT" "${ORCHESTRATOR_PORT:-8080}" "rust-rag"
ensure_free_port "SURREAL_PORT" "${SURREAL_PORT:-8000}" "surreal"
ensure_free_port "TEI_EMBED_PORT" "${TEI_EMBED_PORT:-8081}" "text-embe"
ensure_free_port "TEI_RERANK_PORT" "${TEI_RERANK_PORT:-8082}" "text-embe"
ensure_free_port "TEI_RERANK_FAST_PORT" "${TEI_RERANK_FAST_PORT:-8083}" "text-embe"

set -a
source .env
set +a

if [[ "$MODE" == "metal" ]]; then
    update_env_var "TEI_EMBED_URL" "http://host.docker.internal:${TEI_EMBED_PORT:-8081}"
    update_env_var "TEI_RERANK_URL" "http://host.docker.internal:${TEI_RERANK_PORT:-8082}"
    update_env_var "TEI_RERANK_FAST_URL" "http://host.docker.internal:${TEI_RERANK_FAST_PORT:-8083}"
else
    update_env_var "TEI_EMBED_URL" "http://tei-embed:80"
    update_env_var "TEI_RERANK_URL" "http://tei-rerank:80"
    update_env_var "TEI_RERANK_FAST_URL" "http://tei-rerank-fast:80"
fi

# Synchronize HNSW dimension in sql/init.surql
if [[ "$OS_TYPE" == "Darwin" ]]; then
    sed -i '' "s|HNSW DIMENSION [0-9]*|HNSW DIMENSION ${EMBED_DIM}|g" sql/init.surql
else
    sed -i "s|HNSW DIMENSION [0-9]*|HNSW DIMENSION ${EMBED_DIM}|g" sql/init.surql
fi

set -a
source .env
set +a

echo -e "${BLUE}[CONFIGURATION]${NC}"
echo -e "  • Compute Profile : ${MODE}"
echo -e "  • Model Preset    : ${PRESET} (${EMBEDDING_MODEL}, ${EMBEDDING_DIM}d + ${RERANKER_MODEL})"
echo -e "  • TEI Endpoints   : Embed=${TEI_EMBED_URL} | Rerank=${TEI_RERANK_URL}"
echo -e "  • LLM Provider    : ${LLM_PROVIDER} (${LLM_MODEL} @ ${LLM_BASE_URL})"
echo -e "  • SurrealDB Port  : ${SURREAL_PORT}"
echo -e "  • RAG API Port    : ${ORCHESTRATOR_PORT}"

# 4A. If --mode metal (Apple Silicon M1–M6): Launch native Metal TEI processes
if [[ "$MODE" == "metal" ]]; then
    echo -e "\n${BLUE}[1/4] Preparing Apple Silicon Metal GPU inference (text-embeddings-router)...${NC}"
    if ! command -v text-embeddings-router &> /dev/null; then
        if command -v brew &> /dev/null; then
            echo -e "${YELLOW}Installing 'text-embeddings-inference' (Rust + Metal GPU) via Homebrew...${NC}"
            brew install text-embeddings-inference
        else
            echo -e "${RED}[ERROR] 'text-embeddings-router' not found and Homebrew is not installed.${NC}"
            echo -e "Install Homebrew or compile TEI with: cargo install --git https://github.com/huggingface/text-embeddings-inference.git text-embeddings-router -F metal"
            exit 1
        fi
    fi

    stop_metal_processes
    mkdir -p .pids logs

    echo -e "${GREEN}Starting native Metal Embedding server (${EMBEDDING_MODEL}, FP16) on port ${TEI_EMBED_PORT}...${NC}"
    nohup text-embeddings-router \
        --model-id "${EMBEDDING_MODEL}" \
        --dtype float16 \
        --port "${TEI_EMBED_PORT}" \
        --max-client-batch-size 32 \
        --auto-truncate > logs/tei_embed_metal.log 2>&1 &
    echo $! > .pids/tei_embed.pid

    echo -e "${GREEN}Starting native Metal Reranker server High (${RERANKER_MODEL}, FP16) on port ${TEI_RERANK_PORT}...${NC}"
    nohup text-embeddings-router \
        --model-id "${RERANKER_MODEL}" \
        --dtype float16 \
        --port "${TEI_RERANK_PORT}" \
        --max-client-batch-size 32 \
        --auto-truncate > logs/tei_rerank_metal.log 2>&1 &
    echo $! > .pids/tei_rerank.pid

    echo -e "${GREEN}Starting native Metal Reranker server Fast (${RERANKER_FAST_MODEL}, FP16) on port ${TEI_RERANK_FAST_PORT}...${NC}"
    nohup text-embeddings-router \
        --model-id "${RERANKER_FAST_MODEL}" \
        --dtype float16 \
        --port "${TEI_RERANK_FAST_PORT}" \
        --max-client-batch-size 32 \
        --auto-truncate > logs/tei_rerank_fast_metal.log 2>&1 &
    echo $! > .pids/tei_rerank_fast.pid

    echo -e "\n${BLUE}[2/4] Building and starting SurrealDB & Rust RAG Orchestrator in Docker (native arm64)...${NC}"
    docker compose up -d --build surrealdb rag-orchestrator
else
    # 4B. Docker Compose stack for 'cpu' or 'gpu' (NVIDIA)
    echo -e "\n${BLUE}[1/3] Building and starting Docker containers (Profile: ${MODE})...${NC}"
    docker compose --profile "$MODE" up -d --build
fi

# 5. Wait for SurrealDB healthcheck and load schema
echo -e "\n${BLUE}Waiting for SurrealDB at http://localhost:${SURREAL_PORT} ...${NC}"
for i in {1..30}; do
    if curl -sf "http://localhost:${SURREAL_PORT}/health" > /dev/null 2>&1; then
        echo -e "${GREEN}[OK] SurrealDB is ready.${NC}"
        break
    fi
    sleep 2
done

echo -e "${BLUE}Initializing SurrealDB schema (HNSW vector index, BM25 full-text, Knowledge Graph)...${NC}"
HTTP_CODE=$(curl -s -o /tmp/surql_init_out.json -w "%{http_code}" \
    -X POST "http://localhost:${SURREAL_PORT}/sql" \
    -u "${SURREAL_USER}:${SURREAL_PASS}" \
    -H "Accept: application/json" \
    -H "surreal-ns: ${SURREAL_NS}" \
    -H "surreal-db: ${SURREAL_DB}" \
    --data-binary @sql/init.surql)

if [[ "$HTTP_CODE" == "200" ]]; then
    echo -e "${GREEN}[OK] SurrealDB schema initialized successfully!${NC}"
else
    echo -e "${RED}[ERROR] Schema initialization failed (HTTP ${HTTP_CODE}):${NC}"
    cat /tmp/surql_init_out.json
    exit 1
fi

# 6. Wait for TEI Embedding & Reranker readiness
echo -e "\n${BLUE}Waiting for TEI Embedding server (${EMBEDDING_MODEL}) at http://localhost:${TEI_EMBED_PORT}/health ...${NC}"
for i in {1..60}; do
    if curl -sf "http://localhost:${TEI_EMBED_PORT}/health" > /dev/null 2>&1; then
        echo -e "${GREEN}[OK] TEI Embedding server is ready and warmed up.${NC}"
        break
    fi
    sleep 2
done

echo -e "${BLUE}Waiting for TEI Reranker High (${RERANKER_MODEL}) at http://localhost:${TEI_RERANK_PORT}/health ...${NC}"
for i in {1..60}; do
    if curl -sf "http://localhost:${TEI_RERANK_PORT}/health" > /dev/null 2>&1; then
        echo -e "${GREEN}[OK] TEI Reranker High is ready and warmed up.${NC}"
        break
    fi
    sleep 2
done

echo -e "${BLUE}Waiting for TEI Reranker Fast (${RERANKER_FAST_MODEL}) at http://localhost:${TEI_RERANK_FAST_PORT}/health ...${NC}"
for i in {1..60}; do
    if curl -sf "http://localhost:${TEI_RERANK_FAST_PORT}/health" > /dev/null 2>&1; then
        echo -e "${GREEN}[OK] TEI Reranker Fast is ready and warmed up.${NC}"
        break
    fi
    sleep 2
done

# 7. Check and warm up local Ollama if configured
if [[ "$LLM_PROVIDER" == "openai_compatible" ]] && [[ "$LLM_BASE_URL" == *"11434"* ]]; then
    echo -e "\n${BLUE}Checking local Ollama at http://localhost:11434 ...${NC}"
    if ! curl -sf http://localhost:11434/api/tags > /dev/null 2>&1; then
        if command -v ollama &> /dev/null; then
            echo -e "${YELLOW}Ollama not running -> starting 'ollama serve' in background...${NC}"
            OLLAMA_HOST=0.0.0.0 OLLAMA_KEEP_ALIVE=-1 nohup ollama serve > logs/ollama.log 2>&1 &
            echo $! > .pids/ollama.pid
            for _ in {1..20}; do
                curl -sf http://localhost:11434/api/tags > /dev/null 2>&1 && break
                sleep 1
            done
        fi
    fi

    echo -e "${BLUE}Pre-loading local Ollama model '${LLM_MODEL}' (keep_alive: -1, num_ctx: 4096)...${NC}"
    if curl -sf http://localhost:11434/api/chat -d "{\"model\": \"${LLM_MODEL}\", \"messages\": [], \"keep_alive\": -1, \"options\": {\"num_ctx\": 4096}}" > /dev/null 2>&1; then
        echo -e "${GREEN}[OK] '${LLM_MODEL}' is ready and resident in memory (no cold-start delay).${NC}"
    else
        echo -e "${YELLOW}[NOTICE] Could not preload model '${LLM_MODEL}' at http://localhost:11434. Please check 'ollama list'.${NC}"
    fi
fi

echo -e "\n${GREEN}==============================================================================${NC}"
echo -e "${GREEN}   RAG Stack successfully started! (Mode: ${MODE})                          ${NC}"
echo -e "${GREEN}==============================================================================${NC}"
if [[ "$MODE" == "metal" ]]; then
    echo -e "Apple Silicon Metal Logs:"
    echo -e "  • Embedding server log : tail -f logs/tei_embed_metal.log"
    echo -e "  • Reranker server log  : tail -f logs/tei_rerank_metal.log"
    echo -e "  • Clean shutdown       : ./setup.sh --stop"
fi
echo -e "\nAvailable endpoints of the Rust RAG Orchestrator:"
echo -e "  • 🦀 Leptos WASM Chat UI   : http://localhost:${ORCHESTRATOR_PORT}/"
echo -e "  • Health Check             : http://localhost:${ORCHESTRATOR_PORT}/health"
echo -e "  • Document Ingestion (JSON): POST http://localhost:${ORCHESTRATOR_PORT}/v1/ingest"
echo -e "  • File Upload (PDF/MD/TXT) : POST http://localhost:${ORCHESTRATOR_PORT}/v1/ingest/file"
echo -e "  • Hybrid + Graph Retrieval : POST http://localhost:${ORCHESTRATOR_PORT}/v1/retrieve"
echo -e "  • OpenAI-compatible Chat   : POST http://localhost:${ORCHESTRATOR_PORT}/v1/chat/completions"
echo -e "  • MCP Server (Tool Calls)  : POST http://localhost:${ORCHESTRATOR_PORT}/mcp"
