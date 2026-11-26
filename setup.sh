#!/usr/bin/env bash
# ==============================================================================
# setup.sh - Automated Setup & Deployment Script
# 100% Rust RAG & GraphRAG Stack (SurrealDB + Hugging Face TEI + Axum Orchestrator)
# Supports:
#   - Container Runtimes: Docker / Colima (--colima / --docker) & Podman (--podman)
#   - Apple Silicon M1–M6 Metal GPU (--mode metal)
#   - NVIDIA CUDA GPU (--mode gpu)
#   - Pure CPU mode (--mode cpu)
# License: MIT / Apache 2.0
# ==============================================================================

set -euo pipefail

# ANSI color codes for terminal output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

MODE=""
PRESET=""
RUNTIME_ARG=""
CONTAINER_RUNTIME=""
CONTAINER_CLI=""
COMPOSE_CMD=()
HOST_ALIAS="host.docker.internal"
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
OS_TYPE="$(uname -s)"

usage() {
    cat <<EOF
================================================================================
  🦀 100% Rust RAG & GraphRAG Pipeline - Setup & Management CLI
  Stack: SurrealDB 4.0 (SurrealKV) + Hugging Face TEI + Axum + Leptos WASM UI
================================================================================

USAGE:
  ./setup.sh [OPTIONS]

CONTAINER RUNTIME:
  --podman               Use Podman (+ Podman Machine on macOS).
  --colima               Use Colima + Docker CLI (macOS lightweight VM).
  --docker               Use Docker Engine / Docker Desktop.
  --runtime <type>       Explicitly set runtime: podman | colima | docker.
                         (Saved to .env as CONTAINER_RUNTIME; prompted once on
                         macOS if unset and no flag is provided).

COMPUTE MODES (--mode):
  --mode metal           Apple Silicon (M1–M6): Runs SurrealDB & Orchestrator
                         in containers (arm64) and native TEI Embedding/Reranker
                         servers on macOS with FP16 Metal GPU acceleration.
  --mode gpu             NVIDIA GPU (CUDA): Runs all components in containers
                         with NVIDIA Container Runtime passthrough.
  --mode cpu             Pure CPU mode: Runs all components in containers
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
  --llm-url <url>        Sets LLM base URL (e.g. http://host.docker.internal:11434/v1
                         or http://host.containers.internal:11434/v1 for Podman).

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
  --stop                 Stops all containers, terminates native Metal
                         inference processes, and frees Ollama memory.
                         (SurrealDB volume data remains intact!).
  --reset-db             Stops stack and completely deletes the SurrealDB
                         volume (fresh factory reset after confirmation).
  --help, -h             Shows this help overview.

EXAMPLES:
  ./setup.sh                                      # Automatic start (detects Apple Silicon / GPU / CPU)
  ./setup.sh --podman                             # Start with Podman runtime
  ./setup.sh --colima                             # Start with Colima + Docker CLI
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
        --runtime)
            RUNTIME_ARG="$2"
            shift 2
            ;;
        --podman)
            RUNTIME_ARG="podman"
            shift
            ;;
        --colima)
            RUNTIME_ARG="colima"
            shift
            ;;
        --docker)
            RUNTIME_ARG="docker"
            shift
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

# Validate explicit --runtime argument if provided
if [[ -n "$RUNTIME_ARG" ]]; then
    case "$RUNTIME_ARG" in
        podman|colima|docker) ;;
        *)
            log_error "Unsupported container runtime: '$RUNTIME_ARG'. Use 'podman', 'colima', or 'docker'."
            exit 1
            ;;
    esac
fi

# ==============================================================================
# Helper Functions: Runtime Detection, Compose Execution & Metal Cleanup
# ==============================================================================

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

detect_podman_compose_cmd() {
    if command -v podman &> /dev/null && podman compose version &> /dev/null 2>&1; then
        COMPOSE_CMD=(podman compose)
        return 0
    elif command -v podman-compose &> /dev/null; then
        COMPOSE_CMD=(podman-compose)
        return 0
    fi
    return 1
}

