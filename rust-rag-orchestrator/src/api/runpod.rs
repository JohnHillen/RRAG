use crate::{db::extract_result_array, AppState};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
    sync::{Arc, OnceLock, RwLock},
    time::Duration,
};
use tracing::{info, warn};

const TUNNEL_ACTIVE_FLAG: &str = "/tmp/runpod_ui_tunnel.active";
const UNLOCKED_KEY_PATH: &str = "/tmp/runpod_ui_unlocked_ed25519";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpodConfigPayload {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub ssh_private_key: Option<String>,
    #[serde(default)]
    pub ssh_passphrase: Option<String>,
    #[serde(default = "default_gpu_type")]
    pub gpu_type: String,
    #[serde(default = "default_true")]
    pub enable_llm: bool,
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    #[serde(default = "default_true")]
    pub enable_embedding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpodGpuTypeItem {
    pub id: String,
    pub display_name: String,
    pub memory_gb: u64,
    pub secure_cloud: bool,
    pub community_cloud: bool,
    pub secure_price: Option<f64>,
    pub community_price: Option<f64>,
    pub lowest_price: Option<f64>,
    pub secure_stock: Option<String>,
    pub community_stock: Option<String>,
    pub available: bool,
}

fn normalize_gpu_type_id(raw: &str) -> String {
    match raw.trim() {
        "A40" => "NVIDIA A40".to_string(),
        "RTX 4090" => "NVIDIA GeForce RTX 4090".to_string(),
        "RTX 3090" => "NVIDIA GeForce RTX 3090".to_string(),
        "RTX A6000" => "NVIDIA RTX A6000".to_string(),
        "L40S" => "NVIDIA L40S".to_string(),
        other => other.to_string(),
    }
}

fn is_valid_runpod_api_key(k: &str) -> bool {
    let t = k.trim();
    !t.is_empty()
        && !t.eq_ignore_ascii_case("api_key")
        && !t.eq_ignore_ascii_case("your_runpod_api_key_here")
        && t.len() >= 12
}

fn derive_keystream_block(secret_seed: &str, block_idx: usize) -> [u8; 16] {
    let input = format!("rp_seal_v1:{}:{}", secret_seed, block_idx);
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x84222325cbf29ce4;
    for (i, &b) in input.as_bytes().iter().enumerate() {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(0x100000001b3).rotate_left(7);
        h2 ^= (b as u64).wrapping_add(i as u64);
        h2 = h2.wrapping_mul(0x100000001b3).rotate_left(11);
    }
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&h1.to_le_bytes());
    out[8..].copy_from_slice(&h2.to_le_bytes());
    out
}

/// Encrypts a credential string using a keystream derived from the instance secret (`surreal_pass`)
/// so credentials are never stored in plaintext on disk in SurrealKV.
fn seal_credential(plain: &str, secret_seed: &str) -> String {
    if plain.is_empty() {
        return String::new();
    }
    let bytes = plain.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    for (i, &b) in bytes.iter().enumerate() {
        let block = derive_keystream_block(secret_seed, i / 16);
        out.push(b ^ block[i % 16]);
    }
    format!("enc_v1:{}", crate::ingestion::pdf::base64_encode(&out))
}

/// Decrypts a credential string previously sealed with `seal_credential`.
fn unseal_credential(stored: &str, secret_seed: &str) -> String {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(b64) = trimmed.strip_prefix("enc_v1:") {
        let enc_bytes = crate::ingestion::pdf::base64_decode(b64);
        let mut out = Vec::with_capacity(enc_bytes.len());
        for (i, &b) in enc_bytes.iter().enumerate() {
            let block = derive_keystream_block(secret_seed, i / 16);
            out.push(b ^ block[i % 16]);
        }
        String::from_utf8(out).unwrap_or_default()
    } else {
        trimmed.to_string()
    }
}

fn default_gpu_type() -> String {
    "NVIDIA A40".to_string()
}

fn default_llm_model() -> String {
    "qwen3:8b".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpodRuntimeState {
    pub phase: String, // "idle" | "deploying" | "waiting_ssh" | "loading_models" | "running" | "stopping" | "error"
    pub message: String,
    pub pod_id: Option<String>,
    pub pod_status: Option<String>,
    pub gpu_type: String,
    pub cost_per_hr: Option<f64>,
    pub ssh_endpoint: Option<String>,
    pub tunnel_connected: bool,
    pub enable_llm: bool,
    pub llm_model: String,
    pub llm_ready: bool,
    pub enable_embedding: bool,
    pub embed_ready: bool,
    pub api_key: String,
    pub ssh_private_key: String,
    pub ssh_passphrase: String,
    pub prev_llm_provider: Option<String>,
    pub prev_llm_base_url: Option<String>,
    pub prev_llm_model: Option<String>,
}

impl Default for RunpodRuntimeState {
    fn default() -> Self {
        let raw_env_key = std::env::var("RUNPOD_API_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
        let env_api_key = if is_valid_runpod_api_key(&raw_env_key) {
            raw_env_key
        } else {
            String::new()
        };
        let env_ssh_pass = std::env::var("RUNPOD_SSH_PASSPHRASE").unwrap_or_default();
        let env_gpu = std::env::var("RUNPOD_GPU_TYPE")
            .unwrap_or_else(|_| "NVIDIA A40".to_string())
            .trim()
            .to_string();
        let default_ssh_key = load_default_ssh_key_from_disk();

        Self {
            phase: "idle".to_string(),
            message: "Kein aktiver RunPod GPU-Pod gestartet.".to_string(),
            pod_id: None,
            pod_status: None,
            gpu_type: if env_gpu.is_empty() {
                "NVIDIA A40".to_string()
            } else {
                env_gpu
            },
            cost_per_hr: None,
            ssh_endpoint: None,
            tunnel_connected: false,
            enable_llm: true,
            llm_model: "qwen3:8b".to_string(),
            llm_ready: false,
            enable_embedding: true,
            embed_ready: false,
            api_key: env_api_key,
            ssh_private_key: default_ssh_key,
            ssh_passphrase: env_ssh_pass,
            prev_llm_provider: None,
            prev_llm_base_url: None,
            prev_llm_model: None,
        }
    }
}

static RUNPOD_STATE: OnceLock<Arc<RwLock<RunpodRuntimeState>>> = OnceLock::new();

fn global_runpod_state() -> &'static Arc<RwLock<RunpodRuntimeState>> {
    RUNPOD_STATE.get_or_init(|| Arc::new(RwLock::new(RunpodRuntimeState::default())))
}

fn load_default_ssh_key_from_disk() -> String {
    let mut candidates = Vec::new();
    if let Ok(custom) = std::env::var("RUNPOD_SSH_KEY_PATH") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            if let Some(stripped) = trimmed.strip_prefix("~/") {
                if let Ok(home) = std::env::var("HOME") {
                    candidates.push(format!("{}/{}", home, stripped));
                }
                candidates.push(format!("/root/{}", stripped));
            } else {
                candidates.push(trimmed.to_string());
            }
        }
    }
    candidates.push("/root/.ssh/id_ed25519_runpod".to_string());
    candidates.push("/root/.ssh/id_ed25519".to_string());
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{}/.ssh/id_ed25519_runpod", home));
        candidates.push(format!("{}/.ssh/id_ed25519", home));
        candidates.push(format!("{}/.ssh/id_rsa", home));
    }

    for path in candidates {
        if let Ok(content) = fs::read_to_string(&path) {
            let trimmed = content.trim();
            if trimmed.contains("PRIVATE KEY") {
                return format!("{}\n", trimmed);
            }
        }
    }
    String::new()
}

