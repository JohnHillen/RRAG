#!/usr/bin/env bash
# ==============================================================================
# Automated RunPod GPU Embedding Offload via RunPod REST API v2 & SSH Tunnel
# Usage:
#   ./runpod_embed.sh start   -> Deploys A40/4090 Pod via RunPod API v2, opens SSH tunnel (:18081), switches rag-orchestrator
#   ./runpod_embed.sh status  -> Checks active RunPod Pod & SSH tunnel status
#   ./runpod_embed.sh stop    -> Terminates RunPod Pod via API v2, closes SSH tunnel, restores local Mac embeddings (:8081)
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

STATE_DIR="$SCRIPT_DIR/.logs"
mkdir -p "$STATE_DIR"
POD_ID_FILE="$STATE_DIR/runpod_embed_pod.id"
TUNNEL_PID_FILE="$STATE_DIR/runpod_ssh_tunnel.pid"

if [ -f "$SCRIPT_DIR/.env" ]; then
    while IFS= read -r line || [ -n "$line" ]; do
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        if [ -z "$line" ] || [[ "$line" == \#* ]] || [[ "$line" != *=* ]]; then
            continue
        fi
        key="${line%%=*}"
        val="${line#*=}"
        if [[ "$val" == \"*\" && "$val" == *\" ]]; then
            val="${val:1:${#val}-2}"
        elif [[ "$val" == \'*\' && "$val" == *\' ]]; then
            val="${val:1:${#val}-2}"
        fi
        export "$key=$val"
    done < "$SCRIPT_DIR/.env"
fi

RUNPOD_API_KEY="${RUNPOD_API_KEY:-}"
RUNPOD_GPU_TYPE="${RUNPOD_GPU_TYPE:-NVIDIA A40}"
RUNPOD_FALLBACK_GPU_TYPE="${RUNPOD_FALLBACK_GPU_TYPE:-NVIDIA GeForce RTX 4090}"
TEI_EMBED_SSH_TUNNEL_PORT="${TEI_EMBED_SSH_TUNNEL_PORT:-18081}"
EMBEDDING_MODEL="${EMBEDDING_MODEL:-BAAI/bge-m3}"

SSH_KEY_PATH="${RUNPOD_SSH_KEY_PATH:-$HOME/.ssh/id_ed25519_runpod}"
# Expand leading ~/ if present
SSH_KEY_PATH="${SSH_KEY_PATH/#\~/$HOME}"
if [ ! -f "$SSH_KEY_PATH" ] && [ -f "$HOME/.ssh/id_ed25519" ]; then
    SSH_KEY_PATH="$HOME/.ssh/id_ed25519"
elif [ ! -f "$SSH_KEY_PATH" ] && [ -f "$HOME/.ssh/id_rsa" ]; then
    SSH_KEY_PATH="$HOME/.ssh/id_rsa"
fi

if [ ! -f "$SSH_KEY_PATH" ]; then
    echo "❌ Error: SSH private key not found at $SSH_KEY_PATH"
    echo "   Set RUNPOD_SSH_KEY_PATH in .env (e.g. RUNPOD_SSH_KEY_PATH=~/.ssh/id_ed25519_runpod)"
    exit 1
fi

check_api_key() {
    if [ -z "$RUNPOD_API_KEY" ] || [ "$RUNPOD_API_KEY" = "your_runpod_api_key_here" ]; then
        echo "❌ Error: RUNPOD_API_KEY is not set in .env"
        echo "   1. Create an API key at: https://www.runpod.io/console/user/settings (API Keys)"
        echo "   2. Add it to .env:       RUNPOD_API_KEY=rpa_..."
        exit 1
    fi
}

runpod_rest() {
    local method="$1"
    local path="$2"
    local body="${3:-}"
    if [ -n "$body" ]; then
        curl -sS -X "$method" "https://api.runpod.io${path}" \
            -H "Authorization: Bearer ${RUNPOD_API_KEY}" \
            -H "Content-Type: application/json" \
            -d "$body"
    else
        curl -sS -X "$method" "https://api.runpod.io${path}" \
            -H "Authorization: Bearer ${RUNPOD_API_KEY}"
    fi
}

update_env_var() {
    local key="$1"
    local val="$2"
    export "$key=$val"
    if [ -f "$SCRIPT_DIR/.env" ]; then
        python3 - "$SCRIPT_DIR/.env" "$key" "$val" << 'PYEOF'
import sys, re
path, key, val = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path, "r", encoding="utf-8") as f:
    lines = f.readlines()
replaced = False
out = []
for line in lines:
    if re.match(rf"^\s*{re.escape(key)}=", line):
        if not replaced:
            out.append(f"{key}={val}\n")
            replaced = True
    else:
        out.append(line)
if not replaced:
    out.append(f"{key}={val}\n")
with open(path, "w", encoding="utf-8") as f:
    f.writelines(out)
PYEOF
    fi
}

build_create_pod_payload() {
    local gpu_type="$1"
    python3 - "$gpu_type" "$EMBEDDING_MODEL" << 'PYEOF'
import json, sys
gpu_type, model_id = sys.argv[1], sys.argv[2]

container_script = f"""
if [ -x /start.sh ]; then
    /start.sh &
    sleep 2
else
    service ssh start || /usr/sbin/sshd
fi
service nginx stop >/dev/null 2>&1 || pkill -9 nginx >/dev/null 2>&1 || true
pip install -q "transformers<4.49" "sentence-transformers<3.4" fastapi uvicorn
python3 - << 'inner_py' > /var/log/tei_embed.log 2>&1 &
import torch, uvicorn
from fastapi import FastAPI, Request
from sentence_transformers import SentenceTransformer

model_id = "{model_id}"
model = SentenceTransformer(model_id, device="cuda", model_kwargs={{"torch_dtype": torch.float16}})
model.max_seq_length = 8192
app = FastAPI()

@app.get("/info")
async def info():
    return {{"model_id": model_id, "device": torch.cuda.get_device_name(0), "dtype": "float16"}}

@app.post("/embed")
async def embed(req: Request):
    data = await req.json()
    inp = data.get("inputs", [])
    if isinstance(inp, str):
        inp = [inp]
    vecs = model.encode(inp, batch_size=64, normalize_embeddings=True, show_progress_bar=False)
    return vecs.tolist()

uvicorn.run(app, host="127.0.0.1", port=8081, log_level="warning")
inner_py
sleep infinity
""".strip()

payload = {
    "name": "rag-bge-m3-embed",
    "image": "runpod/pytorch:2.4.0-py3.11-cuda12.4.1-devel-ubuntu22.04",
    "gpu": {
        "id": gpu_type,
        "count": 1,
        "minRamPerGpu": 16,
        "minVcpuCountPerGpu": 4
    },
    "disk": 20,
    "ports": ["22/tcp"],
    "startSsh": True,
    "entrypoint": ["/bin/bash", "-c"],
    "cmd": [container_script]
}
print(json.dumps(payload))
PYEOF
}

start_runpod_tunnel() {
    check_api_key

    # Ensure SSH key is unlocked in ssh-agent if it is passphrase-protected
    if ! ssh-keygen -y -P "" -f "$SSH_KEY_PATH" >/dev/null 2>&1; then
        if ! ssh-add -l >/dev/null 2>&1; then
            echo "🔐 Unlocking SSH key ($SSH_KEY_PATH) via ssh-agent..."
            ssh-add "$SSH_KEY_PATH"
        fi
    fi

    local pod_id=""
    if [ -f "$POD_ID_FILE" ]; then
        local existing_id
        existing_id="$(cat "$POD_ID_FILE")"
        if [ -n "$existing_id" ]; then
            local existing_status
            existing_status="$(runpod_rest GET "/v2/pods/${existing_id}" | python3 -c 'import sys, json; print(json.load(sys.stdin).get("status") or "")' 2>/dev/null || true)"
            if [ "$existing_status" = "RUNNING" ] || [ "$existing_status" = "PROVISIONING" ] || [ "$existing_status" = "STARTING" ]; then
                echo "♻️ Reusing existing active RunPod Pod: ID = $existing_id (Status: $existing_status)"
                pod_id="$existing_id"
            else
                rm -f "$POD_ID_FILE"
            fi
        fi
    fi

    if [ -z "$pod_id" ]; then
        echo "🚀 Deploying RunPod GPU Pod ('${RUNPOD_GPU_TYPE}') via RunPod REST API v2..."

        local deploy_payload
        deploy_payload="$(build_create_pod_payload "$RUNPOD_GPU_TYPE")"

        local resp
        resp="$(runpod_rest POST "/v2/pods" "$deploy_payload")"
        pod_id="$(echo "$resp" | python3 -c "import sys, json; d=json.load(sys.stdin); print(d.get('id') or '')" 2>/dev/null || true)"

        if [ -z "$pod_id" ] && [ -n "$RUNPOD_FALLBACK_GPU_TYPE" ]; then
            echo "⚠️ '${RUNPOD_GPU_TYPE}' unavailable (${resp}). Trying fallback GPU '${RUNPOD_FALLBACK_GPU_TYPE}'..."
            deploy_payload="$(build_create_pod_payload "$RUNPOD_FALLBACK_GPU_TYPE")"
            resp="$(runpod_rest POST "/v2/pods" "$deploy_payload")"
            pod_id="$(echo "$resp" | python3 -c "import sys, json; d=json.load(sys.stdin); print(d.get('id') or '')" 2>/dev/null || true)"
        fi

        if [ -z "$pod_id" ]; then
            echo "❌ Failed to deploy RunPod Pod. API response:"
            echo "$resp"
            exit 1
        fi

        echo "$pod_id" > "$POD_ID_FILE"
        echo "✅ RunPod Pod created: ID = $pod_id"
    fi
    echo "⏳ Waiting for Pod image pull & direct SSH port assignment (22/tcp)..."

    local ssh_ip=""
    local ssh_port=""
    local ssh_user="root"
    for attempt in $(seq 1 180); do
        local status_resp
        status_resp="$(runpod_rest GET "/v2/pods/${pod_id}" || true)"
        local parsed
        parsed="$(echo "$status_resp" | python3 -c '
import sys, json
d = json.load(sys.stdin)
if d.get("status") == "RUNNING":
    direct = (d.get("ssh") or {}).get("direct") or {}
    if direct.get("host") and direct.get("port"):
        u = direct.get("username") or "root"
        h = direct["host"]
        p = direct["port"]
        print(f"{u}@{h}:{p}")
        sys.exit(0)
    rt = d.get("runtime") or {}
    for pt in (rt.get("ports") or []):
        priv = pt.get("privatePort") or pt.get("private")
        pub = pt.get("publicPort") or pt.get("public")
        ip = pt.get("ip")
        if priv == 22 and ip and pub:
            print(f"root@{ip}:{pub}")
            sys.exit(0)
' 2>/dev/null || true)"

        if [ -n "$parsed" ]; then
            ssh_user="${parsed%%@*}"
            local hostport="${parsed#*@}"
            ssh_ip="${hostport%%:*}"
            ssh_port="${hostport##*:}"
            break
        fi
        if (( attempt % 5 == 0 )); then
            echo "   ... still pulling container image on RunPod node ($((attempt * 3))s elapsed)"
        fi
        sleep 3
    done

    if [ -z "$ssh_ip" ] || [ -z "$ssh_port" ]; then
        echo "❌ Timed out waiting for RunPod SSH port assignment. Run './runpod_embed.sh stop' to terminate the pod."
        exit 1
    fi

    echo "🔒 Pod SSH endpoint ready: ${ssh_user}@${ssh_ip} (TCP Port ${ssh_port})"
    echo "⏳ Waiting for sshd inside container to accept TCP connections..."

    for _ in $(seq 1 40); do
        if nc -z -w 3 "$ssh_ip" "$ssh_port" >/dev/null 2>&1; then
            break
        fi
        sleep 2
    done

    echo "⏳ Opening encrypted SSH tunnel on local port ${TEI_EMBED_SSH_TUNNEL_PORT} -> RunPod 127.0.0.1:8081..."
    pkill -f "0.0.0.0:${TEI_EMBED_SSH_TUNNEL_PORT}:127.0.0.1:8081" >/dev/null 2>&1 || true

    for _ in $(seq 1 15); do
        if ssh -f -N \
            -o StrictHostKeyChecking=no \
            -o UserKnownHostsFile=/dev/null \
            -o ServerAliveInterval=15 \
            -o ConnectTimeout=5 \
            -o ExitOnForwardFailure=yes \
            -L "0.0.0.0:${TEI_EMBED_SSH_TUNNEL_PORT}:127.0.0.1:8081" \
            -p "$ssh_port" \
            -i "$SSH_KEY_PATH" \
            "${ssh_user}@${ssh_ip}"; then
            break
        fi
        sleep 3
    done

    pgrep -f "0.0.0.0:${TEI_EMBED_SSH_TUNNEL_PORT}:127.0.0.1:8081" | head -n 1 > "$TUNNEL_PID_FILE" || true

    echo "⏳ Waiting for ${EMBEDDING_MODEL} CUDA server inside RunPod to finish downloading & loading weights (~45–90s)..."
    local server_ready=0
    for _ in $(seq 1 90); do
        if curl -sf "http://localhost:${TEI_EMBED_SSH_TUNNEL_PORT}/info" 2>/dev/null | grep -q '"model_id"'; then
            echo "✅ RunPod GPU Embedding Server is online through SSH tunnel:"
            curl -s "http://localhost:${TEI_EMBED_SSH_TUNNEL_PORT}/info"
            echo ""
            server_ready=1
            break
        fi
        sleep 3
    done

    if [ "$server_ready" -ne 1 ]; then
        echo "⚠️ Embedding server did not respond on port ${TEI_EMBED_SSH_TUNNEL_PORT} within timeout."
        echo "   Check './runpod_embed.sh status' in a few seconds or run './runpod_embed.sh stop' to abort."
        exit 1
    fi

    update_env_var "TEI_EMBED_URL" "http://host.docker.internal:${TEI_EMBED_SSH_TUNNEL_PORT}"
    docker compose up -d rag-orchestrator

    echo ""
    echo "🎉 Ready! Your local rag-orchestrator (http://localhost:8080) now embeds via the RunPod GPU over SSH."
    echo "👉 When finished uploading, run: ./runpod_embed.sh stop"
}

stop_runpod_tunnel() {
    if [ -f "$TUNNEL_PID_FILE" ]; then
        local tpid
        tpid="$(cat "$TUNNEL_PID_FILE")"
        if [ -n "$tpid" ]; then
            kill "$tpid" >/dev/null 2>&1 || true
        fi
        rm -f "$TUNNEL_PID_FILE"
    fi
    pkill -f "0.0.0.0:${TEI_EMBED_SSH_TUNNEL_PORT}:127.0.0.1:8081" >/dev/null 2>&1 || true

    if [ -f "$POD_ID_FILE" ]; then
        check_api_key
        local pod_id
        pod_id="$(cat "$POD_ID_FILE")"
        if [ -n "$pod_id" ]; then
            echo "🗑️ Terminating RunPod Pod ($pod_id) via RunPod REST API v2..."
            runpod_rest DELETE "/v2/pods/${pod_id}" >/dev/null || true
            echo "✅ RunPod Pod ($pod_id) terminated."
        fi
        rm -f "$POD_ID_FILE"
    fi

    update_env_var "TEI_EMBED_URL" "http://host.docker.internal:${TEI_EMBED_PORT:-8081}"
    docker compose up -d rag-orchestrator
    echo "✅ Restored TEI_EMBED_URL to local Mac (http://host.docker.internal:${TEI_EMBED_PORT:-8081})."
}

status_runpod_tunnel() {
    if [ -f "$POD_ID_FILE" ]; then
        local pod_id
        pod_id="$(cat "$POD_ID_FILE")"
        echo "🟢 Active RunPod Pod ID: $pod_id"
        if [ -n "$RUNPOD_API_KEY" ]; then
            runpod_rest GET "/v2/pods/${pod_id}" | python3 -c '
import sys, json
d = json.load(sys.stdin)
st = d.get("status")
gpu_id = (d.get("gpu") or {}).get("id")
cost = d.get("cost")
print(f"   Status: {st} | GPU: {gpu_id} | Cost: ${cost}/hr")
' 2>/dev/null || true
        fi
    else
        echo "⚪ No active RunPod Pod ID recorded."
    fi
    if curl -sf "http://localhost:${TEI_EMBED_SSH_TUNNEL_PORT}/info" 2>/dev/null | grep -q '"model_id"'; then
        echo "🟢 SSH Tunnel on port ${TEI_EMBED_SSH_TUNNEL_PORT} is ONLINE:"
        curl -s "http://localhost:${TEI_EMBED_SSH_TUNNEL_PORT}/info"
        echo ""
    else
        echo "⚪ SSH Tunnel on port ${TEI_EMBED_SSH_TUNNEL_PORT} is not connected."
    fi
}

case "${1:-}" in
    start)
        start_runpod_tunnel
        ;;
    stop)
        stop_runpod_tunnel
        ;;
    status)
        status_runpod_tunnel
        ;;
    *)
        echo "Usage: $0 {start|status|stop}"
        exit 1
        ;;
esac