detect_docker_compose_cmd() {
    if command -v docker &> /dev/null && docker compose version &> /dev/null 2>&1; then
        COMPOSE_CMD=(docker compose)
        return 0
    elif command -v docker-compose &> /dev/null; then
        COMPOSE_CMD=(docker-compose)
        return 0
    fi
    return 1
}

# Runs compose down without starting any stopped VM (used by --stop and --reset-db)
compose_down_active() {
    local with_volumes=false
    if [[ "${1:-}" == "-v" ]]; then
        with_volumes=true
    fi
    local stopped_any=false

    # Check Podman if requested or running
    if [[ "${RUNTIME_ARG:-${CONTAINER_RUNTIME:-}}" == "podman" ]] || \
       (command -v podman &> /dev/null && podman info &> /dev/null 2>&1); then
        if detect_podman_compose_cmd; then
            if [[ "$with_volumes" == "true" ]]; then
                "${COMPOSE_CMD[@]}" --profile cpu --profile gpu down -v 2>/dev/null \
                    || "${COMPOSE_CMD[@]}" down -v 2>/dev/null \
                    || true
            else
                "${COMPOSE_CMD[@]}" --profile cpu --profile gpu down 2>/dev/null \
                    || "${COMPOSE_CMD[@]}" down 2>/dev/null \
                    || true
            fi
            stopped_any=true
        fi
    fi

    # Check Docker / Colima if requested or running
    if [[ "${RUNTIME_ARG:-${CONTAINER_RUNTIME:-}}" != "podman" ]] && \
       command -v docker &> /dev/null && docker info &> /dev/null 2>&1; then
        if detect_docker_compose_cmd; then
            if [[ "$with_volumes" == "true" ]]; then
                "${COMPOSE_CMD[@]}" --profile cpu --profile gpu down -v 2>/dev/null \
                    || "${COMPOSE_CMD[@]}" down -v 2>/dev/null \
                    || true
            else
                "${COMPOSE_CMD[@]}" --profile cpu --profile gpu down 2>/dev/null \
                    || "${COMPOSE_CMD[@]}" down 2>/dev/null \
                    || true
            fi
            stopped_any=true
        fi
    fi

    if [[ "$stopped_any" == "false" ]]; then
        log_warn "No running container engine detected; skipped container shutdown."
    fi
}

# ==============================================================================
# Early Management Commands (No interactive prompts, no VM startup)
# ==============================================================================

if [[ "$STOP_ALL" == "true" ]]; then
    echo -e "${BLUE}Stopping all services...${NC}"
    stop_metal_processes
    if [[ -f .env ]]; then
        source .env
        if [[ "${LLM_PROVIDER:-}" == "openai_compatible" ]] && [[ "${LLM_BASE_URL:-}" == *"11434"* ]]; then
            echo -e "${YELLOW}Releasing Ollama model memory (${LLM_MODEL:-})...${NC}"
            curl -sf http://localhost:11434/api/generate -d "{\"model\": \"${LLM_MODEL:-}\", \"keep_alive\": 0}" > /dev/null 2>&1 || true
        fi
    fi
    compose_down_active
    echo -e "${GREEN}[OK] All containers and native Metal processes stopped.${NC}"
    exit 0
fi

if [[ "$RESET_DB" == "true" ]]; then
    [[ -f .env ]] && source .env
    echo -e "${RED}[WARNING] This will permanently delete all stored documents, vectors, and graphs in SurrealDB!${NC}"
    read -r -p "Are you sure you want to reset the database? [y/N]: " confirm_reset
    if [[ "$confirm_reset" =~ ^[yYjJ]$ ]]; then
        stop_metal_processes
        compose_down_active -v
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
    echo -e "  • Container Runtime          : ${CONTAINER_RUNTIME:-auto}"
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

# ==============================================================================
# 1. Container Runtime Selection & Preparation (Docker / Colima / Podman)
# ==============================================================================