/// Restores RunPod preferences and encrypted credentials (`api_key_enc`, `ssh_passphrase_enc`, `ssh_private_key_enc`)
/// from the default SurrealDB database (`system_settings:runpod`) on startup.
pub async fn restore_runpod_settings_from_db(state: &AppState) {
    let default_db = &state.config.surreal_db;
    let secret_seed = &state.config.surreal_pass;

    let sql = "SELECT api_key_enc, ssh_private_key_enc, ssh_passphrase_enc, gpu_type, enable_llm, llm_model, enable_embedding, pod_id FROM system_settings:runpod;";
    let Ok(res) = state.db.execute_sql_on_db(default_db, sql).await else {
        return;
    };
    let arr = extract_result_array(res.first());
    let Some(item) = arr.first() else {
        return;
    };

    let snapshot = {
        let mut guard = global_runpod_state().write().unwrap();
        if let Some(enc_k) = item
            .get("api_key_enc")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let dec = unseal_credential(enc_k, secret_seed);
            if is_valid_runpod_api_key(&dec) {
                guard.api_key = dec;
            }
        }
        if let Some(enc_sk) = item
            .get("ssh_private_key_enc")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let dec = unseal_credential(enc_sk, secret_seed);
            if dec.contains("PRIVATE KEY") {
                guard.ssh_private_key = format!("{}\n", dec.trim());
            }
        }
        if let Some(enc_sp) = item
            .get("ssh_passphrase_enc")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let dec = unseal_credential(enc_sp, secret_seed);
            if !dec.is_empty() {
                guard.ssh_passphrase = dec;
            }
        }
        if let Some(gpu) = item.get("gpu_type").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            guard.gpu_type = normalize_gpu_type_id(gpu);
        }
        if let Some(en_llm) = item.get("enable_llm").and_then(|v| v.as_bool()) {
            guard.enable_llm = en_llm;
        }
        if let Some(m) = item.get("llm_model").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            guard.llm_model = m.to_string();
        }
        if let Some(en_emb) = item.get("enable_embedding").and_then(|v| v.as_bool()) {
            guard.enable_embedding = en_emb;
        }
        if let Some(pid) = item
            .get("pod_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            guard.pod_id = Some(pid.to_string());
        }
        guard.clone()
    };

    // If an active RunPod API key & valid SSH key exist and a Pod is still running on RunPod, automatically reconnect
    if is_valid_runpod_api_key(&snapshot.api_key) && snapshot.ssh_private_key.contains("PRIVATE KEY") {
        if let Ok(pub_key) =
            prepare_unlocked_ssh_key(&snapshot.ssh_private_key, &snapshot.ssh_passphrase)
        {
            let _ = fs::remove_file(UNLOCKED_KEY_PATH);
            let http = state.db.http_client();
            let mut active_pid = snapshot.pod_id.clone();
            if active_pid.is_none() {
                if let Ok(r) = http
                    .get("https://api.runpod.io/v2/pods")
                    .bearer_auth(&snapshot.api_key)
                    .send()
                    .await
                {
                    if let Ok(val) = r.json::<Value>().await {
                        let arr_opt = val.as_array().or_else(|| val["data"].as_array());
                        if let Some(arr) = arr_opt {
                            for p in arr {
                                if p["name"].as_str() == Some("rag-studio-gpu-pod")
                                    && p["status"].as_str() == Some("RUNNING")
                                {
                                    active_pid = p["id"].as_str().map(|s| s.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if let Some(pid) = active_pid {
                let check_url = format!("https://api.runpod.io/v2/pods/{}", pid);
                if let Ok(r) = http.get(&check_url).bearer_auth(&snapshot.api_key).send().await {
                    if let Ok(d) = r.json::<Value>().await {
                        if d["status"].as_str() == Some("RUNNING") {
                            {
                                let mut g = global_runpod_state().write().unwrap();
                                g.pod_id = Some(pid.clone());
                                g.phase = "waiting_ssh".to_string();
                                g.message = format!(
                                    "🔄 Verbinde automatisch mit laufendem RunPod Pod '{}'...",
                                    pid
                                );
                            }
                            let st_clone = state.clone();
                            tokio::spawn(async move {
                                let _ = orchestrate_runpod_deployment(&st_clone, &pub_key).await;
                                let _ = fs::remove_file(UNLOCKED_KEY_PATH);
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Persists RunPod preferences and encrypted credentials (`api_key_enc`, `ssh_passphrase_enc`, `ssh_private_key_enc`)
/// to `system_settings:runpod` in the default SurrealDB database (`state.config.surreal_db`).
/// Plaintext fields (`api_key`, `ssh_private_key`, `ssh_passphrase`) are always set to `NONE`,
/// and `system_settings` is excluded from `.surql` exports.
async fn persist_runpod_settings_to_db(state: &AppState, st: &RunpodRuntimeState) {
    let default_db = &state.config.surreal_db;
    let secret_seed = &state.config.surreal_pass;

    let enc_api = if is_valid_runpod_api_key(&st.api_key) {
        seal_credential(&st.api_key, secret_seed)
    } else {
        String::new()
    };
    let enc_ssh_key = if st.ssh_private_key.contains("PRIVATE KEY") {
        seal_credential(&st.ssh_private_key, secret_seed)
    } else {
        String::new()
    };
    let enc_ssh_pass = if !st.ssh_passphrase.is_empty() {
        seal_credential(&st.ssh_passphrase, secret_seed)
    } else {
        String::new()
    };

    let pod_id_json = match st.pod_id.as_deref() {
        Some(p) if !p.is_empty() => serde_json::to_string(p).unwrap_or_else(|_| "NONE".into()),
        _ => "NONE".to_string(),
    };
    let sql = format!(
        "UPSERT system_settings:runpod SET api_key = NONE, ssh_private_key = NONE, ssh_passphrase = NONE, api_key_enc = {}, ssh_private_key_enc = {}, ssh_passphrase_enc = {}, gpu_type = {}, enable_llm = {}, llm_model = {}, enable_embedding = {}, pod_id = {}, updated_at = time::now();",
        serde_json::to_string(&enc_api).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&enc_ssh_key).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&enc_ssh_pass).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&st.gpu_type).unwrap_or_else(|_| "\"NVIDIA A40\"".into()),
        st.enable_llm,
        serde_json::to_string(&st.llm_model).unwrap_or_else(|_| "\"qwen3:8b\"".into()),
        st.enable_embedding,
        pod_id_json,
    );
    let _ = state.db.execute_sql_on_db(default_db, &sql).await;
}

/// Checks if the SSH private key can be unlocked with `passphrase`, writes an unencrypted temporary
/// key to `UNLOCKED_KEY_PATH` (`0600`), and returns the derived OpenSSH public key string.
fn prepare_unlocked_ssh_key(private_key_pem: &str, passphrase: &str) -> Result<String> {
    let trimmed = private_key_pem.trim();
    if !trimmed.contains("PRIVATE KEY") {
        anyhow::bail!("Kein gültiger SSH-Private-Key hinterlegt (muss 'BEGIN ... PRIVATE KEY' enthalten).");
    }

    let tmp_check = "/tmp/runpod_key_check.tmp";
    fs::write(tmp_check, format!("{}\n", trimmed))
        .context("Konnte temporäre SSH-Key-Datei nicht schreiben")?;
    let _ = fs::set_permissions(tmp_check, fs::Permissions::from_mode(0o600));

    // Extract public key using provided passphrase (or empty passphrase)
    let pub_out = Command::new("ssh-keygen")
        .args(["-y", "-P", passphrase, "-f", tmp_check])
        .output()
        .context("Konnte 'ssh-keygen' nicht ausführen (ist openssh-client installiert?)")?;

    if !pub_out.status.success() {
        let _ = fs::remove_file(tmp_check);
        if passphrase.is_empty() {
            anyhow::bail!(
                "Der SSH-Key ist mit einer Passphrase geschützt. Bitte trage die SSH-Key Passphrase im Feld ein."
            );
        } else {
            anyhow::bail!(
                "Die eingegebene SSH-Key Passphrase ist falsch oder das Key-Format ist ungültig."
            );
        }
    }

    let pub_key = String::from_utf8_lossy(&pub_out.stdout).trim().to_string();

    // If the key had a passphrase, strip it on the temporary copy in /tmp so background `ssh -f -N` works non-interactively
    if !passphrase.is_empty() {
        let _ = Command::new("ssh-keygen")
            .args(["-p", "-P", passphrase, "-N", "", "-f", tmp_check])
            .output();
    }

    fs::copy(tmp_check, UNLOCKED_KEY_PATH)?;
    let _ = fs::set_permissions(UNLOCKED_KEY_PATH, fs::Permissions::from_mode(0o600));
    let _ = fs::remove_file(tmp_check);

    Ok(pub_key)
}

fn check_key_needs_passphrase(private_key_pem: &str, passphrase: &str) -> bool {
    let trimmed = private_key_pem.trim();
    if !trimmed.contains("PRIVATE KEY") {
        return false;
    }
    let tmp_probe = "/tmp/runpod_key_probe.tmp";
    if fs::write(tmp_probe, format!("{}\n", trimmed)).is_err() {
        return false;
    }
    let _ = fs::set_permissions(tmp_probe, fs::Permissions::from_mode(0o600));
    let ok = Command::new("ssh-keygen")
        .args(["-y", "-P", passphrase, "-f", tmp_probe])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let _ = fs::remove_file(tmp_probe);
    !ok
}

fn is_tunnel_process_alive() -> bool {
    if !Path::new(TUNNEL_ACTIVE_FLAG).exists() {
        return false;
    }
    Command::new("pgrep")
        .args(["-f", "127.0.0.1:11435:127.0.0.1:11434"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || Command::new("pgrep")
            .args(["-f", "127.0.0.1:18081:127.0.0.1:8081"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn kill_tunnel_processes() {
    let _ = fs::remove_file(TUNNEL_ACTIVE_FLAG);
    let _ = fs::remove_file(UNLOCKED_KEY_PATH);
    let _ = Command::new("pkill")
        .args(["-f", "127.0.0.1:11435:127.0.0.1:11434"])
        .output();
    let _ = Command::new("pkill")
        .args(["-f", "127.0.0.1:18081:127.0.0.1:8081"])
        .output();
}

fn build_status_json(state: &AppState, st: &RunpodRuntimeState) -> Value {
    let has_api_key = is_valid_runpod_api_key(&st.api_key);
    let has_ssh_key = st.ssh_private_key.contains("PRIVATE KEY");
    let has_passphrase = !st.ssh_passphrase.is_empty();
    let needs_pass = has_ssh_key && check_key_needs_passphrase(&st.ssh_private_key, &st.ssh_passphrase);
    let llm_cfg = state.llm.get_runtime_config();

    json!({
        "phase": st.phase,
        "message": st.message,
        "pod_id": st.pod_id,
        "pod_status": st.pod_status,
        "gpu_type": st.gpu_type,
        "cost_per_hr": st.cost_per_hr,
        "ssh_endpoint": st.ssh_endpoint,
        "tunnel_connected": st.tunnel_connected,
        "enable_llm": st.enable_llm,
        "llm_model": st.llm_model,
        "llm_ready": st.llm_ready,
        "enable_embedding": st.enable_embedding,
        "embed_ready": st.embed_ready,
        "has_api_key": has_api_key,
        "has_ssh_key": has_ssh_key,
        "has_ssh_passphrase": has_passphrase,
        "ssh_key_needs_passphrase": needs_pass,
        "active_embed_url": state.inference.get_embed_url(),
        "active_llm_url": llm_cfg.llm_base_url,
        "active_llm_model": llm_cfg.llm_model,
    })
}

/// Returns current RunPod Pod & SSH tunnel status (`GET /v1/runpod/status`)
pub async fn get_runpod_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let st = {
        let mut guard = global_runpod_state().write().unwrap();
        if guard.ssh_private_key.is_empty() {
            guard.ssh_private_key = load_default_ssh_key_from_disk();
        }
        guard.gpu_type = normalize_gpu_type_id(&guard.gpu_type);
        if guard.phase == "running" {
            guard.tunnel_connected = is_tunnel_process_alive();
        }
        guard.clone()
    };
    Json(build_status_json(&state, &st))
}

/// Queries RunPod's live GraphQL catalog (`https://api.runpod.io/graphql`) for all GPU types,
/// live Secure/Community hourly pricing, and real-time stock status (`GET /v1/runpod/gpus`).
pub async fn list_runpod_gpus_handler(State(state): State<AppState>) -> impl IntoResponse {
    let query = r#"
    query GpuTypes {
        gpuTypes {
            id
            displayName
            memoryInGb
            secureCloud
            communityCloud
            securePrice
            communityPrice
            anyLow: lowestPrice(input: { gpuCount: 1 }) {
                uninterruptablePrice
                stockStatus
            }
            secLow: lowestPrice(input: { gpuCount: 1, secureCloud: true }) {
                uninterruptablePrice
                stockStatus
            }
            comLow: lowestPrice(input: { gpuCount: 1, secureCloud: false }) {
                uninterruptablePrice
                stockStatus
            }
        }
    }
    "#;

    let http = state.db.http_client();
    let resp_res = http
        .post("https://api.runpod.io/graphql")
        .header("Content-Type", "application/json")
        .json(&json!({ "query": query }))
        .send()
        .await;

    let mut gpus: Vec<RunpodGpuTypeItem> = Vec::new();

    if let Ok(resp) = resp_res {
        if let Ok(val) = resp.json::<Value>().await {
            if let Some(arr) = val["data"]["gpuTypes"].as_array() {
                for g in arr {
                    let id = g["id"].as_str().unwrap_or_default().trim().to_string();
                    let mem = g["memoryInGb"].as_u64().unwrap_or(0);
                    if id.is_empty() || id.eq_ignore_ascii_case("unknown") || mem == 0 {
                        continue;
                    }
                    let display_name = g["displayName"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(&id)
                        .trim()
                        .to_string();
                    let secure_cloud = g["secureCloud"].as_bool().unwrap_or(false);
                    let community_cloud = g["communityCloud"].as_bool().unwrap_or(false);
                    if !secure_cloud && !community_cloud {
                        continue;
                    }

                    let secure_price = if secure_cloud {
                        g["secLow"]["uninterruptablePrice"]
                            .as_f64()
                            .filter(|p| *p > 0.0)
                            .or_else(|| g["securePrice"].as_f64().filter(|p| *p > 0.0))
                    } else {
                        None
                    };

                    let community_price = if community_cloud {
                        g["comLow"]["uninterruptablePrice"]
                            .as_f64()
                            .filter(|p| *p > 0.0)
                            .or_else(|| g["communityPrice"].as_f64().filter(|p| *p > 0.0))
                    } else {
                        None
                    };

                    let lowest_price = match (secure_price, community_price) {
                        (Some(sp), Some(cp)) => Some(sp.min(cp)),
                        (Some(sp), None) => Some(sp),
                        (None, Some(cp)) => Some(cp),
                        (None, None) => g["anyLow"]["uninterruptablePrice"]
                            .as_f64()
                            .filter(|p| *p > 0.0),
                    };

                    let secure_stock = if secure_cloud {
                        g["secLow"]["stockStatus"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                    } else {
                        None
                    };

                    let community_stock = if community_cloud {
                        g["comLow"]["stockStatus"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                    } else {
                        None
                    };

                    let available = secure_stock.is_some()
                        || community_stock.is_some()
                        || g["anyLow"]["stockStatus"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .is_some();

                    gpus.push(RunpodGpuTypeItem {
                        id,
                        display_name,
                        memory_gb: mem,
                        secure_cloud,
                        community_cloud,
                        secure_price,
                        community_price,
                        lowest_price,
                        secure_stock,
                        community_stock,
                        available,
                    });
                }
            }
        }
    }

    gpus.sort_by(|a, b| {
        a.memory_gb
            .cmp(&b.memory_gb)
            .then_with(|| {
                let pa = a.lowest_price.unwrap_or(999.0);
                let pb = b.lowest_price.unwrap_or(999.0);
                pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    Json(json!({ "gpus": gpus }))
}

/// Saves and validates RunPod credentials & settings (`POST /v1/runpod/config`)
pub async fn save_runpod_config_handler(
    State(state): State<AppState>,
    Json(req): Json<RunpodConfigPayload>,
) -> impl IntoResponse {
    let mut updated_state = {
        let mut guard = global_runpod_state().write().unwrap();
        if let Some(k) = req.api_key {
            let t = k.trim();
            if !t.is_empty() {
                guard.api_key = t.to_string();
            }
        }
        if let Some(sk) = req.ssh_private_key {
            let t = sk.trim();
            if !t.is_empty() {
                guard.ssh_private_key = format!("{}\n", t);
            }
        }
        if let Some(sp) = req.ssh_passphrase {
            if !sp.is_empty() {
                guard.ssh_passphrase = sp;
            }
        }
        if !req.gpu_type.trim().is_empty() {
            guard.gpu_type = req.gpu_type.trim().to_string();
        }
        guard.enable_llm = req.enable_llm;
        if !req.llm_model.trim().is_empty() {
            guard.llm_model = req.llm_model.trim().to_string();
        }
        guard.enable_embedding = req.enable_embedding;
        guard.clone()
    };

    persist_runpod_settings_to_db(&state, &updated_state).await;

    // Validate SSH key + passphrase immediately in RAM and wipe temporary unlocked file right away
    if updated_state.ssh_private_key.contains("PRIVATE KEY") {
        let res = prepare_unlocked_ssh_key(
            &updated_state.ssh_private_key,
            &updated_state.ssh_passphrase,
        );
        let _ = fs::remove_file(UNLOCKED_KEY_PATH);
        match res {
            Ok(_) => {
                if updated_state.phase == "idle" || updated_state.phase == "error" {
                    updated_state.message = "✅ RunPod API-Key & SSH-Key erfolgreich geprüft und verschlüsselt gespeichert (überlebt Seiten-Reloads & Container-Neustarts).".to_string();
                    if updated_state.phase == "error" {
                        updated_state.phase = "idle".to_string();
                    }
                }
            }
            Err(e) => {
                updated_state.message = format!("⚠️ {}", e);
            }
        }
        let mut guard = global_runpod_state().write().unwrap();
        guard.message = updated_state.message.clone();
        guard.phase = updated_state.phase.clone();
    }

    (StatusCode::OK, Json(build_status_json(&state, &updated_state)))
}

/// Starts a RunPod GPU Pod (or switches model on an already running Pod) and opens the SSH tunnel (`POST /v1/runpod/start`)
pub async fn start_runpod_handler(
    State(state): State<AppState>,
    Json(req): Json<RunpodConfigPayload>,
) -> impl IntoResponse {
    // 1. Update saved config fields if provided
    let snapshot = {
        let mut guard = global_runpod_state().write().unwrap();
        if guard.phase == "deploying" || guard.phase == "waiting_ssh" || guard.phase == "loading_models" {
            return (
                StatusCode::CONFLICT,
                Json(build_status_json(&state, &guard)),
            );
        }
        if let Some(k) = req.api_key {
            let t = k.trim();
            if !t.is_empty() {
                guard.api_key = t.to_string();
            }
        }
        if let Some(sk) = req.ssh_private_key {
            let t = sk.trim();
            if !t.is_empty() {
                guard.ssh_private_key = format!("{}\n", t);
            }
        }
        if let Some(sp) = req.ssh_passphrase {
            if !sp.is_empty() {
                guard.ssh_passphrase = sp;
            }
        }
        if !req.gpu_type.trim().is_empty() {
            guard.gpu_type = req.gpu_type.trim().to_string();
        }
        guard.enable_llm = req.enable_llm;
        if !req.llm_model.trim().is_empty() {
            guard.llm_model = req.llm_model.trim().to_string();
        }
        guard.enable_embedding = req.enable_embedding;
        guard.clone()
    };

    persist_runpod_settings_to_db(&state, &snapshot).await;

    if !is_valid_runpod_api_key(&snapshot.api_key) {
        let mut guard = global_runpod_state().write().unwrap();
        guard.phase = "error".to_string();
        guard.message = "❌ Bitte zuerst einen gültigen RunPod API-Key (rpa_...) eingeben.".to_string();
        return (StatusCode::BAD_REQUEST, Json(build_status_json(&state, &guard)));
    }

    if !snapshot.enable_llm && !snapshot.enable_embedding {
        let mut guard = global_runpod_state().write().unwrap();
        guard.phase = "error".to_string();
        guard.message = "❌ Bitte mindestens einen Dienst auswählen (LLM und/oder GPU-Embedding).".to_string();
        return (StatusCode::BAD_REQUEST, Json(build_status_json(&state, &guard)));
    }

    let pub_key = match prepare_unlocked_ssh_key(&snapshot.ssh_private_key, &snapshot.ssh_passphrase) {
        Ok(pk) => pk,
        Err(e) => {
            let mut guard = global_runpod_state().write().unwrap();
            guard.phase = "error".to_string();
            guard.message = format!("❌ SSH-Key Fehler: {}", e);
            return (StatusCode::BAD_REQUEST, Json(build_status_json(&state, &guard)));
        }
    };

    // Save current local LLM configuration if not already saved so we can restore it on stop
    {
        let mut guard = global_runpod_state().write().unwrap();
        if guard.prev_llm_base_url.is_none() {
            let cur_llm = state.llm.get_runtime_config();
            if !cur_llm.llm_base_url.contains(":11435") {
                guard.prev_llm_provider = Some(cur_llm.llm_provider);
                guard.prev_llm_base_url = Some(cur_llm.llm_base_url);
                guard.prev_llm_model = Some(cur_llm.llm_model);
            }
        }
        guard.phase = "deploying".to_string();
        guard.message = format!(
            "🚀 Starte RunPod GPU Pod ('{}') über RunPod REST API v2...",
            guard.gpu_type
        );
    }

    let state_bg = state.clone();
    tokio::spawn(async move {
        let res = orchestrate_runpod_deployment(&state_bg, &pub_key).await;
        // Always wipe temporary unlocked key file from /tmp as soon as tunnel setup finishes or fails
        let _ = fs::remove_file(UNLOCKED_KEY_PATH);
        if let Err(err) = res {
            warn!("RunPod deployment failed: {}", err);
            let mut guard = global_runpod_state().write().unwrap();
            guard.phase = "error".to_string();
            guard.message = format!("❌ {}", err);
        }
    });

    let cur = global_runpod_state().read().unwrap().clone();
    (StatusCode::OK, Json(build_status_json(&state, &cur)))
}

/// Terminates the active RunPod Pod, closes the SSH tunnel, and restores local Mac LLM & Embedding (`POST /v1/runpod/stop`)
pub async fn stop_runpod_handler(State(state): State<AppState>) -> impl IntoResponse {
    let (api_key, pod_id, prev_prov, prev_url, prev_mod) = {
        let mut guard = global_runpod_state().write().unwrap();
        guard.phase = "stopping".to_string();
        guard.message = "⏳ Beende RunPod Pod & schließe SSH-Tunnel...".to_string();
        (
            guard.api_key.clone(),
            guard.pod_id.clone(),
            guard.prev_llm_provider.take(),
            guard.prev_llm_base_url.take(),
            guard.prev_llm_model.take(),
        )
    };

    // 1. Kill SSH tunnel
    kill_tunnel_processes();

    // 2. Delete Pod via RunPod REST API v2
    if let Some(pid) = pod_id.filter(|s| !s.is_empty()) {
        if !api_key.is_empty() {
            let url = format!("https://api.runpod.io/v2/pods/{}", pid);
            let _ = state
                .db
                .http_client()
                .delete(&url)
                .bearer_auth(&api_key)
                .send()
                .await;
            info!("Terminated RunPod Pod '{}' via REST API v2", pid);
        }
    }

    // 3. Restore local Embedding URL
    state
        .inference
        .set_embed_url(state.config.tei_embed_url.clone());

    // 4. Restore local LLM settings
    let restore_prov = prev_prov.unwrap_or_else(|| "openai_compatible".to_string());
    let restore_url = prev_url.unwrap_or_else(|| "http://host.docker.internal:11434/v1".to_string());
    let restore_mod = prev_mod.unwrap_or_else(|| state.config.llm_model.clone());
    let restored = state.llm.update_runtime_config(
        restore_prov,
        restore_url,
        Some("ollama".to_string()),
        restore_mod,
    );
    let _ = state
        .db
        .save_system_llm_settings(
            &restored.llm_provider,
            &restored.llm_base_url,
            Some(&restored.llm_api_key),
            &restored.llm_model,
        )
        .await;

    let final_st = {
        let mut guard = global_runpod_state().write().unwrap();
        guard.phase = "idle".to_string();
        guard.pod_id = None;
        guard.pod_status = None;
        guard.cost_per_hr = None;
        guard.ssh_endpoint = None;
        guard.tunnel_connected = false;
        guard.llm_ready = false;
        guard.embed_ready = false;
        guard.message = format!(
            "🟢 RunPod Pod beendet (keine laufenden GPU-Kosten). Zurückgeschaltet auf lokales System ({}).",
            restored.llm_model
        );
        guard.clone()
    };
    persist_runpod_settings_to_db(&state, &final_st).await;

    (StatusCode::OK, Json(build_status_json(&state, &final_st)))
}

async fn orchestrate_runpod_deployment(state: &AppState, pub_key: &str) -> Result<()> {
    let (api_key, gpu_type, enable_llm, llm_model, enable_embedding, mut existing_pod_id) = {
        let guard = global_runpod_state().read().unwrap();
        (
            guard.api_key.clone(),
            guard.gpu_type.clone(),
            guard.enable_llm,
            guard.llm_model.clone(),
            guard.enable_embedding,
            guard.pod_id.clone(),
        )
    };

    let http = state.db.http_client();
    let mut pod_id = String::new();

    // If no existing_pod_id in memory, check if a pod named "rag-studio-gpu-pod" is already RUNNING on RunPod
    if existing_pod_id.is_none() {
        if let Ok(r) = http
            .get("https://api.runpod.io/v2/pods")
            .bearer_auth(&api_key)
            .send()
            .await
        {
            if let Ok(val) = r.json::<Value>().await {
                let arr_opt = val.as_array().or_else(|| val["data"].as_array());
                if let Some(arr) = arr_opt {
                    for p in arr {
                        let name = p["name"].as_str().unwrap_or_default();
                        let st = p["status"].as_str().unwrap_or_default();
                        if name == "rag-studio-gpu-pod"
                            && (st == "RUNNING" || st == "PROVISIONING" || st == "STARTING")
                        {
                            if let Some(id_str) = p["id"].as_str() {
                                existing_pod_id = Some(id_str.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Reuse existing active Pod if it is still RUNNING / PROVISIONING on RunPod
    if let Some(ref pid) = existing_pod_id {
        let check_url = format!("https://api.runpod.io/v2/pods/{}", pid);
        if let Ok(r) = http.get(&check_url).bearer_auth(&api_key).send().await {
            if let Ok(j) = r.json::<Value>().await {
                let st_str = j["status"].as_str().unwrap_or_default();
                if st_str == "RUNNING" || st_str == "PROVISIONING" || st_str == "STARTING" {
                    info!("Reusing active RunPod Pod {} (status={})", pid, st_str);
                    pod_id = pid.clone();
                    if st_str == "RUNNING" && is_tunnel_process_alive() {
                        if enable_llm {
                            pull_and_activate_runpod_ollama_model(state, &llm_model).await?;
                        }
                        if enable_embedding {
                            state
                                .inference
                                .set_embed_url("http://127.0.0.1:18081".to_string());
                        }
                        let mut guard = global_runpod_state().write().unwrap();
                        guard.phase = "running".to_string();
                        guard.tunnel_connected = true;
                        guard.llm_ready = enable_llm;
                        guard.embed_ready = enable_embedding;
                        guard.message = format!(
                            "🟢 RunPod GPU aktiv (Pod: {}) — Modell '{}' bereit!",
                            pid, llm_model
                        );
                        return Ok(());
                    }
                }
            }
        }
    }

    // Build container startup script for the RunPod Pod
    let embed_startup = if enable_embedding {
        r#"
pip install -q "transformers<4.49" "sentence-transformers<3.4" fastapi uvicorn
python3 - << 'inner_py' > /var/log/tei_embed.log 2>&1 &
import torch, uvicorn
from fastapi import FastAPI, Request
from sentence_transformers import SentenceTransformer

model_id = "BAAI/bge-m3"
model = SentenceTransformer(model_id, device="cuda", model_kwargs={"torch_dtype": torch.float16})
model.max_seq_length = 8192
app = FastAPI()

@app.get("/info")
async def info():
    return {"model_id": model_id, "device": torch.cuda.get_device_name(0), "dtype": "float16"}

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
"#
    } else {
        ""
    };

    let llm_startup = if enable_llm {
        format!(
            r#"
(
  if [ ! -f /tmp/ollama_install.done ]; then
    apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq zstd >/dev/null 2>&1
    curl -fsSL https://ollama.com/install.sh | sh >/var/log/ollama_install.log 2>&1
    touch /tmp/ollama_install.done
  fi
  OLLAMA_HOST=127.0.0.1:11434 OLLAMA_KEEP_ALIVE=24h ollama serve > /var/log/ollama.log 2>&1 &
  sleep 3
  ollama pull "{model}" >> /var/log/ollama.log 2>&1
) &
"#,
            model = llm_model.replace('"', "")
        )
    } else {
        String::new()
    };

    let safe_pub = pub_key.replace('\'', "");
    let container_script = format!(
        r#"if [ -x /start.sh ]; then
    /start.sh &
    sleep 2
else
    service ssh start || /usr/sbin/sshd
fi
mkdir -p /root/.ssh && chmod 700 /root/.ssh
grep -qF '{pub_key}' /root/.ssh/authorized_keys 2>/dev/null || echo '{pub_key}' >> /root/.ssh/authorized_keys
chmod 600 /root/.ssh/authorized_keys
service nginx stop >/dev/null 2>&1 || pkill -9 nginx >/dev/null 2>&1 || true
{embed_block}
{llm_block}
sleep infinity"#,
        pub_key = safe_pub,
        embed_block = embed_startup,
        llm_block = llm_startup
    );

    if pod_id.is_empty() {
        let make_payload = |gpu: &str| {
            json!({
                "name": "rag-studio-gpu-pod",
                "image": "runpod/pytorch:2.4.0-py3.11-cuda12.4.1-devel-ubuntu22.04",
                "gpu": {
                    "id": gpu,
                    "count": 1,
                    "minRamPerGpu": 16,
                    "minVcpuCountPerGpu": 4
                },
                "disk": 35,
                "ports": ["22/tcp"],
                "startSsh": true,
                "entrypoint": ["/bin/bash", "-c"],
                "cmd": [&container_script]
            })
        };

        let mut resp = http
            .post("https://api.runpod.io/v2/pods")
            .bearer_auth(&api_key)
            .json(&make_payload(&gpu_type))
            .send()
            .await
            .context("Fehler beim Verbinden mit RunPod REST API v2")?;

        let mut resp_json: Value = resp.json().await.unwrap_or_default();
        pod_id = resp_json["id"].as_str().unwrap_or_default().to_string();

        if pod_id.is_empty() {
            let fallback_gpu = if gpu_type == "NVIDIA A40" {
                "NVIDIA GeForce RTX 4090"
            } else {
                "NVIDIA A40"
            };
            {
                let mut guard = global_runpod_state().write().unwrap();
                guard.message = format!(
                    "⚠️ '{}' aktuell belegt — versuche Fallback-GPU '{}'...",
                    gpu_type, fallback_gpu
                );
            }
            resp = http
                .post("https://api.runpod.io/v2/pods")
                .bearer_auth(&api_key)
                .json(&make_payload(fallback_gpu))
                .send()
                .await
                .context("Fehler beim Fallback-Request an RunPod REST API v2")?;
            resp_json = resp.json().await.unwrap_or_default();
            pod_id = resp_json["id"].as_str().unwrap_or_default().to_string();
            if !pod_id.is_empty() {
                let mut guard = global_runpod_state().write().unwrap();
                guard.gpu_type = fallback_gpu.to_string();
            }
        }

        if pod_id.is_empty() {
            anyhow::bail!("RunPod Pod konnte nicht erstellt werden: {}", resp_json);
        }
    }

    {
        let snapshot = {
            let mut guard = global_runpod_state().write().unwrap();
            guard.pod_id = Some(pod_id.clone());
            guard.pod_status = Some("PROVISIONING".to_string());
            guard.phase = "waiting_ssh".to_string();
            guard.message = format!(
                "⏳ Pod '{}' bereit — warte auf Container-Image & SSH-Port (22/tcp)...",
                pod_id
            );
            guard.clone()
        };
        persist_runpod_settings_to_db(state, &snapshot).await;
    }

    // 2. Poll until direct SSH endpoint is ready
    let mut ssh_host = String::new();
    let mut ssh_port: u64 = 0;
    for attempt in 1..=90 {
        // Check if user cancelled in the meantime
        if global_runpod_state().read().unwrap().phase == "stopping"
            || global_runpod_state().read().unwrap().phase == "idle"
        {
            return Ok(());
        }

        let pod_url = format!("https://api.runpod.io/v2/pods/{}", pod_id);
        if let Ok(r) = http.get(&pod_url).bearer_auth(&api_key).send().await {
            if let Ok(d) = r.json::<Value>().await {
                let status_str = d["status"].as_str().unwrap_or("PROVISIONING").to_string();
                let cost = d["costPerHr"].as_f64();
                let actual_gpu = d["gpu"]["id"]
                    .as_str()
                    .or_else(|| d["gpu"]["displayName"].as_str())
                    .map(normalize_gpu_type_id);

                {
                    let mut guard = global_runpod_state().write().unwrap();
                    guard.pod_status = Some(status_str.clone());
                    if let Some(c) = cost {
                        guard.cost_per_hr = Some(c);
                    }
                    if let Some(g) = actual_gpu {
                        guard.gpu_type = g;
                    }
                }

                if status_str == "RUNNING" {
                    if let (Some(h), Some(p)) = (
                        d["ssh"]["direct"]["host"].as_str(),
                        d["ssh"]["direct"]["port"].as_u64(),
                    ) {
                        ssh_host = h.to_string();
                        ssh_port = p;
                        break;
                    }
                    if let Some(ports) = d["runtime"]["ports"].as_array() {
                        for pt in ports {
                            let priv_p = pt["privatePort"].as_u64().or_else(|| pt["private"].as_u64());
                            let pub_p = pt["publicPort"].as_u64().or_else(|| pt["public"].as_u64());
                            let ip = pt["ip"].as_str();
                            if priv_p == Some(22) {
                                if let (Some(h), Some(p)) = (ip, pub_p) {
                                    ssh_host = h.to_string();
                                    ssh_port = p;
                                    break;
                                }
                            }
                        }
                    }
                    if !ssh_host.is_empty() && ssh_port > 0 {
                        break;
                    }
                }
            }
        }

        if attempt % 5 == 0 {
            let mut guard = global_runpod_state().write().unwrap();
            guard.message = format!(
                "⏳ Warte auf Container-Start auf RunPod Node ({}s)...",
                attempt * 3
            );
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    if ssh_host.is_empty() || ssh_port == 0 {
        anyhow::bail!(
            "Zeitüberschreitung beim Warten auf den SSH-Port von Pod '{}'.",
            pod_id
        );
    }

    let ssh_ep = format!("root@{}:{}", ssh_host, ssh_port);
    {
        let mut guard = global_runpod_state().write().unwrap();
        guard.ssh_endpoint = Some(ssh_ep.clone());
        guard.message = format!(
            "🔒 SSH-Endpunkt bereit ({}) — verbinde verschlüsselten SSH-Tunnel...",
            ssh_ep
        );
    }

    // Re-create temporary unlocked key file right before SSH handshake and wipe it immediately after tunnel opens
    {
        let (priv_k, pass) = {
            let g = global_runpod_state().read().unwrap();
            (g.ssh_private_key.clone(), g.ssh_passphrase.clone())
        };
        let _ = prepare_unlocked_ssh_key(&priv_k, &pass);
    }

    // 3. Wait for sshd inside the container to accept connections
    let port_str = ssh_port.to_string();
    let host_target = format!("root@{}", ssh_host);
    let mut ssh_ready = false;
    for _ in 1..=25 {
        let probe = Command::new("ssh")
            .args([
                "-F",
                "/dev/null",
                "-p",
                &port_str,
                "-i",
                UNLOCKED_KEY_PATH,
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=4",
                &host_target,
                "echo ok",
            ])
            .output();
        if let Ok(out) = probe {
            if out.status.success() {
                ssh_ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    if !ssh_ready {
        anyhow::bail!(
            "SSH-Login auf {} fehlgeschlagen. Bitte prüfe, ob dein SSH Public Key in deinem RunPod-Account hinterlegt ist.",
            ssh_ep
        );
    }

    // Ensure Ollama installation has fully finished extracting CUDA libs and daemon is running
    if enable_llm {
        let _ = Command::new("ssh")
            .args([
                "-F",
                "/dev/null",
                "-p",
                &port_str,
                "-i",
                UNLOCKED_KEY_PATH,
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "BatchMode=yes",
                &host_target,
                "for i in $(seq 1 45); do if [ -f /tmp/ollama_install.done ]; then break; fi; if ! pgrep -f '[o]llama.com/install.sh' >/dev/null 2>&1 && command -v ollama >/dev/null 2>&1; then touch /tmp/ollama_install.done; break; fi; sleep 2; done; if ! command -v ollama >/dev/null 2>&1; then apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq zstd >/dev/null 2>&1 && curl -fsSL https://ollama.com/install.sh | sh >/var/log/ollama_install.log 2>&1 && touch /tmp/ollama_install.done; fi; if ! pgrep -x ollama >/dev/null 2>&1; then nohup env OLLAMA_HOST=127.0.0.1:11434 OLLAMA_KEEP_ALIVE=24h ollama serve >/var/log/ollama.log 2>&1 </dev/null & sleep 3; fi",
            ])
            .output();
    }

    // 4. Open background SSH tunnel forwarding 18081 -> 8081 (Embedding) and 11435 -> 11434 (Ollama)
    kill_tunnel_processes();
    {
        let (priv_k, pass) = {
            let g = global_runpod_state().read().unwrap();
            (g.ssh_private_key.clone(), g.ssh_passphrase.clone())
        };
        let _ = prepare_unlocked_ssh_key(&priv_k, &pass);
    }
    let tunnel_out = Command::new("ssh")
        .args([
            "-F",
            "/dev/null",
            "-f",
            "-N",
            "-L",
            "127.0.0.1:18081:127.0.0.1:8081",
            "-L",
            "127.0.0.1:11435:127.0.0.1:11434",
            "-p",
            &port_str,
            "-i",
            UNLOCKED_KEY_PATH,
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "ExitOnForwardFailure=yes",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=4",
            &host_target,
        ])
        .output()
        .context("Konnte SSH-Tunnel nicht starten");

    // Immediately delete the temporary unlocked SSH key from /tmp now that `ssh -f -N` has authenticated!
    let _ = fs::remove_file(UNLOCKED_KEY_PATH);

    let tunnel_out = tunnel_out?;
    if !tunnel_out.status.success() {
        let stderr = String::from_utf8_lossy(&tunnel_out.stderr);
        anyhow::bail!("SSH-Tunnel Fehler: {}", stderr);
    }

    let _ = fs::write(TUNNEL_ACTIVE_FLAG, &pod_id);

    {
        let mut guard = global_runpod_state().write().unwrap();
        guard.tunnel_connected = true;
        guard.phase = "loading_models".to_string();
        guard.message = "⏳ SSH-Tunnel steht! Lade Modelle (Ollama / BGE-M3) auf RunPod GPU...".to_string();
    }

    // 5A. If embedding is enabled, wait for http://127.0.0.1:18081/info
    if enable_embedding {
        {
            let mut guard = global_runpod_state().write().unwrap();
            guard.message = "⏳ Lade BAAI/bge-m3 GPU-Embedding-Server auf RunPod (~30-60s)...".to_string();
        }
        for _ in 1..=90 {
            if let Ok(r) = http
                .get("http://127.0.0.1:18081/info")
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                if r.status().is_success() {
                    state.inference.set_embed_url("http://127.0.0.1:18081".to_string());
                    let mut guard = global_runpod_state().write().unwrap();
                    guard.embed_ready = true;
                    info!("Switched live embedding URL to RunPod GPU tunnel (http://127.0.0.1:18081)");
                    break;
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    // 5B. If LLM is enabled, wait for Ollama on http://127.0.0.1:11435 and pull/verify model
    if enable_llm {
        pull_and_activate_runpod_ollama_model(state, &llm_model).await?;
    }

    {
        let mut guard = global_runpod_state().write().unwrap();
        guard.phase = "running".to_string();
        let services_desc = match (enable_llm, enable_embedding) {
            (true, true) => format!("LLM '{}' + BGE-M3 GPU-Embedding", llm_model),
            (true, false) => format!("LLM '{}'", llm_model),
            (false, true) => "BGE-M3 GPU-Embedding".to_string(),
            (false, false) => "Verbunden".to_string(),
        };
        guard.message = format!(
            "🟢 RunPod GPU aktiv ({}) — {} über SSH-Tunnel verbunden!",
            guard.gpu_type, services_desc
        );
    }

    Ok(())
}

async fn pull_and_activate_runpod_ollama_model(state: &AppState, llm_model: &str) -> Result<()> {
    let http = state.db.http_client();
    {
        let mut guard = global_runpod_state().write().unwrap();
        guard.phase = "loading_models".to_string();
        guard.message = format!(
            "⏳ Warte auf Ollama-Server & lade Modell '{}' auf RunPod GPU...",
            llm_model
        );
    }

    // Wait for Ollama daemon to respond on 127.0.0.1:11435/api/tags
    let mut ollama_up = false;
    for _ in 1..=60 {
        if let Ok(r) = http
            .get("http://127.0.0.1:11435/api/tags")
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            if r.status().is_success() {
                ollama_up = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    if !ollama_up {
        anyhow::bail!("Ollama-Daemon im RunPod-Container antwortet nicht auf Port 11434.");
    }

    // Trigger/await `POST /api/pull` with `stream: false` so it blocks until the model weights are downloaded
    {
        let mut guard = global_runpod_state().write().unwrap();
        guard.message = format!(
            "📥 Lade Sprachmodell '{}' in den RunPod GPU-VRAM (bitte kurz warten)...",
            llm_model
        );
    }

    let pull_resp = http
        .post("http://127.0.0.1:11435/api/pull")
        .timeout(Duration::from_secs(600))
        .json(&json!({
            "name": llm_model,
            "stream": false
        }))
        .send()
        .await
        .context("Fehler beim Pullen des Modells über den RunPod Ollama-Tunnel")?;

    if !pull_resp.status().is_success() {
        let err_txt = pull_resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama Modell-Download ('{}') fehlgeschlagen: {}", llm_model, err_txt);
    }

    // Activate the RunPod Ollama endpoint in `state.llm`
    let updated = state.llm.update_runtime_config(
        "openai_compatible".to_string(),
        "http://127.0.0.1:11435/v1".to_string(),
        Some("ollama".to_string()),
        llm_model.to_string(),
    );
    let _ = state
        .db
        .save_system_llm_settings(
            &updated.llm_provider,
            &updated.llm_base_url,
            Some(&updated.llm_api_key),
            &updated.llm_model,
        )
        .await;

    {
        let mut guard = global_runpod_state().write().unwrap();
        guard.llm_ready = true;
        guard.llm_model = llm_model.to_string();
    }
    info!(
        "Activated RunPod Ollama LLM '{}' via SSH tunnel (http://127.0.0.1:11435/v1)",
        llm_model
    );
    Ok(())
}