# Load previously saved CONTAINER_RUNTIME from .env if no CLI override was given
if [[ -z "$RUNTIME_ARG" && -f .env ]]; then
    SAVED_RUNTIME=$(grep -E '^CONTAINER_RUNTIME=' .env | tail -n 1 | cut -d'=' -f2- | tr -d '[:space:]' || true)
    if [[ "$SAVED_RUNTIME" == "podman" || "$SAVED_RUNTIME" == "colima" || "$SAVED_RUNTIME" == "docker" ]]; then
        CONTAINER_RUNTIME="$SAVED_RUNTIME"
    fi
elif [[ -n "$RUNTIME_ARG" ]]; then
    CONTAINER_RUNTIME="$RUNTIME_ARG"
fi

select_container_runtime() {
    if [[ -n "$CONTAINER_RUNTIME" ]]; then
        return 0
    fi

    # On Linux, auto-detect available runtime without prompting
    if [[ "$OS_TYPE" != "Darwin" ]]; then
        if command -v docker &> /dev/null && docker info &> /dev/null 2>&1; then
            CONTAINER_RUNTIME="docker"
        elif command -v podman &> /dev/null && podman info &> /dev/null 2>&1; then
            CONTAINER_RUNTIME="podman"
        elif command -v docker &> /dev/null; then
            CONTAINER_RUNTIME="docker"
        elif command -v podman &> /dev/null; then
            CONTAINER_RUNTIME="podman"
        else
            CONTAINER_RUNTIME="docker"
        fi
        return 0
    fi

    # On macOS: if non-interactive terminal, auto-detect or default to colima
    if [[ ! -t 0 ]]; then
        if command -v podman &> /dev/null && podman info &> /dev/null 2>&1; then
            CONTAINER_RUNTIME="podman"
        elif command -v docker &> /dev/null && docker info &> /dev/null 2>&1; then
            if command -v colima &> /dev/null && colima status &> /dev/null 2>&1; then
                CONTAINER_RUNTIME="colima"
            else
                CONTAINER_RUNTIME="docker"
            fi
        elif command -v podman &> /dev/null && ! command -v colima &> /dev/null; then
            CONTAINER_RUNTIME="podman"
        else
            CONTAINER_RUNTIME="colima"
        fi
        return 0
    fi

    echo ""
    echo -e "${BLUE}Select your container runtime (saved in .env; override anytime via --podman / --colima / --docker):${NC}"
    echo -e "  1) Colima + Docker CLI  (Recommended: lightweight open-source VM on macOS)"
    echo -e "  2) Podman               (Daemonless OCI runtime via Podman Machine)"
    echo -e "  3) Docker Desktop       (Docker Desktop for Mac GUI)"
    while true; do
        read -r -p "Selection [1/2/3] (Default: 1): " runtime_choice
        case "${runtime_choice:-1}" in
            1) CONTAINER_RUNTIME="colima"; break ;;
            2) CONTAINER_RUNTIME="podman"; break ;;
            3) CONTAINER_RUNTIME="docker"; break ;;
            *) log_error "Invalid choice. Please enter 1, 2, or 3." ;;
        esac
    done
}

install_docker_runtime() {
    if [[ "$OS_TYPE" == "Darwin" ]]; then
        if ! command -v brew &> /dev/null; then
            log_error "Homebrew ('brew') is required for automatic installation on macOS."
            echo -e "Install Homebrew first with: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/stable/install.sh)\""
            exit 1
        fi

        if [[ "$CONTAINER_RUNTIME" == "docker" ]]; then
            log_info "Installing Docker Desktop via Homebrew Cask..."
            brew install --cask docker
            log_warn "Starting Docker Desktop... Please wait until the engine is ready."
            open -a Docker
        else
            log_info "Installing Docker CLI, Docker Compose, Buildx & Colima via Homebrew..."
            brew install docker docker-compose docker-buildx colima
            mkdir -p ~/.docker/cli-plugins
            ln -sfn "$(brew --prefix)/opt/docker-compose/bin/docker-compose" ~/.docker/cli-plugins/docker-compose
            ln -sfn "$(brew --prefix)/opt/docker-buildx/bin/docker-buildx" ~/.docker/cli-plugins/docker-buildx
            log_info "Starting Colima VM (4 CPUs, 8 GB RAM)..."
            colima start --cpu 4 --memory 8 --vm-type vz --vz-rosetta --mount-type virtiofs \
                || colima start --cpu 4 --memory 8 --vm-type vz --vz-rosetta \
                || colima start --cpu 4 --memory 8
        fi
    else
        log_info "Installing Docker Engine & Docker Compose Plugin for Linux..."
        curl -fsSL https://get.docker.com | sh
        if [[ "$EUID" -ne 0 ]]; then
            sudo usermod -aG docker "$USER" || true
            sudo systemctl enable --now docker || true
        fi
    fi

    log_info "Waiting for Docker daemon to become responsive..."
    for _ in {1..30}; do
        if docker info &> /dev/null 2>&1; then
            log_success "Docker successfully installed and started!"
            return 0
        fi
        sleep 2
    done

    log_error "Docker daemon not responding. Please check status manually."
    exit 1
}

install_podman_runtime() {
    if [[ "$OS_TYPE" == "Darwin" ]]; then
        if ! command -v brew &> /dev/null; then
            log_error "Homebrew ('brew') is required for automatic Podman installation on macOS."
            echo -e "Install Homebrew first with: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/stable/install.sh)\""
            exit 1
        fi
        log_info "Installing Podman & podman-compose via Homebrew..."
        brew install podman podman-compose
    else
        log_info "Installing Podman & podman-compose for Linux..."
        if [[ -f /etc/os-release ]]; then
            # shellcheck disable=SC1091
            source /etc/os-release
            case "${ID:-}" in
                ubuntu|debian|linuxmint|pop)
                    sudo apt-get update && sudo apt-get install -y podman podman-compose
                    ;;
                fedora|rhel|centos|rocky|almalinux)
                    sudo dnf install -y podman podman-compose
                    ;;
                arch|manjaro)
                    sudo pacman -Sy --noconfirm podman podman-compose
                    ;;
                *)
                    log_error "Unsupported Linux distribution '${ID:-unknown}' for automatic Podman install. Please install 'podman' and 'podman-compose' manually."
                    exit 1
                    ;;
            esac
        else
            log_error "Could not detect Linux distribution. Please install 'podman' and 'podman-compose' manually."
            exit 1
        fi
    fi
}

ensure_podman_ready() {
    if ! command -v podman &> /dev/null; then
        log_warn "Podman is not yet installed on this system."
        echo -e "What would you like to do?"
        echo -e "  1) Install Podman & podman-compose"
        echo -e "  2) Abort"
        read -r -p "Please select [1/2]: " podman_choice
        case "$podman_choice" in
            1) install_podman_runtime ;;
            *) echo -e "${RED}Setup aborted.${NC}"; exit 1 ;;
        esac
    fi

    # On macOS, ensure Podman Machine exists and is running
    if [[ "$OS_TYPE" == "Darwin" ]]; then
        if ! podman info &> /dev/null 2>&1; then
            local existing_machines
            existing_machines=$(podman machine list --format '{{.Name}}' 2>/dev/null || true)
            if [[ -z "$existing_machines" ]]; then
                log_info "Initializing Podman Machine (4 CPUs, 8 GB RAM, 50 GB disk)..."
                podman machine init --cpus 4 --memory 8192 --disk-size 50 \
                    || podman machine init
            fi

            log_info "Starting Podman Machine..."
            podman machine start || true

            log_info "Waiting for Podman Machine to become responsive..."
            local ready=false
            for _ in {1..30}; do
                if podman info &> /dev/null 2>&1; then
                    ready=true
                    break
                fi
                sleep 2
            done

            if [[ "$ready" != "true" ]]; then
                log_error "Podman Machine did not become responsive. Check 'podman machine list' and 'podman info'."
                exit 1
            fi
        fi
    else
        if ! podman info &> /dev/null 2>&1; then
            log_error "Podman is installed, but 'podman info' failed. Please verify your Podman installation."
            exit 1
        fi
    fi

    # Ensure either 'podman compose' or 'podman-compose' is available
    if ! detect_podman_compose_cmd; then
        log_warn "Neither 'podman compose' nor 'podman-compose' is available."
        if [[ "$OS_TYPE" == "Darwin" ]] && command -v brew &> /dev/null; then
            log_info "Installing 'podman-compose' via Homebrew..."
            brew install podman-compose
            detect_podman_compose_cmd || {
                log_error "Failed to configure 'podman-compose'."
                exit 1
            }
        else
            log_error "Please install 'podman-compose' (or configure a compose provider for 'podman compose') and re-run ./setup.sh."
            exit 1
        fi
    fi

    CONTAINER_CLI="podman"
    HOST_ALIAS="host.containers.internal"

    # Pre-flight check on macOS: warn if existing Podman Machine has low free disk space or RAM
    if [[ "$OS_TYPE" == "Darwin" ]]; then
        local vm_avail_kb vm_mem_mb
        vm_avail_kb=$(podman machine ssh "df -Pk /sysroot 2>/dev/null | awk 'NR==2 {print \$4}'" 2>/dev/null | tr -d '[:space:]' || true)
        vm_mem_mb=$(podman machine inspect --format '{{.Resources.Memory}}' 2>/dev/null | head -n 1 | tr -d '[:space:]' || true)
        if [[ "$vm_avail_kb" =~ ^[0-9]+$ && "$vm_avail_kb" -lt 3145728 ]]; then
            local vm_avail_mb=$(( vm_avail_kb / 1024 ))
            log_warn "Podman Machine root filesystem has only ${vm_avail_mb} MiB free (< 3 GiB). Rust builds may fail with 'No space left on device'."
            log_warn "Tip: Run 'podman system prune -af' or recreate the VM ('podman machine rm -f && podman machine init --cpus 4 --memory 8192 --disk-size 50')."
        fi
        if [[ "$vm_mem_mb" =~ ^[0-9]+$ && "$vm_mem_mb" -lt 4096 ]]; then
            log_warn "Podman Machine has only ${vm_mem_mb} MiB RAM (< 4 GiB). Consider 'podman machine stop && podman machine set --memory 8192 && podman machine start'."
        fi
    fi

    log_success "Podman & '${COMPOSE_CMD[*]}' are ready (Host alias: ${HOST_ALIAS})."
}

ensure_docker_ready() {
    if ! command -v docker &> /dev/null || ! detect_docker_compose_cmd; then
        log_warn "Docker or 'docker compose' is not yet installed on this system."
        echo -e "What would you like to do?"
        echo -e "  1) Install Docker (${CONTAINER_RUNTIME})"
        echo -e "  2) Abort"
        read -r -p "Please select [1/2]: " docker_choice
        case "$docker_choice" in
            1) install_docker_runtime ;;
            *) echo -e "${RED}Setup aborted.${NC}"; exit 1 ;;
        esac
    fi

    if ! docker info &> /dev/null 2>&1; then
        log_warn "Docker is installed, but the daemon is not currently running."
        if [[ "$OS_TYPE" == "Darwin" && "$CONTAINER_RUNTIME" == "colima" ]]; then
            if ! command -v colima &> /dev/null; then
                if command -v brew &> /dev/null; then
                    log_info "Installing Colima via Homebrew..."
                    brew install colima
                else
                    log_error "Colima is not installed and Homebrew is unavailable."
                    exit 1
                fi
            fi
            log_info "Starting Colima VM (4 CPUs, 8 GB RAM)..."
            colima start --cpu 4 --memory 8 --vm-type vz --vz-rosetta --mount-type virtiofs \
                || colima start --cpu 4 --memory 8 --vm-type vz --vz-rosetta \
                || colima start --cpu 4 --memory 8
        elif [[ "$OS_TYPE" == "Darwin" && -d "/Applications/Docker.app" ]]; then
            log_info "Starting Docker Desktop..."
            open -a Docker
            for _ in {1..30}; do
                docker info &> /dev/null 2>&1 && break
                sleep 2
            done
        elif [[ "$OS_TYPE" == "Darwin" ]] && command -v colima &> /dev/null; then
            log_info "Starting Colima VM (4 CPUs, 8 GB RAM)..."
            colima start --cpu 4 --memory 8 --vm-type vz --vz-rosetta --mount-type virtiofs \
                || colima start --cpu 4 --memory 8 --vm-type vz --vz-rosetta \
                || colima start --cpu 4 --memory 8
        else
            log_error "Please start the Docker daemon (or Colima) and run ./setup.sh again."
            exit 1
        fi
    fi

    if ! docker info &> /dev/null 2>&1; then
        log_error "Docker daemon is still not reachable after startup attempt."
        exit 1
    fi

    detect_docker_compose_cmd || {
        log_error "Neither 'docker compose' nor 'docker-compose' is available."
        exit 1
    }

    CONTAINER_CLI="docker"
    HOST_ALIAS="host.docker.internal"
    log_success "Docker (${CONTAINER_RUNTIME}) & '${COMPOSE_CMD[*]}' are ready (Host alias: ${HOST_ALIAS})."
}

select_container_runtime
if [[ "$CONTAINER_RUNTIME" == "podman" ]]; then
    ensure_podman_ready
else
    ensure_docker_ready
fi

# ==============================================================================
# 2. Check hardware resources (OS, architecture, RAM, CPU cores, Metal / NVIDIA GPU)
# ==============================================================================

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

# ==============================================================================
# 3. Create or update .env file
# ==============================================================================

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

update_env_var "CONTAINER_RUNTIME" "$CONTAINER_RUNTIME"
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

# Align local host alias in LLM_BASE_URL when switching between Docker/Colima and Podman
if [[ -z "$LLM_URL_ARG" && -n "${LLM_BASE_URL:-}" ]]; then
    if [[ "$CONTAINER_RUNTIME" == "podman" && "$LLM_BASE_URL" == *"://host.docker.internal"* ]]; then
        LLM_BASE_URL="${LLM_BASE_URL//:\/\/host.docker.internal/:\/\/host.containers.internal}"
        update_env_var "LLM_BASE_URL" "$LLM_BASE_URL"
    elif [[ "$CONTAINER_RUNTIME" != "podman" && "$LLM_BASE_URL" == *"://host.containers.internal"* ]]; then
        LLM_BASE_URL="${LLM_BASE_URL//:\/\/host.containers.internal/:\/\/host.docker.internal}"
        update_env_var "LLM_BASE_URL" "$LLM_BASE_URL"
    fi
fi

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
        if "$CONTAINER_CLI" ps --format '{{.Names}} {{.Ports}}' 2>/dev/null | grep -E "^rag-" | grep -q ":${p}->"; then
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
    update_env_var "TEI_EMBED_URL" "http://${HOST_ALIAS}:${TEI_EMBED_PORT:-8081}"
    update_env_var "TEI_RERANK_URL" "http://${HOST_ALIAS}:${TEI_RERANK_PORT:-8082}"
    update_env_var "TEI_RERANK_FAST_URL" "http://${HOST_ALIAS}:${TEI_RERANK_FAST_PORT:-8083}"
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
echo -e "  • Runtime         : ${CONTAINER_RUNTIME} (${COMPOSE_CMD[*]})"
echo -e "  • Compute Profile : ${MODE}"
echo -e "  • Model Preset    : ${PRESET} (${EMBEDDING_MODEL}, ${EMBEDDING_DIM}d + ${RERANKER_MODEL})"
echo -e "  • TEI Endpoints   : Embed=${TEI_EMBED_URL} | Rerank=${TEI_RERANK_URL}"
echo -e "  • LLM Provider    : ${LLM_PROVIDER} (${LLM_MODEL} @ ${LLM_BASE_URL})"
echo -e "  • SurrealDB Port  : ${SURREAL_PORT}"
echo -e "  • RAG API Port    : ${ORCHESTRATOR_PORT}"

# ==============================================================================
# 4. Launch Services (Metal Native TEI + Containers, or Full Container Stack)
# ==============================================================================

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

    echo -e "\n${BLUE}[2/4] Building and starting SurrealDB & Rust RAG Orchestrator via ${CONTAINER_RUNTIME} (native arm64)...${NC}"
    "${COMPOSE_CMD[@]}" up -d --build surrealdb rag-orchestrator
else
    # 4B. Full Container Compose stack for 'cpu' or 'gpu' (NVIDIA)
    echo -e "\n${BLUE}[1/3] Building and starting containers via ${CONTAINER_RUNTIME} (Profile: ${MODE})...${NC}"
    "${COMPOSE_CMD[@]}" --profile "$MODE" up -d --build
fi

# ==============================================================================
# 5. Wait for SurrealDB healthcheck and load schema
# ==============================================================================

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

# ==============================================================================
# 6. Wait for TEI Embedding & Reranker readiness
# ==============================================================================

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

# ==============================================================================
# 7. Check and warm up local Ollama if configured
# ==============================================================================

if [[ "$LLM_PROVIDER" == "openai_compatible" ]] && [[ "$LLM_BASE_URL" == *"11434"* ]]; then
    echo -e "\n${BLUE}Checking local Ollama at http://localhost:11434 ...${NC}"
    if ! curl -sf http://localhost:11434/api/tags > /dev/null 2>&1; then
        if command -v ollama &> /dev/null; then
            echo -e "${YELLOW}Ollama not running -> starting 'ollama serve' in background...${NC}"
            mkdir -p .pids logs
            OLLAMA_HOST=0.0.0.0 OLLAMA_KEEP_ALIVE=-1 nohup ollama serve > logs/ollama.log 2>&1 &
            echo $! > .pids/ollama.pid
            for _ in {1..20}; do
                curl -sf http://localhost:11434/api/tags > /dev/null 2>&1 && break
                sleep 1
            done
        fi
    fi

    # Check if the user previously selected another model via Chat-UI (stored in SurrealDB)
    ACTIVE_TARGET_MODEL="${LLM_MODEL}"
    SURREAL_SETTING=$(curl -s "http://localhost:${SURREAL_PORT}/sql" \
        -u "${SURREAL_USER}:${SURREAL_PASS}" \
        -H "Accept: application/json" \
        -H "surreal-ns: ${SURREAL_NS}" \
        -H "surreal-db: ${SURREAL_DB}" \
        -d "SELECT llm_model FROM system_settings:app;" 2>/dev/null || true)
    STORED_MODEL=$(echo "$SURREAL_SETTING" | grep -o '"llm_model":"[^"]*"' | head -n 1 | cut -d'"' -f4 || true)
    if [[ -n "$STORED_MODEL" && "$STORED_MODEL" != "null" ]]; then
        ACTIVE_TARGET_MODEL="$STORED_MODEL"
        echo -e "${GREEN}[PERSISTENCE] Restoring user-selected LLM model from SurrealDB: '${ACTIVE_TARGET_MODEL}'${NC}"
        update_env_var "LLM_MODEL" "$ACTIVE_TARGET_MODEL"
    fi

    echo -e "${BLUE}Pre-loading local Ollama model '${ACTIVE_TARGET_MODEL}' (keep_alive: -1, num_ctx: 4096)...${NC}"
    if curl -sf http://localhost:11434/api/chat -d "{\"model\": \"${ACTIVE_TARGET_MODEL}\", \"messages\": [], \"keep_alive\": -1, \"options\": {\"num_ctx\": 4096}}" > /dev/null 2>&1; then
        echo -e "${GREEN}[OK] '${ACTIVE_TARGET_MODEL}' is ready and resident in memory (no cold-start delay).${NC}"
    else
        echo -e "${YELLOW}[NOTICE] Could not preload model '${ACTIVE_TARGET_MODEL}' at http://localhost:11434. Please check 'ollama list'.${NC}"
    fi
fi

echo -e "\n${GREEN}==============================================================================${NC}"
echo -e "${GREEN}   RAG Stack successfully started! (Runtime: ${CONTAINER_RUNTIME} | Mode: ${MODE}) ${NC}"
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
