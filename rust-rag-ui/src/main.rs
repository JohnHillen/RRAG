// ==============================================================================
// 100% Rust WebAssembly Frontend mit Leptos (CSR)
// Features:
// - Modernes Sidebar-Menü
// - Modal-Dialog für Wissen einspeisen (Multi-File Upload & Text/Markdown)
// - Echtzeit-Streaming Chat mit ausklappbarem SurrealDB-Quellen-Inspektor
// Lizenz: MIT / Apache 2.0
// ==============================================================================

use gloo_net::http::Request;
use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use web_sys::{FormData, HtmlInputElement};

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = AbortController)]
    #[derive(Clone, Debug)]
    pub type JsAbortController;

    #[wasm_bindgen::prelude::wasm_bindgen(constructor, js_class = "AbortController")]
    pub fn new() -> JsAbortController;

    #[wasm_bindgen::prelude::wasm_bindgen(method, getter)]
    pub fn signal(this: &JsAbortController) -> web_sys::AbortSignal;

    #[wasm_bindgen::prelude::wasm_bindgen(method)]
    pub fn abort(this: &JsAbortController);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEntity {
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GraphRelation {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub to: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub id: String,
    pub doc_id: String,
    pub doc_title: String,
    pub chunk_index: i64,
    pub content: String,
    #[serde(default)]
    pub rrf_score: f64,
    #[serde(default)]
    pub rerank_score: Option<f64>,
    #[serde(default)]
    pub entities: Vec<GraphEntity>,
    #[serde(default)]
    pub related_entities: Vec<GraphEntity>,
    #[serde(default)]
    pub relations: Vec<GraphRelation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessagePayload {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentItem {
    pub doc_id: String,
    pub doc_title: String,
    pub chunk_count: usize,
    pub created_at: String,
    #[serde(default)]
    pub relations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrmEmployeeItem {
    pub emp_id: String,
    pub name: String,
    pub role: String,
    pub department: String,
    #[serde(default)]
    pub skills: Vec<String>,
    pub english_level: String,
    #[serde(default)]
    pub certifications: Vec<String>,
    pub current_project: String,
    #[serde(default)]
    pub availability_percent: i64,
    #[serde(default)]
    pub hourly_rate: f64,
    #[serde(default)]
    pub location: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FleetVehicleItem {
    pub truck_id: String,
    pub license_plate: String,
    pub model: String,
    pub vehicle_type: String,
    pub emission_class: String,
    pub assigned_driver: String,
    pub mileage_km: i64,
    pub next_inspection_due_km: i64,
    pub km_until_inspection: i64,
    pub next_inspection_date: String,
    pub inspection_status: String,
    pub oil_level_percent: i64,
    pub oil_status: String,
    pub adblue_level_percent: i64,
    pub brake_wear_percent: i64,
    pub tire_pressure_bar: f64,
    pub fuel_consumption_l_100km: f64,
    pub cooling_unit_status: String,
    #[serde(default)]
    pub adr_certified: bool,
    pub current_location: String,
    pub operational_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpsWaypointItem {
    pub timestamp: String,
    pub lat: f64,
    pub lon: f64,
    pub location_name: String,
    pub speed_kmh: i64,
    #[serde(default)]
    pub temp_c: Option<f64>,
    #[serde(default)]
    pub satellites_used: i64,
    #[serde(default)]
    pub hdop: f64,
    #[serde(default)]
    pub gps_accuracy_m: f64,
    #[serde(default)]
    pub fix_type: String,
    #[serde(default)]
    pub weather_condition: String,
    #[serde(default)]
    pub ambient_temp_c: f64,
    #[serde(default)]
    pub precipitation_mm_h: f64,
    #[serde(default)]
    pub wind_speed_kmh: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FleetTourItem {
    pub tour_id: String,
    pub truck_id: String,
    pub license_plate: String,
    pub driver_name: String,
    pub departure_date: String,
    pub arrival_date: String,
    pub month: String,
    pub origin_warehouse_id: String,
    pub origin_city: String,
    pub destination_warehouse_id: String,
    pub destination_city: String,
    pub customer_name: String,
    pub cargo_description: String,
    pub cargo_category: String,
    pub cargo_weight_tons: f64,
    pub distance_km: i64,
    pub gps_route: String,
    #[serde(default)]
    pub gps_waypoints: Vec<GpsWaypointItem>,
    #[serde(default)]
    pub google_maps_url: String,
    #[serde(default)]
    pub recorded_temp_c: Option<f64>,
    #[serde(default)]
    pub delay_minutes: i64,
    pub delivery_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FleetWarehouseItem {
    pub warehouse_id: String,
    pub name: String,
    pub city: String,
    pub address: String,
    pub gps_coords: String,
    pub storage_type: String,
    pub capacity_pallets: i64,
    pub utilization_percent: i64,
    #[serde(default)]
    pub certifications: Vec<String>,
    pub restrictions: String,
    pub manager_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachedChatDoc {
    pub filename: String,
    pub text: String,
    pub char_count: usize,
    pub chunk_count: usize,
}

#[derive(Clone, Debug)]
pub struct UiChatMessage {
    pub id: usize,
    pub role: String,
    pub content: RwSignal<String>,
    pub sources: RwSignal<Vec<RetrievedChunk>>,
    pub metrics_info: RwSignal<Option<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub sources: Vec<RetrievedChunk>,
    #[serde(default)]
    pub metrics_info: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: usize,
    pub title: String,
    pub messages: Vec<StoredChatMessage>,
    #[serde(default)]
    pub project_db: Option<String>,
}

fn session_belongs_to_project(sess: &ChatSession, active_db: &str) -> bool {
    let sess_db = sess
        .project_db
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("knowledge_base");
    sess_db == active_db
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDatabaseItem {
    pub db_name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub document_count: usize,
    pub chunk_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

const STORAGE_KEY_SESSIONS: &str = "rust_rag_chat_sessions_v1";
const STORAGE_KEY_LANG: &str = "rust_rag_ui_lang_v1";
const STORAGE_KEY_RERANKER: &str = "rust_rag_reranker_v1";
const STORAGE_KEY_TOP_N: &str = "rust_rag_top_n_v1";
const STORAGE_KEY_USE_RAG: &str = "rust_rag_use_rag_v1";
const STORAGE_KEY_NUM_CTX: &str = "rust_rag_num_ctx_v1";
const STORAGE_KEY_THINK: &str = "rust_rag_think_mode_v1";
const STORAGE_KEY_TEMPERATURE: &str = "rust_rag_temperature_v1";
const STORAGE_KEY_LLM_MODEL: &str = "rust_rag_llm_model_v1";
const STORAGE_KEY_LLM_PROVIDER: &str = "rust_rag_llm_provider_v1";
const STORAGE_KEY_LLM_URL: &str = "rust_rag_llm_url_v1";
const STORAGE_KEY_RP_API_SEALED: &str = "rust_rag_rp_api_sealed_v1";
const STORAGE_KEY_RP_PASS_SEALED: &str = "rust_rag_rp_pass_sealed_v1";

fn xor_hex_seal(plain: &str) -> String {
    if plain.is_empty() {
        return String::new();
    }
    let key = b"rust_rag_studio_local_vault_2026";
    let mut hex = String::with_capacity(plain.len() * 2);
    for (i, &b) in plain.as_bytes().iter().enumerate() {
        let enc = b ^ key[i % key.len()];
        hex.push_str(&format!("{:02x}", enc));
    }
    hex
}

fn xor_hex_unseal(hex: &str) -> String {
    let trimmed = hex.trim();
    if trimmed.is_empty() || trimmed.len() % 2 != 0 {
        return String::new();
    }
    let key = b"rust_rag_studio_local_vault_2026";
    let mut bytes = Vec::with_capacity(trimmed.len() / 2);
    for i in (0..trimmed.len()).step_by(2) {
        if let Ok(b) = u8::from_str_radix(&trimmed[i..i + 2], 16) {
            bytes.push(b ^ key[(i / 2) % key.len()]);
        } else {
            return String::new();
        }
    }
    String::from_utf8(bytes).unwrap_or_default()
}

fn save_rp_creds_to_storage(api_key: Option<&str>, ssh_pass: Option<&str>) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Some(k) = api_key.map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let _ = storage.set_item(STORAGE_KEY_RP_API_SEALED, &xor_hex_seal(k));
            }
            if let Some(p) = ssh_pass.filter(|s| !s.is_empty()) {
                let _ = storage.set_item(STORAGE_KEY_RP_PASS_SEALED, &xor_hex_seal(p));
            }
        }
    }
}

fn load_rp_creds_from_storage() -> (Option<String>, Option<String>) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let api = storage
                .get_item(STORAGE_KEY_RP_API_SEALED)
                .ok()
                .flatten()
                .map(|h| xor_hex_unseal(&h))
                .filter(|s| !s.trim().is_empty());
            let pass = storage
                .get_item(STORAGE_KEY_RP_PASS_SEALED)
                .ok()
                .flatten()
                .map(|h| xor_hex_unseal(&h))
                .filter(|s| !s.is_empty());
            return (api, pass);
        }
    }
    (None, None)
}

fn load_llm_pref_from_storage() -> Option<(String, String, String)> {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let model = storage.get_item(STORAGE_KEY_LLM_MODEL).ok().flatten();
            let prov = storage.get_item(STORAGE_KEY_LLM_PROVIDER).ok().flatten();
            let url = storage.get_item(STORAGE_KEY_LLM_URL).ok().flatten();
            if let Some(m) = model {
                if !m.trim().is_empty() {
                    return Some((
                        prov.unwrap_or_else(|| "openai_compatible".to_string()),
                        url.unwrap_or_else(|| "http://host.docker.internal:11434/v1".to_string()),
                        m.trim().to_string(),
                    ));
                }
            }
        }
    }
    None
}

fn save_llm_pref_to_storage(provider: &str, base_url: &str, model: &str) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_LLM_PROVIDER, provider);
            let _ = storage.set_item(STORAGE_KEY_LLM_URL, base_url);
            let _ = storage.set_item(STORAGE_KEY_LLM_MODEL, model);
        }
    }
}
const WELCOME_TEXT_DE: &str = "Willkommen im Rust RAG & GraphRAG Studio! Unten links in der Seitenleiste findest du die Icons für Datei-Upload (➕), Dokumenten-Verwaltung (🗂️) und die Einstellungen (⚙️) für RAG, Reranker und Sprachmodelle.";
const WELCOME_TEXT_EN: &str = "Welcome to Rust RAG & GraphRAG Studio! At the bottom left of the sidebar you'll find quick icons for File Upload (➕), Document Management (🗂️), and Settings (⚙️) for RAG, Rerankers, and LLMs.";

fn load_lang_from_storage() -> String {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(lang)) = storage.get_item(STORAGE_KEY_LANG) {
                if lang == "en" || lang == "de" {
                    return lang;
                }
            }
        }
    }
    "de".to_string()
}

fn save_lang_to_storage(lang: &str) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_LANG, lang);
        }
    }
}

fn load_reranker_from_storage() -> String {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(r)) = storage.get_item(STORAGE_KEY_RERANKER) {
                if r == "fast" || r == "high" || r == "none" {
                    return r;
                }
            }
        }
    }
    "fast".to_string()
}

fn save_reranker_to_storage(r: &str) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_RERANKER, r);
        }
    }
}

fn load_top_n_from_storage() -> usize {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(raw)) = storage.get_item(STORAGE_KEY_TOP_N) {
                if let Ok(val) = raw.parse::<usize>() {
                    if (1..=30).contains(&val) {
                        return val;
                    }
                }
            }
        }
    }
    4usize
}

fn save_top_n_to_storage(top_n: usize) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_TOP_N, &top_n.to_string());
        }
    }
}

fn load_use_rag_from_storage() -> bool {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(raw)) = storage.get_item(STORAGE_KEY_USE_RAG) {
                if raw == "false" {
                    return false;
                }
            }
        }
    }
    true
}

fn save_use_rag_to_storage(val: bool) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_USE_RAG, if val { "true" } else { "false" });
        }
    }
}

fn load_num_ctx_from_storage() -> u64 {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(raw)) = storage.get_item(STORAGE_KEY_NUM_CTX) {
                if let Ok(val) = raw.parse::<u64>() {
                    if (2048..=131072).contains(&val) {
                        return val;
                    }
                }
            }
        }
    }
    16384u64
}

fn save_num_ctx_to_storage(ctx: u64) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_NUM_CTX, &ctx.to_string());
        }
    }
}

fn load_temperature_from_storage() -> f32 {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(raw)) = storage.get_item(STORAGE_KEY_TEMPERATURE) {
                if let Ok(val) = raw.parse::<f32>() {
                    if (0.0..=2.0).contains(&val) {
                        return val;
                    }
                }
            }
        }
    }
    0.2f32
}

fn save_temperature_to_storage(temp: f32) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_TEMPERATURE, &format!("{:.2}", temp));
        }
    }
}

fn load_think_from_storage() -> bool {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(raw)) = storage.get_item(STORAGE_KEY_THINK) {
                if raw == "false" {
                    return false;
                }
            }
        }
    }
    true
}

fn save_think_to_storage(val: bool) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            let _ = storage.set_item(STORAGE_KEY_THINK, if val { "true" } else { "false" });
        }
    }
}

fn default_welcome_stored() -> StoredChatMessage {
    let lang = load_lang_from_storage();
    StoredChatMessage {
        role: "assistant".to_string(),
        content: if lang == "en" {
            WELCOME_TEXT_EN.to_string()
        } else {
            WELCOME_TEXT_DE.to_string()
        },
        sources: vec![],
        metrics_info: None,
    }
}

fn load_sessions_from_storage() -> Vec<ChatSession> {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(raw)) = storage.get_item(STORAGE_KEY_SESSIONS) {
                if let Ok(parsed) = serde_json::from_str::<Vec<ChatSession>>(&raw) {
                    if !parsed.is_empty() {
                        return parsed;
                    }
                }
            }
        }
    }
    vec![ChatSession {
        id: 1,
        title: "Neuer Chat".to_string(),
        messages: vec![default_welcome_stored()],
        project_db: None,
    }]
}

fn save_sessions_to_storage(sessions: &[ChatSession]) {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(raw) = serde_json::to_string(sessions) {
                let _ = storage.set_item(STORAGE_KEY_SESSIONS, &raw);
            }
        }
    }
}

fn stored_to_ui_messages(
    stored: &[StoredChatMessage],
    next_id_sig: RwSignal<usize>,
) -> Vec<UiChatMessage> {
    let mut base_id = next_id_sig.get_untracked();
    let mut out = Vec::with_capacity(stored.len());
    for m in stored {
        out.push(UiChatMessage {
            id: base_id,
            role: m.role.clone(),
            content: create_rw_signal(m.content.clone()),
            sources: create_rw_signal(m.sources.clone()),
            metrics_info: create_rw_signal(m.metrics_info.clone()),
        });
        base_id += 1;
    }
    next_id_sig.set(base_id + 1);
    out
}

fn ui_to_stored_messages(ui_msgs: &[UiChatMessage]) -> Vec<StoredChatMessage> {
    ui_msgs
        .iter()
        .map(|m| StoredChatMessage {
            role: m.role.clone(),
            content: m.content.get_untracked(),
            sources: m.sources.get_untracked(),
            metrics_info: m.metrics_info.get_untracked(),
        })
        .collect()
}

fn trigger_download(filename: &str, content: &str) {
    if let Some(win) = web_sys::window() {
        if let Some(doc) = win.document() {
            let parts = js_sys::Array::new();
            parts.push(&wasm_bindgen::JsValue::from_str(content));

            if let Ok(blob) = web_sys::Blob::new_with_str_sequence(&parts) {
                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                    if let Ok(element) = doc.create_element("a") {
                        if let Ok(a) = element.dyn_into::<web_sys::HtmlAnchorElement>() {
                            a.set_href(&url);
                            a.set_download(filename);
                            let _ = a.click();
                            let _ = web_sys::Url::revoke_object_url(&url);
                        }
                    }
                }
            }
        }
    }
}

fn generate_chat_markdown(
    title: &str,
    model: &str,
    use_rag: bool,
    reranker_mode: &str,
    top_n: usize,
    num_ctx: u64,
    think_mode: bool,
    messages: &[StoredChatMessage],
) -> String {
    let now = js_sys::Date::new_0();
    let iso = now.to_iso_string().as_string().unwrap_or_default();

    let mut out = String::new();
    out.push_str(&format!("# 💬 Chat Export: {}\n\n", title));
    out.push_str(&format!("- **Export-Zeitpunkt**: {}\n", iso));
    out.push_str(&format!("- **LLM Modell**: `{}`\n", model));
    out.push_str(&format!(
        "- **RAG-Status**: `{}` | **Reranker-Modus**: `{}` | **Top-N Chunks**: `{}` | **Kontext-Fenster**: `{}` Tokens | **Denkmodus**: `{}`\n\n",
        if use_rag { "Aktiv (Hybrid HNSW+BM25)" } else { "Inaktiv" },
        reranker_mode,
        top_n,
        num_ctx,
        if think_mode { "Aktiv (Thinking ON)" } else { "Inaktiv (Ohne Thinking)" }
    ));
    out.push_str("---\n\n");

    for msg in messages {
        let content = &msg.content;
        let is_user = msg.role == "user";
        if is_user {
            out.push_str("## 👤 User\n\n");
        } else {
            out.push_str(&format!("## 🤖 Assistant ({})\n\n", model));
        }
        out.push_str(content);
        out.push_str("\n\n");

        if let Some(ref metrics) = msg.metrics_info {
            out.push_str(&format!("> ⚡ *Performance & Retrieval*: {}\n\n", metrics));
        }

        let srcs = &msg.sources;
        if !srcs.is_empty() {
            let total_rels: usize = srcs.iter().map(|s| s.relations.len()).sum();
            out.push_str(&format!(
                "<details>\n<summary><b>📚 Referenzierte Quellen & Wissensgraph ({} Chunks, {} Relationen)</b></summary>\n\n",
                srcs.len(),
                total_rels
            ));

            for (idx, s) in srcs.iter().enumerate() {
                out.push_str(&format!(
                    "### Quelle #{}: `{}` (Chunk #{})\n",
                    idx + 1,
                    s.doc_title,
                    s.chunk_index
                ));
                out.push_str(&format!("- **RRF Score**: {:.4}", s.rrf_score));
                if let Some(score) = s.rerank_score {
                    out.push_str(&format!(" | **Rerank Score**: {:.4}", score));
                }
                out.push_str("\n\n");

                out.push_str("**Auszug (Chunk Content):**\n```\n");
                out.push_str(&s.content);
                out.push_str("\n```\n\n");

                if !s.entities.is_empty() {
                    let ent_strs: Vec<String> = s
                        .entities
                        .iter()
                        .map(|e| format!("{}: {}", e.entity_type, e.name))
                        .collect();
                    out.push_str(&format!("- **Entitäten**: {}\n", ent_strs.join(", ")));
                }

                if !s.relations.is_empty() {
                    out.push_str("- **Wissensgraph-Relationen**:\n");
                    for rel in &s.relations {
                        out.push_str(&format!(
                            "  - `{}` --[{}]--> `{}`\n",
                            rel.from, rel.relation, rel.to
                        ));
                    }
                }
                out.push_str("\n");
            }
            out.push_str("</details>\n\n");
        }

        out.push_str("---\n\n");
    }

    out
}

fn export_chat_session_markdown(
    title: &str,
    model: &str,
    use_rag: bool,
    reranker_mode: &str,
    top_n: usize,
    num_ctx: u64,
    think_mode: bool,
    messages: &[StoredChatMessage],
) {
    let md = generate_chat_markdown(title, model, use_rag, reranker_mode, top_n, num_ctx, think_mode, messages);

    let now = js_sys::Date::new_0();
    let year = now.get_full_year();
    let month = now.get_month() + 1;
    let day = now.get_date();
    let hours = now.get_hours();
    let mins = now.get_minutes();

    let safe_title: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe_title = safe_title.trim_matches('_');
    let safe_title = if safe_title.is_empty() { "chat" } else { safe_title };

    let filename = format!(
        "chat_export_{}_{:04}{:02}{:02}_{:02}{:02}.md",
        safe_title, year, month, day, hours, mins
    );

    trigger_download(&filename, &md);
}

/// Teilt eine LLM-Antwort in optionalen `<think>...</think>`-Denkblock `(think_text, is_currently_streaming)`
/// und die eigentliche Hauptantwort auf.
fn parse_think_and_answer(raw: &str) -> (Option<(String, bool)>, String) {
    if let Some(start_idx) = raw.find("<think>") {
        let prefix = &raw[..start_idx];
        let rest = &raw[start_idx + 7..];
        if let Some(end_idx) = rest.find("</think>") {
            let think_txt = rest[..end_idx].trim().to_string();
            let after = &rest[end_idx + 8..];
            let answer = format!("{}{}", prefix, after).trim().to_string();
            if think_txt.is_empty() {
                (None, answer)
            } else {
                (Some((think_txt, false)), answer)
            }
        } else {
            let think_txt = rest.trim().to_string();
            let answer = prefix.trim().to_string();
            (Some((think_txt, true)), answer)
        }
    } else if let Some(end_idx) = raw.find("</think>") {
        let think_txt = raw[..end_idx].trim().to_string();
        let answer = raw[end_idx + 8..].trim().to_string();
        if think_txt.is_empty() {
            (None, answer)
        } else {
            (Some((think_txt, false)), answer)
        }
    } else {
        (None, raw.to_string())
    }
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn replace_delimited(input: &str, delim: &str, open_tag: &str, close_tag: &str) -> String {
    let mut out = String::with_capacity(input.len() + 32);
    let mut rest = input;
    let dlen = delim.len();
    while let Some(start) = rest.find(delim) {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + dlen..];
        if let Some(end) = after_start.find(delim) {
            let inner = &after_start[..end];
            if !inner.trim().is_empty() {
                out.push_str(open_tag);
                out.push_str(inner);
                out.push_str(close_tag);
                rest = &after_start[end + dlen..];
                continue;
            }
        }
        out.push_str(delim);
        rest = after_start;
    }
    out.push_str(rest);
    out
}

fn highlight_citations(input: &str) -> String {
    let mut s = input.to_string();
    for i in 1..=20 {
        let src_en = format!("[Source {}]", i);
        let src_de = format!("[Quelle {}]", i);
        let src_hash = format!("[Quelle #{}]", i);
        let badge_en = format!("<span class=\"md-cite-badge\">[Source {}]</span>", i);
        let badge_de = format!("<span class=\"md-cite-badge\">[Quelle {}]</span>", i);
        if s.contains(&src_en) {
            s = s.replace(&src_en, &badge_en);
        }
        if s.contains(&src_de) {
            s = s.replace(&src_de, &badge_de);
        }
        if s.contains(&src_hash) {
            s = s.replace(&src_hash, &badge_de);
        }
    }
    s
}

fn format_inline_markdown(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 32);
    for (idx, segment) in line.split('`').enumerate() {
        if idx % 2 == 1 {
            out.push_str("<code class=\"md-inline-code\">");
            out.push_str(&escape_html(segment));
            out.push_str("</code>");
        } else {
            let mut s = escape_html(segment);
            // Clean LaTeX wrappers like \[ \boxed{...} \]
            s = s.replace("\\[", "").replace("\\]", "").replace("\\(", "").replace("\\)", "");
            while let Some(b_start) = s.find("\\boxed{") {
                let before = &s[..b_start];
                let after_b = &s[b_start + 7..];
                if let Some(b_end) = after_b.find('}') {
                    let val = &after_b[..b_end];
                    let tail = &after_b[b_end + 1..];
                    s = format!("{}<strong>{}</strong>{}", before, val, tail);
                } else {
                    break;
                }
            }
            s = replace_delimited(&s, "***", "<strong><em>", "</em></strong>");
            s = replace_delimited(&s, "**", "<strong>", "</strong>");
            s = replace_delimited(&s, "*", "<em>", "</em>");
            s = highlight_citations(&s);
            out.push_str(&s);
        }
    }
    out
}

fn parse_unordered_list_item(trimmed: &str) -> Option<&str> {
    for prefix in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some(rest);
        }
    }
    None
}

fn parse_ordered_list_item(trimmed: &str) -> Option<(usize, &str)> {
    let dot_pos = trimmed.find(". ").or_else(|| trimmed.find(") "))?;
    if dot_pos == 0 || dot_pos > 4 {
        return None;
    }
    let num_str = &trimmed[..dot_pos];
    if !num_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let num = num_str.parse::<usize>().ok()?;
    Some((num, &trimmed[dot_pos + 2..]))
}

/// Konvertiert Markdown-Text sicher in strukturiertes HTML für die Chat-Anzeige.
fn render_markdown_to_html(md: &str) -> String {
    let mut html = String::with_capacity(md.len() + 256);
    let mut in_code_block = false;
    let mut code_lines: Vec<String> = Vec::new();
    let mut open_list: Option<&'static str> = None;
    let mut paragraph_lines: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();

    let flush_paragraph = |para: &mut Vec<String>, out: &mut String| {
        if !para.is_empty() {
            out.push_str("<p>");
            out.push_str(&para.join("<br/>"));
            out.push_str("</p>");
            para.clear();
        }
    };

    let flush_list = |list_state: &mut Option<&'static str>, out: &mut String| {
        if let Some(tag) = list_state.take() {
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
    };

    let flush_table = |rows: &mut Vec<Vec<String>>, out: &mut String| {
        if !rows.is_empty() {
            out.push_str("<div class=\"md-table-wrap\"><table class=\"md-table\">");
            for (r_idx, row) in rows.iter().enumerate() {
                if r_idx == 0 {
                    out.push_str("<thead><tr>");
                    for cell in row {
                        out.push_str("<th>");
                        out.push_str(&format_inline_markdown(cell.trim()));
                        out.push_str("</th>");
                    }
                    out.push_str("</tr></thead><tbody>");
                } else {
                    out.push_str("<tr>");
                    for cell in row {
                        out.push_str("<td>");
                        out.push_str(&format_inline_markdown(cell.trim()));
                        out.push_str("</td>");
                    }
                    out.push_str("</tr>");
                }
            }
            out.push_str("</tbody></table></div>");
            rows.clear();
        }
    };

    for line in md.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            flush_table(&mut table_rows, &mut html);

            if in_code_block {
                html.push_str("<pre class=\"md-code-block\"><code>");
                html.push_str(&escape_html(&code_lines.join("\n")));
                html.push_str("</code></pre>");
                code_lines.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line.to_string());
            continue;
        }

        // Table row detection
        if trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 2 {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            let inner = &trimmed[1..trimmed.len() - 1];
            let is_sep = inner
                .chars()
                .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace());
            if !is_sep {
                let cells: Vec<String> = inner.split('|').map(|s| s.trim().to_string()).collect();
                table_rows.push(cells);
            }
            continue;
        } else if !table_rows.is_empty() {
            flush_table(&mut table_rows, &mut html);
        }

        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            continue;
        }

        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            html.push_str("<hr class=\"md-hr\"/>");
            continue;
        }

        if let Some(h4) = trimmed.strip_prefix("#### ") {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            html.push_str("<h4>");
            html.push_str(&format_inline_markdown(h4));
            html.push_str("</h4>");
            continue;
        }
        if let Some(h3) = trimmed.strip_prefix("### ") {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            html.push_str("<h3>");
            html.push_str(&format_inline_markdown(h3));
            html.push_str("</h3>");
            continue;
        }
        if let Some(h2) = trimmed.strip_prefix("## ") {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            html.push_str("<h2>");
            html.push_str(&format_inline_markdown(h2));
            html.push_str("</h2>");
            continue;
        }
        if let Some(h1) = trimmed.strip_prefix("# ") {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            html.push_str("<h1>");
            html.push_str(&format_inline_markdown(h1));
            html.push_str("</h1>");
            continue;
        }

        if let Some(quote) = trimmed.strip_prefix("> ") {
            flush_paragraph(&mut paragraph_lines, &mut html);
            flush_list(&mut open_list, &mut html);
            html.push_str("<blockquote class=\"md-quote\">");
            html.push_str(&format_inline_markdown(quote));
            html.push_str("</blockquote>");
            continue;
        }

        let indent = line.len().saturating_sub(line.trim_start().len());

        if let Some(item_txt) = parse_unordered_list_item(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut html);
            if open_list != Some("ul") {
                flush_list(&mut open_list, &mut html);
                html.push_str("<ul>");
                open_list = Some("ul");
            }
            if indent >= 2 {
                html.push_str("<li class=\"md-li-sub\">");
            } else {
                html.push_str("<li>");
            }
            html.push_str(&format_inline_markdown(item_txt));
            html.push_str("</li>");
            continue;
        }

        if let Some((num, item_txt)) = parse_ordered_list_item(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut html);
            if open_list != Some("ol") {
                flush_list(&mut open_list, &mut html);
                html.push_str("<ol>");
                open_list = Some("ol");
            }
            if indent >= 2 {
                html.push_str(&format!("<li class=\"md-li-sub\" value=\"{}\">", num));
            } else {
                html.push_str(&format!("<li value=\"{}\">", num));
            }
            html.push_str(&format_inline_markdown(item_txt));
            html.push_str("</li>");
            continue;
        }

        flush_list(&mut open_list, &mut html);
        paragraph_lines.push(format_inline_markdown(trimmed));
    }

    if in_code_block && !code_lines.is_empty() {
        html.push_str("<pre class=\"md-code-block\"><code>");
        html.push_str(&escape_html(&code_lines.join("\n")));
        html.push_str("</code></pre>");
    }
    flush_table(&mut table_rows, &mut html);
    flush_list(&mut open_list, &mut html);
    flush_paragraph(&mut paragraph_lines, &mut html);

    html
}

fn make_session_title(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.chars().count() <= 34 {
        trimmed.to_string()
    } else {
        let short: String = trimmed.chars().take(32).collect();
        format!("{}…", short.trim_end())
    }
}

fn get_file_relative_path(file: &web_sys::File) -> String {
    js_sys::Reflect::get(file.as_ref(), &wasm_bindgen::JsValue::from_str("webkitRelativePath"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}

fn extract_category_from_path(rel_path: &str) -> Option<String> {
    let clean = rel_path.trim().replace('\\', "/");
    let parts: Vec<&str> = clean.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 3 {
        // e.g. "showcase_documents/HR/Mitarbeiter.pdf" -> "HR"
        let folder_parts = &parts[1..parts.len() - 1];
        Some(folder_parts.join(" / "))
    } else if parts.len() == 2 {
        // e.g. "HR/Mitarbeiter.pdf" -> "HR"
        Some(parts[0].to_string())
    } else {
        None
    }
}

fn parse_category_and_title(raw_title: &str) -> (String, String) {
    let trimmed = raw_title.trim();
    if trimmed.starts_with('[') {
        if let Some(end_idx) = trimmed.find(']') {
            let cat = trimmed[1..end_idx].trim().to_string();
            let title = trimmed[end_idx + 1..].trim().to_string();
            if !cat.is_empty() {
                return (cat, if title.is_empty() { raw_title.to_string() } else { title });
            }
        }
    }
    ("General".to_string(), trimmed.to_string())
}

/// Returns (icon, view_button_label, view_tooltip, close_button_label) based on the document title's file extension.
fn doc_format_labels(
    doc_title: &str,
    is_en: bool,
) -> (&'static str, &'static str, &'static str, &'static str) {
    let lower = doc_title.trim().to_lowercase();
    if lower.ends_with(".pdf") {
        if is_en {
            ("📄", "📄 View PDF", "Open PDF document in viewer", "✕ Close PDF")
        } else {
            ("📄", "📄 PDF anzeigen", "PDF-Dokument im Viewer anzeigen", "✕ PDF schließen")
        }
    } else if lower.ends_with(".xml") {
        if is_en {
            ("📜", "📜 View XML", "Open XML document in viewer", "✕ Close XML")
        } else {
            ("📜", "📜 XML anzeigen", "XML-Dokument im Viewer anzeigen", "✕ XML schließen")
        }
    } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
        if is_en {
            ("📝", "📝 View Markdown", "Open Markdown document in viewer", "✕ Close Document")
        } else {
            ("📝", "📝 Markdown anzeigen", "Markdown-Dokument im Viewer anzeigen", "✕ Dokument schließen")
        }
    } else if lower.ends_with(".html") || lower.ends_with(".htm") {
        if is_en {
            ("🌐", "🌐 View HTML", "Open HTML document in viewer", "✕ Close Document")
        } else {
            ("🌐", "🌐 HTML anzeigen", "HTML-Dokument im Viewer anzeigen", "✕ Dokument schließen")
        }
    } else if lower.ends_with(".txt") {
        if is_en {
            ("📃", "📃 View Text", "Open text document in viewer", "✕ Close Document")
        } else {
            ("📃", "📃 Text anzeigen", "Textdokument im Viewer anzeigen", "✕ Dokument schließen")
        }
    } else if lower.ends_with(".csv") || lower.ends_with(".json") {
        if is_en {
            ("📊", "📊 View File", "Open document in viewer", "✕ Close Document")
        } else {
            ("📊", "📊 Datei anzeigen", "Dokument im Viewer anzeigen", "✕ Dokument schließen")
        }
    } else if is_en {
        ("📄", "📄 View Document", "Open document in viewer", "✕ Close Document")
    } else {
        ("📄", "📄 Dokument anzeigen", "Dokument im Viewer anzeigen", "✕ Dokument schließen")
    }
}

async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(win) = web_sys::window() {
            let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

async fn read_browser_file_as_text(file: &web_sys::File) -> Option<String> {
    let func = js_sys::Reflect::get(file.as_ref(), &wasm_bindgen::JsValue::from_str("text"))
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let promise = func
        .call0(file.as_ref())
        .ok()?
        .dyn_into::<js_sys::Promise>()
        .ok()?;
    let val = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
    val.as_string()
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}

#[component]
fn App() -> impl IntoView {
    // --- UI Sprache (Deutsch "de" / English "en") ---
    let (ui_lang, set_ui_lang) = create_signal(load_lang_from_storage());
    let is_en = move || ui_lang.get() == "en";

    // --- Multi-Chat Sessions & Chat Signals ---
    let initial_sessions = load_sessions_from_storage();
    let initial_active_id = initial_sessions.first().map(|s| s.id).unwrap_or(1);
    let max_session_id = initial_sessions.iter().map(|s| s.id).max().unwrap_or(1);

    let next_msg_id = create_rw_signal(1usize);
    let initial_ui_msgs = initial_sessions
        .first()
        .map(|s| stored_to_ui_messages(&s.messages, next_msg_id))
        .unwrap_or_else(|| stored_to_ui_messages(&[default_welcome_stored()], next_msg_id));

    let (chat_sessions, set_chat_sessions) = create_signal(initial_sessions);
    let (active_session_id, set_active_session_id) = create_signal(initial_active_id);
    let (next_session_id, set_next_session_id) = create_signal(max_session_id + 1);
    let (messages, set_messages) = create_signal(initial_ui_msgs);

    let (active_project_db, set_active_project_db) = create_signal("knowledge_base".to_string());
    let (project_databases, set_project_databases) =
        create_signal(Vec::<ProjectDatabaseItem>::new());
    let last_session_per_db = create_rw_signal(std::collections::HashMap::<String, usize>::new());

    // Speichert die Nachrichten des aktuell geöffneten Chats in `chat_sessions` + `localStorage`
    let persist_current_chat = move |custom_title: Option<String>| {
        let cur_id = active_session_id.get_untracked();
        let cur_db = active_project_db.get_untracked();
        let stored_msgs = ui_to_stored_messages(&messages.get_untracked());
        set_chat_sessions.update(|sessions| {
            if let Some(sess) = sessions.iter_mut().find(|s| s.id == cur_id) {
                sess.messages = stored_msgs;
                if sess.project_db.as_deref().unwrap_or("").trim().is_empty() {
                    sess.project_db = Some(cur_db.clone());
                }
                if let Some(new_title) = custom_title {
                    if sess.title == "Neuer Chat" || sess.title.starts_with("Chat #") {
                        sess.title = new_title;
                    }
                }
            }
            save_sessions_to_storage(sessions);
        });
        last_session_per_db.update(|m| {
            m.insert(cur_db, cur_id);
        });
    };

    let (user_input, set_user_input) = create_signal(String::new());
    let (use_rag, set_use_rag) = create_signal(load_use_rag_from_storage());
    let (reranker_mode, set_reranker_mode) = create_signal(load_reranker_from_storage());
    let (top_n, set_top_n) = create_signal(load_top_n_from_storage());
    let (num_ctx_sig, set_num_ctx_sig) = create_signal(load_num_ctx_from_storage());
    let (think_mode_sig, set_think_mode_sig) = create_signal(load_think_from_storage());
    let (temperature_sig, set_temperature_sig) = create_signal(load_temperature_from_storage());
    let (open_rag_help, set_open_rag_help) = create_signal(None::<&'static str>);
    let (is_loading, set_is_loading) = create_signal(false);
    let (chat_abort_gen, set_chat_abort_gen) = create_signal(0u64);
    let chat_abort_ctrl = create_rw_signal(None::<JsAbortController>);

    // --- Temporäre Chat-Anhänge (📎 mehrere Dateien auch aus verschiedenen Ordnern ohne DB-Speicherung) ---
    let (chat_attachments, set_chat_attachments) = create_signal(Vec::<AttachedChatDoc>::new());
    let (attachment_only, set_attachment_only) = create_signal(true);
    let (is_extracting_attachment, set_is_extracting_attachment) = create_signal(false);
    let (attachment_error, set_attachment_error) = create_signal(None::<String>);
    let chat_attach_input_ref = create_node_ref::<leptos::html::Input>();

    // --- Chat-Eingabe-Historie (Pfeiltaste Hoch / Runter) ---
    let (history_cursor, set_history_cursor) = create_signal(None::<usize>);
    let (draft_before_history, set_draft_before_history) = create_signal(String::new());

    // Stellt sicher, dass beim Wechsel einer Projekt-Datenbank ein zu diesem Projekt gehörender Chat aktiv ist
    let activate_chat_for_project = move |target_db: String| {
        if is_loading.get_untracked() {
            return;
        }
        let cur_id = active_session_id.get_untracked();
        let sessions_snap = chat_sessions.get_untracked();

        if let Some(cur_sess) = sessions_snap.iter().find(|s| s.id == cur_id) {
            if session_belongs_to_project(cur_sess, &target_db) {
                last_session_per_db.update(|m| {
                    m.insert(target_db, cur_id);
                });
                return;
            }
        }

        let preferred_id = last_session_per_db.get_untracked().get(&target_db).copied();
        let existing_target = preferred_id
            .and_then(|pid| {
                sessions_snap
                    .iter()
                    .find(|s| s.id == pid && session_belongs_to_project(s, &target_db))
            })
            .or_else(|| {
                sessions_snap
                    .iter()
                    .find(|s| session_belongs_to_project(s, &target_db))
            })
            .cloned();

        if let Some(found) = existing_target {
            let ui_msgs = stored_to_ui_messages(&found.messages, next_msg_id);
            set_active_session_id.set(found.id);
            set_messages.set(ui_msgs);
            set_chat_attachments.set(Vec::new());
            set_history_cursor.set(None);
            set_attachment_error.set(None);
            last_session_per_db.update(|m| {
                m.insert(target_db, found.id);
            });
        } else {
            let new_id = next_session_id.get_untracked();
            set_next_session_id.set(new_id + 1);
            let new_session = ChatSession {
                id: new_id,
                title: "Neuer Chat".to_string(),
                messages: vec![default_welcome_stored()],
                project_db: Some(target_db.clone()),
            };
            let ui_msgs = stored_to_ui_messages(&new_session.messages, next_msg_id);
            set_chat_sessions.update(|sessions| {
                sessions.insert(0, new_session);
                save_sessions_to_storage(sessions);
            });
            set_active_session_id.set(new_id);
            set_messages.set(ui_msgs);
            set_chat_attachments.set(Vec::new());
            set_history_cursor.set(None);
            set_attachment_error.set(None);
            last_session_per_db.update(|m| {
                m.insert(target_db, new_id);
            });
        }
    };

    // --- Modal & Ingestion Signals ---
    let (show_ingest_modal, set_show_ingest_modal) = create_signal(false);
    let (is_modal_fullscreen, set_is_modal_fullscreen) = create_signal(true);
    let (viewing_pdf_doc, set_viewing_pdf_doc) = create_signal::<Option<(String, String)>>(None);
    let (viewing_tour_map, set_viewing_tour_map) = create_signal::<Option<(String, String, String)>>(None);
    let (doc_search_term, set_doc_search_term) = create_signal(String::new());
    let (selected_category_filter, set_selected_category_filter) = create_signal("all".to_string());
    let (is_filter_expanded, set_is_filter_expanded) = create_signal(false);
    // Tab innerhalb des Dialogs: "files" (Multi-File Upload), "text" (Manueller Text) oder "docs" (Dokumenten-Verwaltung)
    let (modal_tab, set_modal_tab) = create_signal("files".to_string());
    let (selected_file_names, set_selected_file_names) = create_signal(Vec::<(String, String)>::new());
    let (selected_files, set_selected_files) = create_signal(Vec::<web_sys::File>::new());
    let (is_uploading, set_is_uploading) = create_signal(false);
    let (upload_cancelled, set_upload_cancelled) = create_signal(false);
    let upload_abort_ctrl = create_rw_signal(None::<JsAbortController>);
    let (upload_progress_pct, set_upload_progress_pct) = create_signal(0.0f64);
    let (upload_eta_text, set_upload_eta_text) = create_signal(String::new());

    let (doc_category, set_doc_category) = create_signal(String::new());
    let (ingest_pipeline, set_ingest_pipeline) = create_signal("auto".to_string());
    let (replace_revision, set_replace_revision) = create_signal(true);
    let (extract_graph, set_extract_graph) = create_signal(false);
    let (stored_docs, set_stored_docs) = create_signal(Vec::<DocumentItem>::new());
    let (crm_employees, set_crm_employees) = create_signal(Vec::<CrmEmployeeItem>::new());
    let (crm_search_term, set_crm_search_term) = create_signal(String::new());
    let (is_seeding_crm, set_is_seeding_crm) = create_signal(false);

    let (fleet_vehicles, set_fleet_vehicles) = create_signal(Vec::<FleetVehicleItem>::new());
    let (fleet_tours, set_fleet_tours) = create_signal(Vec::<FleetTourItem>::new());
    let (fleet_warehouses, set_fleet_warehouses) = create_signal(Vec::<FleetWarehouseItem>::new());
    let (fleet_subtab, set_fleet_subtab) = create_signal("vehicles".to_string());
    let (fleet_search_term, set_fleet_search_term) = create_signal(String::new());
    let (is_seeding_fleet, set_is_seeding_fleet) = create_signal(false);
    let (webstore_counts, set_webstore_counts) = create_signal((0usize, 0usize, 0usize));

    let (doc_title, set_doc_title) = create_signal(String::new());
    let (doc_text, set_doc_text) = create_signal(String::new());
    let (ingest_status, set_ingest_status) = create_signal(String::new());
    let file_input_ref = create_node_ref::<leptos::html::Input>();
    let folder_input_ref = create_node_ref::<leptos::html::Input>();

    // Lädt die Liste aller indexierten Dokumente aus SurrealDB (`GET /v1/documents`)
    let refresh_documents = move || {
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/documents").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let list: Vec<DocumentItem> =
                        serde_json::from_value(json["documents"].clone()).unwrap_or_default();
                    set_stored_docs.set(list);
                }
            }
            if let Ok(resp) = Request::get("/v1/db/projects").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(act) = json["active_db"].as_str().filter(|s| !s.is_empty()) {
                        let act_str = act.to_string();
                        set_active_project_db.set(act_str.clone());
                        activate_chat_for_project(act_str);
                    }
                    let list: Vec<ProjectDatabaseItem> =
                        serde_json::from_value(json["databases"].clone()).unwrap_or_default();
                    set_project_databases.set(list);
                }
            }
        });
    };

    // Lädt alle strukturierten Mitarbeiter-Datensätze aus SurrealDB (`GET /v1/crm/employees`)
    let refresh_crm_employees = move || {
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/crm/employees").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let list: Vec<CrmEmployeeItem> =
                        serde_json::from_value(json["employees"].clone()).unwrap_or_default();
                    set_crm_employees.set(list);
                }
            }
        });
    };

    // Lädt alle strukturierten Speditions-Datensätze (LKWs, GPS-Touren, Lagerhallen) aus SurrealDB (`GET /v1/fleet/overview`)
    let refresh_fleet_data = move || {
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/fleet/overview").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let v_list: Vec<FleetVehicleItem> =
                        serde_json::from_value(json["vehicles"].clone()).unwrap_or_default();
                    let t_list: Vec<FleetTourItem> =
                        serde_json::from_value(json["tours"].clone()).unwrap_or_default();
                    let w_list: Vec<FleetWarehouseItem> =
                        serde_json::from_value(json["warehouses"].clone()).unwrap_or_default();
                    set_fleet_vehicles.set(v_list);
                    set_fleet_tours.set(t_list);
                    set_fleet_warehouses.set(w_list);
                }
            }
        });
    };

    let refresh_webstore_data = move || {
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/webstore/overview").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let p_cnt = json["product_count"].as_u64().unwrap_or(0) as usize;
                    let s_cnt = json["sale_count"].as_u64().unwrap_or(0) as usize;
                    let c_cnt = json["contract_count"].as_u64().unwrap_or(0) as usize;
                    set_webstore_counts.set((p_cnt, s_cnt, c_cnt));
                }
            }
        });
    };

    let saved_llm_pref = load_llm_pref_from_storage();
    let initial_llm_model = saved_llm_pref.as_ref().map(|p| p.2.clone()).unwrap_or_else(|| "qwen2.5:7b".to_string());
    let initial_llm_prov = saved_llm_pref.as_ref().map(|p| p.0.clone()).unwrap_or_else(|| "openai_compatible".to_string());
    let initial_llm_url = saved_llm_pref.as_ref().map(|p| p.1.clone()).unwrap_or_else(|| "http://host.docker.internal:11434/v1".to_string());

    let (backend_info, set_backend_info) = create_signal("Verbinde...".to_string());
    let (show_llm_modal, set_show_llm_modal) = create_signal(false);
    // Aktiver Menüpunkt im Einstellungs-Menü: "rag" | "llm" | "runpod" | "system"
    let (settings_tab, set_settings_tab) = create_signal("rag".to_string());
    let (llm_provider_sig, set_llm_provider_sig) = create_signal(initial_llm_prov);
    let (llm_base_url_sig, set_llm_base_url_sig) = create_signal(initial_llm_url);
    let (llm_api_key_sig, set_llm_api_key_sig) = create_signal(String::new());
    let (show_api_key, set_show_api_key) = create_signal(false);
    let (llm_has_saved_key, set_llm_has_saved_key) = create_signal(false);
    let (llm_model_sig, set_llm_model_sig) = create_signal(initial_llm_model.clone());
    let (available_models, set_available_models) =
        create_signal(vec![initial_llm_model]);
    let (llm_config_status, set_llm_config_status) = create_signal(String::new());
    let (is_discovering_models, set_is_discovering_models) = create_signal(false);

    // --- RunPod Cloud GPU Manager Signals ---
    let (rp_phase, set_rp_phase) = create_signal("idle".to_string());
    let (rp_message, set_rp_message) = create_signal(String::new());
    let (rp_pod_id, set_rp_pod_id) = create_signal(None::<String>);
    let (rp_pod_status, set_rp_pod_status) = create_signal(None::<String>);
    let (rp_gpu_type, set_rp_gpu_type) = create_signal("NVIDIA A40".to_string());
    let (rp_cost_hr, set_rp_cost_hr) = create_signal(None::<f64>);
    let (rp_ssh_ep, set_rp_ssh_ep) = create_signal(None::<String>);
    let (rp_tunnel_ok, set_rp_tunnel_ok) = create_signal(false);
    let (rp_enable_llm, set_rp_enable_llm) = create_signal(true);
    let (rp_llm_model, set_rp_llm_model) = create_signal("qwen3:8b".to_string());
    let (rp_llm_ready, set_rp_llm_ready) = create_signal(false);
    let (rp_enable_embed, set_rp_enable_embed) = create_signal(true);
    let (rp_embed_ready, set_rp_embed_ready) = create_signal(false);
    let (rp_has_api_key, set_rp_has_api_key) = create_signal(false);
    let (rp_has_ssh_key, set_rp_has_ssh_key) = create_signal(false);
    let (rp_has_passphrase, set_rp_has_passphrase) = create_signal(false);
    let (rp_needs_passphrase, set_rp_needs_passphrase) = create_signal(false);
    let (rp_api_key_input, set_rp_api_key_input) = create_signal(String::new());
    let (rp_show_api_key, set_rp_show_api_key) = create_signal(false);
    let (rp_ssh_key_input, set_rp_ssh_key_input) = create_signal(String::new());
    let (rp_ssh_key_filename, set_rp_ssh_key_filename) = create_signal(String::new());
    let (rp_ssh_pass_input, set_rp_ssh_pass_input) = create_signal(String::new());
    let (rp_show_ssh_pass, set_rp_show_ssh_pass) = create_signal(false);
    let (rp_polling, set_rp_polling) = create_signal(false);
    let (rp_gpu_catalog, set_rp_gpu_catalog) = create_signal(Vec::<RunpodGpuTypeItem>::new());
    let (rp_only_available_gpus, set_rp_only_available_gpus) = create_signal(false);
    let (rp_loading_gpus, set_rp_loading_gpus) = create_signal(false);
    let rp_ssh_file_input_ref = create_node_ref::<leptos::html::Input>();

    // --- SurrealDB Multi-Project Database & Backup (Import / Export) Signals ---
    let (new_project_db_input, set_new_project_db_input) = create_signal(String::new());
    let (show_sidebar_new_db, set_show_sidebar_new_db) = create_signal(false);
    let (db_import_file, set_db_import_file) = create_signal(None::<web_sys::File>);
    let (db_import_file_label, set_db_import_file_label) = create_signal(String::new());
    let (db_import_mode, set_db_import_mode) = create_signal("merge".to_string());
    let (is_importing_db, set_is_importing_db) = create_signal(false);
    let (db_backup_status, set_db_backup_status) = create_signal(String::new());
    let db_import_input_ref = create_node_ref::<leptos::html::Input>();

    let refresh_project_databases = move || {
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/db/projects").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(act) = json["active_db"].as_str().filter(|s| !s.is_empty()) {
                        let act_str = act.to_string();
                        set_active_project_db.set(act_str.clone());
                        activate_chat_for_project(act_str);
                    }
                    let list: Vec<ProjectDatabaseItem> =
                        serde_json::from_value(json["databases"].clone()).unwrap_or_default();
                    set_project_databases.set(list);
                }
            }
        });
    };

    let switch_or_create_project_db = move |target_db: String| {
        if is_loading.get_untracked() {
            return;
        }
        let trimmed = target_db.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        // Aktuellen Chat noch unter dem bisherigen Projekt sichern, bevor die Projekt-DB wechselt
        persist_current_chat(None);

        spawn_local(async move {
            let payload = serde_json::json!({ "db_name": trimmed });
            if let Ok(resp) = Request::post("/v1/db/projects")
                .header("Content-Type", "application/json")
                .body(payload.to_string())
                .unwrap()
                .send()
                .await
            {
                if resp.ok() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let act = json["active_db"]
                            .as_str()
                            .unwrap_or(&trimmed)
                            .to_string();
                        set_active_project_db.set(act.clone());
                        let list: Vec<ProjectDatabaseItem> =
                            serde_json::from_value(json["databases"].clone()).unwrap_or_default();
                        set_project_databases.set(list);

                        // Automatisch zum passenden Chat des gewählten Projekts wechseln (oder neuen anlegen)
                        activate_chat_for_project(act);

                        refresh_documents();
                        refresh_crm_employees();
                        refresh_fleet_data();
                        refresh_webstore_data();
                    }
                }
            }
        });
    };

    let load_llm_config = move || {
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/llm/config").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let provider = json["llm_provider"]
                        .as_str()
                        .unwrap_or("openai_compatible")
                        .to_string();
                    let base_url = json["llm_base_url"]
                        .as_str()
                        .unwrap_or("http://host.docker.internal:11434/v1")
                        .to_string();
                    let model = json["llm_model"]
                        .as_str()
                        .unwrap_or("qwen2.5:7b")
                        .to_string();
                    let has_key = json["has_api_key"].as_bool().unwrap_or(false);
                    let mut models: Vec<String> =
                        serde_json::from_value(json["available_models"].clone())
                            .unwrap_or_default();
                    if !model.is_empty() && !models.contains(&model) {
                        models.insert(0, model.clone());
                    }

                    // If server is currently running a RunPod tunnel (:11435), always use the server's active RunPod LLM
                    let current_storage_model = load_llm_pref_from_storage();
                    let (eff_prov, eff_url, eff_mod) = if base_url.contains(":11435") {
                        (provider, base_url, model)
                    } else {
                        match current_storage_model {
                            Some((p, u, m)) if m != model && !u.contains(":11435") => {
                                let payload = serde_json::json!({
                                    "llm_provider": p,
                                    "llm_base_url": u,
                                    "llm_model": m
                                });
                                let _ = Request::post("/v1/llm/config")
                                    .header("Content-Type", "application/json")
                                    .body(payload.to_string())
                                    .unwrap()
                                    .send()
                                    .await;
                                (p, u, m)
                            }
                            _ => (provider, base_url, model),
                        }
                    };

                    if !models.contains(&eff_mod) {
                        models.insert(0, eff_mod.clone());
                    }

                    set_llm_provider_sig.set(eff_prov.clone());
                    set_llm_base_url_sig.set(eff_url);
                    set_llm_model_sig.set(eff_mod.clone());
                    set_llm_has_saved_key.set(has_key);
                    set_available_models.set(models);
                    set_backend_info.set(format!("LLM: {} ({})", eff_mod, eff_prov));
                }
            }
        });
    };

    let apply_runpod_json = move |json: &serde_json::Value| -> String {
        let phase = json["phase"].as_str().unwrap_or("idle").to_string();
        let msg = json["message"].as_str().unwrap_or("").to_string();
        set_rp_phase.set(phase.clone());
        set_rp_message.set(msg);
        set_rp_pod_id.set(json["pod_id"].as_str().map(|s| s.to_string()));
        set_rp_pod_status.set(json["pod_status"].as_str().map(|s| s.to_string()));
        if let Some(g) = json["gpu_type"].as_str().filter(|s| !s.is_empty()) {
            set_rp_gpu_type.set(g.to_string());
        }
        set_rp_cost_hr.set(json["cost_per_hr"].as_f64());
        set_rp_ssh_ep.set(json["ssh_endpoint"].as_str().map(|s| s.to_string()));
        set_rp_tunnel_ok.set(json["tunnel_connected"].as_bool().unwrap_or(false));
        if let Some(el) = json["enable_llm"].as_bool() {
            set_rp_enable_llm.set(el);
        }
        if let Some(m) = json["llm_model"].as_str().filter(|s| !s.is_empty()) {
            set_rp_llm_model.set(m.to_string());
        }
        set_rp_llm_ready.set(json["llm_ready"].as_bool().unwrap_or(false));
        if let Some(ee) = json["enable_embedding"].as_bool() {
            set_rp_enable_embed.set(ee);
        }
        set_rp_embed_ready.set(json["embed_ready"].as_bool().unwrap_or(false));
        set_rp_has_api_key.set(json["has_api_key"].as_bool().unwrap_or(false));
        set_rp_has_ssh_key.set(json["has_ssh_key"].as_bool().unwrap_or(false));
        set_rp_has_passphrase.set(json["has_ssh_passphrase"].as_bool().unwrap_or(false));
        set_rp_needs_passphrase.set(json["ssh_key_needs_passphrase"].as_bool().unwrap_or(false));

        if let Some(act_mod) = json["active_llm_model"].as_str().filter(|s| !s.is_empty()) {
            set_llm_model_sig.set(act_mod.to_string());
        }
        if let Some(act_url) = json["active_llm_url"].as_str().filter(|s| !s.is_empty()) {
            set_llm_base_url_sig.set(act_url.to_string());
        }
        phase
    };

    let refresh_runpod_gpus = move || {
        set_rp_loading_gpus.set(true);
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/runpod/gpus").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let list: Vec<RunpodGpuTypeItem> =
                        serde_json::from_value(json["gpus"].clone()).unwrap_or_default();
                    if !list.is_empty() {
                        set_rp_gpu_catalog.set(list);
                    }
                }
            }
            set_rp_loading_gpus.set(false);
        });
    };

    let refresh_runpod_status = move || {
        if rp_gpu_catalog.get_untracked().is_empty() && !rp_loading_gpus.get_untracked() {
            refresh_runpod_gpus();
        }
        spawn_local(async move {
            if let Ok(resp) = Request::get("/v1/runpod/status").send().await {
                if let Ok(mut json) = resp.json::<serde_json::Value>().await {
                    let has_api = json["has_api_key"].as_bool().unwrap_or(false);
                    let needs_pass = json["ssh_key_needs_passphrase"].as_bool().unwrap_or(false);
                    let has_pass = json["has_ssh_passphrase"].as_bool().unwrap_or(false);

                    if !has_api || needs_pass || !has_pass {
                        let (cached_api, cached_pass) = load_rp_creds_from_storage();
                        let should_restore_api = !has_api && cached_api.is_some();
                        let should_restore_pass = (needs_pass || !has_pass) && cached_pass.is_some();

                        if should_restore_api || should_restore_pass {
                            let payload = serde_json::json!({
                                "api_key": if should_restore_api { cached_api } else { None },
                                "ssh_passphrase": if should_restore_pass { cached_pass } else { None },
                                "gpu_type": json["gpu_type"].as_str().unwrap_or("NVIDIA A40"),
                                "enable_llm": json["enable_llm"].as_bool().unwrap_or(true),
                                "llm_model": json["llm_model"].as_str().unwrap_or("qwen3:8b"),
                                "enable_embedding": json["enable_embedding"].as_bool().unwrap_or(true),
                            });
                            if let Ok(r_sync) = Request::post("/v1/runpod/config")
                                .header("Content-Type", "application/json")
                                .body(payload.to_string())
                                .unwrap()
                                .send()
                                .await
                            {
                                if let Ok(synced_json) = r_sync.json::<serde_json::Value>().await {
                                    json = synced_json;
                                }
                            }
                        }
                    }

                    let phase = apply_runpod_json(&json);
                    let is_busy = phase == "deploying"
                        || phase == "waiting_ssh"
                        || phase == "loading_models"
                        || phase == "stopping";
                    if is_busy && !rp_polling.get_untracked() {
                        set_rp_polling.set(true);
                        loop {
                            sleep_ms(2500).await;
                            let Ok(r2) = Request::get("/v1/runpod/status").send().await else {
                                break;
                            };
                            let Ok(j2) = r2.json::<serde_json::Value>().await else {
                                break;
                            };
                            let p2 = apply_runpod_json(&j2);
                            if p2 != "deploying"
                                && p2 != "waiting_ssh"
                                && p2 != "loading_models"
                                && p2 != "stopping"
                            {
                                load_llm_config();
                                break;
                            }
                        }
                        set_rp_polling.set(false);
                    }
                }
            }
        });
    };

    create_effect(move |_| {
        load_llm_config();
        refresh_runpod_status();
        refresh_project_databases();
        refresh_documents();
        refresh_crm_employees();
        refresh_fleet_data();
        refresh_webstore_data();
    });

    // --- Handler: Dateiauswahl (Dateien oder ganzer Ordner) geändert — akkumuliert über mehrere Ordner hinweg ---
    let on_files_selected = move |ev: leptos::ev::Event| {
        let Some(target) = ev.target() else { return };
        let Ok(dom_input) = target.dyn_into::<HtmlInputElement>() else { return };
        let Some(files) = dom_input.files() else { return };

        let mut list = selected_file_names.get_untracked();
        let mut file_vec = selected_files.get_untracked();
        for i in 0..files.length() {
            if let Some(f) = files.get(i) {
                let name = f.name();
                if name.starts_with('.') {
                    continue;
                }
                let blob: &web_sys::Blob = f.as_ref();
                let size_kb = (blob.size() / 1024.0).max(1.0) as u64;
                let rel_path = get_file_relative_path(&f);
                let display_name = if let Some(cat) = extract_category_from_path(&rel_path) {
                    format!("[{}] {}", cat, name)
                } else {
                    name
                };
                if let Some(existing_idx) = list.iter().position(|(existing_name, _)| existing_name == &display_name) {
                    list[existing_idx] = (display_name, format!("{} KB", size_kb));
                    file_vec[existing_idx] = f;
                } else {
                    list.push((display_name, format!("{} KB", size_kb)));
                    file_vec.push(f);
                }
            }
        }
        dom_input.set_value("");
        set_selected_file_names.set(list);
        set_selected_files.set(file_vec);
        set_ingest_status.set(String::new());
    };

    // --- Handler: Multi-File / Folder Upload absenden (Sequenziell pro Datei mit Live-Fortschritt) ---
    let on_upload_files = move |_: leptos::ev::MouseEvent| {
        let file_vec = selected_files.get();
        let count = file_vec.len();
        if count == 0 {
            set_ingest_status.set(if is_en() {
                "Please select at least one file or folder first.".to_string()
            } else {
                "Bitte zuerst mindestens eine Datei oder einen Ordner auswählen.".to_string()
            });
            return;
        }

        set_upload_cancelled.set(false);
        set_is_uploading.set(true);
        set_upload_progress_pct.set(0.0);
        let category_val = doc_category.get();
        let pipeline_val = ingest_pipeline.get();
        let replace_rev_val = replace_revision.get();
        let extract_graph_val = extract_graph.get();
        let is_en_val = is_en();

        spawn_local(async move {
            let total_files = file_vec.len();
            let file_kbs: Vec<f64> = file_vec
                .iter()
                .map(|f| {
                    let blob: &web_sys::Blob = f.as_ref();
                    (blob.size() / 1024.0).max(1.0)
                })
                .collect();
            let total_kb: f64 = file_kbs.iter().sum::<f64>().max(1.0);
            let mut completed_kb: f64 = 0.0;
            // Empirical baseline on local Apple Silicon Metal BGE-M3 (~320 ms / KB for XML statutes)
            let mut ms_per_kb: f64 = 320.0;
            let mut measured_embed_kb: f64 = 0.0;
            let mut measured_embed_ms: f64 = 0.0;
            let upload_start_ms = js_sys::Date::now();

            let mut success_files = 0usize;
            let mut skipped_files = 0usize;
            let mut replaced_files = 0usize;
            let mut total_chunks = 0u64;
            let mut total_entities = 0u64;
            let mut total_relations = 0u64;
            let mut error_messages: Vec<String> = Vec::new();

            // Adaptive Multi-File Batching:
            // Fasst bis zu 16 kleine/mittlere Dateien (max. ~20 MB pro Paket) zusammen, damit PDF/OCR-Extraktion
            // im Backend parallel auf allen CPU-Kernen läuft und alle Chunks in einem einzigen gebündelten
            // GPU-Batch über den SSH-Tunnel berechnet werden (statt 400 einzelner 1-Chunk-Requests).
            let max_files_per_batch: usize = if extract_graph_val && pipeline_val != "legal" {
                4
            } else {
                16
            };
            let max_kb_per_batch: f64 = 20_480.0; // 20 MB

            let mut batches: Vec<Vec<(usize, web_sys::File, f64)>> = Vec::new();
            let mut cur_batch: Vec<(usize, web_sys::File, f64)> = Vec::new();
            let mut cur_batch_kb: f64 = 0.0;

            for (idx, file) in file_vec.into_iter().enumerate() {
                let this_kb = file_kbs.get(idx).copied().unwrap_or(10.0);
                if !cur_batch.is_empty()
                    && (cur_batch.len() >= max_files_per_batch
                        || (cur_batch_kb + this_kb) > max_kb_per_batch)
                {
                    batches.push(cur_batch);
                    cur_batch = Vec::new();
                    cur_batch_kb = 0.0;
                }
                cur_batch_kb += this_kb;
                cur_batch.push((idx, file, this_kb));
            }
            if !cur_batch.is_empty() {
                batches.push(cur_batch);
            }

            let total_batches = batches.len().max(1);

            for (batch_idx, batch) in batches.into_iter().enumerate() {
                if upload_cancelled.get_untracked() {
                    break;
                }
                let first_file_idx = batch.first().map(|(i, _, _)| *i + 1).unwrap_or(1);
                let last_file_idx = batch.last().map(|(i, _, _)| *i + 1).unwrap_or(first_file_idx);
                let batch_kb: f64 = batch.iter().map(|(_, _, kb)| *kb).sum();
                let batch_file_count = batch.len();

                let remaining_kb = (total_kb - completed_kb).max(0.0);
                let est_rem_sec = ((remaining_kb * ms_per_kb) / 1000.0).round() as u64;
                let elapsed_sec = ((js_sys::Date::now() - upload_start_ms) / 1000.0).max(0.0) as u64;
                let pct = ((completed_kb / total_kb) * 100.0).clamp(0.0, 99.0);
                set_upload_progress_pct.set(pct);
                set_upload_eta_text.set(if is_en_val {
                    format!(
                        "Files {}–{}/{} (Batch {}/{} · {:.0}/{:.0} KB · {:.0}%) · Elapsed: {:02}:{:02} · Est. remaining: ~{:02}:{:02}",
                        first_file_idx,
                        last_file_idx,
                        total_files,
                        batch_idx + 1,
                        total_batches,
                        completed_kb,
                        total_kb,
                        pct,
                        elapsed_sec / 60,
                        elapsed_sec % 60,
                        est_rem_sec / 60,
                        est_rem_sec % 60
                    )
                } else {
                    format!(
                        "Dateien {}–{}/{} (Paket {}/{} · {:.0}/{:.0} KB · {:.0}%) · Verstrichen: {:02}:{:02} · Geschätzte Restzeit: ~{:02}:{:02} Min.",
                        first_file_idx,
                        last_file_idx,
                        total_files,
                        batch_idx + 1,
                        total_batches,
                        completed_kb,
                        total_kb,
                        pct,
                        elapsed_sec / 60,
                        elapsed_sec % 60,
                        est_rem_sec / 60,
                        est_rem_sec % 60
                    )
                });

                let batch_start_ms = js_sys::Date::now();
                let form_data = FormData::new().unwrap();
                let _ = form_data.append_with_str("pipeline", &pipeline_val);
                let _ = form_data.append_with_str(
                    "replace_revision",
                    if replace_rev_val { "true" } else { "false" },
                );
                let _ = form_data.append_with_str(
                    "extract_graph",
                    if extract_graph_val { "true" } else { "false" },
                );

                let mut preview_names: Vec<String> = Vec::with_capacity(batch_file_count);
                for (_, file, _) in &batch {
                    let filename = file.name();
                    let rel_path = get_file_relative_path(file);
                    let auto_folder = extract_category_from_path(&rel_path);
                    let effective_category = match (category_val.trim(), auto_folder) {
                        (cat, Some(folder)) if !cat.is_empty() => format!("{} / {}", cat, folder),
                        ("", Some(folder)) => folder,
                        (cat, None) if !cat.is_empty() => cat.to_string(),
                        _ => String::new(),
                    };
                    let upload_filename = if !effective_category.is_empty()
                        && !filename.trim_start().starts_with('[')
                    {
                        format!("[{}] {}", effective_category, filename)
                    } else {
                        filename.clone()
                    };
                    if preview_names.len() < 3 {
                        preview_names.push(upload_filename.clone());
                    }
                    let _ =
                        form_data.append_with_blob_and_filename("files", file, &upload_filename);
                }

                let preview_label = if batch_file_count > 3 {
                    format!("{} (+{} weitere)", preview_names.join(", "), batch_file_count - 3)
                } else {
                    preview_names.join(", ")
                };

                let step_desc = if is_en_val {
                    "Parallel PDF/OCR text extraction & batched GPU embedding..."
                } else {
                    "Parallele PDF/OCR-Textextraktion & gebündeltes GPU-Batch-Embedding..."
                };
                set_ingest_status.set(format!(
                    "⏳ {} {}–{}/{} ({}): {}",
                    if is_en_val { "Files" } else { "Dateien" },
                    first_file_idx,
                    last_file_idx,
                    total_files,
                    preview_label,
                    step_desc
                ));

                let ctrl = JsAbortController::new();
                let signal = ctrl.signal();
                upload_abort_ctrl.set(Some(ctrl));

                match Request::post("/v1/ingest/file")
                    .abort_signal(Some(&signal))
                    .body(form_data)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        let data: serde_json::Value = resp.json().await.unwrap_or_default();
                        let skipped = data["skipped_duplicates"].as_u64().unwrap_or(0) as usize;
                        let replaced = data["replaced_revisions"].as_u64().unwrap_or(0) as usize;
                        let files_proc = data["files_processed"]
                            .as_u64()
                            .unwrap_or(batch_file_count as u64)
                            as usize;
                        let chunks = data["chunks_indexed"].as_u64().unwrap_or(0);
                        let g_ent = data["graph_entities_extracted"].as_u64().unwrap_or(0);
                        let g_rel = data["graph_relations_extracted"].as_u64().unwrap_or(0);
                        total_entities += g_ent;
                        total_relations += g_rel;
                        replaced_files += replaced;
                        skipped_files += skipped;

                        let newly_indexed = files_proc.saturating_sub(skipped);
                        total_chunks += chunks;
                        success_files += newly_indexed;

                        if let Some(errs) = data["errors"].as_array() {
                            for err_val in errs {
                                if let Some(s) = err_val.as_str() {
                                    if !s.is_empty() {
                                        error_messages.push(s.to_string());
                                    }
                                }
                            }
                        }

                        if newly_indexed > 0 {
                            let batch_took_ms = (js_sys::Date::now() - batch_start_ms).max(50.0);
                            measured_embed_kb += batch_kb;
                            measured_embed_ms += batch_took_ms;
                            if measured_embed_kb >= 5.0 {
                                ms_per_kb = (measured_embed_ms / measured_embed_kb).clamp(5.0, 2000.0);
                            }
                        }
                        completed_kb += batch_kb;
                    }
                    Ok(resp) => {
                        if upload_cancelled.get_untracked() {
                            break;
                        }
                        let err = resp.text().await.unwrap_or_default();
                        error_messages.push(format!("Batch {}: {}", batch_idx + 1, err));
                        completed_kb += batch_kb;
                    }
                    Err(e) => {
                        if upload_cancelled.get_untracked() {
                            break;
                        }
                        let net_lbl = if is_en_val { "Network" } else { "Netzwerk" };
                        error_messages.push(format!("Batch {} ({}: {})", batch_idx + 1, net_lbl, e));
                        completed_kb += batch_kb;
                    }
                }
            }

            upload_abort_ctrl.set(None);
            set_upload_progress_pct.set(100.0);
            set_upload_eta_text.set(String::new());

            if upload_cancelled.get_untracked() {
                set_ingest_status.set(if is_en_val {
                    format!(
                        "⏹️ Upload cancelled by user ({} file(s) indexed, {} chunks before cancellation).",
                        success_files, total_chunks
                    )
                } else {
                    format!(
                        "⏹️ Upload vom Nutzer abgebrochen ({} Datei(en) bzw. {} Chunks bis zum Abbruch indexiert).",
                        success_files, total_chunks
                    )
                });
                refresh_documents();
                set_is_uploading.set(false);
                return;
            }

            let mut notes = Vec::new();
            if total_entities > 0 || total_relations > 0 {
                notes.push(if is_en_val {
                    format!("🕸️ {} graph entities & {} relations linked", total_entities, total_relations)
                } else {
                    format!("🕸️ {} Graph-Entitäten & {} Beziehungen verknüpft", total_entities, total_relations)
                });
            }
            if replaced_files > 0 {
                notes.push(if is_en_val {
                    format!("🔄 {} older revision(s) replaced", replaced_files)
                } else {
                    format!("🔄 {} ältere Revision(en) ersetzt", replaced_files)
                });
            }
            if skipped_files > 0 {
                notes.push(if is_en_val {
                    format!("⏭️ {} unchanged file(s) skipped", skipped_files)
                } else {
                    format!("⏭️ {} unveränderte Datei(en) übersprungen", skipped_files)
                });
            }
            let extra_note = if notes.is_empty() {
                String::new()
            } else {
                format!(" ({})", notes.join(" · "))
            };

            if error_messages.is_empty() {
                set_ingest_status.set(if is_en_val {
                    format!(
                        "✅ {} file(s) indexed ({} chunks){}",
                        success_files, total_chunks, extra_note
                    )
                } else {
                    format!(
                        "✅ {} Datei(en) neu indexiert ({} Chunks){}",
                        success_files, total_chunks, extra_note
                    )
                });
                set_selected_file_names.set(vec![]);
                set_selected_files.set(vec![]);
                if let Some(el) = file_input_ref.get() {
                    el.set_value("");
                }
                if let Some(el) = folder_input_ref.get() {
                    el.set_value("");
                }
            } else if success_files > 0 || skipped_files > 0 {
                set_ingest_status.set(if is_en_val {
                    format!(
                        "⚠️ {} indexed ({} chunks){}. Errors in: {}",
                        success_files,
                        total_chunks,
                        extra_note,
                        error_messages.join(" | ")
                    )
                } else {
                    format!(
                        "⚠️ {} neu indexiert ({} Chunks){}. Fehler bei: {}",
                        success_files,
                        total_chunks,
                        extra_note,
                        error_messages.join(" | ")
                    )
                });
            } else {
                set_ingest_status.set(if is_en_val {
                    format!(
                        "❌ Upload failed: {}",
                        error_messages.join(" | ")
                    )
                } else {
                    format!(
                        "❌ Upload fehlgeschlagen: {}",
                        error_messages.join(" | ")
                    )
                });
            }

            if error_messages.is_empty() {
                set_selected_file_names.set(Vec::new());
                set_selected_files.set(Vec::new());
            }
            refresh_documents();
            set_is_uploading.set(false);
        });
    };

    // --- Handler: Manuellen Text in SurrealDB speichern ---
    let on_ingest_text = move |_: leptos::ev::MouseEvent| {
        let title = doc_title.get();
        let text = doc_text.get();
        let cat = doc_category.get();
        let replace_rev = replace_revision.get();
        let extract_graph_val = extract_graph.get();
        let is_en_val = is_en();
        if title.trim().is_empty() || text.trim().is_empty() {
            set_ingest_status.set(if is_en_val {
                "Please enter a title and text.".to_string()
            } else {
                "Bitte Titel und Text eingeben.".to_string()
            });
            return;
        }

        set_is_uploading.set(true);
        set_ingest_status.set(if is_en_val {
            if extract_graph_val {
                "⏳ Indexing text & extracting 🕸️ Knowledge Graph relations into SurrealDB...".to_string()
            } else {
                "⏳ Indexing text in SurrealDB (HNSW + BM25)...".to_string()
            }
        } else {
            if extract_graph_val {
                "⏳ Indexiere Text & extrahiere 🕸️ Wissensgraph-Beziehungen in SurrealDB...".to_string()
            } else {
                "⏳ Indexiere Text in SurrealDB (HNSW + BM25)...".to_string()
            }
        });
        spawn_local(async move {
            let payload = serde_json::json!({
                "title": title,
                "category": if cat.trim().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(cat.trim().to_string()) },
                "replace_revision": replace_rev,
                "extract_graph": extract_graph_val,
                "source": "leptos_web_ui",
                "text": text
            });

            match Request::post("/v1/ingest")
                .header("Content-Type", "application/json")
                .body(payload.to_string())
                .unwrap()
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    let data: serde_json::Value = resp.json().await.unwrap_or_default();
                    let chunks = data["chunks_indexed"].as_u64().unwrap_or(0);
                    let g_ent = data["graph_entities_extracted"].as_u64().unwrap_or(0);
                    let g_rel = data["graph_relations_extracted"].as_u64().unwrap_or(0);
                    let replaced = data["replaced_revision"].as_bool().unwrap_or(false);
                    let skipped = data["skipped_duplicate"].as_bool().unwrap_or(false);
                    let graph_note = if g_ent > 0 || g_rel > 0 {
                        if is_en_val {
                            format!(" · 🕸️ {} entities & {} relations", g_ent, g_rel)
                        } else {
                            format!(" · 🕸️ {} Entitäten & {} Beziehungen", g_ent, g_rel)
                        }
                    } else {
                        String::new()
                    };
                    if skipped {
                        set_ingest_status.set(if is_en_val {
                            format!("⏭️ '{}' already exists in SurrealDB with identical content.", title)
                        } else {
                            format!("⏭️ '{}' ist mit identischem Inhalt bereits in SurrealDB vorhanden.", title)
                        });
                    } else if replaced {
                        set_ingest_status.set(if is_en_val {
                            format!(
                                "✅ New revision of '{}' saved ({} chunks{}, older version automatically replaced)!",
                                title, chunks, graph_note
                            )
                        } else {
                            format!(
                                "✅ Neue Revision von '{}' gespeichert ({} Chunks{}, ältere Fassung wurde automatisch ersetzt)!",
                                title, chunks, graph_note
                            )
                        });
                    } else {
                        set_ingest_status.set(if is_en_val {
                            format!("✅ Success! '{}' was indexed into {} chunks{}.", title, chunks, graph_note)
                        } else {
                            format!("✅ Erfolgreich! '{}' wurde in {} Chunks indexiert{}.", title, chunks, graph_note)
                        });
                    }
                    set_doc_title.set(String::new());
                    set_doc_text.set(String::new());
                    refresh_documents();
                }
                Ok(resp) => {
                    let err = resp.text().await.unwrap_or_default();
                    let err_lbl = if is_en_val { "Error" } else { "Fehler" };
                    set_ingest_status.set(format!("❌ {}: {}", err_lbl, err));
                }
                Err(e) => {
                    let net_lbl = if is_en_val { "Network error" } else { "Netzwerkfehler" };
                    set_ingest_status.set(format!("❌ {}: {}", net_lbl, e));
                }
            }
            set_is_uploading.set(false);
        });
    };

    // --- Handler: Temporäre Dateien (📎) im Chat-Fenster auswählen (akkumuliert mehrere Dateien auch aus unterschiedlichen Ordnern im RAM) ---
    let on_chat_attachment_selected = move |ev: leptos::ev::Event| {
        let Some(target) = ev.target() else { return };
        let Ok(dom_input) = target.dyn_into::<HtmlInputElement>() else { return };
        let Some(files) = dom_input.files() else { return };
        if files.length() == 0 {
            return;
        }

        let mut picked_files = Vec::new();
        for i in 0..files.length() {
            if let Some(f) = files.get(i) {
                picked_files.push(f);
            }
        }
        if picked_files.is_empty() {
            return;
        }

        set_is_extracting_attachment.set(true);
        set_attachment_error.set(None);

        spawn_local(async move {
            for file in picked_files {
                let filename = file.name();
                let form_data = FormData::new().unwrap();
                let _ = form_data.append_with_blob_and_filename("file", &file, &filename);

                match Request::post("/v1/extract")
                    .body(form_data)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        let data: serde_json::Value = resp.json().await.unwrap_or_default();
                        let text = data["text"].as_str().unwrap_or_default().to_string();
                        let char_count = data["char_count"].as_u64().unwrap_or(0) as usize;
                        let chunk_count = data["chunk_count"].as_u64().unwrap_or(0) as usize;
                        let new_doc = AttachedChatDoc {
                            filename: filename.clone(),
                            text,
                            char_count,
                            chunk_count,
                        };
                        set_chat_attachments.update(|list| {
                            if let Some(existing) = list.iter_mut().find(|d| d.filename == filename) {
                                *existing = new_doc;
                            } else {
                                list.push(new_doc);
                            }
                        });
                        set_attachment_only.set(true);
                    }
                    Ok(resp) => {
                        let err = resp.text().await.unwrap_or_default();
                        let msg = if is_en() {
                            format!("Error reading '{}': {}", filename, err)
                        } else {
                            format!("Fehler beim Lesen von '{}': {}", filename, err)
                        };
                        set_attachment_error.set(Some(msg));
                    }
                    Err(e) => {
                        let msg = if is_en() {
                            format!("Network error: {}", e)
                        } else {
                            format!("Netzwerkfehler: {}", e)
                        };
                        set_attachment_error.set(Some(msg));
                    }
                }
            }
            if let Some(el) = chat_attach_input_ref.get() {
                el.set_value("");
            }
            set_is_extracting_attachment.set(false);
        });
    };

    // --- Handler: Laufenden Upload abbrechen ---
    let cancel_upload = move |_: leptos::ev::MouseEvent| {
        set_upload_cancelled.set(true);
        if let Some(ctrl) = upload_abort_ctrl.get_untracked() {
            ctrl.abort();
        }
        upload_abort_ctrl.set(None);
    };

    // --- Handler: Laufende Chat-Antwort / Prompt abbrechen ---
    let stop_chat_generation = move || {
        if !is_loading.get_untracked() {
            return;
        }
        set_chat_abort_gen.update(|g| *g = g.wrapping_add(1));
        if let Some(ctrl) = chat_abort_ctrl.get_untracked() {
            ctrl.abort();
        }
        chat_abort_ctrl.set(None);

        let is_en_now = is_en();
        set_messages.update(|msgs| {
            if let Some(last) = msgs.last_mut() {
                if last.role == "assistant" {
                    let cur = last.content.get_untracked();
                    let is_status_placeholder = cur.starts_with("🔍 ")
                        || cur.starts_with("📎 ")
                        || cur.starts_with("✍️ ");
                    if is_status_placeholder || cur.trim().is_empty() {
                        last.content.set(if is_en_now {
                            "⏹️ Generation stopped by user.".to_string()
                        } else {
                            "⏹️ Antwortgenerierung vom Nutzer abgebrochen.".to_string()
                        });
                    } else {
                        let mut next_txt = cur;
                        if next_txt.contains("<think>") && !next_txt.contains("</think>") {
                            next_txt.push_str("\n</think>\n\n");
                        } else {
                            next_txt.push_str("\n\n");
                        }
                        next_txt.push_str(if is_en_now {
                            "*(⏹️ Stopped by user)*"
                        } else {
                            "*(⏹️ Vom Nutzer abgebrochen)*"
                        });
                        last.content.set(next_txt);
                    }
                }
            }
        });
        persist_current_chat(None);
        set_is_loading.set(false);
    };

    // --- Handler: Chat-Frage absenden (Echtzeit-Streaming) ---
    let send_message = move || {
        let prompt = user_input.get();
        if prompt.trim().is_empty() || is_loading.get() || is_extracting_attachment.get() {
            return;
        }

        let my_gen = chat_abort_gen.get_untracked().wrapping_add(1);
        set_chat_abort_gen.set(my_gen);

        set_user_input.set(String::new());
        set_history_cursor.set(None);
        set_draft_before_history.set(String::new());
        set_is_loading.set(true);

        let active_attachments = chat_attachments.get();
        let has_attachments = !active_attachments.is_empty();
        let attach_only_mode = has_attachments && attachment_only.get();

        let user_msg_id = next_msg_id.get();
        let assistant_msg_id = user_msg_id + 1;
        next_msg_id.set(assistant_msg_id + 1);

        let displayed_user_prompt = if has_attachments {
            let names = active_attachments
                .iter()
                .map(|a| a.filename.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let mode_badge = if is_en() {
                if attach_only_mode { "Attachment only" } else { "Attachment + DB" }
            } else {
                if attach_only_mode { "Nur Anhang" } else { "Anhang + DB" }
            };
            format!("📎 [{} · {}] {}", names, mode_badge, prompt)
        } else {
            prompt.clone()
        };

        set_messages.update(|msgs| {
            msgs.push(UiChatMessage {
                id: user_msg_id,
                role: "user".to_string(),
                content: create_rw_signal(displayed_user_prompt),
                sources: create_rw_signal(vec![]),
                metrics_info: create_rw_signal(None),
            });
        });

        let auto_title = make_session_title(&prompt);
        persist_current_chat(Some(auto_title));

        let mut history_payload: Vec<ChatMessagePayload> = messages
            .get()
            .into_iter()
            .map(|m| ChatMessagePayload {
                role: m.role,
                content: m.content.get_untracked(),
            })
            .collect();
        // Stelle sicher, dass die aktuelle Nutzerfrage ohne das visuelle "📎 [...]" Badge an Retrieval & LLM geht
        if let Some(last_msg) = history_payload.last_mut() {
            if last_msg.role == "user" {
                last_msg.content = prompt.clone();
            }
        }
        if let Some(last_msg) = history_payload.last_mut() {
            last_msg.content = prompt.clone();
        }

        let initial_status_msg = if is_en() {
            if has_attachments && attach_only_mode {
                format!("📎 Analyzing {} attached file(s) in RAM...", active_attachments.len())
            } else {
                "🔍 Searching in SurrealDB & starting reranking...".to_string()
            }
        } else {
            if has_attachments && attach_only_mode {
                format!("📎 Analysiere {} angehängte Datei(en) im Arbeitsspeicher...", active_attachments.len())
            } else {
                "🔍 Suche in SurrealDB & starte Reranking...".to_string()
            }
        };
        let assistant_content = create_rw_signal(initial_status_msg);
        let assistant_sources = create_rw_signal(Vec::<RetrievedChunk>::new());
        let assistant_metrics = create_rw_signal(None::<String>);

        set_messages.update(|msgs| {
            msgs.push(UiChatMessage {
                id: assistant_msg_id,
                role: "assistant".to_string(),
                content: assistant_content,
                sources: assistant_sources,
                metrics_info: assistant_metrics,
            });
        });

        let rag_enabled = use_rag.get();
        let selected_reranker = reranker_mode.get();
        let k = top_n.get();
        let current_ctx = num_ctx_sig.get();
        let current_think = think_mode_sig.get();
        let current_temp = temperature_sig.get();
        let active_llm_model = llm_model_sig.get();

        spawn_local(async move {
            let attachment_json = if active_attachments.is_empty() {
                None
            } else if active_attachments.len() == 1 {
                let att = &active_attachments[0];
                Some(serde_json::json!({
                    "filename": att.filename,
                    "text": att.text
                }))
            } else {
                let combined_filenames = active_attachments
                    .iter()
                    .map(|a| a.filename.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let combined_text = active_attachments
                    .iter()
                    .map(|a| format!("# {}\n\n{}", a.filename, a.text))
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n");
                Some(serde_json::json!({
                    "filename": combined_filenames,
                    "text": combined_text
                }))
            };

            let body = serde_json::json!({
                "model": active_llm_model,
                "temperature": current_temp,
                "use_rag": rag_enabled,
                "reranker": selected_reranker,
                "top_n": k,
                "num_ctx": current_ctx,
                "think": current_think,
                "attachment": attachment_json,
                "attachment_only": attach_only_mode,
                "messages": history_payload
            });

            let ctrl = JsAbortController::new();
            let signal = ctrl.signal();
            chat_abort_ctrl.set(Some(ctrl));

            match Request::post("/v1/chat/stream")
                .abort_signal(Some(&signal))
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .unwrap()
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    if chat_abort_gen.get_untracked() != my_gen {
                        return;
                    }
                    if let Some(body_stream) = resp.body() {
                        let reader: web_sys::ReadableStreamDefaultReader =
                            body_stream.get_reader().unchecked_into();

                        let mut raw_bytes: Vec<u8> = Vec::new();
                        let mut first_token = true;
                        let mut base_metrics_str = String::new();

                        loop {
                            if chat_abort_gen.get_untracked() != my_gen {
                                let _ = reader.cancel();
                                return;
                            }
                            let read_promise = reader.read();
                            let Ok(js_result) =
                                wasm_bindgen_futures::JsFuture::from(read_promise).await
                            else {
                                break;
                            };
                            if chat_abort_gen.get_untracked() != my_gen {
                                let _ = reader.cancel();
                                return;
                            }

                            let done = js_sys::Reflect::get(&js_result, &"done".into())
                                .ok()
                                .and_then(|v| v.as_bool())
                                .unwrap_or(true);
                            if done {
                                break;
                            }

                            if let Ok(val) = js_sys::Reflect::get(&js_result, &"value".into()) {
                                let chunk_vec = js_sys::Uint8Array::new(&val).to_vec();
                                raw_bytes.extend_from_slice(&chunk_vec);

                                while let Some(newline_pos) =
                                    raw_bytes.iter().position(|&b| b == b'\n')
                                {
                                    let line_bytes: Vec<u8> =
                                        raw_bytes.drain(..=newline_pos).collect();
                                    let line = String::from_utf8_lossy(&line_bytes)
                                        .trim()
                                        .to_string();

                                    if line.is_empty() {
                                        continue;
                                    }

                                    if let Ok(event) =
                                        serde_json::from_str::<serde_json::Value>(&line)
                                    {
                                        match event["type"].as_str().unwrap_or_default() {
                                            "sources" => {
                                                let parsed_sources: Vec<RetrievedChunk> =
                                                    serde_json::from_value(
                                                        event["sources"].clone(),
                                                    )
                                                    .unwrap_or_default();
                                                assistant_sources.set(parsed_sources);

                                                if let Some(m) = event.get("metrics") {
                                                    if !m.is_null() {
                                                        let embed_ms =
                                                            m["embed_ms"].as_u64().unwrap_or(0);
                                                        let db_ms =
                                                            m["db_ms"].as_u64().unwrap_or(0);
                                                        let rerank_ms =
                                                            m["rerank_ms"].as_u64().unwrap_or(0);
                                                        let reranker_used = m["reranker_used"]
                                                            .as_str()
                                                            .unwrap_or("unbekannt");
                                                        base_metrics_str = format!(
                                                            "⏱️ Embed: {} ms · SurrealDB: {} ms · Rerank: {} ms ({})",
                                                            embed_ms, db_ms, rerank_ms, reranker_used
                                                        );
                                                        assistant_metrics
                                                            .set(Some(base_metrics_str.clone()));
                                                    }
                                                }

                                                assistant_content
                                                    .set("✍️ Generiere Antwort...".to_string());
                                            }
                                            "token" => {
                                                let tok =
                                                    event["content"].as_str().unwrap_or_default();
                                                if first_token {
                                                    assistant_content.set(tok.to_string());
                                                    first_token = false;
                                                } else {
                                                    assistant_content.update(|c| c.push_str(tok));
                                                }
                                            }
                                            "done" => {
                                                let ttft_ms = event["llm_first_token_ms"]
                                                    .as_u64()
                                                    .unwrap_or(0);
                                                let total_ms =
                                                    event["total_ms"].as_u64().unwrap_or(0);
                                                let full_metrics = if base_metrics_str.is_empty() {
                                                    format!(
                                                        "⏱️ LLM First-Token: {} ms · Gesamt: {:.2} s",
                                                        ttft_ms,
                                                        total_ms as f64 / 1000.0
                                                    )
                                                } else {
                                                    format!(
                                                        "{} · LLM First-Token: {} ms · Gesamt: {:.2} s",
                                                        base_metrics_str,
                                                        ttft_ms,
                                                        total_ms as f64 / 1000.0
                                                    )
                                                };
                                                assistant_metrics.set(Some(full_metrics));
                                            }
                                            "error" => {
                                                let err = event["error"]
                                                    .as_str()
                                                    .unwrap_or("Unbekannter Fehler");
                                                if first_token {
                                                    assistant_content
                                                        .set(format!("❌ Fehler: {}", err));
                                                    first_token = false;
                                                } else {
                                                    assistant_content.update(|c| {
                                                        c.push_str(&format!(
                                                            "\n\n---\n❌ **Stream-Fehler:** {}",
                                                            err
                                                        ));
                                                    });
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }

                        if first_token {
                            assistant_content.set(
                                "⚠️ Das LLM hat eine leere Antwort zurückgegeben.".to_string(),
                            );
                        }
                    }
                }
                Ok(resp) => {
                    if chat_abort_gen.get_untracked() != my_gen {
                        return;
                    }
                    let err_txt = resp.text().await.unwrap_or_default();
                    assistant_content.set(format!("❌ Fehler vom Server: {}", err_txt));
                }
                Err(e) => {
                    if chat_abort_gen.get_untracked() != my_gen {
                        return;
                    }
                    assistant_content.set(format!("❌ Verbindungsfehler: {}", e));
                }
            }
            if chat_abort_gen.get_untracked() == my_gen {
                chat_abort_ctrl.set(None);
                persist_current_chat(None);
                set_is_loading.set(false);
            }
        });
    };

    // --- Handler: Neuen Chat anlegen ---
    let create_new_chat = move || {
        if is_loading.get() {
            return;
        }
        persist_current_chat(None);
        let new_id = next_session_id.get();
        set_next_session_id.set(new_id + 1);
        let cur_db = active_project_db.get_untracked();

        let new_session = ChatSession {
            id: new_id,
            title: "Neuer Chat".to_string(),
            messages: vec![default_welcome_stored()],
            project_db: Some(cur_db.clone()),
        };
        let ui_msgs = stored_to_ui_messages(&new_session.messages, next_msg_id);

        set_chat_sessions.update(|sessions| {
            sessions.insert(0, new_session);
            save_sessions_to_storage(sessions);
        });
        set_active_session_id.set(new_id);
        set_messages.set(ui_msgs);
        set_chat_attachments.set(Vec::new());
        set_history_cursor.set(None);
        set_attachment_error.set(None);
        last_session_per_db.update(|m| {
            m.insert(cur_db, new_id);
        });
    };

    // --- Handler: Zu einem bestehenden Chat wechseln ---
    let switch_to_chat = move |target_id: usize| {
        if is_loading.get() || active_session_id.get() == target_id {
            return;
        }
        persist_current_chat(None);
        let sessions = chat_sessions.get();
        if let Some(target_sess) = sessions.iter().find(|s| s.id == target_id) {
            let ui_msgs = stored_to_ui_messages(&target_sess.messages, next_msg_id);
            let linked_db = target_sess
                .project_db
                .clone()
                .filter(|d| !d.trim().is_empty())
                .unwrap_or_else(|| "knowledge_base".to_string());
            set_active_session_id.set(target_id);
            set_messages.set(ui_msgs);
            set_chat_attachments.set(Vec::new());
            set_history_cursor.set(None);
            set_attachment_error.set(None);
            last_session_per_db.update(|m| {
                m.insert(linked_db.clone(), target_id);
            });
            if linked_db != active_project_db.get_untracked() {
                switch_or_create_project_db(linked_db);
            }
        }
    };

    // --- Handler: Einen Chat aus der Seitenleiste löschen ---
    let delete_chat_session = move |delete_id: usize| {
        if is_loading.get() {
            return;
        }
        persist_current_chat(None);
        let cur_db = active_project_db.get_untracked();
        let mut sessions = chat_sessions.get();
        sessions.retain(|s| s.id != delete_id);

        if !sessions.iter().any(|s| session_belongs_to_project(s, &cur_db)) {
            let fresh_id = next_session_id.get();
            set_next_session_id.set(fresh_id + 1);
            sessions.insert(
                0,
                ChatSession {
                    id: fresh_id,
                    title: "Neuer Chat".to_string(),
                    messages: vec![default_welcome_stored()],
                    project_db: Some(cur_db.clone()),
                },
            );
        }

        let current_active = active_session_id.get();
        if current_active == delete_id
            || !sessions
                .iter()
                .any(|s| s.id == current_active && session_belongs_to_project(s, &cur_db))
        {
            if let Some(next_active) = sessions
                .iter()
                .find(|s| session_belongs_to_project(s, &cur_db))
                .cloned()
            {
                let ui_msgs = stored_to_ui_messages(&next_active.messages, next_msg_id);
                set_active_session_id.set(next_active.id);
                set_messages.set(ui_msgs);
                set_chat_attachments.set(Vec::new());
                set_history_cursor.set(None);
                set_attachment_error.set(None);
                last_session_per_db.update(|m| {
                    m.insert(cur_db, next_active.id);
                });
            }
        }

        save_sessions_to_storage(&sessions);
        set_chat_sessions.set(sessions);
    };

    // --- State & Handler für das 3-Punkte-Menü (⋯) und Umbenennen in der Sidebar ---
    let (open_chat_menu_id, set_open_chat_menu_id) = create_signal::<Option<usize>>(None);
    let (editing_chat_id, set_editing_chat_id) = create_signal::<Option<usize>>(None);
    let (editing_chat_title, set_editing_chat_title) = create_signal(String::new());

    let save_renamed_chat = move |target_id: usize| {
        let new_title = editing_chat_title.get().trim().to_string();
        if !new_title.is_empty() {
            set_chat_sessions.update(|sessions| {
                if let Some(sess) = sessions.iter_mut().find(|s| s.id == target_id) {
                    sess.title = new_title;
                }
                save_sessions_to_storage(sessions);
            });
        }
        set_editing_chat_id.set(None);
    };

    let export_session_by_id = move |target_id: usize| {
        if active_session_id.get_untracked() == target_id {
            persist_current_chat(None);
        }
        let sessions = chat_sessions.get_untracked();
        if let Some(sess) = sessions.into_iter().find(|s| s.id == target_id) {
            let model = llm_model_sig.get_untracked();
            let rag = use_rag.get_untracked();
            let r_mode = reranker_mode.get_untracked();
            let tn = top_n.get_untracked();
            let ctx = num_ctx_sig.get_untracked();
            let think = think_mode_sig.get_untracked();
            export_chat_session_markdown(&sess.title, &model, rag, &r_mode, tn, ctx, think, &sess.messages);
        }
    };

    view! {
        <div
            class="app-layout"
            on:click=move |_| {
                if open_chat_menu_id.get().is_some() {
                    set_open_chat_menu_id.set(None);
                }
            }
        >
            // --- Linke Navigation / Sidebar ---
            <aside class="sidebar">
                <div class="sidebar-top">
                    <div class="brand">
                        <h1>"🦀 Rust RAG Studio"</h1>
                        <span>"SurrealDB 4.0 • TEI • Leptos WASM"</span>
                        <span style="color: #818cf8; margin-top: 4px;">{move || backend_info.get()}</span>
                    </div>

                    // --- Multi-Projekt / SurrealDB Datenbank-Umschalter direkt in der Sidebar ---
                    <div style="background: rgba(15, 23, 42, 0.65); border: 1px solid #334155; border-radius: 0.55rem; padding: 0.55rem 0.65rem; display: flex; flex-direction: column; gap: 0.4rem;">
                        <div style="display: flex; justify-content: space-between; align-items: center;">
                            <span style="font-size: 0.7rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #a5b4fc;">
                                {move || if is_en() { "🗄️ Project DB (SurrealDB)" } else { "🗄️ Projekt-DB (SurrealDB)" }}
                            </span>
                            <div style="display:flex; gap:0.25rem;">
                                <button
                                    type="button"
                                    style="background: rgba(99, 102, 241, 0.2); border: 1px solid rgba(99, 102, 241, 0.45); color: #e0e7ff; font-size: 0.7rem; padding: 0.12rem 0.42rem; border-radius: 0.3rem; cursor: pointer;"
                                    title=move || if is_en() { "Create new project database" } else { "Neue Projekt-Datenbank anlegen" }
                                    on:click=move |_| set_show_sidebar_new_db.update(|v| *v = !*v)
                                >
                                    "+ DB"
                                </button>
                                <button
                                    type="button"
                                    style="background: transparent; border: 1px solid #334155; color: #94a3b8; font-size: 0.7rem; padding: 0.12rem 0.38rem; border-radius: 0.3rem; cursor: pointer;"
                                    title=move || if is_en() { "Manage project databases & backups" } else { "Projekt-Datenbanken & Backups verwalten" }
                                    on:click=move |_| {
                                        set_db_backup_status.set(String::new());
                                        refresh_project_databases();
                                        refresh_documents();
                                        set_settings_tab.set("backup".to_string());
                                        set_show_llm_modal.set(true);
                                    }
                                >
                                    "⚙️"
                                </button>
                            </div>
                        </div>

                        <select
                            style="padding: 0.38rem 0.55rem; font-size: 0.79rem; font-weight: 600; background: #0b0f19; border-color: #475569; color: #f8fafc;"
                            disabled=move || is_loading.get()
                            prop:value=move || active_project_db.get()
                            on:change=move |ev| {
                                let chosen = event_target_value(&ev);
                                if !chosen.is_empty() && chosen != active_project_db.get_untracked() {
                                    switch_or_create_project_db(chosen);
                                }
                            }
                        >
                            <For
                                each=move || {
                                    let list = project_databases.get();
                                    if list.is_empty() {
                                        vec![ProjectDatabaseItem {
                                            db_name: active_project_db.get(),
                                            is_active: true,
                                            is_default: true,
                                            document_count: stored_docs.get().len(),
                                            chunk_count: stored_docs.get().iter().map(|d| d.chunk_count).sum(),
                                        }]
                                    } else {
                                        list
                                    }
                                }
                                key=|db| format!("{}_{}_{}", db.db_name, db.document_count, db.chunk_count)
                                children=move |db| {
                                    let val = db.db_name.clone();
                                    let label = format!("{} ({} Dok.)", db.db_name, db.document_count);
                                    let is_act = db.db_name == active_project_db.get_untracked();
                                    view! {
                                        <option value=val selected=is_act>{label}</option>
                                    }
                                }
                            />
                        </select>

                        {move || show_sidebar_new_db.get().then(|| view! {
                            <div style="display:flex; gap:0.3rem; margin-top:0.15rem;">
                                <input
                                    type="text"
                                    style="padding: 0.3rem 0.5rem; font-size: 0.76rem;"
                                    placeholder=move || if is_en() { "e.g. jabber_code..." } else { "z.B. jabber_code..." }
                                    prop:value=move || new_project_db_input.get()
                                    on:input=move |ev| set_new_project_db_input.set(event_target_value(&ev))
                                    on:keydown=move |ev| {
                                        if ev.key() == "Enter" {
                                            let name = new_project_db_input.get_untracked();
                                            if !name.trim().is_empty() {
                                                switch_or_create_project_db(name);
                                                set_new_project_db_input.set(String::new());
                                                set_show_sidebar_new_db.set(false);
                                            }
                                        }
                                    }
                                />
                                <button
                                    type="button"
                                    class="btn"
                                    style="padding: 0.3rem 0.55rem; font-size: 0.75rem;"
                                    on:click=move |_| {
                                        let name = new_project_db_input.get_untracked();
                                        if !name.trim().is_empty() {
                                            switch_or_create_project_db(name);
                                            set_new_project_db_input.set(String::new());
                                            set_show_sidebar_new_db.set(false);
                                        }
                                    }
                                >
                                    "✓"
                                </button>
                            </div>
                        })}
                    </div>

                    <nav class="nav-menu">
                        <button
                            class="nav-btn nav-btn-primary"
                            on:click=move |_| create_new_chat()
                        >
                            <span>"💬"</span>
                            <span>{move || if is_en() { "+ New Chat" } else { "+ Neuer Chat" }}</span>
                        </button>
                    </nav>

                    // --- Multi-Chat Sitzungs-Panel (gefiltert nach aktiver Projekt-DB) ---
                    <div class="chat-sessions-section">
                        <div class="chat-sessions-header">
                            <span>
                                {move || {
                                    let cur_db = active_project_db.get();
                                    let count = chat_sessions
                                        .get()
                                        .iter()
                                        .filter(|s| session_belongs_to_project(s, &cur_db))
                                        .count();
                                    if is_en() {
                                        format!("Project Chats ({})", count)
                                    } else {
                                        format!("Projekt-Chats ({})", count)
                                    }
                                }}
                            </span>
                            <button
                                style="background:transparent; border:none; color:#94a3b8; font-size:0.72rem; cursor:pointer;"
                                title=move || if is_en() { "Clear current chat history" } else { "Aktuellen Chat-Verlauf zurücksetzen" }
                                on:click=move |_| {
                                    if is_loading.get() {
                                        return;
                                    }
                                    let clear_id = next_msg_id.get();
                                    next_msg_id.set(clear_id + 1);
                                    set_chat_attachments.set(Vec::new());
                                    set_history_cursor.set(None);
                                    set_attachment_error.set(None);
                                    let cleared_txt = if is_en() {
                                        "Chat history cleared. Ask your knowledge base a new question!".to_string()
                                    } else {
                                        "Chat-Verlauf geleert. Stelle eine neue Frage an deine Wissensdatenbank!".to_string()
                                    };
                                    let new_title = if is_en() { "New Chat" } else { "Neuer Chat" };
                                    set_messages.set(vec![UiChatMessage {
                                        id: clear_id,
                                        role: "assistant".to_string(),
                                        content: create_rw_signal(cleared_txt),
                                        sources: create_rw_signal(vec![]),
                                        metrics_info: create_rw_signal(None),
                                    }]);
                                    set_chat_sessions.update(|sessions| {
                                        let cur = active_session_id.get_untracked();
                                        if let Some(s) = sessions.iter_mut().find(|x| x.id == cur) {
                                            s.title = new_title.to_string();
                                        }
                                    });
                                    persist_current_chat(None);
                                }
                            >
                                {move || if is_en() { "🧹 Clear" } else { "🧹 Leeren" }}
                            </button>
                        </div>

                        <div class="chat-sessions-list">
                            <For
                                each=move || {
                                    let cur_db = active_project_db.get();
                                    chat_sessions
                                        .get()
                                        .into_iter()
                                        .filter(|s| session_belongs_to_project(s, &cur_db))
                                        .collect::<Vec<_>>()
                                }
                                key=|sess| {
                                    format!(
                                        "{}_{}_{}_{}",
                                        sess.id,
                                        sess.title,
                                        sess.messages.len(),
                                        sess.project_db.as_deref().unwrap_or("knowledge_base")
                                    )
                                }
                                children=move |sess| {
                                    let sid = sess.id;
                                    let title = sess.title.clone();
                                    let title_for_rename = title.clone();
                                    let user_turns = sess.messages.iter().filter(|m| m.role == "user").count();
                                    view! {
                                        <div
                                            class=move || if active_session_id.get() == sid {
                                                "chat-session-item active"
                                            } else {
                                                "chat-session-item"
                                            }
                                            on:click=move |_| {
                                                if editing_chat_id.get() != Some(sid) {
                                                    set_open_chat_menu_id.set(None);
                                                    switch_to_chat(sid);
                                                }
                                            }
                                        >
                                            <span style="font-size: 0.85rem;">"💭"</span>
                                            {move || if editing_chat_id.get() == Some(sid) {
                                                view! {
                                                    <div
                                                        class="chat-session-rename-row"
                                                        on:click=move |ev| ev.stop_propagation()
                                                    >
                                                        <input
                                                            type="text"
                                                            class="chat-session-rename-input"
                                                            prop:value=move || editing_chat_title.get()
                                                            on:input=move |ev| set_editing_chat_title.set(event_target_value(&ev))
                                                            on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                                                if ev.key() == "Enter" {
                                                                    ev.prevent_default();
                                                                    save_renamed_chat(sid);
                                                                } else if ev.key() == "Escape" {
                                                                    ev.prevent_default();
                                                                    set_editing_chat_id.set(None);
                                                                }
                                                            }
                                                        />
                                                        <button
                                                            class="chat-session-rename-btn"
                                                            title=move || if is_en() { "Save title" } else { "Titel speichern" }
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                save_renamed_chat(sid);
                                                            }
                                                        >
                                                            "✓"
                                                        </button>
                                                        <button
                                                            class="chat-session-rename-btn"
                                                            title=move || if is_en() { "Cancel" } else { "Abbrechen" }
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                set_editing_chat_id.set(None);
                                                            }
                                                        >
                                                            "✕"
                                                        </button>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                let t = title.clone();
                                                view! {
                                                    <span class="chat-session-title" title=t.clone()>
                                                        {t}
                                                    </span>
                                                }.into_view()
                                            }}
                                            {move || if editing_chat_id.get() != Some(sid) && user_turns > 0 {
                                                view! {
                                                    <span style="font-size: 0.68rem; color: #94a3b8; background: rgba(15,23,42,0.7); padding: 0.1rem 0.35rem; border-radius: 0.3rem;">
                                                        {user_turns}
                                                    </span>
                                                }.into_view()
                                            } else {
                                                view! { <span></span> }.into_view()
                                            }}
                                            {move || if editing_chat_id.get() != Some(sid) {
                                                let t_rename = title_for_rename.clone();
                                                view! {
                                                    <div style="display:flex; align-items:center; gap:0.1rem;">
                                                        <button
                                                            class="chat-session-menu-btn"
                                                            title=move || if is_en() { "More options (Rename / Export)" } else { "Optionen (Umbenennen / Export)" }
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                if open_chat_menu_id.get() == Some(sid) {
                                                                    set_open_chat_menu_id.set(None);
                                                                } else {
                                                                    set_open_chat_menu_id.set(Some(sid));
                                                                }
                                                            }
                                                        >
                                                            "⋮"
                                                        </button>
                                                        <button
                                                            class="chat-session-delete"
                                                            title=move || if is_en() { "Delete this chat" } else { "Diesen Chat löschen" }
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                set_open_chat_menu_id.set(None);
                                                                delete_chat_session(sid);
                                                            }
                                                        >
                                                            "✕"
                                                        </button>
                                                    </div>
                                                    {move || (open_chat_menu_id.get() == Some(sid)).then(|| {
                                                        let t_r = t_rename.clone();
                                                        view! {
                                                            <div
                                                                class="chat-session-dropdown"
                                                                on:click=move |ev| ev.stop_propagation()
                                                            >
                                                                <button
                                                                    class="chat-session-dropdown-item"
                                                                    on:click=move |ev| {
                                                                        ev.stop_propagation();
                                                                        set_open_chat_menu_id.set(None);
                                                                        set_editing_chat_title.set(t_r.clone());
                                                                        set_editing_chat_id.set(Some(sid));
                                                                    }
                                                                >
                                                                    <span>"✏️"</span>
                                                                    <span>{move || if is_en() { "Rename" } else { "Umbenennen" }}</span>
                                                                </button>
                                                                <button
                                                                    class="chat-session-dropdown-item"
                                                                    on:click=move |ev| {
                                                                        ev.stop_propagation();
                                                                        set_open_chat_menu_id.set(None);
                                                                        export_session_by_id(sid);
                                                                    }
                                                                >
                                                                    <span>"📥"</span>
                                                                    <span>{move || if is_en() { "Export (.md)" } else { "Export (.md)" }}</span>
                                                                </button>
                                                            </div>
                                                        }
                                                    })}
                                                }.into_view()
                                            } else {
                                                view! { <span></span> }.into_view()
                                            }}
                                        </div>
                                    }
                                }
                            />
                        </div>
                    </div>
                </div>

                // --- Untere Icon-Leiste in der Sidebar (Upload, Dokumente, Settings) ---
                <div class="sidebar-bottom">
                    <div class="sidebar-icon-bar">
                        <button
                            class="sidebar-icon-btn"
                            title=move || if is_en() { "Ingest knowledge / Upload files" } else { "Wissen einspeisen / Dateien hochladen" }
                            on:click=move |_| {
                                set_ingest_status.set(String::new());
                                set_modal_tab.set("files".to_string());
                                refresh_documents();
                                set_show_ingest_modal.set(true);
                            }
                        >
                            <span>"➕"</span>
                            <span class="icon-caption">"Upload"</span>
                        </button>

                        <button
                            class="sidebar-icon-btn"
                            title=move || if is_en() { "Document Management (SurrealDB)" } else { "Dokumenten-Verwaltung (SurrealDB)" }
                            on:click=move |_| {
                                set_ingest_status.set(String::new());
                                set_modal_tab.set("docs".to_string());
                                refresh_documents();
                                refresh_crm_employees();
                                set_show_ingest_modal.set(true);
                            }
                        >
                            <span>"🗂️"</span>
                            <span class="icon-caption">{move || if is_en() { "Docs" } else { "Dokumente" }}</span>
                        </button>

                        {move || (!crm_employees.get().is_empty() || !fleet_vehicles.get().is_empty() || webstore_counts.get().0 > 0).then(|| view! {
                            <button
                                class="sidebar-icon-btn"
                                title=move || if is_en() { "Active Showcase SQL Database" } else { "Aktive Showcase SQL-Datenbank" }
                                on:click=move |_| {
                                    set_ingest_status.set(String::new());
                                    refresh_crm_employees();
                                    refresh_fleet_data();
                                    refresh_webstore_data();
                                    if !fleet_vehicles.get_untracked().is_empty() {
                                        set_modal_tab.set("fleet".to_string());
                                    } else if !crm_employees.get_untracked().is_empty() {
                                        set_modal_tab.set("crm".to_string());
                                    } else if webstore_counts.get_untracked().0 > 0 {
                                        set_modal_tab.set("webstore".to_string());
                                    } else {
                                        set_modal_tab.set("docs".to_string());
                                    }
                                    set_show_ingest_modal.set(true);
                                }
                            >
                                <span>"🗄️"</span>
                                <span class="icon-caption">"SQL/DB"</span>
                            </button>
                        })}

                        <button
                            class="sidebar-icon-btn"
                            title=move || if is_en() { "Settings (RAG, LLM, RunPod GPU & System)" } else { "Einstellungen (RAG, LLM, RunPod GPU & System)" }
                            on:click=move |_| {
                                set_llm_config_status.set(String::new());
                                refresh_runpod_status();
                                set_show_llm_modal.set(true);
                            }
                        >
                            <span>"⚙️"</span>
                            <span class="icon-caption">"Settings"</span>
                        </button>
                    </div>

                    <div style="font-size: 0.7rem; color: #64748b; text-align: center;">
                        "SurrealKV (HNSW + BM25 + RRF) • MIT/Apache 2.0"
                    </div>
                </div>
            </aside>

            // --- Hauptbereich: Chat & RAG-Quellen-Inspektor ---
            <main class="chat-main">
                <header class="chat-header" style="padding: 0.7rem 1.5rem; display: flex; justify-content: space-between; align-items: center; gap: 1rem;">
                    <div style="font-size: 0.9rem; font-weight: 600; color: #e2e8f0; display: flex; align-items: center; gap: 0.5rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                        <span>"💭"</span>
                        <span>
                            {move || {
                                let cur = active_session_id.get();
                                chat_sessions
                                    .get()
                                    .into_iter()
                                    .find(|s| s.id == cur)
                                    .map(|s| s.title)
                                    .unwrap_or_else(|| if is_en() { "New Chat".to_string() } else { "Neuer Chat".to_string() })
                            }}
                        </span>
                    </div>

                    <div style="display: flex; align-items: center; gap: 0.55rem; flex-shrink: 0; flex-wrap: wrap;">
                        // --- Aktiver Showcase Indikator im Header (nur sichtbar, wenn ein Showcase geladen ist) ---
                        {move || {
                            let v_cnt = fleet_vehicles.get().len();
                            let t_cnt = fleet_tours.get().len();
                            let e_cnt = crm_employees.get().len();
                            let (wp_cnt, ws_cnt, _) = webstore_counts.get();
                            let d_cnt = stored_docs.get().len();
                            let is_showcase_active = v_cnt > 0 || e_cnt > 0 || wp_cnt > 0;

                            is_showcase_active.then(|| {
                                let btn_style = if v_cnt > 0 {
                                    "padding: 0.35rem 0.78rem; font-size: 0.76rem; font-weight: 700; display: flex; align-items: center; gap: 0.45rem; border-radius: 999px; background: rgba(16, 185, 129, 0.16); border: 1px solid #10b981; color: #6ee7b7; cursor: pointer;"
                                } else if e_cnt > 0 {
                                    "padding: 0.35rem 0.78rem; font-size: 0.76rem; font-weight: 700; display: flex; align-items: center; gap: 0.45rem; border-radius: 999px; background: rgba(99, 102, 241, 0.18); border: 1px solid #818cf8; color: #c7d2fe; cursor: pointer;"
                                } else {
                                    "padding: 0.35rem 0.78rem; font-size: 0.76rem; font-weight: 700; display: flex; align-items: center; gap: 0.45rem; border-radius: 999px; background: rgba(245, 158, 11, 0.18); border: 1px solid #f59e0b; color: #fde68a; cursor: pointer;"
                                };

                                let label = if v_cnt > 0 {
                                    if is_en() {
                                        format!("🚛 Showcase 02: Spedition ({} Trucks · {} Tours · {} PDFs)", v_cnt, t_cnt, d_cnt)
                                    } else {
                                        format!("🚛 Showcase 02: Spedition ({} LKWs · {} Touren · {} PDFs)", v_cnt, t_cnt, d_cnt)
                                    }
                                } else if e_cnt > 0 {
                                    if is_en() {
                                        format!("🏢 Showcase 01: HR & Consulting ({} Employees · {} PDFs)", e_cnt, d_cnt)
                                    } else {
                                        format!("🏢 Showcase 01: HR & Consulting ({} Mitarbeiter · {} PDFs)", e_cnt, d_cnt)
                                    }
                                } else if is_en() {
                                    format!("🛒 Showcase 03: Webstore Multi-DB ({} Products · {} Sales · {} PDFs)", wp_cnt, ws_cnt, d_cnt)
                                } else {
                                    format!("🛒 Showcase 03: Webstore Multi-DB ({} Artikel · {} Verkäufe · {} PDFs)", wp_cnt, ws_cnt, d_cnt)
                                };

                                view! {
                                    <button
                                        type="button"
                                        style=btn_style
                                        title=move || if is_en() {
                                            "Active Showcase in SurrealDB (click to open database explorer)"
                                        } else {
                                            "Aktives Showcase in SurrealDB (Klicken, um die Datenbank-Ansicht zu öffnen)"
                                        }
                                        on:click=move |_| {
                                            set_ingest_status.set(String::new());
                                            refresh_documents();
                                            refresh_crm_employees();
                                            refresh_fleet_data();
                                            refresh_webstore_data();
                                            if !fleet_vehicles.get_untracked().is_empty() {
                                                set_modal_tab.set("fleet".to_string());
                                            } else if !crm_employees.get_untracked().is_empty() {
                                                set_modal_tab.set("crm".to_string());
                                            } else if webstore_counts.get_untracked().0 > 0 {
                                                set_modal_tab.set("webstore".to_string());
                                            } else {
                                                set_modal_tab.set("docs".to_string());
                                            }
                                            set_is_modal_fullscreen.set(true);
                                            set_show_ingest_modal.set(true);
                                        }
                                    >
                                        {label}
                                    </button>
                                }
                            })
                        }}

                        {move || {
                            let phase = rp_phase.get();
                            (phase != "idle").then(|| {
                                let is_running = phase == "running";
                                let badge_style = if is_running {
                                    "padding: 0.35rem 0.78rem; font-size: 0.76rem; font-weight: 700; display: flex; align-items: center; gap: 0.4rem; border-radius: 999px; background: rgba(16, 185, 129, 0.16); border: 1px solid #10b981; color: #6ee7b7; cursor: pointer;"
                                } else if phase == "error" {
                                    "padding: 0.35rem 0.78rem; font-size: 0.76rem; font-weight: 700; display: flex; align-items: center; gap: 0.4rem; border-radius: 999px; background: rgba(239, 68, 68, 0.16); border: 1px solid #ef4444; color: #fca5a5; cursor: pointer;"
                                } else {
                                    "padding: 0.35rem 0.78rem; font-size: 0.76rem; font-weight: 700; display: flex; align-items: center; gap: 0.4rem; border-radius: 999px; background: rgba(245, 158, 11, 0.18); border: 1px solid #f59e0b; color: #fde68a; cursor: pointer;"
                                };
                                let badge_label = if is_running {
                                    format!("🚀 RunPod: {}", rp_gpu_type.get())
                                } else if phase == "error" {
                                    "🚀 RunPod: Fehler".to_string()
                                } else {
                                    "🚀 RunPod: Startet...".to_string()
                                };
                                view! {
                                    <button
                                        type="button"
                                        style=badge_style
                                        title=move || rp_message.get()
                                        on:click=move |_| {
                                            set_settings_tab.set("runpod".to_string());
                                            refresh_runpod_status();
                                            set_show_llm_modal.set(true);
                                        }
                                    >
                                        {badge_label}
                                    </button>
                                }
                            })
                        }}

                        <button
                            class="btn btn-secondary"
                            style="padding: 0.35rem 0.75rem; font-size: 0.76rem; display: flex; align-items: center; gap: 0.5rem; border-radius: 999px;"
                            title=move || if is_en() { "Click to configure RAG, Reranker, Context Window & LLM settings" } else { "Klicken, um RAG-, Reranker-, Kontextfenster- und LLM-Einstellungen anzupassen" }
                            on:click=move |_| {
                                set_llm_config_status.set(String::new());
                                refresh_runpod_status();
                                set_show_llm_modal.set(true);
                            }
                        >
                            <span>
                                {move || match (use_rag.get(), is_en()) {
                                    (true, true) => "🟢 RAG active",
                                    (true, false) => "🟢 RAG aktiv",
                                    (false, true) => "⚪ RAG off",
                                    (false, false) => "⚪ RAG aus",
                                }}
                            </span>
                            <span style="color:#475569;">"|"</span>
                            <span style="background: rgba(79, 70, 229, 0.28); border: 1px solid #818cf8; color: #e0e7ff; font-weight: 700; padding: 0.14rem 0.55rem; border-radius: 999px; box-shadow: 0 0 8px rgba(99, 102, 241, 0.35);">
                                {move || format!("🤖 {}", llm_model_sig.get())}
                            </span>
                            <span style="color:#475569;">"|"</span>
                            <span style="color:#94a3b8;">
                                {move || {
                                    let r = match reranker_mode.get().as_str() {
                                        "high" => "🎯 High",
                                        "none" => "🚀 RRF",
                                        _ => "⚡ Fast",
                                    };
                                    let ctx_str = match num_ctx_sig.get() {
                                        4096 => "4k",
                                        8192 => "8k",
                                        16384 => "16k",
                                        32768 => "32k",
                                        65536 => "64k",
                                        _ => "16k",
                                    };
                                    let think_ico = if think_mode_sig.get() { "🧠" } else { "⚡" };
                                    format!("{} · Top-{} · {} · T={:.1} · {}", r, top_n.get(), ctx_str, temperature_sig.get(), think_ico)
                                }}
                            </span>
                            <span>"⚙️"</span>
                        </button>
                    </div>
                </header>

                <div class="messages-container">
                    <For
                        each=move || messages.get()
                        key=|msg| msg.id
                        children=move |msg| {
                            let is_user = msg.role == "user";
                            let bubble_class = if is_user { "msg-bubble msg-user" } else { "msg-bubble msg-assistant" };
                            let content_sig = msg.content;
                            let sources_sig = msg.sources;
                            let metrics_sig = msg.metrics_info;
                            let (user_think_open, set_user_think_open) = create_signal::<Option<bool>>(None);
                            view! {
                                <div class=bubble_class>
                                    {move || {
                                        let raw = content_sig.get();
                                        if is_user {
                                            view! { <div>{raw}</div> }.into_view()
                                        } else {
                                            let (think_opt, answer_md) = parse_think_and_answer(&raw);
                                            let answer_html = render_markdown_to_html(&answer_md);
                                            view! {
                                                <div>
                                                    {think_opt.map(|(think_md, is_streaming_think)| {
                                                        let word_count = think_md.split_whitespace().count();
                                                        let is_open = user_think_open.get().unwrap_or(is_streaming_think);
                                                        let think_html = render_markdown_to_html(&think_md);
                                                        view! {
                                                            <div class="think-box">
                                                                <div
                                                                    class="think-header"
                                                                    on:click=move |_| {
                                                                        let cur = user_think_open.get().unwrap_or(is_streaming_think);
                                                                        set_user_think_open.set(Some(!cur));
                                                                    }
                                                                >
                                                                    <span style="display:flex; align-items:center; gap:0.45rem;">
                                                                        <span>{if is_open { "▼" } else { "▶" }}</span>
                                                                        <span class=if is_streaming_think { "thinking-pulse" } else { "" }>"🧠"</span>
                                                                        <span>
                                                                            {if is_streaming_think {
                                                                                if is_en() {
                                                                                    format!("Thinking… ({} words)", word_count)
                                                                                } else {
                                                                                    format!("Denkt nach… ({} Wörter)", word_count)
                                                                                }
                                                                            } else if is_en() {
                                                                                format!("Thought Process ({} words)", word_count)
                                                                            } else {
                                                                                format!("Gedankengang ({} Wörter)", word_count)
                                                                            }}
                                                                        </span>
                                                                    </span>
                                                                    <span style="font-size:0.72rem; color:#818cf8; font-weight:500;">
                                                                        {if is_open {
                                                                            if is_en() { "Click to collapse" } else { "Einklappen" }
                                                                        } else if is_en() {
                                                                            "Click to expand"
                                                                        } else {
                                                                            "Ausklappen"
                                                                        }}
                                                                    </span>
                                                                </div>
                                                                {is_open.then(|| view! {
                                                                    <div class="think-body">
                                                                        <div class="md-content" inner_html=think_html></div>
                                                                    </div>
                                                                })}
                                                            </div>
                                                        }
                                                    })}

                                                    {(!answer_md.is_empty()).then(|| view! {
                                                        <div class="md-content" inner_html=answer_html></div>
                                                    })}
                                                </div>
                                            }.into_view()
                                        }
                                    }}

                                    {move || metrics_sig.get().map(|m_txt| view! {
                                        <div style="margin-top: 0.65rem; padding-top: 0.45rem; border-top: 1px dashed #334155; font-size: 0.74rem; color: #38bdf8;">
                                            {m_txt}
                                        </div>
                                    })}

                                    {move || {
                                        let srcs = sources_sig.get();
                                        let srcs_count = srcs.len();
                                        let total_rels: usize = srcs.iter().map(|s| s.relations.len()).sum();
                                        (!srcs.is_empty()).then(|| view! {
                                            <div class="sources-box">
                                                <details>
                                                    <summary style="cursor:pointer; color:#a5b4fc; font-weight:600;">
                                                        {move || {
                                                            let rel_text = if total_rels > 0 {
                                                                format!(" + 🕸️ {} Relations", total_rels)
                                                            } else {
                                                                String::new()
                                                            };
                                                            if is_en() {
                                                                format!("🔍 {} SurrealDB sources used (HNSW + BM25 + Reranker{})", srcs_count, rel_text)
                                                            } else {
                                                                format!("🔍 {} verwendete SurrealDB-Quellen (HNSW + BM25 + Reranker{})", srcs_count, rel_text)
                                                            }
                                                        }}
                                                    </summary>
                                                    {srcs.clone().into_iter().enumerate().map(|(i, src)| {
                                                        let score = src.rerank_score.unwrap_or(src.rrf_score);
                                                        let mut graph_tags: Vec<String> = src
                                                            .entities
                                                            .iter()
                                                            .map(|e| format!("{} ({})", e.name, e.entity_type))
                                                            .collect();
                                                        for rel in &src.related_entities {
                                                            let tag = format!("➔ {} ({})", rel.name, rel.entity_type);
                                                            if !graph_tags.contains(&tag) {
                                                                graph_tags.push(tag);
                                                            }
                                                        }
                                                        let relations_list = src.relations.clone();
                                                        let src_doc_id = src.doc_id.clone();
                                                        let src_doc_title = src.doc_title.clone();
                                                        let is_pdf_source = !src.doc_id.is_empty()
                                                            && !src.doc_id.starts_with("crm_")
                                                            && !src.doc_id.starts_with("fleet_")
                                                            && !src.doc_id.starts_with("webstore_")
                                                            && !src.doc_id.starts_with("internal_")
                                                            && !src.doc_id.starts_with("external_")
                                                            && !src.doc_id.starts_with("ephemeral_")
                                                            && !src.doc_id.starts_with("surrealdb_");
                                                        let src_html = render_markdown_to_html(&src.content);
                                                        view! {
                                                            <details class="source-item">
                                                                <summary>
                                                                    <div style="display:flex; align-items:center; flex-wrap:wrap; gap:0.35rem;">
                                                                        <span class="source-chevron">"▶"</span>
                                                                        <span class="badge">
                                                                            {move || if is_en() {
                                                                                format!("Source #{}", i + 1)
                                                                            } else {
                                                                                format!("Quelle #{}", i + 1)
                                                                            }}
                                                                        </span>
                                                                        <strong>{src.doc_title.clone()}</strong>
                                                                        <span style="color:#94a3b8; margin-left:4px;">
                                                                            {format!("(Chunk #{}, Score: {:.4})", src.chunk_index, score)}
                                                                        </span>
                                                                    </div>
                                                                    <div style="display:flex; align-items:center; gap:0.4rem; flex-wrap:wrap;">
                                                                        <span style="font-size:0.71rem; color:#818cf8; font-weight:500;">
                                                                            {move || if is_en() { "Click to toggle" } else { "Ein-/Ausklappen" }}
                                                                        </span>
                                                                        {is_pdf_source.then(|| {
                                                                            let did = src_doc_id.clone();
                                                                            let dtitle = src_doc_title.clone();
                                                                            let dtitle_for_lbl = src_doc_title.clone();
                                                                            let dtitle_for_tip = src_doc_title.clone();
                                                                            view! {
                                                                                <button
                                                                                    type="button"
                                                                                    class="btn btn-secondary"
                                                                                    style="padding:0.2rem 0.6rem; font-size:0.73rem; background:rgba(99,102,241,0.18); border-color:#6366f1; color:#c7d2fe;"
                                                                                    title=move || doc_format_labels(&dtitle_for_tip, is_en()).2
                                                                                    on:click=move |ev| {
                                                                                        ev.stop_propagation();
                                                                                        ev.prevent_default();
                                                                                        set_viewing_pdf_doc.set(Some((did.clone(), dtitle.clone())));
                                                                                    }
                                                                                >
                                                                                    {move || doc_format_labels(&dtitle_for_lbl, is_en()).1}
                                                                                </button>
                                                                            }
                                                                        })}
                                                                        {(src_doc_id == "fleet_logistics_tables").then(|| {
                                                                            let all_known_tours = fleet_tours.get();
                                                                            let matched_tours_in_chunk: Vec<FleetTourItem> = all_known_tours
                                                                                .into_iter()
                                                                                .filter(|t| src.content.contains(&t.tour_id))
                                                                                .take(6)
                                                                                .collect();
                                                                            view! {
                                                                                <div style="display:flex; gap:0.35rem; flex-wrap:wrap;">
                                                                                    {matched_tours_in_chunk.into_iter().map(|t| {
                                                                                        let tid = t.tour_id.clone();
                                                                                        let tlabel = format!("{} ({} ➔ {}, {})", t.tour_id, t.origin_city, t.destination_city, t.truck_id);
                                                                                        let gmap = t.google_maps_url.clone();
                                                                                        view! {
                                                                                            <button
                                                                                                type="button"
                                                                                                class="btn btn-secondary"
                                                                                                style="padding:0.2rem 0.55rem; font-size:0.71rem; background:rgba(16,185,129,0.18); border-color:#10b981; color:#6ee7b7;"
                                                                                                title="Tour-Route auf OpenStreetMap im Chat-UI anzeigen"
                                                                                                on:click=move |ev| {
                                                                                                    ev.stop_propagation();
                                                                                                    ev.prevent_default();
                                                                                                    set_viewing_tour_map.set(Some((tid.clone(), tlabel.clone(), gmap.clone())));
                                                                                                }
                                                                                            >
                                                                                                {format!("🗺️ {} Karte", t.tour_id)}
                                                                                            </button>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                </div>
                                                                            }
                                                                        })}
                                                                    </div>
                                                                </summary>
                                                                <div class="source-body">
                                                                    <div class="md-content" inner_html=src_html></div>
                                                                    {(!graph_tags.is_empty()).then(|| view! {
                                                                        <div style="margin-top:6px; padding-top:5px; border-top:1px dotted #334155; font-size:0.74rem; color:#c084fc;">
                                                                            <strong>
                                                                                {move || if is_en() { "🏷️ Entities: " } else { "🏷️ Entitäten: " }}
                                                                            </strong>
                                                                            {graph_tags.join(" · ")}
                                                                        </div>
                                                                    })}
                                                                    {(!relations_list.is_empty()).then(|| view! {
                                                                        <div style="margin-top:8px; padding:7px 10px; background:rgba(99, 102, 241, 0.08); border:1px solid rgba(99, 102, 241, 0.3); border-radius:6px; font-size:0.75rem;">
                                                                            <div style="color:#a5b4fc; font-weight:700; margin-bottom:5px; display:flex; align-items:center; gap:5px;">
                                                                                <span>"🕸️"</span>
                                                                                <span>{move || if is_en() { "Knowledge Graph Relationships:" } else { "Wissensgraph-Beziehungen (GraphRAG):" }}</span>
                                                                            </div>
                                                                            <div style="display:flex; flex-wrap:wrap; gap:5px;">
                                                                                {relations_list.into_iter().map(|rel| {
                                                                                    view! {
                                                                                        <span style="display:inline-flex; align-items:center; gap:4px; background:#1e1b4b; border:1px solid #4338ca; color:#e0e7ff; padding:2px 8px; border-radius:4px; font-size:0.72rem;">
                                                                                            <strong style="color:#c7d2fe;">{rel.from}</strong>
                                                                                            <span style="color:#818cf8; font-size:0.68rem; font-weight:600;">"➔ [" {rel.relation} "] ➔"</span>
                                                                                            <strong style="color:#38bdf8;">{rel.to}</strong>
                                                                                        </span>
                                                                                    }
                                                                                }).collect_view()}
                                                                            </div>
                                                                        </div>
                                                                    })}
                                                                </div>
                                                            </details>
                                                        }
                                                    }).collect_view()}
                                                </details>
                                            </div>
                                        })
                                    }}
                                </div>
                            }
                        }
                    />
                </div>

                // --- Status-Leiste für temporären Datei-Anhang (📎 ohne DB-Speicherung) ---
                {move || {
                    if is_extracting_attachment.get() {
                        view! {
                            <div style="margin: 0 1.25rem 0.5rem 1.25rem; padding: 0.6rem 0.9rem; background: #1e293b; border: 1px solid #38bdf8; border-radius: 8px; color: #38bdf8; font-size: 0.83rem; display: flex; align-items: center; gap: 0.5rem;">
                                <span>{move || if is_en() { "⏳ Reading file in RAM (incl. OCR for scans, no database storage)..." } else { "⏳ Lese Datei im Arbeitsspeicher (inkl. OCR bei Scans, keine DB-Speicherung)..." }}</span>
                            </div>
                        }.into_view()
                    } else if let Some(err) = attachment_error.get() {
                        view! {
                            <div style="margin: 0 1.25rem 0.5rem 1.25rem; padding: 0.6rem 0.9rem; background: #450a0a; border: 1px solid #ef4444; border-radius: 8px; color: #fca5a5; font-size: 0.83rem; display: flex; justify-content: space-between; align-items: center;">
                                <span>{format!("❌ {}", err)}</span>
                                <button
                                    class="btn btn-secondary"
                                    style="padding: 0.2rem 0.5rem; font-size: 0.75rem;"
                                    on:click=move |_| set_attachment_error.set(None)
                                >
                                    "✕"
                                </button>
                            </div>
                        }.into_view()
                    } else if !chat_attachments.get().is_empty() {
                        let current_list = chat_attachments.get();
                        let total_chars: usize = current_list.iter().map(|d| d.char_count).sum();
                        let total_chunks: usize = current_list.iter().map(|d| d.chunk_count).sum();
                        let file_count = current_list.len();
                        view! {
                            <div style="margin: 0 1.25rem 0.5rem 1.25rem; padding: 0.6rem 0.9rem; background: #0f172a; border: 1px solid #6366f1; border-radius: 8px; display: flex; flex-direction: column; gap: 0.55rem; font-size: 0.82rem;">
                                <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.6rem;">
                                    <div style="display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;">
                                        <span class="badge" style="background: #4f46e5; color: #fff;">
                                            {move || if is_en() {
                                                format!("📎 {} In-Memory Attachment(s)", file_count)
                                            } else {
                                                format!("📎 {} Datei(en) temporär im RAM", file_count)
                                            }}
                                        </span>
                                        <span style="color: #94a3b8;">
                                            {move || if is_en() {
                                                format!("({} chars total · {} chunks · not in DB)", total_chars, total_chunks)
                                            } else {
                                                format!("({} Zeichen gesamt · {} Chunks · nicht in DB)", total_chars, total_chunks)
                                            }}
                                        </span>
                                    </div>
                                    <div style="display: flex; align-items: center; gap: 0.55rem; flex-wrap: wrap;">
                                        <button
                                            class="btn btn-secondary"
                                            style="padding: 0.25rem 0.6rem; font-size: 0.76rem; border-color: #6366f1; color: #c7d2fe;"
                                            title=move || if is_en() { "Add more files from the same or another folder" } else { "Weitere Dateien (auch aus anderen Ordnern) hinzufügen" }
                                            on:click=move |_| {
                                                if let Some(input_el) = chat_attach_input_ref.get() {
                                                    input_el.click();
                                                }
                                            }
                                        >
                                            {move || if is_en() { "+ 📎 Add file(s)" } else { "+ 📎 Weitere Datei(en)" }}
                                        </button>
                                        <select
                                            style="width: 245px; padding: 0.28rem 0.5rem; font-size: 0.78rem;"
                                            prop:value=move || if attachment_only.get() { "only" } else { "combine" }
                                            on:change=move |ev| {
                                                set_attachment_only.set(event_target_value(&ev) == "only");
                                            }
                                        >
                                            <option value="only" selected>{move || if is_en() { "📄 Query attached file(s) only" } else { "📄 Nur angehängte Datei(en) befragen" }}</option>
                                            <option value="combine">{move || if is_en() { "🔗 Combine attachment(s) + SurrealDB" } else { "🔗 Anhänge + SurrealDB kombinieren" }}</option>
                                        </select>
                                        <button
                                            class="btn btn-secondary"
                                            style="padding: 0.25rem 0.6rem; font-size: 0.76rem; background: #7f1d1d; color: #fecaca;"
                                            title=move || if is_en() { "Remove all attachments from chat" } else { "Alle Anhänge aus dem Chat entfernen" }
                                            on:click=move |_| set_chat_attachments.set(Vec::new())
                                        >
                                            {move || if is_en() { "✕ Remove all" } else { "✕ Alle entfernen" }}
                                        </button>
                                    </div>
                                </div>
                                <div style="display: flex; flex-wrap: wrap; gap: 0.45rem;">
                                    {current_list.into_iter().map(|att| {
                                        let fname_for_remove = att.filename.clone();
                                        view! {
                                            <div style="display: inline-flex; align-items: center; gap: 0.4rem; background: #1e293b; border: 1px solid #334155; border-radius: 6px; padding: 0.22rem 0.55rem; font-size: 0.78rem;">
                                                <span style="color: #f8fafc; font-weight: 600;">"📄 " {att.filename}</span>
                                                <span style="color: #94a3b8; font-size: 0.72rem;">
                                                    {format!("({} Chunks)", att.chunk_count)}
                                                </span>
                                                <button
                                                    style="background: transparent; border: none; color: #fca5a5; cursor: pointer; font-size: 0.78rem; padding: 0 0.15rem;"
                                                    title=move || if is_en() { "Remove this file" } else { "Diese Datei entfernen" }
                                                    on:click=move |_| {
                                                        let target_name = fname_for_remove.clone();
                                                        set_chat_attachments.update(|list| {
                                                            list.retain(|item| item.filename != target_name);
                                                        });
                                                    }
                                                >
                                                    "✕"
                                                </button>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! { <div style="display:none;"></div> }.into_view()
                    }
                }}

                <div class="chat-input-bar">
                    <input
                        type="file"
                        multiple=true
                        accept=".pdf,.xml,.md,.txt,.html,.htm,.csv,.json"
                        style="display: none;"
                        node_ref=chat_attach_input_ref
                        on:change=on_chat_attachment_selected
                    />
                    <button
                        class="btn btn-secondary"
                        style="padding: 0.65rem 0.85rem; font-size: 1.05rem;"
                        title=move || if is_en() {
                            "Attach PDF or text files for this chat (click multiple times to add files from different folders)"
                        } else {
                            "PDF- oder Textdateien für diesen Chat anhängen (mehrfach klickbar für Dateien aus verschiedenen Ordnern)"
                        }
                        disabled=move || is_loading.get() || is_extracting_attachment.get()
                        on:click=move |_| {
                            if let Some(input_el) = chat_attach_input_ref.get() {
                                input_el.click();
                            }
                        }
                    >
                        "📎"
                    </button>
                    <textarea
                        class="chat-input-textarea"
                        rows="1"
                        placeholder=move || {
                            let list = chat_attachments.get();
                            if !list.is_empty() {
                                let names = list.iter().map(|a| a.filename.as_str()).collect::<Vec<_>>().join(", ");
                                if is_en() {
                                    format!("Ask a question about '{}' (Shift+Enter = new line, ↑/↓ = history)...", names)
                                } else {
                                    format!("Frage zu '{}' stellen (Shift+Enter = Zeilenumbruch, ↑/↓ = Verlauf)...", names)
                                }
                            } else if is_en() {
                                "Ask your knowledge base & LLM (Shift+Enter = new line, ↑/↓ = previous messages, 📎 = attach files)...".to_string()
                            } else {
                                "Frage stellen (Shift+Enter = Zeilenumbruch, ↑/↓ = vorherige Nachrichten, 📎 = Anhänge hinzufügen)...".to_string()
                            }
                        }
                        prop:value=move || user_input.get()
                        on:input=move |ev| {
                            set_user_input.set(event_target_value(&ev));
                            set_history_cursor.set(None);
                        }
                        on:keydown=move |ev| {
                            let key = ev.key();
                            if key == "Enter" && !ev.shift_key() {
                                ev.prevent_default();
                                send_message();
                            } else if key == "ArrowUp" && !ev.shift_key() && !ev.ctrl_key() && !ev.alt_key() {
                                let current_text = user_input.get_untracked();
                                // Pfeil-Hoch lädt vorherige Nachrichten, wenn wir schon im Verlauf blättern oder kein mehrzeiliger Entwurf aktiv ist
                                if history_cursor.get_untracked().is_some() || !current_text.contains('\n') {
                                    let user_prompts: Vec<String> = messages
                                        .get_untracked()
                                        .into_iter()
                                        .filter(|m| m.role == "user")
                                        .map(|m| {
                                            let raw = m.content.get_untracked();
                                            if raw.starts_with("📎 [") {
                                                if let Some(end_bracket) = raw.find("] ") {
                                                    raw[end_bracket + 2..].to_string()
                                                } else {
                                                    raw
                                                }
                                            } else {
                                                raw
                                            }
                                        })
                                        .filter(|s| !s.trim().is_empty())
                                        .collect();

                                    if !user_prompts.is_empty() {
                                        ev.prevent_default();
                                        let next_idx = match history_cursor.get_untracked() {
                                            None => {
                                                set_draft_before_history.set(current_text);
                                                user_prompts.len() - 1
                                            }
                                            Some(0) => 0,
                                            Some(idx) => idx.saturating_sub(1),
                                        };
                                        set_history_cursor.set(Some(next_idx));
                                        if let Some(prev_msg) = user_prompts.get(next_idx) {
                                            set_user_input.set(prev_msg.clone());
                                        }
                                    }
                                }
                            } else if key == "ArrowDown" && !ev.shift_key() && !ev.ctrl_key() && !ev.alt_key() {
                                if let Some(cur_idx) = history_cursor.get_untracked() {
                                    let user_prompts: Vec<String> = messages
                                        .get_untracked()
                                        .into_iter()
                                        .filter(|m| m.role == "user")
                                        .map(|m| {
                                            let raw = m.content.get_untracked();
                                            if raw.starts_with("📎 [") {
                                                if let Some(end_bracket) = raw.find("] ") {
                                                    raw[end_bracket + 2..].to_string()
                                                } else {
                                                    raw
                                                }
                                            } else {
                                                raw
                                            }
                                        })
                                        .filter(|s| !s.trim().is_empty())
                                        .collect();

                                    ev.prevent_default();
                                    if cur_idx + 1 < user_prompts.len() {
                                        let next_idx = cur_idx + 1;
                                        set_history_cursor.set(Some(next_idx));
                                        set_user_input.set(user_prompts[next_idx].clone());
                                    } else {
                                        set_history_cursor.set(None);
                                        set_user_input.set(draft_before_history.get_untracked());
                                    }
                                }
                            }
                        }
                    />
                    {move || if is_loading.get() {
                        view! {
                            <button
                                class="btn-stop-subtle"
                                title=move || if is_en() { "Stop current generation" } else { "Aktuelle Antwortgenerierung abbrechen" }
                                on:click=move |_| stop_chat_generation()
                            >
                                <span class="stop-square"></span>
                                <span>{move || if is_en() { "Stop" } else { "Stopp" }}</span>
                            </button>
                        }.into_view()
                    } else {
                        view! {
                            <button
                                class="btn"
                                disabled=move || is_extracting_attachment.get()
                                on:click=move |_| send_message()
                            >
                                {move || if is_en() { "Send" } else { "Senden" }}
                            </button>
                        }.into_view()
                    }}
                </div>
            </main>

            // --- Modal-Dialog für Wissen einspeisen, Dokumente & SQL-Datenbanken (Immer im Focus Screen / Vollbild) ---
            {move || show_ingest_modal.get().then(|| {
                let backdrop_class = "modal-backdrop fullscreen";
                let dialog_class = "modal-dialog fullscreen";

                view! {
                    <div class=backdrop_class>
                        <div class=dialog_class>
                            <div class="modal-header">
                                <h2 style="display:flex; align-items:center; gap:0.5rem;">
                                    {move || if modal_tab.get() == "docs" {
                                        if is_en() {
                                            "🗂️ Stored Documents & Knowledge Base"
                                        } else {
                                            "🗂️ Gespeicherte Dokumente & Wissensbasis"
                                        }
                                    } else if modal_tab.get() == "crm" {
                                        if is_en() {
                                            "🏢 Showcase 01: Enterprise CRM / HR Employee Database (3,000 Records)"
                                        } else {
                                            "🏢 Showcase 01: Enterprise CRM / HR Mitarbeiter-Datenbank (3.000 Datensätze)"
                                        }
                                    } else if modal_tab.get() == "fleet" {
                                        if is_en() {
                                            "🚛 Showcase 02: Spedition, Fleet Telemetry, GPS Tours & Warehouses"
                                        } else {
                                            "🚛 Showcase 02: Spedition, Fuhrpark-Telemetrie, GPS-Touren & Lagerhallen"
                                        }
                                    } else if is_en() {
                                        "📚 Ingest Knowledge into SurrealDB"
                                    } else {
                                        "📚 Wissen in SurrealDB einspeisen"
                                    }}
                                </h2>
                                <div style="display:flex; align-items:center; gap:0.5rem;">
                                    <button
                                        class="btn btn-secondary"
                                        style="padding: 0.35rem 0.75rem; font-size:0.8rem;"
                                        on:click=move |_| {
                                            set_show_ingest_modal.set(false);
                                        }
                                    >
                                        {move || if is_en() { "✕ Close" } else { "✕ Schließen" }}
                                    </button>
                                </div>
                            </div>

                        <div class="modal-tabs">
                            <button
                                class=move || if modal_tab.get() == "files" { "tab-btn active" } else { "tab-btn" }
                                on:click=move |_| set_modal_tab.set("files".to_string())
                            >
                                {move || if is_en() { "📎 Multi-File Upload" } else { "📎 Multi-Datei-Upload" }}
                            </button>
                            <button
                                class=move || if modal_tab.get() == "text" { "tab-btn active" } else { "tab-btn" }
                                on:click=move |_| set_modal_tab.set("text".to_string())
                            >
                                "📝 Text / Markdown"
                            </button>
                            <button
                                class=move || if modal_tab.get() == "docs" { "tab-btn active" } else { "tab-btn" }
                                on:click=move |_| {
                                    refresh_documents();
                                    set_modal_tab.set("docs".to_string());
                                }
                            >
                                {move || if is_en() {
                                    format!("🗂️ Stored Documents ({})", stored_docs.get().len())
                                } else {
                                    format!("🗂️ Gespeicherte Dokumente ({})", stored_docs.get().len())
                                }}
                            </button>
                            {move || (!crm_employees.get().is_empty()).then(|| view! {
                                <button
                                    class=move || if modal_tab.get() == "crm" { "tab-btn active" } else { "tab-btn" }
                                    on:click=move |_| {
                                        refresh_crm_employees();
                                        set_modal_tab.set("crm".to_string());
                                    }
                                >
                                    {move || if is_en() {
                                        format!("🏢 Showcase 01: HR / CRM DB ({})", crm_employees.get().len())
                                    } else {
                                        format!("🏢 Showcase 01: HR / CRM DB ({})", crm_employees.get().len())
                                    }}
                                </button>
                            })}
                            {move || (!fleet_vehicles.get().is_empty()).then(|| view! {
                                <button
                                    class=move || if modal_tab.get() == "fleet" { "tab-btn active" } else { "tab-btn" }
                                    on:click=move |_| {
                                        refresh_fleet_data();
                                        set_modal_tab.set("fleet".to_string());
                                    }
                                >
                                    {move || if is_en() {
                                        format!("🚛 Showcase 02: Spedition ({} Trucks / {} Tours)", fleet_vehicles.get().len(), fleet_tours.get().len())
                                    } else {
                                        format!("🚛 Showcase 02: Spedition ({} LKWs / {} Touren)", fleet_vehicles.get().len(), fleet_tours.get().len())
                                    }}
                                </button>
                            })}
                        </div>

                        <div class="modal-body">
                            {move || if modal_tab.get() == "files" {
                                view! {
                                    <div style="display:flex; flex-direction:column; gap:1rem;">
                                        // Revisions-, Pipeline- & Projekt-Einstellungen
                                        <div style="display:flex; flex-direction:column; gap:0.65rem; background:#0f172a; padding:0.85rem; border-radius:8px; border:1px solid #334155;">
                                            <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(240px, 1fr)); gap:0.75rem;">
                                                <div>
                                                    <label style="font-size:0.78rem; color:#94a3b8; display:block; margin-bottom:0.25rem;">
                                                        {move || if is_en() {
                                                            "⚙️ Ingestion Pipeline (Document Domain & Parser)"
                                                        } else {
                                                            "⚙️ Ingestion-Pipeline (Dokumenttyp & Parser-Profil)"
                                                        }}
                                                    </label>
                                                    <select
                                                        prop:value=move || ingest_pipeline.get()
                                                        on:change=move |ev| set_ingest_pipeline.set(event_target_value(&ev))
                                                    >
                                                        <option value="auto">{move || if is_en() { "🔄 Auto-detect (Legal XML vs. Generic)" } else { "🔄 Automatisch erkennen (Gesetzes-XML vs. Generisch)" }}</option>
                                                        <option value="generic">{move || if is_en() { "📄 Generic Pipeline (PDF, Markdown, Contracts, Free Text)" } else { "📄 Generische Pipeline (PDF, Markdown, Verträge, Freitext)" }}</option>
                                                        <option value="legal">{move || if is_en() { "⚖️ Legal & Statutes Pipeline (gii-norm XML, EUR-Lex Formex, §§)" } else { "⚖️ Gesetze & Verordnungen Pipeline (gii-norm XML, EUR-Lex Formex, §§)" }}</option>
                                                    </select>
                                                </div>
                                                <div>
                                                    <label style="font-size:0.78rem; color:#94a3b8; display:block; margin-bottom:0.25rem;">
                                                        {move || if is_en() {
                                                            "📁 Project / Folder / Category (optional)"
                                                        } else {
                                                            "📁 Projekt / Akte / Kategorie (optional)"
                                                        }}
                                                    </label>
                                                    <input
                                                        type="text"
                                                        placeholder=move || if is_en() { "e.g. Case 53-20, Project North, or Directives" } else { "z.B. Akte 53-20, Objekt Dichtervillen oder Dienstanweisungen" }
                                                        prop:value=move || doc_category.get()
                                                        on:input=move |ev| set_doc_category.set(event_target_value(&ev))
                                                    />
                                                </div>
                                            </div>
                                            <div class="toggle-row" style="margin-top:0.2rem;">
                                                <input
                                                    type="checkbox"
                                                    id="rev-toggle-files"
                                                    prop:checked=move || replace_revision.get()
                                                    on:change=move |ev| {
                                                        let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                        set_replace_revision.set(checked);
                                                    }
                                                />
                                                <label for="rev-toggle-files" style="margin:0; font-size:0.82rem; color:#e2e8f0; cursor:pointer;">
                                                    {move || if is_en() {
                                                        "🔄 Automatically replace older revisions sharing the same document title"
                                                    } else {
                                                        "🔄 Ältere Revisionen bei gleichem Dokumenttitel automatisch ersetzen"
                                                    }}
                                                </label>
                                            </div>
                                            <div class="toggle-row" style="margin-top:0.1rem;">
                                                <input
                                                    type="checkbox"
                                                    id="graph-toggle-files"
                                                    prop:checked=move || extract_graph.get()
                                                    on:change=move |ev| {
                                                        let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                        set_extract_graph.set(checked);
                                                    }
                                                />
                                                <label for="graph-toggle-files" style="margin:0; font-size:0.82rem; color:#c084fc; cursor:pointer;">
                                                    {move || if is_en() {
                                                        "🕸️ Automatically extract Knowledge Graph relations (entities & edges via LLM) during upload"
                                                    } else {
                                                        "🕸️ Wissensgraph-Beziehungen (Personen, Firmen, Projekte) beim Upload automatisch extrahieren (GraphRAG)"
                                                    }}
                                                </label>
                                            </div>
                                        </div>

                                        // Verstecktes natives File-Input für einzelne/mehrere Dateien
                                        <input
                                            type="file"
                                            id="multi-file-upload"
                                            multiple=true
                                            accept=".pdf,.xml,.md,.txt,.html,.htm,.json,.csv"
                                            style="display:none;"
                                            node_ref=file_input_ref
                                            on:change=on_files_selected
                                        />

                                        // Verstecktes natives File-Input für ganze Ordnerstrukturen
                                        <input
                                            type="file"
                                            id="multi-folder-upload"
                                            multiple=true
                                            attr:webkitdirectory=""
                                            attr:directory=""
                                            style="display:none;"
                                            node_ref=folder_input_ref
                                            on:change=on_files_selected
                                        />

                                        <div class="dropzone-label">
                                            <div class="dropzone-icon">"📂"</div>
                                            <div style="font-weight:600; color:#f8fafc; font-size:0.95rem;">
                                                {move || if is_en() {
                                                    "Select Files or an Entire Folder"
                                                } else {
                                                    "Dateien oder ganzen Ordner auswählen"
                                                }}
                                            </div>
                                            <div style="font-size:0.78rem; color:#94a3b8; max-width:440px; text-align:center;">
                                                {move || if is_en() {
                                                    "Select individual files or a complete folder hierarchy (subfolder names are automatically assigned as document categories & graph projects)."
                                                } else {
                                                    "Wähle einzelne Dateien oder einen Ordner (Unterordner werden automatisch als Kategorie & Graph-Gruppe übernommen)."
                                                }}
                                            </div>
                                            <div style="display:flex; gap:0.65rem; margin-top:0.4rem;">
                                                <label for="multi-file-upload" class="btn btn-secondary" style="cursor:pointer; font-size:0.8rem; padding:0.45rem 0.85rem;">
                                                    "📄 " {move || if is_en() { "Select Files" } else { "Dateien wählen" }}
                                                </label>
                                                <label for="multi-folder-upload" class="btn btn-secondary" style="cursor:pointer; font-size:0.8rem; padding:0.45rem 0.85rem; border-color:#6366f1; color:#c7d2fe; background:rgba(99, 102, 241, 0.15);">
                                                    "📁 " {move || if is_en() { "Select Folder (Batch)" } else { "Ordner wählen (Batch)" }}
                                                </label>
                                            </div>
                                        </div>

                                        // Liste der ausgewählten Dateien als sauber ausgerichtetes 3-Spalten-Grid (Dateiname | Größe rechtsbündig | ✕)
                                        {move || (!selected_file_names.get().is_empty()).then(|| {
                                            let total_queued_kb: u64 = selected_files
                                                .get()
                                                .iter()
                                                .map(|f| {
                                                    let b: &web_sys::Blob = f.as_ref();
                                                    (b.size() / 1024.0).max(1.0) as u64
                                                })
                                                .sum();
                                            view! {
                                                <div>
                                                    <div style="display:flex; justify-content:space-between; align-items:center; margin-top:0.55rem; margin-bottom:0.35rem; font-size:0.78rem; color:#94a3b8;">
                                                        <span>
                                                            {move || if is_en() {
                                                                format!(
                                                                    "{} file(s) queued ({} KB total)",
                                                                    selected_file_names.get().len(),
                                                                    total_queued_kb
                                                                )
                                                            } else {
                                                                format!(
                                                                    "{} Datei(en) in der Warteschlange ({} KB gesamt)",
                                                                    selected_file_names.get().len(),
                                                                    total_queued_kb
                                                                )
                                                            }}
                                                        </span>
                                                        <button
                                                            class="btn btn-secondary"
                                                            style="padding:0.2rem 0.55rem; font-size:0.73rem; background:#7f1d1d; color:#fecaca;"
                                                            disabled=move || is_uploading.get()
                                                            on:click=move |_| {
                                                                set_selected_file_names.set(Vec::new());
                                                                set_selected_files.set(Vec::new());
                                                            }
                                                        >
                                                            {move || if is_en() { "✕ Clear all" } else { "✕ Alle entfernen" }}
                                                        </button>
                                                    </div>
                                                    <div class="file-pill-list">
                                                        {selected_file_names.get().into_iter().map(|(name, size)| {
                                                            let name_to_remove = name.clone();
                                                            view! {
                                                                <div class="file-pill">
                                                                    <span class="file-pill-name" title=name.clone()>"📄 " {name}</span>
                                                                    <span class="file-pill-size">{size}</span>
                                                                    <button
                                                                        style="background:transparent; border:none; color:#fca5a5; cursor:pointer; font-size:0.76rem; padding:0; text-align:right;"
                                                                        title=move || if is_en() { "Remove file from selection" } else { "Datei aus Auswahl entfernen" }
                                                                        disabled=move || is_uploading.get()
                                                                        on:click=move |_| {
                                                                            let target = name_to_remove.clone();
                                                                            let mut names = selected_file_names.get_untracked();
                                                                            let mut files = selected_files.get_untracked();
                                                                            if let Some(pos) = names.iter().position(|(n, _)| n == &target) {
                                                                                names.remove(pos);
                                                                                if pos < files.len() {
                                                                                    files.remove(pos);
                                                                                }
                                                                                set_selected_file_names.set(names);
                                                                                set_selected_files.set(files);
                                                                            }
                                                                        }
                                                                    >
                                                                        "✕"
                                                                    </button>
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            }
                                        })}

                                        // Live-Fortschrittsbalken & dynamische Restzeit-Schätzung (ETA) während des Uploads
                                        {move || (is_uploading.get() && !upload_eta_text.get().is_empty()).then(|| view! {
                                            <div style="background:#0f172a; border:1px solid #4f46e5; border-radius:0.5rem; padding:0.65rem 0.85rem; display:flex; flex-direction:column; gap:0.4rem;">
                                                <div style="display:flex; justify-content:space-between; align-items:center; font-size:0.78rem; color:#e2e8f0; font-variant-numeric:tabular-nums;">
                                                    <span>"⏱️ " {move || upload_eta_text.get()}</span>
                                                </div>
                                                <div style="width:100%; height:7px; background:#1e293b; border-radius:999px; overflow:hidden;">
                                                    <div
                                                        style=move || format!(
                                                            "height:100%; width:{:.1}%; background:linear-gradient(90deg, #6366f1, #38bdf8); transition:width 0.35s ease;",
                                                            upload_progress_pct.get()
                                                        )
                                                    ></div>
                                                </div>
                                            </div>
                                        })}

                                        <div style="display:flex; gap:0.65rem; align-items:center;">
                                            <button
                                                class="btn"
                                                style="flex:1;"
                                                disabled=move || is_uploading.get() || selected_file_names.get().is_empty()
                                                on:click=on_upload_files
                                            >
                                                {move || if is_uploading.get() {
                                                    if is_en() { "⏳ Processing files...".to_string() } else { "⏳ Verarbeite Dateien...".to_string() }
                                                } else {
                                                    let n = selected_file_names.get().len();
                                                    if n > 0 {
                                                        if is_en() {
                                                            format!("🚀 Upload & index {} selected file(s)", n)
                                                        } else {
                                                            format!("🚀 {} ausgewählte Datei(en) hochladen & indexieren", n)
                                                        }
                                                    } else if is_en() {
                                                        "Upload & index files".to_string()
                                                    } else {
                                                        "Dateien hochladen & indexieren".to_string()
                                                    }
                                                }}
                                            </button>
                                            {move || is_uploading.get().then(|| view! {
                                                <button
                                                    type="button"
                                                    class="btn-stop-subtle"
                                                    style="height:auto; padding:0.62rem 0.85rem;"
                                                    title=move || if is_en() { "Cancel running upload" } else { "Laufenden Upload sofort abbrechen" }
                                                    on:click=cancel_upload
                                                >
                                                    <span class="stop-square"></span>
                                                    <span>{move || if is_en() { "Cancel" } else { "Abbrechen" }}</span>
                                                </button>
                                            })}
                                        </div>
                                    </div>
                                }.into_view()
                            } else if modal_tab.get() == "text" {
                                view! {
                                    <div style="display:flex; flex-direction:column; gap:0.9rem;">
                                        <div>
                                            <label style="font-size:0.8rem; color:#94a3b8; display:block; margin-bottom:0.3rem;">
                                                {move || if is_en() { "Project / Folder / Category (optional)" } else { "Projekt / Akte / Kategorie (optional)" }}
                                            </label>
                                            <input
                                                type="text"
                                                placeholder=move || if is_en() { "e.g. Documentation or Directives" } else { "z.B. Dienstanweisungen oder Akte 53-20" }
                                                prop:value=move || doc_category.get()
                                                on:input=move |ev| set_doc_category.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div>
                                            <label style="font-size:0.8rem; color:#94a3b8; display:block; margin-bottom:0.3rem;">
                                                {move || if is_en() { "Document Title" } else { "Dokumenten-Titel" }}
                                            </label>
                                            <input
                                                type="text"
                                                placeholder=move || if is_en() { "e.g. Architecture Guideline" } else { "z.B. Dienstanweisung A oder Architektur-Richtlinie" }
                                                prop:value=move || doc_title.get()
                                                on:input=move |ev| set_doc_title.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="toggle-row">
                                            <input
                                                type="checkbox"
                                                id="rev-toggle-text"
                                                prop:checked=move || replace_revision.get()
                                                on:change=move |ev| {
                                                    let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                    set_replace_revision.set(checked);
                                                }
                                            />
                                            <label for="rev-toggle-text" style="margin:0; font-size:0.82rem; color:#e2e8f0; cursor:pointer;">
                                                {move || if is_en() {
                                                    "🔄 Automatically replace older revisions sharing the same document title"
                                                } else {
                                                    "🔄 Ältere Revisionen bei gleichem Dokumenttitel automatisch ersetzen"
                                                }}
                                            </label>
                                        </div>
                                        <div class="toggle-row">
                                            <input
                                                type="checkbox"
                                                id="graph-toggle-text"
                                                prop:checked=move || extract_graph.get()
                                                on:change=move |ev| {
                                                    let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                    set_extract_graph.set(checked);
                                                }
                                            />
                                            <label for="graph-toggle-text" style="margin:0; font-size:0.82rem; color:#c084fc; cursor:pointer;">
                                                {move || if is_en() {
                                                    "🕸️ Automatically extract Knowledge Graph relations (GraphRAG)"
                                                } else {
                                                    "🕸️ Wissensgraph-Beziehungen (Personen, Firmen, Projekte) beim Speichern automatisch extrahieren (GraphRAG)"
                                                }}
                                            </label>
                                        </div>
                                        <div>
                                            <label style="font-size:0.8rem; color:#94a3b8; display:block; margin-bottom:0.3rem;">
                                                {move || if is_en() { "Content (Markdown or Plaintext)" } else { "Inhalt (Markdown oder Text)" }}
                                            </label>
                                            <textarea
                                                placeholder=move || if is_en() { "Paste text or markdown content here..." } else { "Text hier einfügen..." }
                                                prop:value=move || doc_text.get()
                                                on:input=move |ev| set_doc_text.set(event_target_value(&ev))
                                            ></textarea>
                                        </div>
                                        <button
                                            class="btn"
                                            disabled=move || is_uploading.get()
                                            on:click=on_ingest_text
                                        >
                                            {move || if is_en() { "🚀 Index into SurrealDB" } else { "🚀 In SurrealDB indexieren" }}
                                        </button>
                                    </div>
                                }.into_view()
                            } else if modal_tab.get() == "docs" {
                                // Tab "docs" -> Strukturierte Dokumentenanzeige
                                view! {
                                    <div style="display:flex; flex-direction:column; gap:1rem; height:100%;">
                                        // 1. Stats Bar & Aktionen (Deduplicate, Refresh)
                                        {move || {
                                            let docs = stored_docs.get();
                                            let total_chunks: usize = docs.iter().map(|d| d.chunk_count).sum();
                                            let mut cat_counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                                            for d in &docs {
                                                let (cat, _) = parse_category_and_title(&d.doc_title);
                                                *cat_counts.entry(cat).or_insert(0) += 1;
                                            }
                                            let total_categories = cat_counts.len();

                                            view! {
                                                <div class="docs-stats-bar">
                                                    <div class="docs-stat-badges">
                                                        <div class="docs-stat-badge">
                                                            <span>"📁"</span>
                                                            <span>{move || if is_en() { "Categories: " } else { "Kategorien: " }}</span>
                                                            <strong>{total_categories}</strong>
                                                        </div>
                                                        <div class="docs-stat-badge">
                                                            <span>"📄"</span>
                                                            <span>{move || if is_en() { "Documents: " } else { "Dokumente: " }}</span>
                                                            <strong>{docs.len()}</strong>
                                                        </div>
                                                        <div class="docs-stat-badge">
                                                            <span>"🧩"</span>
                                                            <span>{move || if is_en() { "Total Chunks: " } else { "Chunks gesamt: " }}</span>
                                                            <strong>{total_chunks}</strong>
                                                        </div>
                                                    </div>

                                                    <div style="display:flex; gap:0.5rem; align-items:center;">
                                                        <button
                                                            class="btn btn-secondary"
                                                            style="padding:0.35rem 0.7rem; font-size:0.78rem;"
                                                            title=move || if is_en() { "Remove older versions of documents with the same name" } else { "Ältere Versionen gleichnamiger Dokumente entfernen" }
                                                            on:click=move |_| {
                                                                spawn_local(async move {
                                                                    if let Ok(resp) = Request::post("/v1/documents/deduplicate").send().await {
                                                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                                            let rem = json["removed_documents"].as_u64().unwrap_or(0);
                                                                            let msg = if is_en() {
                                                                                format!("🧹 Pruned {} older duplicate revision(s)!", rem)
                                                                            } else {
                                                                                format!("🧹 {} ältere Duplikat-Einträge bereinigt!", rem)
                                                                            };
                                                                            set_ingest_status.set(msg);
                                                                            refresh_documents();
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        >
                                                            {move || if is_en() { "🧹 Prune Duplicates" } else { "🧹 Duplikate bereinigen" }}
                                                        </button>
                                                        <button
                                                            class="btn btn-secondary"
                                                            style="padding:0.35rem 0.7rem; font-size:0.78rem;"
                                                            on:click=move |_| refresh_documents()
                                                        >
                                                            {move || if is_en() { "🔄 Refresh" } else { "🔄 Aktualisieren" }}
                                                        </button>
                                                    </div>
                                                </div>
                                            }
                                        }}

                                        // 2. Suche & Aufklappbarer Kategorie-Filter (in EINER Zeile)
                                        {move || {
                                            let docs = stored_docs.get();
                                            if docs.is_empty() {
                                                view! { <div></div> }.into_view()
                                            } else {
                                                let mut cat_counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                                                for d in &docs {
                                                    let (cat, _) = parse_category_and_title(&d.doc_title);
                                                    *cat_counts.entry(cat).or_insert(0) += 1;
                                                }

                                                let current_cat = selected_category_filter.get();
                                                let is_filtered = current_cat != "all";
                                                let total_cats = cat_counts.len();

                                                view! {
                                                    <div class="docs-filter-bar">
                                                        <div class="docs-search-filter-row">
                                                            <input
                                                                class="docs-search-input"
                                                                type="text"
                                                                placeholder=move || if is_en() {
                                                                    "🔍 Search documents by title, folder, or ID..."
                                                                } else {
                                                                    "🔍 Dokumente nach Titel, Ordner oder ID durchsuchen..."
                                                                }
                                                                prop:value=move || doc_search_term.get()
                                                                on:input=move |ev| set_doc_search_term.set(event_target_value(&ev))
                                                            />

                                                            <button
                                                                class=move || {
                                                                    if is_filter_expanded.get() || is_filtered {
                                                                        "docs-filter-expand-btn active"
                                                                    } else {
                                                                        "docs-filter-expand-btn"
                                                                    }
                                                                }
                                                                title=move || if is_en() { "Toggle Category Filter list" } else { "Kategorien-Filterliste ein-/ausklappen" }
                                                                on:click=move |_| set_is_filter_expanded.update(|exp| *exp = !*exp)
                                                            >
                                                                <span>"📁"</span>
                                                                <span>
                                                                    {if is_filtered {
                                                                        format!("Filter: {} ▾", current_cat)
                                                                    } else {
                                                                        format!("Categories ({}) ▾", total_cats)
                                                                    }}
                                                                </span>
                                                            </button>

                                                            {if is_filtered {
                                                                view! {
                                                                    <button
                                                                        class="btn btn-secondary"
                                                                        style="padding:0.45rem 0.65rem; font-size:0.78rem;"
                                                                        title=move || if is_en() { "Clear category filter" } else { "Kategorie-Filter aufheben" }
                                                                        on:click=move |_| set_selected_category_filter.set("all".to_string())
                                                                    >
                                                                        "✕"
                                                                    </button>
                                                                }.into_view()
                                                            } else {
                                                                view! { <span></span> }.into_view()
                                                            }}
                                                        </div>

                                                        {move || is_filter_expanded.get().then(|| {
                                                            let c_cat = selected_category_filter.get();
                                                            let cats_clone = cat_counts.clone();
                                                            let total_docs_len = docs.len();

                                                            view! {
                                                                <div class="docs-categories-popover">
                                                                    <div style="display:flex; justify-content:space-between; align-items:center; border-bottom:1px solid rgba(51,65,85,0.4); padding-bottom:0.4rem;">
                                                                        <span style="font-size:0.75rem; color:#94a3b8; font-weight:600;">
                                                                            {move || if is_en() {
                                                                                "Select category to filter view:"
                                                                            } else {
                                                                                "Kategorie zur Ansichtsfilterung wählen:"
                                                                            }}
                                                                        </span>
                                                                        <button
                                                                            class="btn btn-secondary"
                                                                            style="padding:0.2rem 0.5rem; font-size:0.72rem;"
                                                                            on:click=move |_| set_is_filter_expanded.set(false)
                                                                        >
                                                                            {move || if is_en() { "Close ✕" } else { "Schließen ✕" }}
                                                                        </button>
                                                                    </div>

                                                                    <div class="category-filter-chips">
                                                                        <button
                                                                            class=if c_cat == "all" { "category-chip active" } else { "category-chip" }
                                                                            on:click=move |_| set_selected_category_filter.set("all".to_string())
                                                                        >
                                                                            {move || if is_en() {
                                                                                format!("All ({})", total_docs_len)
                                                                            } else {
                                                                                format!("Alle ({})", total_docs_len)
                                                                            }}
                                                                        </button>

                                                                        {cats_clone.into_iter().map(|(cat_name, count)| {
                                                                            let c_name_click = cat_name.clone();
                                                                            let is_active = c_cat.eq_ignore_ascii_case(&cat_name);
                                                                            let chip_class = if is_active { "category-chip active" } else { "category-chip" };
                                                                            view! {
                                                                                <button
                                                                                    class=chip_class
                                                                                    on:click=move |_| {
                                                                                        if is_active {
                                                                                            set_selected_category_filter.set("all".to_string());
                                                                                        } else {
                                                                                            set_selected_category_filter.set(c_name_click.clone());
                                                                                        }
                                                                                    }
                                                                                >
                                                                                    {format!("📁 {} ({})", cat_name, count)}
                                                                                </button>
                                                                            }
                                                                        }).collect_view()}
                                                                    </div>
                                                                </div>
                                                            }
                                                        })}
                                                    </div>
                                                }.into_view()
                                            }
                                        }}

                                        // 3. Gruppierte Anzeige der Dokumente nach Kategorien
                                        {move || {
                                            let docs = stored_docs.get();
                                            if docs.is_empty() {
                                                view! {
                                                    <div style="padding:2.5rem; text-align:center; color:#64748b; border:1px dashed #334155; border-radius:8px;">
                                                        <div style="font-size:2rem; margin-bottom:0.5rem;">"📂"</div>
                                                        <strong style="display:block; color:#94a3b8; font-size:0.95rem;">
                                                            {move || if is_en() { "No documents stored in SurrealDB yet." } else { "Noch keine Dokumente in SurrealDB gespeichert." }}
                                                        </strong>
                                                        <span style="font-size:0.8rem; color:#64748b;">
                                                            {move || if is_en() { "Upload files or paste markdown to build your knowledge base." } else { "Lade Dateien hoch oder füge Markdown-Texte ein, um deine Wissensbasis aufzubauen." }}
                                                        </span>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                let search_term = doc_search_term.get().trim().to_lowercase();
                                                let selected_cat = selected_category_filter.get();

                                                let mut grouped: std::collections::BTreeMap<String, Vec<DocumentItem>> = std::collections::BTreeMap::new();

                                                for doc in docs {
                                                    let (cat, _clean_title) = parse_category_and_title(&doc.doc_title);
                                                    
                                                    // Kategorie-Filter
                                                    if selected_cat != "all" && !cat.eq_ignore_ascii_case(&selected_cat) {
                                                        continue;
                                                    }

                                                    // Suchfilter
                                                    if !search_term.is_empty() {
                                                        let matches = doc.doc_title.to_lowercase().contains(&search_term)
                                                            || doc.doc_id.to_lowercase().contains(&search_term)
                                                            || cat.to_lowercase().contains(&search_term);
                                                        if !matches {
                                                            continue;
                                                        }
                                                    }

                                                    grouped.entry(cat).or_default().push(doc);
                                                }

                                                if grouped.is_empty() {
                                                    view! {
                                                        <div style="padding:2.5rem; text-align:center; color:#64748b; border:1px dashed #334155; border-radius:8px;">
                                                            <div style="font-size:2rem; margin-bottom:0.5rem;">"🔍"</div>
                                                            <span style="font-size:0.9rem; color:#94a3b8;">
                                                                {move || if is_en() {
                                                                    "No documents match the current filter or search criteria."
                                                                } else {
                                                                    "Keine Dokumente für die aktuellen Filter- oder Suchkriterien gefunden."
                                                                }}
                                                            </span>
                                                        </div>
                                                    }.into_view()
                                                } else {
                                                    view! {
                                                        <div style="display:flex; flex-direction:column; gap:1.1rem; overflow-y:auto; padding-right:0.25rem;">
                                                            {grouped.into_iter().map(|(cat_name, cat_docs)| {
                                                                let cat_chunks: usize = cat_docs.iter().map(|d| d.chunk_count).sum();
                                                                let cat_count = cat_docs.len();
                                                                view! {
                                                                    <div class="category-section">
                                                                        <div class="category-section-header">
                                                                            <span class="category-title">
                                                                                <span>"📁"</span>
                                                                                <span>{cat_name}</span>
                                                                            </span>
                                                                            <span class="category-badge-count">
                                                                                {move || if is_en() {
                                                                                    format!("{} docs · {} chunks", cat_count, cat_chunks)
                                                                                } else {
                                                                                    format!("{} Dok. · {} Chunks", cat_count, cat_chunks)
                                                                                }}
                                                                            </span>
                                                                        </div>

                                                                        <div class="docs-grid">
                                                                            {cat_docs.into_iter().map(|doc| {
                                                                                let id_for_del = doc.doc_id.clone();
                                                                                let title_for_del = doc.doc_title.clone();
                                                                                let id_for_view = doc.doc_id.clone();
                                                                                let title_for_view = doc.doc_title.clone();
                                                                                let id_for_title_click = doc.doc_id.clone();
                                                                                let title_for_title_click = doc.doc_title.clone();
                                                                                let title_for_hdr_tip = doc.doc_title.clone();
                                                                                let title_for_btn_tip = doc.doc_title.clone();
                                                                                let title_for_btn_lbl = doc.doc_title.clone();
                                                                                let (doc_icon, _, _, _) = doc_format_labels(&doc.doc_title, false);
                                                                                let (_, clean_title) = parse_category_and_title(&doc.doc_title);
                                                                                let chunk_badge = format!("🧩 {} Chunks", doc.chunk_count);
                                                                                let date_badge = format!("🕒 {}", doc.created_at);
                                                                                let id_badge = format!("🏷️ {}", doc.doc_id);
                                                                                let rel_count = doc.relations.len();
                                                                                let rels_for_view = doc.relations.clone();

                                                                                view! {
                                                                                    <div class="doc-card">
                                                                                        <div
                                                                                            class="doc-card-header"
                                                                                            style="cursor:pointer;"
                                                                                            title=move || doc_format_labels(&title_for_hdr_tip, is_en()).2
                                                                                            on:click=move |_| {
                                                                                                set_viewing_pdf_doc.set(Some((id_for_title_click.clone(), title_for_title_click.clone())));
                                                                                            }
                                                                                        >
                                                                                            <span class="doc-card-icon">{doc_icon}</span>
                                                                                            <span class="doc-card-title" title=doc.doc_title.clone()>
                                                                                                {clean_title}
                                                                                            </span>
                                                                                        </div>

                                                                                        <div class="doc-card-meta">
                                                                                            <span class="meta-pill">{chunk_badge}</span>
                                                                                            <span class="meta-pill">{date_badge}</span>
                                                                                            <span class="meta-pill" title="SurrealDB Record ID">{id_badge}</span>

                                                                                            {if rel_count == 0 {
                                                                                                view! {
                                                                                                    <span class="meta-pill" style="opacity:0.55;">
                                                                                                        "🕸️ 0 Relations"
                                                                                                    </span>
                                                                                                }.into_view()
                                                                                            } else {
                                                                                                view! {
                                                                                                    <details class="doc-relations-details">
                                                                                                        <summary class="doc-relations-summary">
                                                                                                            <span>{format!("🕸️ {} Relations ▾", rel_count)}</span>
                                                                                                        </summary>
                                                                                                        <div class="doc-relations-list">
                                                                                                            {rels_for_view.into_iter().map(|rel| view! {
                                                                                                                <div class="doc-relation-item">
                                                                                                                    <span class="rel-icon">"🔗"</span>
                                                                                                                    <span>{rel}</span>
                                                                                                                </div>
                                                                                                            }).collect_view()}
                                                                                                        </div>
                                                                                                    </details>
                                                                                                }.into_view()
                                                                                            }}
                                                                                        </div>

                                                                                        <div class="doc-card-actions" style="display:flex; justify-content:space-between; align-items:center; gap:0.5rem;">
                                                                                            <button
                                                                                                class="btn btn-secondary"
                                                                                                style="padding:0.25rem 0.6rem; font-size:0.75rem; background:rgba(99,102,241,0.18); color:#c7d2fe; border-color:#6366f1;"
                                                                                                title=move || doc_format_labels(&title_for_btn_tip, is_en()).2
                                                                                                on:click=move |_| {
                                                                                                    set_viewing_pdf_doc.set(Some((id_for_view.clone(), title_for_view.clone())));
                                                                                                }
                                                                                            >
                                                                                                {move || doc_format_labels(&title_for_btn_lbl, is_en()).1}
                                                                                            </button>
                                                                                            <button
                                                                                                class="btn btn-secondary"
                                                                                                style="padding:0.25rem 0.55rem; font-size:0.75rem; color:#fca5a5; border-color:#7f1d1d;"
                                                                                                title=move || if is_en() { "Delete document & vector chunks" } else { "Dokument & Vektor-Chunks löschen" }
                                                                                                on:click=move |_| {
                                                                                                    let del_id = id_for_del.clone();
                                                                                                    let del_title = title_for_del.clone();
                                                                                                    spawn_local(async move {
                                                                                                        let url = format!("/v1/documents/{}", del_id);
                                                                                                        if let Ok(resp) = Request::delete(&url).send().await {
                                                                                                            if resp.ok() {
                                                                                                                let msg = if is_en() {
                                                                                                                    format!("🗑️ '{}' and all associated vector chunks deleted.", del_title)
                                                                                                                } else {
                                                                                                                    format!("🗑️ '{}' wurde samt allen Vektor-Chunks gelöscht.", del_title)
                                                                                                                };
                                                                                                                set_ingest_status.set(msg);
                                                                                                                refresh_documents();
                                                                                                            }
                                                                                                        }
                                                                                                    });
                                                                                                }
                                                                                            >
                                                                                                {move || if is_en() { "🗑️ Delete" } else { "🗑️ Löschen" }}
                                                                                            </button>
                                                                                        </div>
                                                                                    </div>
                                                                                }
                                                                            }).collect_view()}
                                                                        </div>
                                                                    </div>
                                                                }
                                                            }).collect_view()}
                                                        </div>
                                                    }.into_view()
                                                }
                                            }
                                        }}
                                    </div>
                                }.into_view()
                            } else if modal_tab.get() == "crm" {
                                // Tab "crm" -> Enterprise CRM / HR Relationale Mitarbeiter-Datenbank (`employee_record` in SurrealDB)
                                view! {
                                    <div style="display:flex; flex-direction:column; gap:1rem; height:100%;">
                                        // 1. Architektur-Hinweis & Aktionsleiste (300 Mitarbeiter generieren / DB leeren)
                                        <div style="background:rgba(15, 23, 42, 0.75); border:1px solid #334155; border-radius:0.65rem; padding:0.85rem 1rem; display:flex; flex-direction:column; gap:0.65rem;">
                                            <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.6rem;">
                                                <div style="display:flex; align-items:center; gap:0.65rem; flex-wrap:wrap;">
                                                    <span style="background:rgba(99, 102, 241, 0.2); border:1px solid #6366f1; color:#e0e7ff; padding:0.28rem 0.7rem; border-radius:999px; font-size:0.8rem; font-weight:700;">
                                                        {move || if is_en() {
                                                            format!("🗄️ {} Employee Records in SurrealDB", crm_employees.get().len())
                                                        } else {
                                                            format!("🗄️ {} Mitarbeiter-Datensätze in SurrealDB", crm_employees.get().len())
                                                        }}
                                                    </span>
                                                    <span style="font-size:0.76rem; color:#38bdf8;">
                                                        {move || if is_en() {
                                                            "100% SQL/SurrealQL Recall (no Top-K vector loss) + 1-Skill Training Gap Analysis"
                                                        } else {
                                                            "100% SQL/SurrealQL-Abdeckung (kein Top-K Vektor-Verlust) + automatische Weiterbildungs-Gap-Analyse"
                                                        }}
                                                    </span>
                                                </div>

                                                <div style="display:flex; gap:0.5rem; align-items:center; flex-wrap:wrap;">
                                                    <button
                                                        class="btn"
                                                        style="padding:0.4rem 0.75rem; font-size:0.76rem; background:linear-gradient(135deg, #10b981 0%, #059669 100%);"
                                                        disabled=move || is_seeding_crm.get()
                                                        on:click=move |_| {
                                                            set_is_seeding_crm.set(true);
                                                            set_ingest_status.set(if is_en() {
                                                                "⏳ Seeding 3,000 employee records (300 full matches, 150 near-matches)...".to_string()
                                                            } else {
                                                                "⏳ Lade Showcase 01: 3.000 Mitarbeiter (300 Volltreffer, 150 Weiterbildungskandidaten)...".to_string()
                                                            });
                                                            spawn_local(async move {
                                                                let payload = serde_json::json!({ "count": 3000 });
                                                                match Request::post("/v1/crm/seed")
                                                                    .header("Content-Type", "application/json")
                                                                    .body(payload.to_string())
                                                                    .unwrap()
                                                                    .send()
                                                                    .await
                                                                {
                                                                    Ok(resp) if resp.ok() => {
                                                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                                            let cnt = json["count"].as_u64().unwrap_or(3000);
                                                                            set_ingest_status.set(if is_en() {
                                                                                format!("✅ Showcase 01 ready: {} structured employees (300 full matches & 150 training candidates) seeded into SurrealDB!", cnt)
                                                                            } else {
                                                                                format!("✅ Showcase 01 geladen: {} strukturierte Mitarbeiter (300 Volltreffer & 150 Weiterbildungskandidaten) in SurrealDB angelegt!", cnt)
                                                                            });
                                                                            refresh_crm_employees();
                                                                        }
                                                                    }
                                                                    _ => {
                                                                        set_ingest_status.set("❌ Fehler beim Laden der 3.000er Mitarbeiter-Datenbank.".to_string());
                                                                    }
                                                                }
                                                                set_is_seeding_crm.set(false);
                                                            });
                                                        }
                                                    >
                                                        {move || if is_seeding_crm.get() {
                                                            if is_en() { "⏳ Loading..." } else { "⏳ Lade..." }
                                                        } else if is_en() {
                                                            "🏢 Load 3,000 Employees (300 Matches)"
                                                        } else {
                                                            "🏢 3.000 Mitarbeiter laden (300 Treffer)"
                                                        }}
                                                    </button>

                                                    <button
                                                        class="btn btn-secondary"
                                                        style="padding:0.4rem 0.7rem; font-size:0.78rem; color:#fca5a5; border-color:#7f1d1d;"
                                                        on:click=move |_| {
                                                            spawn_local(async move {
                                                                if let Ok(resp) = Request::delete("/v1/crm/employees").send().await {
                                                                    if resp.ok() {
                                                                        set_ingest_status.set(if is_en() {
                                                                            "🗑️ Structured CRM/HR employee table cleared.".to_string()
                                                                        } else {
                                                                            "🗑️ Strukturierte CRM/HR-Mitarbeitertabelle geleert.".to_string()
                                                                        });
                                                                        refresh_crm_employees();
                                                                        set_modal_tab.set("docs".to_string());
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    >
                                                        {move || if is_en() { "🗑️ Clear DB" } else { "🗑️ DB leeren" }}
                                                    </button>

                                                    <button
                                                        class="btn btn-secondary"
                                                        style="padding:0.4rem 0.7rem; font-size:0.78rem;"
                                                        on:click=move |_| refresh_crm_employees()
                                                    >
                                                        {move || if is_en() { "🔄 Refresh" } else { "🔄 Aktualisieren" }}
                                                    </button>
                                                </div>
                                            </div>

                                            <div style="font-size:0.76rem; color:#94a3b8; line-height:1.45;">
                                                {move || if is_en() {
                                                    "💡 Hybrid Enterprise Architecture: Unstructured customer contracts & SLAs (e.g. BMW AG) are retrieved via Vector/BM25 (Hop 1), and their extracted mandatory requirements (e.g. Rust, SQL, English >= B2) are automatically executed as a deterministic SQL query across all 300+ structured employee records (Hop 2)."
                                                } else {
                                                    "💡 Hybride Enterprise-Architektur: Unstrukturierte Kundenverträge & SLAs (z.B. BMW AG) werden per Vektor/BM25 gelesen (Hop 1) und deren Pflichtkriterien (z.B. Rust, SQL, Englisch >= B2) automatisch als deterministische SQL-Abfrage gegen alle 300+ Mitarbeiter-Datensätze ausgeführt (Hop 2)."
                                                }}
                                            </div>
                                        </div>

                                        // 2. Suchfeld für Mitarbeiter (Name, ID, Skill, Englisch-Level, Abteilung, Standort)
                                        <input
                                            class="docs-search-input"
                                            type="text"
                                            placeholder=move || if is_en() {
                                                "🔍 Filter 300+ employees by name, ID (EMP-1042), skill (Rust, SQL), English (B2, C1), role, or city..."
                                            } else {
                                                "🔍 300+ Mitarbeiter nach Name, ID (EMP-1042), Skill (Rust, SQL), Englisch (B2, C1), Rolle oder Standort filtern..."
                                            }
                                            prop:value=move || crm_search_term.get()
                                            on:input=move |ev| set_crm_search_term.set(event_target_value(&ev))
                                        />

                                        // 3. Mitarbeiter-Tabelle / Grid
                                        {move || {
                                            let all_emps = crm_employees.get();
                                            if all_emps.is_empty() {
                                                view! {
                                                    <div style="padding:2.5rem; text-align:center; color:#64748b; border:1px dashed #334155; border-radius:8px;">
                                                        <div style="font-size:2rem; margin-bottom:0.5rem;">"🏢"</div>
                                                        <strong style="display:block; color:#94a3b8; font-size:0.95rem;">
                                                            {move || if is_en() {
                                                                "No structured employee records in SurrealDB (`employee_record`) yet."
                                                            } else {
                                                                "Noch keine strukturierten Mitarbeiter-Datensätze in SurrealDB (`employee_record`)."
                                                            }}
                                                        </strong>
                                                        <span style="font-size:0.8rem; color:#64748b;">
                                                            {move || if is_en() {
                                                                "Run './setup.sh --showcase 1' or click '🏢 Load 3,000 Employees' above."
                                                            } else {
                                                                "Starte mit './setup.sh --showcase 1' oder klicke oben auf '🏢 3.000 Mitarbeiter laden (300 Treffer)'."
                                                            }}
                                                        </span>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                let q = crm_search_term.get().trim().to_lowercase();
                                                let filtered: Vec<CrmEmployeeItem> = all_emps
                                                    .into_iter()
                                                    .filter(|e| {
                                                        if q.is_empty() {
                                                            return true;
                                                        }
                                                        e.emp_id.to_lowercase().contains(&q)
                                                            || e.name.to_lowercase().contains(&q)
                                                            || e.role.to_lowercase().contains(&q)
                                                            || e.department.to_lowercase().contains(&q)
                                                            || e.english_level.to_lowercase().contains(&q)
                                                            || e.location.to_lowercase().contains(&q)
                                                            || e.current_project.to_lowercase().contains(&q)
                                                            || e.skills.iter().any(|s| s.to_lowercase().contains(&q))
                                                            || e.certifications.iter().any(|c| c.to_lowercase().contains(&q))
                                                    })
                                                    .collect();

                                                let total_filtered = filtered.len();
                                                let bmw_matches_in_filtered = filtered
                                                    .iter()
                                                    .filter(|emp| {
                                                        emp.skills.iter().any(|s| s.eq_ignore_ascii_case("rust"))
                                                            && emp.skills.iter().any(|s| s.eq_ignore_ascii_case("sql"))
                                                            && matches!(emp.english_level.as_str(), "B2" | "C1" | "C2")
                                                    })
                                                    .count();

                                                view! {
                                                    <div style="display:flex; flex-direction:column; gap:0.55rem; flex:1; min-height:0; overflow-y:auto; padding-right:0.25rem;">
                                                        <div style="font-size:0.75rem; color:#94a3b8; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.5rem;">
                                                            <span>
                                                                {if is_en() {
                                                                    if total_filtered > 100 {
                                                                        format!("Showing first 100 of {} matching employee records (use search bar above to narrow down)", total_filtered)
                                                                    } else {
                                                                        format!("Showing {} matching employee record(s)", total_filtered)
                                                                    }
                                                                } else if total_filtered > 100 {
                                                                    format!("Zeige die ersten 100 von {} passenden Mitarbeitern (nutze das Suchfeld oben zum Filtern)", total_filtered)
                                                                } else {
                                                                    format!("Zeige {} passende Mitarbeiter-Datensätze", total_filtered)
                                                                }}
                                                            </span>
                                                            <span style="color:#34d399; font-weight:600;">
                                                                {format!("✅ {} Volltreffer (Rust + SQL + >=B2) in dieser Auswahl", bmw_matches_in_filtered)}
                                                            </span>
                                                        </div>
                                                        {filtered.into_iter().take(100).map(|emp| {
                                                            let has_rust = emp.skills.iter().any(|s| s.eq_ignore_ascii_case("rust"));
                                                            let has_sql = emp.skills.iter().any(|s| s.eq_ignore_ascii_case("sql"));
                                                            let is_b2_plus = matches!(emp.english_level.as_str(), "B2" | "C1" | "C2");
                                                            let is_bmw_full_match = has_rust && has_sql && is_b2_plus;
                                                            let border_style = if is_bmw_full_match {
                                                                "background:rgba(16, 185, 129, 0.07); border:1px solid rgba(16, 185, 129, 0.45); border-radius:0.55rem; padding:0.65rem 0.85rem; display:flex; justify-content:space-between; align-items:center; gap:0.75rem; flex-wrap:wrap;"
                                                            } else {
                                                                "background:rgba(15, 23, 42, 0.6); border:1px solid #334155; border-radius:0.55rem; padding:0.65rem 0.85rem; display:flex; justify-content:space-between; align-items:center; gap:0.75rem; flex-wrap:wrap;"
                                                            };
                                                            view! {
                                                                <div style=border_style>
                                                                    <div style="display:flex; flex-direction:column; gap:0.25rem; min-width:220px; flex:1;">
                                                                        <div style="display:flex; align-items:center; gap:0.45rem; flex-wrap:wrap;">
                                                                            <span style="font-family:monospace; font-size:0.73rem; background:#1e293b; color:#38bdf8; padding:0.1rem 0.4rem; border-radius:4px; border:1px solid #334155;">
                                                                                {emp.emp_id}
                                                                            </span>
                                                                            <strong style="color:#f8fafc; font-size:0.88rem;">{emp.name}</strong>
                                                                            <span style="font-size:0.76rem; color:#94a3b8;">"· " {emp.role}</span>
                                                                            {is_bmw_full_match.then(|| view! {
                                                                                <span style="font-size:0.68rem; background:rgba(16,185,129,0.2); color:#34d399; border:1px solid rgba(16,185,129,0.4); padding:0.08rem 0.4rem; border-radius:999px; font-weight:600;">
                                                                                    "✅ Rust + SQL + >=B2"
                                                                                </span>
                                                                            })}
                                                                        </div>
                                                                        <div style="font-size:0.74rem; color:#64748b; display:flex; gap:0.6rem; flex-wrap:wrap;">
                                                                            <span>"🏢 " {emp.department}</span>
                                                                            <span>"📍 " {emp.location}</span>
                                                                            <span>"📂 " {emp.current_project}</span>
                                                                            <span>{format!("💶 {:.0} €/h · ⏱️ {}% frei", emp.hourly_rate, emp.availability_percent)}</span>
                                                                        </div>
                                                                    </div>

                                                                    <div style="display:flex; align-items:center; gap:0.35rem; flex-wrap:wrap;">
                                                                        {emp.skills.into_iter().map(|sk| {
                                                                            let is_key = sk.eq_ignore_ascii_case("rust") || sk.eq_ignore_ascii_case("sql");
                                                                            let pill_style = if is_key {
                                                                                "font-size:0.7rem; background:rgba(99,102,241,0.22); color:#c7d2fe; border:1px solid #6366f1; padding:0.12rem 0.45rem; border-radius:4px; font-weight:600;"
                                                                            } else {
                                                                                "font-size:0.7rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; padding:0.12rem 0.45rem; border-radius:4px;"
                                                                            };
                                                                            view! { <span style=pill_style>{sk}</span> }
                                                                        }).collect_view()}
                                                                        <span style="font-size:0.72rem; background:#1e1b4b; color:#a5b4fc; border:1px solid #4338ca; padding:0.12rem 0.48rem; border-radius:4px; font-weight:700;">
                                                                            {format!("🇬🇧 {}", emp.english_level)}
                                                                        </span>
                                                                    </div>
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                }.into_view()
                                            }
                                        }}
                                    </div>
                                }.into_view()
                            } else if modal_tab.get() == "fleet" {
                                // Tab "fleet" -> Showcase 02: Spedition, LKW-Zustandsdaten, GPS-Touren (März & April) & Lagerhallen
                                view! {
                                    <div style="display:flex; flex-direction:column; gap:0.9rem; height:100%;">
                                        // 1. Header & Seed-Aktionsleiste
                                        <div style="background:rgba(15, 23, 42, 0.75); border:1px solid #334155; border-radius:0.65rem; padding:0.85rem 1rem; display:flex; flex-direction:column; gap:0.65rem;">
                                            <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.6rem;">
                                                <div style="display:flex; align-items:center; gap:0.5rem; flex-wrap:wrap;">
                                                    <span style="background:rgba(56, 189, 248, 0.18); border:1px solid #38bdf8; color:#e0f2fe; padding:0.25rem 0.65rem; border-radius:999px; font-size:0.78rem; font-weight:700;">
                                                        {move || format!("🚛 {} LKWs", fleet_vehicles.get().len())}
                                                    </span>
                                                    <span style="background:rgba(16, 185, 129, 0.18); border:1px solid #10b981; color:#d1fae5; padding:0.25rem 0.65rem; border-radius:999px; font-size:0.78rem; font-weight:700;">
                                                        {move || format!("📍 {} GPS-Touren (März & April)", fleet_tours.get().len())}
                                                    </span>
                                                    <span style="background:rgba(168, 85, 247, 0.18); border:1px solid #a855f7; color:#f3e8ff; padding:0.25rem 0.65rem; border-radius:999px; font-size:0.78rem; font-weight:700;">
                                                        {move || format!("🏭 {} Lagerhallen", fleet_warehouses.get().len())}
                                                    </span>
                                                </div>

                                                <div style="display:flex; gap:0.45rem; align-items:center; flex-wrap:wrap;">
                                                    <button
                                                        class="btn"
                                                        style="padding:0.4rem 0.75rem; font-size:0.76rem; background:linear-gradient(135deg, #10b981 0%, #059669 100%);"
                                                        disabled=move || is_seeding_fleet.get()
                                                        on:click=move |_| {
                                                            set_is_seeding_fleet.set(true);
                                                            set_ingest_status.set("⏳ Lade Showcase 02: Speditions-Flotte (120 LKWs, 232 GPS-Touren März/April, 5 Lagerhallen)...".to_string());
                                                            spawn_local(async move {
                                                                let payload = serde_json::json!({ "truck_count": 120 });
                                                                if let Ok(resp) = Request::post("/v1/fleet/seed")
                                                                    .header("Content-Type", "application/json")
                                                                    .body(payload.to_string())
                                                                    .unwrap()
                                                                    .send()
                                                                    .await
                                                                {
                                                                    if resp.ok() {
                                                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                                            let vc = json["vehicle_count"].as_u64().unwrap_or(120);
                                                                            let tc = json["tour_count"].as_u64().unwrap_or(232);
                                                                            let wc = json["warehouse_count"].as_u64().unwrap_or(5);
                                                                            set_ingest_status.set(format!("✅ Showcase 02 geladen: {} LKWs, {} GPS-Touren (März & April) & {} Lagerhallen in SurrealDB!", vc, tc, wc));
                                                                            refresh_fleet_data();
                                                                        }
                                                                    }
                                                                }
                                                                set_is_seeding_fleet.set(false);
                                                            });
                                                        }
                                                    >
                                                        "🚛 120 LKWs + 232 Touren laden"
                                                    </button>

                                                    <button
                                                        class="btn btn-secondary"
                                                        style="padding:0.4rem 0.7rem; font-size:0.78rem; color:#fca5a5; border-color:#7f1d1d;"
                                                        on:click=move |_| {
                                                            spawn_local(async move {
                                                                if let Ok(resp) = Request::delete("/v1/fleet/overview").send().await {
                                                                    if resp.ok() {
                                                                        set_ingest_status.set("🗑️ Speditions-Tabellen (LKWs, Touren, Lagerhallen) geleert.".to_string());
                                                                        refresh_fleet_data();
                                                                        set_modal_tab.set("docs".to_string());
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    >
                                                        "🗑️ DB leeren"
                                                    </button>

                                                    <button
                                                        class="btn btn-secondary"
                                                        style="padding:0.4rem 0.7rem; font-size:0.78rem;"
                                                        on:click=move |_| refresh_fleet_data()
                                                    >
                                                        "🔄 Aktualisieren"
                                                    </button>
                                                </div>
                                            </div>

                                            // Sub-Tabs: LKWs | GPS-Touren | Lagerhallen
                                            <div style="display:flex; gap:0.45rem; flex-wrap:wrap; padding-top:0.25rem; border-top:1px solid #1e293b;">
                                                <button
                                                    type="button"
                                                    style=move || if fleet_subtab.get() == "vehicles" {
                                                        "padding:0.35rem 0.75rem; font-size:0.76rem; font-weight:700; border-radius:6px; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer;"
                                                    } else {
                                                        "padding:0.35rem 0.75rem; font-size:0.76rem; border-radius:6px; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer;"
                                                    }
                                                    on:click=move |_| set_fleet_subtab.set("vehicles".to_string())
                                                >
                                                    {move || format!("🔧 Fuhrpark & Telemetrie ({} LKWs)", fleet_vehicles.get().len())}
                                                </button>
                                                <button
                                                    type="button"
                                                    style=move || if fleet_subtab.get() == "tours" {
                                                        "padding:0.35rem 0.75rem; font-size:0.76rem; font-weight:700; border-radius:6px; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer;"
                                                    } else {
                                                        "padding:0.35rem 0.75rem; font-size:0.76rem; border-radius:6px; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer;"
                                                    }
                                                    on:click=move |_| set_fleet_subtab.set("tours".to_string())
                                                >
                                                    {move || format!("📍 GPS-Touren März & April ({})", fleet_tours.get().len())}
                                                </button>
                                                <button
                                                    type="button"
                                                    style=move || if fleet_subtab.get() == "warehouses" {
                                                        "padding:0.35rem 0.75rem; font-size:0.76rem; font-weight:700; border-radius:6px; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer;"
                                                    } else {
                                                        "padding:0.35rem 0.75rem; font-size:0.76rem; border-radius:6px; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer;"
                                                    }
                                                    on:click=move |_| set_fleet_subtab.set("warehouses".to_string())
                                                >
                                                    {move || format!("🏭 Lagerhallen-Netzwerk ({})", fleet_warehouses.get().len())}
                                                </button>
                                            </div>
                                        </div>

                                        <input
                                            class="docs-search-input"
                                            type="text"
                                            placeholder="🔍 Filtern nach LKW-ID (LKW-01), Kennzeichen, Fahrer, Stadt (Berlin), Monat (April), Kunde (BioPharma) oder Lagerhalle..."
                                            prop:value=move || fleet_search_term.get()
                                            on:input=move |ev| set_fleet_search_term.set(event_target_value(&ev))
                                        />

                                        {move || {
                                            let q = fleet_search_term.get().trim().to_lowercase();
                                            let sub = fleet_subtab.get();

                                            if fleet_vehicles.get().is_empty() && fleet_tours.get().is_empty() {
                                                view! {
                                                    <div style="padding:2.5rem; text-align:center; color:#64748b; border:1px dashed #334155; border-radius:8px;">
                                                        <div style="font-size:2rem; margin-bottom:0.5rem;">"🚛"</div>
                                                        <strong style="display:block; color:#94a3b8; font-size:0.95rem;">
                                                            "Noch keine Speditions- & Fuhrpark-Daten in SurrealDB geladen."
                                                        </strong>
                                                        <span style="font-size:0.8rem; color:#64748b;">
                                                            "Starte mit './setup.sh --showcase 2' oder klicke oben auf '🚛 120 LKWs + 232 Touren laden'."
                                                        </span>
                                                    </div>
                                                }.into_view()
                                            } else if sub == "vehicles" {
                                                let filtered: Vec<FleetVehicleItem> = fleet_vehicles
                                                    .get()
                                                    .into_iter()
                                                    .filter(|v| {
                                                        q.is_empty()
                                                            || v.truck_id.to_lowercase().contains(&q)
                                                            || v.license_plate.to_lowercase().contains(&q)
                                                            || v.model.to_lowercase().contains(&q)
                                                            || v.assigned_driver.to_lowercase().contains(&q)
                                                            || v.current_location.to_lowercase().contains(&q)
                                                            || v.inspection_status.to_lowercase().contains(&q)
                                                            || v.oil_status.to_lowercase().contains(&q)
                                                    })
                                                    .take(80)
                                                    .collect();
                                                view! {
                                                    <div style="display:flex; flex-direction:column; gap:0.55rem; flex:1; min-height:0; overflow-y:auto; padding-right:0.25rem;">
                                                        {filtered.into_iter().map(|v| {
                                                            let is_crit = v.km_until_inspection <= 2500 || v.oil_level_percent < 60 || v.brake_wear_percent >= 75;
                                                            let card_style = if is_crit {
                                                                "background:rgba(239, 68, 68, 0.08); border:1px solid rgba(239, 68, 68, 0.45); border-radius:0.55rem; padding:0.65rem 0.85rem; display:flex; justify-content:space-between; align-items:center; gap:0.75rem; flex-wrap:wrap;"
                                                            } else {
                                                                "background:rgba(15, 23, 42, 0.6); border:1px solid #334155; border-radius:0.55rem; padding:0.65rem 0.85rem; display:flex; justify-content:space-between; align-items:center; gap:0.75rem; flex-wrap:wrap;"
                                                            };
                                                            view! {
                                                                <div style=card_style>
                                                                    <div style="display:flex; flex-direction:column; gap:0.25rem; flex:1; min-width:250px;">
                                                                        <div style="display:flex; align-items:center; gap:0.45rem; flex-wrap:wrap;">
                                                                            <span style="font-family:monospace; font-size:0.75rem; background:#1e293b; color:#38bdf8; padding:0.1rem 0.45rem; border-radius:4px; border:1px solid #334155; font-weight:700;">
                                                                                {v.truck_id}
                                                                            </span>
                                                                            <span style="font-family:monospace; font-size:0.74rem; color:#fde047;">
                                                                                {format!("[{}]", v.license_plate)}
                                                                            </span>
                                                                            <strong style="color:#f8fafc; font-size:0.86rem;">{v.model}</strong>
                                                                            <span style="font-size:0.75rem; color:#94a3b8;">{format!("· {} ({})", v.vehicle_type, v.emission_class)}</span>
                                                                        </div>
                                                                        <div style="font-size:0.74rem; color:#cbd5e1; display:flex; gap:0.7rem; flex-wrap:wrap;">
                                                                            <span>"👤 " {v.assigned_driver}</span>
                                                                            <span>"📍 " {v.current_location}</span>
                                                                            <span>{format!("🛣️ {} km (Inspektion in {} km am {})", v.mileage_km, v.km_until_inspection, v.next_inspection_date)}</span>
                                                                        </div>
                                                                    </div>
                                                                    <div style="display:flex; align-items:center; gap:0.35rem; flex-wrap:wrap; font-size:0.71rem;">
                                                                        <span style="background:#0f172a; color:#e2e8f0; border:1px solid #334155; padding:0.14rem 0.45rem; border-radius:4px;">
                                                                            {format!("🛢️ Öl: {}%", v.oil_level_percent)}
                                                                        </span>
                                                                        <span style="background:#0f172a; color:#e2e8f0; border:1px solid #334155; padding:0.14rem 0.45rem; border-radius:4px;">
                                                                            {format!("💧 AdBlue: {}%", v.adblue_level_percent)}
                                                                        </span>
                                                                        <span style="background:#0f172a; color:#e2e8f0; border:1px solid #334155; padding:0.14rem 0.45rem; border-radius:4px;">
                                                                            {format!("🛑 Bremsen: {}%", v.brake_wear_percent)}
                                                                        </span>
                                                                        <span style="background:#0f172a; color:#e2e8f0; border:1px solid #334155; padding:0.14rem 0.45rem; border-radius:4px;">
                                                                            {format!("🛞 {:.1} bar", v.tire_pressure_bar)}
                                                                        </span>
                                                                        {v.adr_certified.then(|| view! {
                                                                            <span style="background:rgba(245,158,11,0.2); color:#fbbf24; border:1px solid #f59e0b; padding:0.12rem 0.45rem; border-radius:4px; font-weight:700;">
                                                                                "☣️ ADR"
                                                                            </span>
                                                                        })}
                                                                    </div>
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                }.into_view()
                                            } else if sub == "tours" {
                                                let filtered: Vec<FleetTourItem> = fleet_tours
                                                    .get()
                                                    .into_iter()
                                                    .filter(|t| {
                                                        q.is_empty()
                                                            || t.tour_id.to_lowercase().contains(&q)
                                                            || t.truck_id.to_lowercase().contains(&q)
                                                            || t.license_plate.to_lowercase().contains(&q)
                                                            || t.driver_name.to_lowercase().contains(&q)
                                                            || t.month.to_lowercase().contains(&q)
                                                            || t.origin_city.to_lowercase().contains(&q)
                                                            || t.destination_city.to_lowercase().contains(&q)
                                                            || t.customer_name.to_lowercase().contains(&q)
                                                            || t.cargo_description.to_lowercase().contains(&q)
                                                            || t.gps_waypoints.iter().any(|w| {
                                                                w.location_name.to_lowercase().contains(&q)
                                                                    || w.weather_condition.to_lowercase().contains(&q)
                                                                    || w.fix_type.to_lowercase().contains(&q)
                                                            })
                                                    })
                                                    .take(80)
                                                    .collect();
                                                view! {
                                                    <div style="display:flex; flex-direction:column; gap:0.55rem; flex:1; min-height:0; overflow-y:auto; padding-right:0.25rem;">
                                                        {filtered.into_iter().map(|t| {
                                                            let is_berlin_april = t.destination_city.eq_ignore_ascii_case("berlin") && t.month.to_lowercase().contains("april");
                                                            let has_gnss_anomaly = t.gps_waypoints.iter().any(|w| w.satellites_used < 4 || w.gps_accuracy_m >= 15.0);
                                                            let card_style = if has_gnss_anomaly {
                                                                "background:rgba(239, 68, 68, 0.08); border:1px solid rgba(239, 68, 68, 0.45); border-radius:0.55rem; padding:0.65rem 0.85rem; display:flex; flex-direction:column; gap:0.35rem;"
                                                            } else if is_berlin_april {
                                                                "background:rgba(16, 185, 129, 0.08); border:1px solid rgba(16, 185, 129, 0.45); border-radius:0.55rem; padding:0.65rem 0.85rem; display:flex; flex-direction:column; gap:0.35rem;"
                                                            } else {
                                                                "background:rgba(15, 23, 42, 0.6); border:1px solid #334155; border-radius:0.55rem; padding:0.65rem 0.85rem; display:flex; flex-direction:column; gap:0.35rem;"
                                                            };
                                                            let tid_for_map = t.tour_id.clone();
                                                            let tlabel_for_map = format!("{} · {} ({}) · {} ➔ {}", t.tour_id, t.truck_id, t.license_plate, t.origin_city, t.destination_city);
                                                            let gmap_for_map = t.google_maps_url.clone();
                                                            let gmap_direct = t.google_maps_url.clone();
                                                            let waypoints_list = t.gps_waypoints.clone();
                                                            let wp_cnt = waypoints_list.len();
                                                            view! {
                                                                <div style=card_style>
                                                                    <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.45rem;">
                                                                        <div style="display:flex; align-items:center; gap:0.45rem; flex-wrap:wrap;">
                                                                            <span style="font-family:monospace; font-size:0.73rem; background:#1e293b; color:#38bdf8; padding:0.1rem 0.4rem; border-radius:4px;">
                                                                                {t.tour_id.clone()}
                                                                            </span>
                                                                            <span style="font-size:0.75rem; color:#a5b4fc; font-weight:600;">
                                                                                {format!("📅 {} ({})", t.departure_date, t.month)}
                                                                            </span>
                                                                            <strong style="color:#f8fafc; font-size:0.84rem;">
                                                                                {format!("{} ({}) · {}", t.truck_id, t.license_plate, t.driver_name)}
                                                                            </strong>
                                                                            <span style="font-size:0.8rem; color:#34d399; font-weight:700;">
                                                                                {format!("{} ({}) ➔ {} ({})", t.origin_city, t.origin_warehouse_id, t.destination_city, t.destination_warehouse_id)}
                                                                            </span>
                                                                            {has_gnss_anomaly.then(|| view! {
                                                                                <span style="background:rgba(239,68,68,0.22); color:#fca5a5; border:1px solid #ef4444; padding:0.1rem 0.45rem; border-radius:4px; font-size:0.7rem; font-weight:700;">
                                                                                    "⚠️ 1-Hz GNSS-Drift (< 4 Sat.) & Unwetter"
                                                                                </span>
                                                                            })}
                                                                        </div>
                                                                        <div style="display:flex; align-items:center; gap:0.45rem; flex-wrap:wrap;">
                                                                            <span style="font-size:0.72rem; color:#fde047;">
                                                                                {format!("🏢 {} · {:.1} t ({} km)", t.customer_name, t.cargo_weight_tons, t.distance_km)}
                                                                            </span>
                                                                            <button
                                                                                type="button"
                                                                                class="btn btn-secondary"
                                                                                style="padding:0.22rem 0.58rem; font-size:0.72rem; background:rgba(16,185,129,0.2); border-color:#10b981; color:#6ee7b7; font-weight:600;"
                                                                                on:click=move |_| {
                                                                                    set_viewing_tour_map.set(Some((tid_for_map.clone(), tlabel_for_map.clone(), gmap_for_map.clone())));
                                                                                }
                                                                            >
                                                                                "🗺️ OpenStreetMap (1-Hz & Wetter)"
                                                                            </button>
                                                                            {(!gmap_direct.is_empty()).then(|| view! {
                                                                                <a
                                                                                    href=gmap_direct
                                                                                    target="_blank"
                                                                                    rel="noopener noreferrer"
                                                                                    class="btn btn-secondary"
                                                                                    style="padding:0.22rem 0.58rem; font-size:0.72rem; background:rgba(37,99,235,0.22); border-color:#3b82f6; color:#93c5fd; text-decoration:none; font-weight:600;"
                                                                                >
                                                                                    "🌍 Google Maps ↗️"
                                                                                </a>
                                                                            })}
                                                                        </div>
                                                                    </div>
                                                                    <div style="font-size:0.73rem; color:#94a3b8; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.4rem;">
                                                                        <span>"📦 " {t.cargo_description} " | 🛣️ " {t.gps_route}</span>
                                                                        <span style="color:#e2e8f0; font-weight:600;">{t.delivery_status}</span>
                                                                    </div>
                                                                    {(wp_cnt > 0).then(|| view! {
                                                                        <details class="doc-relations-details">
                                                                            <summary class="doc-relations-summary" style="background:rgba(56,189,248,0.12); border-color:rgba(56,189,248,0.35); color:#7dd3fc;">
                                                                                <span>{format!("🛰️⛈️ {} GPS-Positionen (1-Hz Uhrzeit · Lat/Lon · Satelliten · HDOP · Genauigkeit ±m · Live-Wetter) anzeigen ▾", wp_cnt)}</span>
                                                                            </summary>
                                                                            <div class="doc-relations-list" style="border-color:rgba(56,189,248,0.3); max-height:360px; overflow-y:auto;">
                                                                                {waypoints_list.into_iter().enumerate().map(|(idx, wp)| {
                                                                                    let temp_info = wp.temp_c.map(|tc| format!(" · ❄️ Kühlraum: {:.1}°C", tc)).unwrap_or_default();
                                                                                    let is_bad_gnss = wp.satellites_used < 4 || wp.gps_accuracy_m >= 15.0;
                                                                                    let gnss_style = if is_bad_gnss {
                                                                                        "background:rgba(239,68,68,0.22); color:#fca5a5; border:1px solid #ef4444; padding:0.06rem 0.38rem; border-radius:4px; font-size:0.7rem; font-weight:700;"
                                                                                    } else {
                                                                                        "background:rgba(16,185,129,0.15); color:#6ee7b7; border:1px solid rgba(16,185,129,0.35); padding:0.06rem 0.38rem; border-radius:4px; font-size:0.7rem;"
                                                                                    };
                                                                                    let rain_info = if wp.precipitation_mm_h > 0.0 {
                                                                                        format!(" · 💧 {:.1} mm/h", wp.precipitation_mm_h)
                                                                                    } else {
                                                                                        String::new()
                                                                                    };
                                                                                    view! {
                                                                                        <div class="doc-relation-item" style="flex-wrap:wrap; gap:0.35rem;">
                                                                                            <span class="rel-icon" style="color:#38bdf8;">{format!("#{}", idx + 1)}</span>
                                                                                            <span style="font-family:monospace; color:#94a3b8;">{format!("[{}]", wp.timestamp)}</span>
                                                                                            <span style="font-family:monospace; color:#fde047;">
                                                                                                {format!("{:.5}°N, {:.5}°E", wp.lat, wp.lon)}
                                                                                            </span>
                                                                                            <strong style="color:#f8fafc;">{wp.location_name}</strong>
                                                                                            <span style=gnss_style>
                                                                                                {format!("🛰️ {} Sat. · HDOP {:.1} · ±{:.1}m ({})", wp.satellites_used, wp.hdop, wp.gps_accuracy_m, wp.fix_type)}
                                                                                            </span>
                                                                                            <span style="background:rgba(56,189,248,0.12); color:#bae6fd; border:1px solid rgba(56,189,248,0.3); padding:0.06rem 0.38rem; border-radius:4px; font-size:0.7rem;">
                                                                                                {format!("{} ({:.1}°C{} · 💨 {} km/h)", wp.weather_condition, wp.ambient_temp_c, rain_info, wp.wind_speed_kmh)}
                                                                                            </span>
                                                                                            <span style="color:#cbd5e1; font-size:0.72rem;">{format!("🚛 {} km/h{}", wp.speed_kmh, temp_info)}</span>
                                                                                        </div>
                                                                                    }
                                                                                }).collect_view()}
                                                                            </div>
                                                                        </details>
                                                                    })}
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                }.into_view()
                                            } else {
                                                let warehouses = fleet_warehouses.get();
                                                view! {
                                                    <div style="display:flex; flex-direction:column; gap:0.55rem; flex:1; min-height:0; overflow-y:auto; padding-right:0.25rem;">
                                                        {warehouses.into_iter().map(|w| {
                                                            view! {
                                                                <div style="background:rgba(15, 23, 42, 0.6); border:1px solid #334155; border-radius:0.55rem; padding:0.7rem 0.9rem; display:flex; justify-content:space-between; align-items:center; gap:0.75rem; flex-wrap:wrap;">
                                                                    <div style="display:flex; flex-direction:column; gap:0.25rem; flex:1;">
                                                                        <div style="display:flex; align-items:center; gap:0.45rem; flex-wrap:wrap;">
                                                                            <span style="font-family:monospace; font-size:0.75rem; background:#1e293b; color:#c084fc; padding:0.1rem 0.45rem; border-radius:4px; border:1px solid #334155; font-weight:700;">
                                                                                {w.warehouse_id}
                                                                            </span>
                                                                            <strong style="color:#f8fafc; font-size:0.88rem;">{w.name}</strong>
                                                                            <span style="font-size:0.76rem; color:#38bdf8;">{format!("📍 {} ({})", w.city, w.gps_coords)}</span>
                                                                        </div>
                                                                        <div style="font-size:0.74rem; color:#94a3b8;">
                                                                            {format!("🏗️ {} · 📦 {} Paletten ({}% Auslastung) · 👤 Leitung: {}", w.storage_type, w.capacity_pallets, w.utilization_percent, w.manager_name)}
                                                                        </div>
                                                                        <div style="font-size:0.72rem; color:#cbd5e1;">
                                                                            "⚠️ " {w.restrictions}
                                                                        </div>
                                                                    </div>
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                }.into_view()
                                            }
                                        }}
                                    </div>
                                }.into_view()
                            } else {
                                view! { <div></div> }.into_view()
                            }}

                            {move || (!ingest_status.get().is_empty()).then(|| view! {
                                <div class="status-banner">{ingest_status.get()}</div>
                            })}
                        </div>
                    </div>
                </div>
            }
        })}

            // --- Modal-Dialog: Strukturiertes Einstellungs-Menü (RAG, LLM, RunPod GPU Cloud & System) ---
            {move || show_llm_modal.get().then(|| view! {
                <div class="modal-backdrop fullscreen" on:click=move |_| set_show_llm_modal.set(false)>
                    <div
                        class="modal-dialog fullscreen"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="modal-header">
                            <h2 style="display:flex; align-items:center; gap:0.55rem;">
                                <span>"⚙️"</span>
                                <span>
                                    {move || if is_en() {
                                        "Settings & Cloud GPU Control Center"
                                    } else {
                                        "Einstellungen & Cloud-GPU Control Center"
                                    }}
                                </span>
                            </h2>
                            <button
                                class="btn btn-secondary"
                                style="padding: 0.35rem 0.75rem; font-size: 0.8rem;"
                                on:click=move |_| set_show_llm_modal.set(false)
                            >
                                {move || if is_en() { "✕ Close" } else { "✕ Schließen" }}
                            </button>
                        </div>

                        <div class="settings-layout">
                            // --- Linke Menü-Spalte (Settings Navigation) ---
                            <aside class="settings-sidebar">
                                <div class="settings-nav-list">
                                    <button
                                        type="button"
                                        class=move || if settings_tab.get() == "rag" { "settings-nav-btn active" } else { "settings-nav-btn" }
                                        on:click=move |_| set_settings_tab.set("rag".to_string())
                                    >
                                        <span class="settings-nav-icon">"🔍"</span>
                                        <div class="settings-nav-text">
                                            <span class="settings-nav-title">
                                                {move || if is_en() { "RAG & Retrieval" } else { "RAG & Retrieval" }}
                                            </span>
                                            <span class="settings-nav-sub">
                                                {move || format!("Top-{} · {} ctx · T={:.1}", top_n.get(), num_ctx_sig.get() / 1024, temperature_sig.get())}
                                            </span>
                                        </div>
                                    </button>

                                    <button
                                        type="button"
                                        class=move || if settings_tab.get() == "llm" { "settings-nav-btn active" } else { "settings-nav-btn" }
                                        on:click=move |_| set_settings_tab.set("llm".to_string())
                                    >
                                        <span class="settings-nav-icon">"🤖"</span>
                                        <div class="settings-nav-text">
                                            <span class="settings-nav-title">
                                                {move || if is_en() { "Language Model (LLM)" } else { "Sprachmodell (LLM)" }}
                                            </span>
                                            <span class="settings-nav-sub">
                                                {move || llm_model_sig.get()}
                                            </span>
                                        </div>
                                    </button>

                                    <button
                                        type="button"
                                        class=move || if settings_tab.get() == "runpod" { "settings-nav-btn active" } else { "settings-nav-btn" }
                                        on:click=move |_| {
                                            set_settings_tab.set("runpod".to_string());
                                            refresh_runpod_status();
                                        }
                                    >
                                        <span class="settings-nav-icon">"🚀"</span>
                                        <div class="settings-nav-text">
                                            <span class="settings-nav-title">"RunPod GPU Cloud"</span>
                                            <span class="settings-nav-sub">
                                                {move || match rp_phase.get().as_str() {
                                                    "running" => format!("🟢 Aktiv ({})", rp_gpu_type.get()),
                                                    "deploying" | "waiting_ssh" | "loading_models" => "🟡 Startet Pod...".to_string(),
                                                    "error" => "🔴 Fehler".to_string(),
                                                    _ => if is_en() { "SSH Tunnel (LLM + Embed)".to_string() } else { "SSH-Tunnel (LLM + Embed)".to_string() }
                                                }}
                                            </span>
                                        </div>
                                    </button>

                                    <button
                                        type="button"
                                        class=move || if settings_tab.get() == "backup" { "settings-nav-btn active" } else { "settings-nav-btn" }
                                        on:click=move |_| {
                                            set_db_backup_status.set(String::new());
                                            refresh_project_databases();
                                            refresh_documents();
                                            set_settings_tab.set("backup".to_string());
                                        }
                                    >
                                        <span class="settings-nav-icon">"💾"</span>
                                        <div class="settings-nav-text">
                                            <span class="settings-nav-title">
                                                {move || if is_en() { "Projects & DB Backup" } else { "Projekte & DB-Backup" }}
                                            </span>
                                            <span class="settings-nav-sub">
                                                {move || format!("DB: {} ({} Dok.)", active_project_db.get(), stored_docs.get().len())}
                                            </span>
                                        </div>
                                    </button>

                                    <button
                                        type="button"
                                        class=move || if settings_tab.get() == "system" { "settings-nav-btn active" } else { "settings-nav-btn" }
                                        on:click=move |_| set_settings_tab.set("system".to_string())
                                    >
                                        <span class="settings-nav-icon">"🌐"</span>
                                        <div class="settings-nav-text">
                                            <span class="settings-nav-title">
                                                {move || if is_en() { "General & System" } else { "Allgemein & System" }}
                                            </span>
                                            <span class="settings-nav-sub">
                                                {move || if is_en() { "Language (EN) & Endpoints" } else { "Sprache (DE) & Ports" }}
                                            </span>
                                        </div>
                                    </button>
                                </div>

                                <div style="padding: 0.7rem; background: rgba(15, 23, 42, 0.75); border: 1px solid #1e293b; border-radius: 0.55rem; font-size: 0.73rem; color: #94a3b8; display: flex; flex-direction: column; gap: 0.3rem;">
                                    <div style="font-weight: 700; color: #cbd5e1;">
                                        {move || if is_en() { "Active Configuration:" } else { "Aktiver Status:" }}
                                    </div>
                                    <div style="white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">
                                        "🤖 " <strong style="color:#a5b4fc;">{move || llm_model_sig.get()}</strong>
                                    </div>
                                    <div>
                                        "🚀 RunPod: "
                                        <strong style=move || if rp_phase.get() == "running" { "color:#34d399;" } else { "color:#94a3b8;" }>
                                            {move || if rp_phase.get() == "running" { "Verbunden" } else { "Aus (Lokal)" }}
                                        </strong>
                                    </div>
                                </div>
                            </aside>

                            // --- Rechte Inhalts-Spalte ---
                            <div class="settings-content-pane">
                                <div class="settings-content-inner">
                                    // =========================================================================
                                    // TAB 1: RAG & Retrieval-Pipeline
                                    // =========================================================================
                                    {move || (settings_tab.get() == "rag").then(|| view! {
                                        <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 1.1rem; display: flex; flex-direction: column; gap: 1rem;">
                                            <div style="font-size: 0.85rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #818cf8; border-bottom: 1px solid #1e293b; padding-bottom: 0.55rem;">
                                                {move || if is_en() {
                                                    "🔍 RAG & Retrieval Pipeline (SurrealDB + TEI)"
                                                } else {
                                                    "🔍 RAG & Retrieval-Pipeline (SurrealDB + TEI)"
                                    }}
                                </div>

                                // 0. Hybrid Search + GraphRAG Toggle
                                <div>
                                    <div class="toggle-row" style="display:flex; align-items:center; gap:0.5rem;">
                                        <input
                                            type="checkbox"
                                            id="rag-toggle-modal"
                                            prop:checked=move || use_rag.get()
                                            on:change=move |ev| {
                                                let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                set_use_rag.set(checked);
                                                save_use_rag_to_storage(checked);
                                            }
                                        />
                                        <label for="rag-toggle-modal" style="margin:0; color:#f8fafc; cursor:pointer; font-size:0.85rem;">
                                            {move || if is_en() {
                                                "Enable Hybrid Search + GraphRAG (search SurrealDB knowledge base on every question)"
                                            } else {
                                                "Hybrid Search + GraphRAG aktivieren (SurrealDB Wissensdatenbank bei Fragen durchsuchen)"
                                            }}
                                        </label>
                                        <button
                                            type="button"
                                            style=move || if open_rag_help.get() == Some("hybrid_rag") {
                                                "width:18px; height:18px; border-radius:50%; border:1px solid #818cf8; background:#4f46e5; color:#fff; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0; flex-shrink:0;"
                                            } else {
                                                "width:18px; height:18px; border-radius:50%; border:1px solid #475569; background:#1e293b; color:#94a3b8; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0; flex-shrink:0;"
                                            }
                                            title=move || if is_en() { "Show explanation" } else { "Erklärung anzeigen" }
                                            on:click=move |_| {
                                                if open_rag_help.get() == Some("hybrid_rag") {
                                                    set_open_rag_help.set(None);
                                                } else {
                                                    set_open_rag_help.set(Some("hybrid_rag"));
                                                }
                                            }
                                        >
                                            "?"
                                        </button>
                                    </div>
                                    {move || (open_rag_help.get() == Some("hybrid_rag")).then(|| view! {
                                        <div style="margin-top:0.45rem; padding:0.65rem 0.85rem; background:rgba(30,41,59,0.9); border:1px solid #334155; border-left:3px solid #6366f1; border-radius:0.45rem; font-size:0.76rem; color:#cbd5e1; line-height:1.5;">
                                            {move || if is_en() {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Combines three search engines in SurrealDB 4.0 via Reciprocal Rank Fusion (RRF):"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"1. HNSW Vector Search (BGE-M3):"</strong> " Finds semantically related passages even when phrased differently."</div>
                                                            <div><strong style="color:#a5b4fc;">"2. BM25 Full-Text Search:"</strong> " Matches exact statutory citations (§ 10 GwG, Art. 11 AMLR), numbers, and technical terms."</div>
                                                            <div><strong style="color:#a5b4fc;">"3. 2-Hop Knowledge Graph:"</strong> " Traverses linked entities and cross-references across documents."</div>
                                                        </div>
                                                        <div style="margin-top:0.35rem; color:#94a3b8; font-size:0.73rem;">
                                                            "💡 When unchecked, the LLM answers purely from its pre-trained weights without querying SurrealDB."
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Kombiniert drei Suchverfahren in SurrealDB 4.0 per Reciprocal Rank Fusion (RRF):"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"1. HNSW-Vektorsuche (BGE-M3):"</strong> " Findet inhaltlich passende Abschnitte auch bei abweichender Wortwahl."</div>
                                                            <div><strong style="color:#a5b4fc;">"2. BM25-Volltextsuche:"</strong> " Trifft exakte Paragraphen (§ 10 GwG, Art. 11 AMLR), Schwellenwerte und Fachbegriffe."</div>
                                                            <div><strong style="color:#a5b4fc;">"3. 2-Hop-Wissensgraph:"</strong> " Verfolgt Querverweise und verknüpfte Entitäten über Dokumentgrenzen hinweg."</div>
                                                        </div>
                                                        <div style="margin-top:0.35rem; color:#94a3b8; font-size:0.73rem;">
                                                            "💡 Wenn deaktiviert, antwortet das LLM ausschließlich aus seinem eigenen Trainingswissen ohne Datenbankabfrage."
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            }}
                                        </div>
                                    })}
                                </div>

                                // 1. Cross-Encoder Reranker Wahl (Sleek Buttons)
                                <div>
                                    <div style="font-size:0.76rem; color:#94a3b8; margin-bottom:0.35rem; display:flex; justify-content:space-between; align-items:center;">
                                        <span style="display:inline-flex; align-items:center; gap:0.4rem;">
                                            <span>"Cross-Encoder Reranker"</span>
                                            <button
                                                type="button"
                                                style=move || if open_rag_help.get() == Some("reranker") {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #818cf8; background:#4f46e5; color:#fff; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                } else {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #475569; background:#1e293b; color:#94a3b8; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                }
                                                title=move || if is_en() { "Show explanation" } else { "Erklärung anzeigen" }
                                                on:click=move |_| {
                                                    if open_rag_help.get() == Some("reranker") {
                                                        set_open_rag_help.set(None);
                                                    } else {
                                                        set_open_rag_help.set(Some("reranker"));
                                                    }
                                                }
                                            >
                                                "?"
                                            </button>
                                        </span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || match reranker_mode.get().as_str() {
                                                "high" => "🎯 High (bge-reranker-v2-m3)",
                                                "none" => "🚀 Aus (Nur SurrealDB RRF)",
                                                _ => "⚡ Fast (mmarco-mMiniLMv2-L12)",
                                            }}
                                        </span>
                                    </div>
                                    {move || (open_rag_help.get() == Some("reranker")).then(|| view! {
                                        <div style="margin-bottom:0.5rem; padding:0.65rem 0.85rem; background:rgba(30,41,59,0.9); border:1px solid #334155; border-left:3px solid #6366f1; border-radius:0.45rem; font-size:0.76rem; color:#cbd5e1; line-height:1.5;">
                                            {move || if is_en() {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "After SurrealDB retrieves 15–25 candidate chunks, the Cross-Encoder reads your question and each chunk together to compute an exact relevance score:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.28rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"⚡ Fast (mmarco-mMiniLMv2-L12):"</strong> " Multilingual 12-layer Cross-Encoder (~250 ms) — best everyday balance of speed and precision."</div>
                                                            <div><strong style="color:#a5b4fc;">"🎯 High (bge-reranker-v2-m3):"</strong> " Large 568M-parameter model (~2.2 s) — highest accuracy for complex legal questions across multiple statutes."</div>
                                                            <div><strong style="color:#a5b4fc;">"🚀 Off (SurrealDB RRF only):"</strong> " Skips neural reranking (~15 ms) and uses SurrealDB's combined vector + full-text ranking directly."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Nachdem SurrealDB die ersten 15–25 Kandidaten-Chunks gefunden hat, prüft der Cross-Encoder Frage und Textabschnitt gemeinsam auf ihre exakte Relevanz:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.28rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"⚡ Fast (mmarco-mMiniLMv2-L12):"</strong> " Mehrsprachiger 12-Layer Cross-Encoder (~250 ms) – optimaler Standard für schnelle und präzise Antworten."</div>
                                                            <div><strong style="color:#a5b4fc;">"🎯 High (bge-reranker-v2-m3):"</strong> " Großes 568M-Parameter-Modell (~2,2 s) – höchste Genauigkeit bei komplexen juristischen Detailfragen über viele Gesetze hinweg."</div>
                                                            <div><strong style="color:#a5b4fc;">"🚀 Aus (Nur SurrealDB RRF):"</strong> " Überspringt das neuronale Reranking (~15 ms) und nutzt direkt das kombinierte Vektor-/Volltext-Ranking aus SurrealDB."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            }}
                                        </div>
                                    })}
                                    <div style="display:flex; gap:0.45rem; flex-wrap:wrap;">
                                        <button
                                            type="button"
                                            style=move || if reranker_mode.get() == "fast" {
                                                "flex:1; min-width:130px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:600; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                            } else {
                                                "flex:1; min-width:130px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                            }
                                            on:click=move |_| {
                                                set_reranker_mode.set("fast".to_string());
                                                save_reranker_to_storage("fast");
                                            }
                                        >
                                            "⚡ Fast (~250ms)"
                                        </button>
                                        <button
                                            type="button"
                                            style=move || if reranker_mode.get() == "high" {
                                                "flex:1; min-width:130px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:600; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                            } else {
                                                "flex:1; min-width:130px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                            }
                                            on:click=move |_| {
                                                set_reranker_mode.set("high".to_string());
                                                save_reranker_to_storage("high");
                                            }
                                        >
                                            "🎯 High (~2.2s)"
                                        </button>
                                        <button
                                            type="button"
                                            style=move || if reranker_mode.get() == "none" {
                                                "flex:1; min-width:130px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:600; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                            } else {
                                                "flex:1; min-width:130px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                            }
                                            on:click=move |_| {
                                                set_reranker_mode.set("none".to_string());
                                                save_reranker_to_storage("none");
                                            }
                                        >
                                            {move || if is_en() { "🚀 Off (SurrealDB RRF)" } else { "🚀 Aus (Nur SurrealDB RRF)" }}
                                        </button>
                                    </div>
                                </div>

                                // 2. Top-N Chunks Wahl (Sleek Buttons)
                                <div>
                                    <div style="font-size:0.76rem; color:#94a3b8; margin-bottom:0.35rem; display:flex; justify-content:space-between; align-items:center;">
                                        <span style="display:inline-flex; align-items:center; gap:0.4rem;">
                                            <span>{move || if is_en() { "Top-N Context Chunks to LLM" } else { "Top-N Dokument-Chunks für LLM" }}</span>
                                            <button
                                                type="button"
                                                style=move || if open_rag_help.get() == Some("top_n") {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #818cf8; background:#4f46e5; color:#fff; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                } else {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #475569; background:#1e293b; color:#94a3b8; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                }
                                                title=move || if is_en() { "Show explanation" } else { "Erklärung anzeigen" }
                                                on:click=move |_| {
                                                    if open_rag_help.get() == Some("top_n") {
                                                        set_open_rag_help.set(None);
                                                    } else {
                                                        set_open_rag_help.set(Some("top_n"));
                                                    }
                                                }
                                            >
                                                "?"
                                            </button>
                                        </span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || format!("Top-{} Chunks", top_n.get())}
                                        </span>
                                    </div>
                                    {move || (open_rag_help.get() == Some("top_n")).then(|| view! {
                                        <div style="margin-bottom:0.5rem; padding:0.65rem 0.85rem; background:rgba(30,41,59,0.9); border:1px solid #334155; border-left:3px solid #6366f1; border-radius:0.45rem; font-size:0.76rem; color:#cbd5e1; line-height:1.5;">
                                            {move || if is_en() {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Number of top-ranked text passages (chunks) passed to the LLM after reranking:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"• 3 – 4 Chunks:"</strong> " Fast & focused — ideal for specific questions about a single paragraph or definition."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 6 – 10 Chunks:"</strong> " Broad coverage — recommended when comparing multiple laws simultaneously (e.g. GwG vs. KWG vs. EU AMLR)."</div>
                                                        </div>
                                                        <div style="margin-top:0.35rem; color:#94a3b8; font-size:0.73rem;">
                                                            "💡 Automatic document diversity ensures a single law cannot monopolize all Top-N slots."
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Anzahl der bestbewerteten Textabschnitte (Chunks), die nach dem Reranking als Kontext an das LLM übergeben werden:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"• 3 – 4 Chunks:"</strong> " Schnell & fokussiert – ideal für gezielte Einzelfragen zu einem bestimmten Paragraphen."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 6 – 10 Chunks:"</strong> " Breite Abdeckung – empfohlen bei übergreifenden Vergleichen mehrerer Gesetze (z. B. GwG, KWG und EU-AMLR)."</div>
                                                        </div>
                                                        <div style="margin-top:0.35rem; color:#94a3b8; font-size:0.73rem;">
                                                            "💡 Die automatische Dokumenten-Diversität stellt sicher, dass nicht ein einziges Gesetz alle Top-N-Plätze belegt."
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            }}
                                        </div>
                                    })}
                                    <div style="display:flex; gap:0.45rem; flex-wrap:wrap;">
                                        {[3, 4, 6, 8, 10].into_iter().map(|n| {
                                            view! {
                                                <button
                                                    type="button"
                                                    style=move || if top_n.get() == n {
                                                        "flex:1; min-width:65px; padding:0.38rem 0.5rem; font-size:0.78rem; font-weight:700; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                                    } else {
                                                        "flex:1; min-width:65px; padding:0.38rem 0.5rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                                    }
                                                    on:click=move |_| {
                                                        set_top_n.set(n);
                                                        save_top_n_to_storage(n);
                                                    }
                                                >
                                                    {format!("{} Chunks", n)}
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>

                                // 3. LLM Context-Fenster (num_ctx in Tokens)
                                <div>
                                    <div style="font-size:0.76rem; color:#94a3b8; margin-bottom:0.35rem; display:flex; justify-content:space-between; align-items:center;">
                                        <span style="display:inline-flex; align-items:center; gap:0.4rem;">
                                            <span>{move || if is_en() { "LLM Context Window (num_ctx in tokens)" } else { "LLM Kontext-Fenster (num_ctx in Tokens)" }}</span>
                                            <button
                                                type="button"
                                                style=move || if open_rag_help.get() == Some("num_ctx") {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #818cf8; background:#4f46e5; color:#fff; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                } else {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #475569; background:#1e293b; color:#94a3b8; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                }
                                                title=move || if is_en() { "Show explanation" } else { "Erklärung anzeigen" }
                                                on:click=move |_| {
                                                    if open_rag_help.get() == Some("num_ctx") {
                                                        set_open_rag_help.set(None);
                                                    } else {
                                                        set_open_rag_help.set(Some("num_ctx"));
                                                    }
                                                }
                                            >
                                                "?"
                                            </button>
                                        </span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || format!("{} Tokens", num_ctx_sig.get())}
                                        </span>
                                    </div>
                                    {move || (open_rag_help.get() == Some("num_ctx")).then(|| view! {
                                        <div style="margin-bottom:0.5rem; padding:0.65rem 0.85rem; background:rgba(30,41,59,0.9); border:1px solid #334155; border-left:3px solid #6366f1; border-radius:0.45rem; font-size:0.76rem; color:#cbd5e1; line-height:1.5;">
                                            {move || if is_en() {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Maximum token memory (num_ctx) allocated for prompt + retrieved chunks + chat history + answer:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"• 4k – 8k:"</strong> " Low RAM/VRAM usage — suitable for short chats with 3–4 chunks."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 16k (Default):"</strong> " ~12,000 words — recommended balance for standard RAG & GraphRAG sessions."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 32k – 64k:"</strong> " Extended memory — ideal for 8–10 chunks, multi-file attachments (📎), or long multi-turn analyses."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Maximales Token-Gedächtnis (num_ctx) für System-Prompt, gefundene RAG-Chunks, Chat-Verlauf und Antwort:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"• 4k – 8k:"</strong> " Geringer RAM-/VRAM-Bedarf – ausreichend für kurze Fragen mit 3–4 Chunks."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 16k (Default):"</strong> " ~12.000 Wörter – empfohlener Standard für normale RAG- und Gesetzesabfragen."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 32k – 64k:"</strong> " Großes Kontextfenster – ideal bei 8–10 Chunks, mehreren Datei-Anhängen (📎) oder langen Chat-Verläufen."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            }}
                                        </div>
                                    })}
                                    <div style="display:flex; gap:0.45rem; flex-wrap:wrap;">
                                        {[(4096u64, "4k"), (8192u64, "8k"), (16384u64, "16k (Default)"), (32768u64, "32k"), (65536u64, "64k")].into_iter().map(|(ctx, lbl)| {
                                            view! {
                                                <button
                                                    type="button"
                                                    style=move || if num_ctx_sig.get() == ctx {
                                                        "flex:1; min-width:65px; padding:0.38rem 0.5rem; font-size:0.78rem; font-weight:700; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                                    } else {
                                                        "flex:1; min-width:65px; padding:0.38rem 0.5rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                                    }
                                                    on:click=move |_| {
                                                        set_num_ctx_sig.set(ctx);
                                                        save_num_ctx_to_storage(ctx);
                                                    }
                                                >
                                                    {lbl}
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>

                                // 4. Reasoning / Denkmodus (DeepSeek-R1, QwQ)
                                <div>
                                    <div style="font-size:0.76rem; color:#94a3b8; margin-bottom:0.35rem; display:flex; justify-content:space-between; align-items:center;">
                                        <span style="display:inline-flex; align-items:center; gap:0.4rem;">
                                            <span>{move || if is_en() { "Thinking / Reasoning Mode (DeepSeek-R1, QwQ)" } else { "Denkprozess / Reasoning (DeepSeek-R1, QwQ)" }}</span>
                                            <button
                                                type="button"
                                                style=move || if open_rag_help.get() == Some("thinking") {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #818cf8; background:#4f46e5; color:#fff; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                } else {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #475569; background:#1e293b; color:#94a3b8; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                }
                                                title=move || if is_en() { "Show explanation" } else { "Erklärung anzeigen" }
                                                on:click=move |_| {
                                                    if open_rag_help.get() == Some("thinking") {
                                                        set_open_rag_help.set(None);
                                                    } else {
                                                        set_open_rag_help.set(Some("thinking"));
                                                    }
                                                }
                                            >
                                                "?"
                                            </button>
                                        </span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || if think_mode_sig.get() {
                                                if is_en() { "🧠 Thinking visible" } else { "🧠 Denken aktiv" }
                                            } else {
                                                if is_en() { "⚡ Direct Answer" } else { "⚡ Direkte Antwort" }
                                            }}
                                        </span>
                                    </div>
                                    {move || (open_rag_help.get() == Some("thinking")).then(|| view! {
                                        <div style="margin-bottom:0.5rem; padding:0.65rem 0.85rem; background:rgba(30,41,59,0.9); border:1px solid #334155; border-left:3px solid #6366f1; border-radius:0.45rem; font-size:0.76rem; color:#cbd5e1; line-height:1.5;">
                                            {move || if is_en() {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Controls the internal chain-of-thought (<think>...</think>) for reasoning models (DeepSeek-R1, QwQ, Qwen3):"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"🧠 Thinking ON:"</strong> " Model verifies legal requirements, exceptions, and calculations step-by-step before answering (collapsible in chat)."</div>
                                                            <div><strong style="color:#a5b4fc;">"⚡ Fast (Suppress <think>):"</strong> " Skips the internal monologue and streams the final answer immediately."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Steuert die interne Denkkette (<think>...</think>) bei Reasoning-Modellen (DeepSeek-R1, QwQ, Qwen3):"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"🧠 Denken AN:"</strong> " Das Modell prüft vor der Antwort Schritt für Schritt alle Tatbestandsmerkmale, Ausnahmen und Fristen (im Chat auf-/zuklappbar)."</div>
                                                            <div><strong style="color:#a5b4fc;">"⚡ Schnell (Ohne Denkschritte):"</strong> " Überspringt den internen Denkprozess und streamt die fertige Antwort sofort ohne Wartezeit."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            }}
                                        </div>
                                    })}
                                    <div style="display:flex; gap:0.45rem; flex-wrap:wrap;">
                                        <button
                                            type="button"
                                            style=move || if think_mode_sig.get() {
                                                "flex:1; min-width:140px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:600; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                            } else {
                                                "flex:1; min-width:140px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                            }
                                            on:click=move |_| {
                                                set_think_mode_sig.set(true);
                                                save_think_to_storage(true);
                                            }
                                        >
                                            {move || if is_en() { "🧠 Thinking ON (<think> steps)" } else { "🧠 Denken AN (inkl. Denkschritte)" }}
                                        </button>
                                        <button
                                            type="button"
                                            style=move || if !think_mode_sig.get() {
                                                "flex:1; min-width:140px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:600; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                            } else {
                                                "flex:1; min-width:140px; padding:0.4rem 0.6rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                            }
                                            on:click=move |_| {
                                                set_think_mode_sig.set(false);
                                                save_think_to_storage(false);
                                            }
                                        >
                                            {move || if is_en() { "⚡ Fast (Suppress <think>)" } else { "⚡ Schnell (Ohne Denkschritte)" }}
                                        </button>
                                    </div>
                                </div>

                                // 5. LLM Temperatur (temperature: 0.0 bis 1.0)
                                <div>
                                    <div style="font-size:0.76rem; color:#94a3b8; margin-bottom:0.35rem; display:flex; justify-content:space-between; align-items:center;">
                                        <span style="display:inline-flex; align-items:center; gap:0.4rem;">
                                            <span>{move || if is_en() { "LLM Temperature (0.0 = strict/legal, 0.7 = creative)" } else { "LLM Temperatur (0.0 = strikt/Gesetze, 0.7 = kreativ)" }}</span>
                                            <button
                                                type="button"
                                                style=move || if open_rag_help.get() == Some("temperature") {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #818cf8; background:#4f46e5; color:#fff; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                } else {
                                                    "width:18px; height:18px; border-radius:50%; border:1px solid #475569; background:#1e293b; color:#94a3b8; font-size:0.7rem; font-weight:700; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; padding:0;"
                                                }
                                                title=move || if is_en() { "Show explanation" } else { "Erklärung anzeigen" }
                                                on:click=move |_| {
                                                    if open_rag_help.get() == Some("temperature") {
                                                        set_open_rag_help.set(None);
                                                    } else {
                                                        set_open_rag_help.set(Some("temperature"));
                                                    }
                                                }
                                            >
                                                "?"
                                            </button>
                                        </span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || format!("T = {:.2}", temperature_sig.get())}
                                        </span>
                                    </div>
                                    {move || (open_rag_help.get() == Some("temperature")).then(|| view! {
                                        <div style="margin-bottom:0.5rem; padding:0.65rem 0.85rem; background:rgba(30,41,59,0.9); border:1px solid #334155; border-left:3px solid #6366f1; border-radius:0.45rem; font-size:0.76rem; color:#cbd5e1; line-height:1.5;">
                                            {move || if is_en() {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Controls sampling randomness and creativity during text generation:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"• 0.0 – 0.1 (Strict):"</strong> " Maximum determinism & factual precision — strongly recommended for statutory interpretation (GwG, KWG, AMLR, tax law)."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 0.2 (Default):"</strong> " Balanced factual accuracy with natural phrasing."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 0.4 – 0.7+ (Creative):"</strong> " Higher linguistic variety for drafting emails, summaries, or brainstorming."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! {
                                                    <div>
                                                        <div style="font-weight:600; color:#e2e8f0; margin-bottom:0.35rem;">
                                                            "Steuert den Zufallsgrad und die Wortwahl bei der Textgenerierung:"
                                                        </div>
                                                        <div style="display:flex; flex-direction:column; gap:0.25rem; padding-left:0.2rem;">
                                                            <div><strong style="color:#a5b4fc;">"• 0.0 – 0.1 (Strikt):"</strong> " Maximal deterministisch & streng am Gesetzestext orientiert – ideal für Geldwäsche-, Bankaufsichts- und Steuerrecht."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 0.2 (Default):"</strong> " Hohe Faktenpräzision bei gleichzeitig flüssiger Formulierung."</div>
                                                            <div><strong style="color:#a5b4fc;">"• 0.4 – 0.7+ (Kreativ):"</strong> " Größere sprachliche Varianz für Textentwürfe, Zusammenfassungen oder Brainstorming."</div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            }}
                                        </div>
                                    })}
                                    <div style="display:flex; gap:0.45rem; flex-wrap:wrap; align-items:center;">
                                        {[(0.0f32, "0.0 (Strikt)"), (0.1f32, "0.1"), (0.2f32, "0.2 (Default)"), (0.4f32, "0.4"), (0.7f32, "0.7 (Kreativ)")].into_iter().map(|(t_val, lbl)| {
                                            view! {
                                                <button
                                                    type="button"
                                                    style=move || if (temperature_sig.get() - t_val).abs() < 0.02 {
                                                        "flex:1; min-width:75px; padding:0.38rem 0.5rem; font-size:0.78rem; font-weight:700; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; transition:all 0.15s ease;"
                                                    } else {
                                                        "flex:1; min-width:75px; padding:0.38rem 0.5rem; font-size:0.78rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; transition:all 0.15s ease;"
                                                    }
                                                    on:click=move |_| {
                                                        set_temperature_sig.set(t_val);
                                                        save_temperature_to_storage(t_val);
                                                    }
                                                >
                                                    {lbl}
                                                </button>
                                            }
                                        }).collect_view()}
                                        <input
                                            type="range"
                                            min="0.0"
                                            max="1.2"
                                            step="0.05"
                                            style="width:110px; accent-color:#6366f1; cursor:pointer;"
                                            prop:value=move || format!("{:.2}", temperature_sig.get())
                                            on:input=move |ev| {
                                                if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                                                    set_temperature_sig.set(v);
                                                    save_temperature_to_storage(v);
                                                }
                                            }
                                        />
                                    </div>
                                </div>
                            </div>
                        })}

                        // =========================================================================
                        // TAB 2: Sprachmodell (Lokal, Remote oder Cloud-API)
                        // =========================================================================
                        {move || (settings_tab.get() == "llm").then(|| view! {
                            <div style="display:flex; flex-direction:column; gap:1.05rem;">
                            <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem; background: rgba(30, 41, 59, 0.65); border: 1px solid #475569; border-radius: 0.65rem; padding: 0.75rem 1rem;">
                                <div style="font-size: 0.82rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #818cf8;">
                                    {move || if is_en() {
                                        "🤖 Language Model (Local, RunPod GPU or Cloud API)"
                                    } else {
                                        "🤖 Sprachmodell (Lokal, RunPod GPU oder Cloud-API)"
                                    }}
                                </div>
                                <div style="display: flex; align-items: center; gap: 0.45rem; background: linear-gradient(135deg, rgba(79, 70, 229, 0.35), rgba(2, 132, 199, 0.35)); border: 1.5px solid #38bdf8; box-shadow: 0 0 12px rgba(56, 189, 248, 0.35); color: #f8fafc; padding: 0.28rem 0.75rem; border-radius: 999px; font-size: 0.8rem; font-weight: 700;">
                                    <span style="color: #34d399;">"●"</span>
                                    <span>
                                        {move || {
                                            let m = llm_model_sig.get();
                                            let p = llm_provider_sig.get();
                                            if is_en() {
                                                format!("Selected LLM: {} ({})", m, p)
                                            } else {
                                                format!("Aktuelles LLM: {} ({})", m, p)
                                            }
                                        }}
                                    </span>
                                </div>
                            </div>

                            // Schnell-Vorlagen (Presets)
                            <div>
                                <label style="font-size:0.78rem; color:#94a3b8; display:block; margin-bottom:0.4rem;">
                                    {move || if is_en() {
                                        "⚡ Quick Presets (1-click prefill – customizable below):"
                                    } else {
                                        "⚡ Schnell-Vorlagen (1-Klick Vorauswahl – Werte unten frei anpassbar):"
                                    }}
                                </label>
                                <div style="display:flex; flex-wrap:wrap; gap:0.45rem;">
                                    <button
                                        class="btn btn-secondary"
                                        style=move || {
                                            let u = llm_base_url_sig.get();
                                            let p = llm_provider_sig.get();
                                            if p != "none" && (u.contains("11434") || p == "ollama") {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem; background:#4f46e5; color:#fff; border:1.5px solid #38bdf8; box-shadow:0 0 10px rgba(79,70,229,0.45); font-weight:700;"
                                            } else {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem;"
                                            }
                                        }
                                        on:click=move |_| {
                                            set_llm_provider_sig.set("openai_compatible".to_string());
                                            set_llm_base_url_sig.set("http://host.docker.internal:11434/v1".to_string());
                                            set_llm_model_sig.set("qwen2.5:7b".to_string());
                                            set_llm_config_status.set(if is_en() {
                                                "Preset 'Local Ollama (Mac)' loaded. Click '🔄 Fetch models' or '💾 Save'.".to_string()
                                            } else {
                                                "Vorlage 'Lokales Ollama (Mac)' geladen. Klicke auf '🔄 Modelle abrufen' oder '💾 Speichern'.".to_string()
                                            });
                                        }
                                    >
                                        {move || if is_en() { "🍏 Local: Ollama (Mac)" } else { "🍏 Lokal: Ollama (Mac)" }}
                                    </button>
                                    <button
                                        class="btn btn-secondary"
                                        style=move || {
                                            let u = llm_base_url_sig.get();
                                            if u.contains(":11435") || u.contains("runpod.net") || u.contains(":8000") {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem; background:#4f46e5; color:#fff; border:1.5px solid #38bdf8; box-shadow:0 0 10px rgba(79,70,229,0.45); font-weight:700;"
                                            } else {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem;"
                                            }
                                        }
                                        on:click=move |_| {
                                            set_settings_tab.set("runpod".to_string());
                                            refresh_runpod_status();
                                        }
                                    >
                                        "🚀 RunPod GPU Cloud (SSH-Tunnel)"
                                    </button>
                                    <button
                                        class="btn btn-secondary"
                                        style=move || {
                                            if llm_base_url_sig.get().contains("api.openai.com") {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem; background:#4f46e5; color:#fff; border:1.5px solid #38bdf8; box-shadow:0 0 10px rgba(79,70,229,0.45); font-weight:700;"
                                            } else {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem;"
                                            }
                                        }
                                        on:click=move |_| {
                                            set_llm_provider_sig.set("openai_compatible".to_string());
                                            set_llm_base_url_sig.set("https://api.openai.com/v1".to_string());
                                            set_llm_model_sig.set("gpt-4o".to_string());
                                            set_available_models.set(vec![
                                                "gpt-4o".to_string(),
                                                "gpt-4o-mini".to_string(),
                                                "o3-mini".to_string(),
                                            ]);
                                            set_llm_config_status.set(if is_en() {
                                                "Preset 'OpenAI Cloud' loaded. Please enter your OpenAI API key (sk-...) below.".to_string()
                                            } else {
                                                "Vorlage 'OpenAI Cloud' geladen. Bitte unten deinen OpenAI API-Key (sk-...) eintragen.".to_string()
                                            });
                                        }
                                    >
                                        "🌐 Cloud: OpenAI"
                                    </button>
                                    <button
                                        class="btn btn-secondary"
                                        style=move || {
                                            let u = llm_base_url_sig.get();
                                            let p = llm_provider_sig.get();
                                            if u.contains("googleapis.com") || p == "gemini_native" {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem; background:#4f46e5; color:#fff; border:1.5px solid #38bdf8; box-shadow:0 0 10px rgba(79,70,229,0.45); font-weight:700;"
                                            } else {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem;"
                                            }
                                        }
                                        on:click=move |_| {
                                            set_llm_provider_sig.set("openai_compatible".to_string());
                                            set_llm_base_url_sig.set("https://generativelanguage.googleapis.com/v1beta/openai".to_string());
                                            set_llm_model_sig.set("gemini-2.5-flash".to_string());
                                            set_available_models.set(vec![
                                                "gemini-2.5-flash".to_string(),
                                                "gemini-2.5-pro".to_string(),
                                                "gemini-2.0-flash".to_string(),
                                            ]);
                                            set_llm_config_status.set(if is_en() {
                                                "Preset 'Google Gemini' loaded. Please enter your Gemini API key (AIza...) below.".to_string()
                                            } else {
                                                "Vorlage 'Google Gemini' geladen. Bitte unten deinen Gemini API-Key (AIza...) eintragen.".to_string()
                                            });
                                        }
                                    >
                                        "✨ Cloud: Google Gemini"
                                    </button>
                                    <button
                                        class="btn btn-secondary"
                                        style=move || {
                                            let u = llm_base_url_sig.get();
                                            let p = llm_provider_sig.get();
                                            if u.contains("anthropic.com") || p == "anthropic" {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem; background:#4f46e5; color:#fff; border:1.5px solid #38bdf8; box-shadow:0 0 10px rgba(79,70,229,0.45); font-weight:700;"
                                            } else {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem;"
                                            }
                                        }
                                        on:click=move |_| {
                                            set_llm_provider_sig.set("anthropic".to_string());
                                            set_llm_base_url_sig.set("https://api.anthropic.com/v1".to_string());
                                            set_llm_model_sig.set("claude-3-7-sonnet-latest".to_string());
                                            set_available_models.set(vec![
                                                "claude-3-7-sonnet-latest".to_string(),
                                                "claude-3-5-sonnet-latest".to_string(),
                                                "claude-3-5-haiku-latest".to_string(),
                                            ]);
                                            set_llm_config_status.set(if is_en() {
                                                "Preset 'Anthropic Claude' loaded. Please enter your Anthropic API key (sk-ant-...) below.".to_string()
                                            } else {
                                                "Vorlage 'Anthropic Claude' geladen. Bitte unten deinen Anthropic API-Key (sk-ant-...) eintragen.".to_string()
                                            });
                                        }
                                    >
                                        "🧠 Cloud: Anthropic Claude"
                                    </button>
                                    <button
                                        class="btn btn-secondary"
                                        style=move || {
                                            if llm_provider_sig.get() == "none" {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem; background:#4f46e5; color:#fff; border:1.5px solid #38bdf8; box-shadow:0 0 10px rgba(79,70,229,0.45); font-weight:700;"
                                            } else {
                                                "padding:0.35rem 0.65rem; font-size:0.76rem;"
                                            }
                                        }
                                        on:click=move |_| {
                                            set_llm_provider_sig.set("none".to_string());
                                            set_llm_model_sig.set("none".to_string());
                                            set_llm_config_status.set(if is_en() {
                                                "Retrieval-only mode (no LLM) selected.".to_string()
                                            } else {
                                                "Reiner Retrieval-Modus (ohne LLM) ausgewählt.".to_string()
                                            });
                                        }
                                    >
                                        {move || if is_en() { "🔍 Retrieval Only (No LLM)" } else { "🔍 Nur Retrieval (Kein LLM)" }}
                                    </button>
                                </div>
                            </div>

                            // 1. Provider-Protokoll
                            <div>
                                <label style="font-size:0.8rem; color:#94a3b8; display:block; margin-bottom:0.3rem;">
                                    {move || if is_en() { "1. API Protocol / Provider" } else { "1. API-Protokoll / Provider" }}
                                </label>
                                <select
                                    prop:value=move || llm_provider_sig.get()
                                    on:change=move |ev| set_llm_provider_sig.set(event_target_value(&ev))
                                >
                                    <option value="openai_compatible">"openai_compatible (Standard: Ollama, RunPod vLLM, llama.cpp, OpenAI, Gemini)"</option>
                                    <option value="ollama">"ollama (Native Ollama /api/chat local or remote)"</option>
                                    <option value="anthropic">"anthropic (Anthropic Claude /v1/messages API)"</option>
                                    <option value="gemini_native">"gemini_native (Google Gemini Native REST API)"</option>
                                    <option value="none">"none (SurrealDB Retrieval only, no LLM)"</option>
                                </select>
                            </div>

                            // 2. Server Base-URL
                            <div>
                                <label style="font-size:0.8rem; color:#94a3b8; display:block; margin-bottom:0.3rem;">
                                    {move || if is_en() {
                                        "2. Server Base URL (Local, LAN IP, RunPod Proxy or Cloud Endpoint)"
                                    } else {
                                        "2. Server Base-URL (Lokal, LAN-IP, RunPod Proxy oder Cloud-Endpunkt)"
                                    }}
                                </label>
                                <input
                                    type="text"
                                    placeholder="http://localhost:11434/v1 | https://pod-8000.proxy.runpod.net/v1"
                                    prop:value=move || llm_base_url_sig.get()
                                    on:input=move |ev| set_llm_base_url_sig.set(event_target_value(&ev))
                                />
                                <span style="font-size:0.72rem; color:#64748b; display:block; margin-top:0.25rem;">
                                    {move || if is_en() {
                                        "Tip: 'localhost' is automatically resolved to 'host.docker.internal' inside the Docker container."
                                    } else {
                                        "Tipp: 'localhost' wird im Docker-Container automatisch zu 'host.docker.internal' aufgelöst."
                                    }}
                                </span>
                            </div>

                            // 3. API-Key / Bearer Token
                            <div>
                                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:0.3rem;">
                                    <label style="font-size:0.8rem; color:#94a3b8;">
                                        "3. API-Key / Auth-Token"
                                    </label>
                                    {move || if llm_has_saved_key.get() {
                                        view! {
                                            <span style="font-size:0.72rem; background:rgba(16, 185, 129, 0.15); color:#34d399; border:1px solid rgba(16, 185, 129, 0.35); padding:0.12rem 0.5rem; border-radius:999px;">
                                                {move || if is_en() { "🔒 API Key stored" } else { "🔒 API-Key aktiv hinterlegt" }}
                                            </span>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <span style="font-size:0.72rem; color:#64748b;">
                                                {move || if is_en() {
                                                    "Optional for local Ollama · Required for Cloud / RunPod"
                                                } else {
                                                    "Optional bei lokalem Ollama · Pflicht bei Cloud / RunPod"
                                                }}
                                            </span>
                                        }.into_view()
                                    }}
                                </div>
                                <div style="display:flex; gap:0.5rem; align-items:center;">
                                    <input
                                        type=move || if show_api_key.get() { "text" } else { "password" }
                                        autocomplete="new-password"
                                        spellcheck="false"
                                        style="font-family: monospace; letter-spacing: 0.03em;"
                                        placeholder=move || match (llm_has_saved_key.get(), is_en()) {
                                            (true, true) => "•••••••••••••••••••••••• (Leave empty to keep current key or enter new one)",
                                            (true, false) => "•••••••••••••••••••••••• (Leer lassen zum Behalten oder neuen Key eingeben)",
                                            (false, true) => "Enter sk-... / AIzaSy... / RunPod Bearer token",
                                            (false, false) => "sk-... / AIzaSy... / RunPod Bearer-Token eingeben",
                                        }
                                        prop:value=move || llm_api_key_sig.get()
                                        on:input=move |ev| set_llm_api_key_sig.set(event_target_value(&ev))
                                    />
                                    <button
                                        type="button"
                                        class="btn btn-secondary"
                                        style="padding:0.62rem 0.8rem; font-size:0.82rem; white-space:nowrap;"
                                        title="API-Key ein-/ausblenden"
                                        on:click=move |_| set_show_api_key.update(|v| *v = !*v)
                                    >
                                        {move || match (show_api_key.get(), is_en()) {
                                            (true, true) => "🙈 Hide",
                                            (true, false) => "🙈 Verbergen",
                                            (false, true) => "👁️ Show",
                                            (false, false) => "👁️ Zeigen",
                                        }}
                                    </button>
                                </div>
                            </div>

                            // 4. Modellname + Live-Abruf vom Server
                            <div>
                                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:0.3rem;">
                                    <label style="font-size:0.8rem; color:#94a3b8;">
                                        {move || if is_en() {
                                            "4. Model Name (type custom model or fetch from server)"
                                        } else {
                                            "4. Modell-Name (Freitext eingeben oder vom Server abrufen)"
                                        }}
                                    </label>
                                    <button
                                        class="btn btn-secondary"
                                        style="padding:0.28rem 0.65rem; font-size:0.75rem; color:#38bdf8; border-color:#0284c7;"
                                        disabled=move || is_discovering_models.get()
                                        on:click=move |_| {
                                            set_is_discovering_models.set(true);
                                            set_llm_config_status.set(if is_en() {
                                                "🔍 Fetching available models from server...".to_string()
                                            } else {
                                                "🔍 Frage verfügbare Modelle vom Server ab...".to_string()
                                            });
                                            let prov = llm_provider_sig.get();
                                            let url = llm_base_url_sig.get();
                                            let key = llm_api_key_sig.get();
                                            let cur_mod = llm_model_sig.get();
                                            spawn_local(async move {
                                                let payload = serde_json::json!({
                                                    "llm_provider": prov,
                                                    "llm_base_url": url,
                                                    "llm_api_key": if key.trim().is_empty() { None } else { Some(key) },
                                                    "llm_model": cur_mod
                                                });
                                                if let Ok(resp) = Request::post("/v1/llm/models")
                                                    .header("Content-Type", "application/json")
                                                    .body(payload.to_string())
                                                    .unwrap()
                                                    .send()
                                                    .await
                                                {
                                                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                        let models: Vec<String> = serde_json::from_value(json["available_models"].clone()).unwrap_or_default();
                                                        if !models.is_empty() {
                                                            let cur = llm_model_sig.get_untracked();
                                                            if !models.iter().any(|m| m.trim().eq_ignore_ascii_case(cur.trim())) {
                                                                set_llm_model_sig.set(models[0].clone());
                                                            }
                                                            set_available_models.set(models.clone());
                                                            set_llm_config_status.set(format!("✅ {} Modell(e) geladen!", models.len()));
                                                        } else {
                                                            set_llm_config_status.set("⚠️ Keine Modell-Liste vom Server erhalten. Du kannst den Modellnamen direkt eintragen.".to_string());
                                                        }
                                                    }
                                                } else {
                                                    set_llm_config_status.set("❌ Verbindungsfehler beim Abrufen der Modell-Liste.".to_string());
                                                }
                                                set_is_discovering_models.set(false);
                                            });
                                        }
                                    >
                                        {move || match (is_discovering_models.get(), is_en()) {
                                            (true, true) => "⏳ Loading models...",
                                            (true, false) => "⏳ Lade Modelle...",
                                            (false, true) => "🔄 Fetch models from server",
                                            (false, false) => "🔄 Modelle vom Server abrufen",
                                        }}
                                    </button>
                                </div>

                                <input
                                    type="text"
                                    placeholder="qwen2.5:7b, llama3.1:8b, Qwen/Qwen2.5-32B-Instruct-AWQ, gpt-4o, gemini-2.5-flash"
                                    prop:value=move || llm_model_sig.get()
                                    on:input=move |ev| set_llm_model_sig.set(event_target_value(&ev))
                                />

                                // Klickbare Chips für alle gefundenen / vorgeschlagenen Modelle
                                <div style="display:flex; flex-wrap:wrap; gap:0.45rem; margin-top:0.55rem; max-height:130px; overflow-y:auto; padding:0.2rem 0.1rem;">
                                    <For
                                        each=move || {
                                            let mut list = available_models.get();
                                            let cur = llm_model_sig.get();
                                            if !cur.trim().is_empty()
                                                && !list.iter().any(|m| m.trim().eq_ignore_ascii_case(cur.trim()))
                                            {
                                                list.insert(0, cur);
                                            }
                                            list
                                        }
                                        key=|m| m.clone()
                                        children=move |m| {
                                            let m_click = m.clone();
                                            let m_check = m.clone();
                                            let m_label = m.clone();
                                            view! {
                                                <button
                                                    type="button"
                                                    style=move || {
                                                        let is_selected = llm_model_sig
                                                            .get()
                                                            .trim()
                                                            .eq_ignore_ascii_case(m_check.trim());
                                                        if is_selected {
                                                            "background: linear-gradient(135deg, #4f46e5, #0284c7); color: #ffffff; border: 2px solid #38bdf8; box-shadow: 0 0 12px rgba(56, 189, 248, 0.5); border-radius: 0.45rem; padding: 0.32rem 0.78rem; font-size: 0.8rem; cursor: pointer; font-weight: 700; display: inline-flex; align-items: center; gap: 0.38rem; transition: all 0.15s ease;"
                                                        } else {
                                                            "background: #0f172a; color: #cbd5e1; border: 1px solid #334155; border-radius: 0.45rem; padding: 0.32rem 0.72rem; font-size: 0.78rem; cursor: pointer; font-weight: 500; display: inline-flex; align-items: center; gap: 0.35rem; transition: all 0.15s ease;"
                                                        }
                                                    }
                                                    on:click=move |_| set_llm_model_sig.set(m_click.clone())
                                                >
                                                    {move || {
                                                        let is_selected = llm_model_sig
                                                            .get()
                                                            .trim()
                                                            .eq_ignore_ascii_case(m_label.trim());
                                                        if is_selected {
                                                            format!("🟢 ✓ {}", m_label)
                                                        } else {
                                                            format!("🤖 {}", m_label)
                                                        }
                                                    }}
                                                </button>
                                            }
                                        }
                                    />
                                </div>
                            </div>

                            <div style="display:flex; justify-content:flex-end; gap:0.65rem; margin-top:0.3rem;">
                                <button
                                    class="btn btn-secondary"
                                    on:click=move |_| set_show_llm_modal.set(false)
                                >
                                    {move || if is_en() { "Cancel" } else { "Abbrechen" }}
                                </button>
                                <button
                                    class="btn"
                                    on:click=move |_| {
                                        let prov = llm_provider_sig.get();
                                        let url = llm_base_url_sig.get();
                                        let key = llm_api_key_sig.get();
                                        let model = llm_model_sig.get();
                                        let save_msg = if is_en() {
                                            "💾 Saving & activating LLM configuration...".to_string()
                                        } else {
                                            "💾 Speichere & aktiviere LLM-Konfiguration...".to_string()
                                        };
                                        set_llm_config_status.set(save_msg);
                                        spawn_local(async move {
                                            let payload = serde_json::json!({
                                                "llm_provider": prov,
                                                "llm_base_url": url,
                                                "llm_api_key": if key.trim().is_empty() { None } else { Some(key) },
                                                "llm_model": model
                                            });
                                            match Request::post("/v1/llm/config")
                                                .header("Content-Type", "application/json")
                                                .body(payload.to_string())
                                                .unwrap()
                                                .send()
                                                .await
                                            {
                                                Ok(resp) if resp.ok() => {
                                                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                        let saved_prov = json["llm_provider"].as_str().unwrap_or("openai_compatible").to_string();
                                                        let saved_url = json["llm_base_url"].as_str().unwrap_or("").to_string();
                                                        let saved_mod = json["llm_model"].as_str().unwrap_or("").to_string();
                                                        let has_k = json["has_api_key"].as_bool().unwrap_or(false);
                                                        let mut mods: Vec<String> = serde_json::from_value(json["available_models"].clone()).unwrap_or_default();
                                                        if !saved_mod.is_empty() && !mods.contains(&saved_mod) {
                                                            mods.insert(0, saved_mod.clone());
                                                        }
                                                        set_llm_provider_sig.set(saved_prov.clone());
                                                        set_llm_base_url_sig.set(saved_url.clone());
                                                        set_llm_model_sig.set(saved_mod.clone());
                                                        set_llm_has_saved_key.set(has_k);
                                                        set_llm_api_key_sig.set(String::new());
                                                        set_available_models.set(mods);
                                                        set_backend_info.set(format!("LLM: {} ({})", saved_mod, saved_prov));
                                                        save_llm_pref_to_storage(&saved_prov, &saved_url, &saved_mod);
                                                        let activated_msg = if is_en() {
                                                            format!("✅ Activated: {} ({})!", saved_mod, saved_prov)
                                                        } else {
                                                            format!("✅ Aktiviert: {} ({})!", saved_mod, saved_prov)
                                                        };
                                                        set_llm_config_status.set(activated_msg);
                                                        // Dialog nach erfolgreichem Speichern automatisch schließen
                                                        set_show_llm_modal.set(false);
                                                    }
                                                }
                                                _ => {
                                                    let err_msg = if is_en() {
                                                        "❌ Error saving LLM configuration.".to_string()
                                                    } else {
                                                        "❌ Fehler beim Speichern der LLM-Konfiguration.".to_string()
                                                    };
                                                    set_llm_config_status.set(err_msg);
                                                }
                                            }
                                        });
                                    }
                                >
                                    {move || if is_en() { "💾 Save & Activate Configuration" } else { "💾 Konfiguration speichern & aktivieren" }}
                                </button>
                            </div>

                            {move || (!llm_config_status.get().is_empty()).then(|| view! {
                                <div class="status-banner">{llm_config_status.get()}</div>
                            })}
                            </div>
                        })}

                        // =========================================================================
                        // TAB 3: RunPod GPU Cloud (1-Klick Pod-Deployment & SSH-Tunnel für LLM & Embedding)
                        // =========================================================================
                        {move || (settings_tab.get() == "runpod").then(|| {
                            let on_ssh_file_change = move |ev: leptos::ev::Event| {
                                let Some(target) = ev.target() else { return };
                                let Ok(input_el) = target.dyn_into::<HtmlInputElement>() else { return };
                                let Some(files) = input_el.files() else { return };
                                let Some(file) = files.get(0) else { return };
                                let fname = file.name();
                                spawn_local(async move {
                                    if let Some(txt) = read_browser_file_as_text(&file).await {
                                        set_rp_ssh_key_filename.set(fname);
                                        set_rp_ssh_key_input.set(txt);
                                    }
                                });
                                input_el.set_value("");
                            };

                            let save_rp_credentials = move |_| {
                                let api_k = rp_api_key_input.get();
                                let ssh_k = rp_ssh_key_input.get();
                                let ssh_p = rp_ssh_pass_input.get();
                                save_rp_creds_to_storage(
                                    if api_k.trim().is_empty() { None } else { Some(api_k.as_str()) },
                                    if ssh_p.is_empty() { None } else { Some(ssh_p.as_str()) },
                                );
                                let gpu = rp_gpu_type.get();
                                let en_llm = rp_enable_llm.get();
                                let mod_name = rp_llm_model.get();
                                let en_emb = rp_enable_embed.get();
                                spawn_local(async move {
                                    let payload = serde_json::json!({
                                        "api_key": if api_k.trim().is_empty() { None } else { Some(api_k) },
                                        "ssh_private_key": if ssh_k.trim().is_empty() { None } else { Some(ssh_k) },
                                        "ssh_passphrase": if ssh_p.is_empty() { None } else { Some(ssh_p) },
                                        "gpu_type": gpu,
                                        "enable_llm": en_llm,
                                        "llm_model": mod_name,
                                        "enable_embedding": en_emb,
                                    });
                                    if let Ok(resp) = Request::post("/v1/runpod/config")
                                        .header("Content-Type", "application/json")
                                        .body(payload.to_string())
                                        .unwrap()
                                        .send()
                                        .await
                                    {
                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                            apply_runpod_json(&json);
                                            set_rp_api_key_input.set(String::new());
                                            set_rp_ssh_pass_input.set(String::new());
                                        }
                                    }
                                });
                            };

                            let start_rp_pod = move |_| {
                                let api_k = rp_api_key_input.get();
                                let ssh_k = rp_ssh_key_input.get();
                                let ssh_p = rp_ssh_pass_input.get();
                                save_rp_creds_to_storage(
                                    if api_k.trim().is_empty() { None } else { Some(api_k.as_str()) },
                                    if ssh_p.is_empty() { None } else { Some(ssh_p.as_str()) },
                                );
                                let gpu = rp_gpu_type.get();
                                let en_llm = rp_enable_llm.get();
                                let mod_name = rp_llm_model.get();
                                let en_emb = rp_enable_embed.get();
                                spawn_local(async move {
                                    let payload = serde_json::json!({
                                        "api_key": if api_k.trim().is_empty() { None } else { Some(api_k) },
                                        "ssh_private_key": if ssh_k.trim().is_empty() { None } else { Some(ssh_k) },
                                        "ssh_passphrase": if ssh_p.is_empty() { None } else { Some(ssh_p) },
                                        "gpu_type": gpu,
                                        "enable_llm": en_llm,
                                        "llm_model": mod_name,
                                        "enable_embedding": en_emb,
                                    });
                                    if let Ok(resp) = Request::post("/v1/runpod/start")
                                        .header("Content-Type", "application/json")
                                        .body(payload.to_string())
                                        .unwrap()
                                        .send()
                                        .await
                                    {
                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                            apply_runpod_json(&json);
                                            set_rp_api_key_input.set(String::new());
                                            set_rp_ssh_pass_input.set(String::new());
                                            refresh_runpod_status();
                                        }
                                    }
                                });
                            };

                            let stop_rp_pod = move |_| {
                                spawn_local(async move {
                                    if let Ok(resp) = Request::post("/v1/runpod/stop").send().await {
                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                            apply_runpod_json(&json);
                                            load_llm_config();
                                        }
                                    }
                                });
                            };

                            view! {
                                <div style="display:flex; flex-direction:column; gap:1.1rem;">
                                    // --- 0. Live Status & 1-Click Pod Steuerung ---
                                    <div style=move || {
                                        match rp_phase.get().as_str() {
                                            "running" => "background: rgba(6, 78, 59, 0.35); border: 1.5px solid #10b981; border-radius: 0.7rem; padding: 1.05rem 1.2rem; display: flex; flex-direction: column; gap: 0.8rem;",
                                            "deploying" | "waiting_ssh" | "loading_models" | "stopping" => "background: rgba(120, 53, 15, 0.3); border: 1.5px solid #f59e0b; border-radius: 0.7rem; padding: 1.05rem 1.2rem; display: flex; flex-direction: column; gap: 0.8rem;",
                                            "error" => "background: rgba(127, 29, 29, 0.35); border: 1.5px solid #ef4444; border-radius: 0.7rem; padding: 1.05rem 1.2rem; display: flex; flex-direction: column; gap: 0.8rem;",
                                            _ => "background: rgba(15, 23, 42, 0.65); border: 1px solid #334155; border-radius: 0.7rem; padding: 1.05rem 1.2rem; display: flex; flex-direction: column; gap: 0.8rem;",
                                        }
                                    }>
                                        <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.65rem;">
                                            <div style="display:flex; align-items:center; gap:0.6rem;">
                                                <span style="font-size:1.35rem;">
                                                    {move || match rp_phase.get().as_str() {
                                                        "running" => "🟢",
                                                        "deploying" | "waiting_ssh" | "loading_models" | "stopping" => "⏳",
                                                        "error" => "🔴",
                                                        _ => "🚀",
                                                    }}
                                                </span>
                                                <div>
                                                    <div style="font-weight:700; font-size:0.95rem; color:#f8fafc;">
                                                        {move || match rp_phase.get().as_str() {
                                                            "running" => if is_en() { "RunPod GPU Pod Active & Connected via SSH Tunnel" } else { "RunPod GPU-Pod aktiv & über SSH-Tunnel verbunden" },
                                                            "deploying" | "waiting_ssh" | "loading_models" => if is_en() { "Deploying RunPod GPU & loading models..." } else { "RunPod GPU-Pod wird gestartet & Modelle geladen..." },
                                                            "stopping" => if is_en() { "Terminating RunPod Pod..." } else { "RunPod Pod wird beendet..." },
                                                            "error" => if is_en() { "RunPod Connection Error" } else { "RunPod Hinweis / Fehler" },
                                                            _ => if is_en() { "On-Demand RunPod GPU (LLM & High-Speed Embedding)" } else { "On-Demand RunPod GPU (Sprachmodell & High-Speed Embedding)" },
                                                        }}
                                                    </div>
                                                    <div style="font-size:0.8rem; color:#cbd5e1; margin-top:0.15rem;">
                                                        {move || rp_message.get()}
                                                    </div>
                                                </div>
                                            </div>

                                            <div style="display:flex; align-items:center; gap:0.55rem; flex-wrap:wrap;">
                                                {move || {
                                                    let p = rp_phase.get();
                                                    let is_busy = p == "deploying" || p == "waiting_ssh" || p == "loading_models" || p == "stopping";
                                                    let is_running = p == "running";
                                                    view! {
                                                        <button
                                                            type="button"
                                                            class="btn"
                                                            style="background: linear-gradient(135deg, #4f46e5, #0284c7); padding: 0.55rem 1.05rem; font-size: 0.82rem;"
                                                            disabled=is_busy
                                                            on:click=start_rp_pod
                                                        >
                                                            {if is_busy {
                                                                if is_en() { "⏳ Working..." } else { "⏳ Bitte warten..." }
                                                            } else if is_running {
                                                                if is_en() { "🔄 Switch / Load Model on Active Pod" } else { "🔄 Modell auf aktivem Pod laden / wechseln" }
                                                            } else if is_en() {
                                                                "🚀 Start RunPod GPU Pod & Connect"
                                                            } else {
                                                                "🚀 RunPod GPU Pod starten & verbinden"
                                                            }}
                                                        </button>

                                                        {(is_running || is_busy || rp_pod_id.get().is_some()).then(|| view! {
                                                            <button
                                                                type="button"
                                                                class="btn-stop-subtle"
                                                                style="height:auto; padding:0.55rem 0.9rem;"
                                                                on:click=stop_rp_pod
                                                            >
                                                                <span class="stop-square"></span>
                                                                <span>
                                                                    {move || if is_en() { "Stop Pod & Return to Mac" } else { "Pod beenden & auf Mac zurückschalten" }}
                                                                </span>
                                                            </button>
                                                        })}
                                                    }
                                                }}
                                            </div>
                                        </div>

                                        // Live-Metriken des laufenden Pods
                                        {move || rp_pod_id.get().map(|pid| view! {
                                            <div style="display:flex; flex-wrap:wrap; gap:0.55rem; padding-top:0.65rem; border-top:1px solid rgba(148,163,184,0.2); font-size:0.76rem;">
                                                <span class="badge" style="background:#1e293b; color:#38bdf8; border:1px solid #334155;">
                                                    {format!("🆔 Pod: {}", pid)}
                                                </span>
                                                <span class="badge" style="background:#1e293b; color:#c7d2fe; border:1px solid #334155;">
                                                    {move || format!("🖥️ GPU: {}", rp_gpu_type.get())}
                                                </span>
                                                {move || rp_cost_hr.get().map(|c| view! {
                                                    <span class="badge" style="background:#1e293b; color:#fde68a; border:1px solid #334155;">
                                                        {format!("💰 ${:.2}/hr", c)}
                                                    </span>
                                                })}
                                                {move || rp_ssh_ep.get().map(|ep| view! {
                                                    <span class="badge" style="background:#1e293b; color:#94a3b8; border:1px solid #334155; font-family:monospace;">
                                                        {format!("🔒 SSH: {}", ep)}
                                                    </span>
                                                })}
                                                {move || rp_llm_ready.get().then(|| view! {
                                                    <span class="badge" style="background:rgba(16,185,129,0.2); color:#6ee7b7; border:1px solid #10b981;">
                                                        {format!("🤖 LLM: {} (:11435)", rp_llm_model.get())}
                                                    </span>
                                                })}
                                                {move || rp_embed_ready.get().then(|| view! {
                                                    <span class="badge" style="background:rgba(16,185,129,0.2); color:#6ee7b7; border:1px solid #10b981;">
                                                        "⚡ BGE-M3 GPU-Embed (:18081)"
                                                    </span>
                                                })}
                                            </div>
                                        })}
                                    </div>

                                    // --- 1. GPU-Auswahl & Dienste auf dem Pod (LLM + Embedding) ---
                                    <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 1.05rem; display: flex; flex-direction: column; gap: 0.95rem;">
                                        <div style="font-size: 0.8rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #818cf8;">
                                            {move || if is_en() {
                                                "1. Select GPU & Services on RunPod (Single Pod for LLM + Embedding)"
                                            } else {
                                                "1. GPU & Dienste wählen (LLM + Embedding gemeinsam auf einem Pod)"
                                            }}
                                        </div>

                                        // GPU Typ (Live-Katalog direkt aus der RunPod GraphQL API)
                                        <div style="display:flex; flex-direction:column; gap:0.55rem;">
                                            <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.5rem;">
                                                <label style="font-size:0.78rem; color:#94a3b8; margin:0;">
                                                    {move || if is_en() {
                                                        "🖥️ RunPod GPU Model (Live Prices & Stock from RunPod API):"
                                                    } else {
                                                        "🖥️ RunPod GPU-Typ (Live-Preise & Verfügbarkeit direkt von RunPod):"
                                                    }}
                                                </label>
                                                <div style="display:flex; align-items:center; gap:0.7rem; flex-wrap:wrap;">
                                                    <label style="display:inline-flex; align-items:center; gap:0.35rem; font-size:0.74rem; color:#cbd5e1; cursor:pointer; margin:0;">
                                                        <input
                                                            type="checkbox"
                                                            prop:checked=move || rp_only_available_gpus.get()
                                                            on:change=move |ev| {
                                                                let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                                set_rp_only_available_gpus.set(checked);
                                                            }
                                                        />
                                                        <span>
                                                            {move || if is_en() { "Only show currently available GPUs (🟢)" } else { "Nur aktuell freie GPUs (🟢)" }}
                                                        </span>
                                                    </label>
                                                    <button
                                                        type="button"
                                                        class="btn btn-secondary"
                                                        style="padding:0.2rem 0.55rem; font-size:0.72rem;"
                                                        disabled=move || rp_loading_gpus.get()
                                                        on:click=move |_| refresh_runpod_gpus()
                                                    >
                                                        {move || if rp_loading_gpus.get() {
                                                            if is_en() { "⏳ Loading..." } else { "⏳ Lade..." }
                                                        } else if is_en() {
                                                            "🔄 Refresh Live Prices"
                                                        } else {
                                                            "🔄 Live-Preise aktualisieren"
                                                        }}
                                                    </button>
                                                </div>
                                            </div>

                                            // Vollständiges Dropdown aller RunPod GPUs
                                            <select
                                                style="padding: 0.5rem 0.7rem; font-size: 0.82rem; font-weight: 600; background: #0b0f19; border-color: #4f46e5; color: #f8fafc;"
                                                prop:value=move || rp_gpu_type.get()
                                                on:change=move |ev| {
                                                    let val = event_target_value(&ev);
                                                    if !val.is_empty() {
                                                        set_rp_gpu_type.set(val);
                                                    }
                                                }
                                            >
                                                <For
                                                    each=move || {
                                                        let catalog = rp_gpu_catalog.get();
                                                        let only_avail = rp_only_available_gpus.get();
                                                        let cur_selected = rp_gpu_type.get();
                                                        if catalog.is_empty() {
                                                            vec![
                                                                RunpodGpuTypeItem {
                                                                    id: "NVIDIA A40".to_string(),
                                                                    display_name: "A40".to_string(),
                                                                    memory_gb: 48,
                                                                    secure_cloud: true,
                                                                    community_cloud: false,
                                                                    secure_price: Some(0.59),
                                                                    community_price: None,
                                                                    lowest_price: Some(0.59),
                                                                    secure_stock: Some("Low".to_string()),
                                                                    community_stock: None,
                                                                    available: true,
                                                                },
                                                                RunpodGpuTypeItem {
                                                                    id: "NVIDIA RTX A6000".to_string(),
                                                                    display_name: "RTX A6000".to_string(),
                                                                    memory_gb: 48,
                                                                    secure_cloud: true,
                                                                    community_cloud: true,
                                                                    secure_price: Some(0.69),
                                                                    community_price: Some(0.33),
                                                                    lowest_price: Some(0.33),
                                                                    secure_stock: Some("Low".to_string()),
                                                                    community_stock: None,
                                                                    available: true,
                                                                },
                                                                RunpodGpuTypeItem {
                                                                    id: "NVIDIA GeForce RTX 4090".to_string(),
                                                                    display_name: "RTX 4090".to_string(),
                                                                    memory_gb: 24,
                                                                    secure_cloud: true,
                                                                    community_cloud: true,
                                                                    secure_price: Some(0.89),
                                                                    community_price: Some(0.34),
                                                                    lowest_price: Some(0.34),
                                                                    secure_stock: Some("Low".to_string()),
                                                                    community_stock: Some("Low".to_string()),
                                                                    available: true,
                                                                },
                                                                RunpodGpuTypeItem {
                                                                    id: "NVIDIA L40S".to_string(),
                                                                    display_name: "L40S".to_string(),
                                                                    memory_gb: 48,
                                                                    secure_cloud: true,
                                                                    community_cloud: true,
                                                                    secure_price: Some(1.09),
                                                                    community_price: Some(0.79),
                                                                    lowest_price: Some(0.79),
                                                                    secure_stock: None,
                                                                    community_stock: None,
                                                                    available: false,
                                                                },
                                                            ]
                                                        } else {
                                                            catalog
                                                                .into_iter()
                                                                .filter(|g| !only_avail || g.available || g.id == cur_selected)
                                                                .collect()
                                                        }
                                                    }
                                                    key=|g| {
                                                        format!(
                                                            "{}_{}_{:?}_{:?}",
                                                            g.id, g.available, g.secure_price, g.community_price
                                                        )
                                                    }
                                                    children=move |g| {
                                                        let val = g.id.clone();
                                                        let is_sel = g.id == rp_gpu_type.get_untracked();
                                                        let dot = if g.available { "🟢" } else { "⚪" };
                                                        let mut price_parts = Vec::new();
                                                        if let Some(cp) = g.community_price {
                                                            let st = g
                                                                .community_stock
                                                                .as_deref()
                                                                .map(|s| format!(" [{}]", s))
                                                                .unwrap_or_default();
                                                            price_parts.push(format!("Community: ${:.2}/h{}", cp, st));
                                                        }
                                                        if let Some(sp) = g.secure_price {
                                                            let st = g
                                                                .secure_stock
                                                                .as_deref()
                                                                .map(|s| format!(" [{}]", s))
                                                                .unwrap_or_default();
                                                            price_parts.push(format!("Secure: ${:.2}/h{}", sp, st));
                                                        }
                                                        let price_str = if price_parts.is_empty() {
                                                            "—".to_string()
                                                        } else {
                                                            price_parts.join(" · ")
                                                        };
                                                        let avail_tag = if !g.available {
                                                            if is_en() { " (currently out of stock)" } else { " (aktuell belegt)" }
                                                        } else {
                                                            ""
                                                        };
                                                        let label = format!(
                                                            "{} {} GB VRAM — {} ({}) — {}{}",
                                                            dot, g.memory_gb, g.display_name, g.id, price_str, avail_tag
                                                        );
                                                        view! {
                                                            <option value=val selected=is_sel>{label}</option>
                                                        }
                                                    }
                                                />
                                            </select>

                                            // Schnellauswahl-Buttons für beliebte RAG-GPUs (mit Live-Preisen aus dem Katalog)
                                            <div style="display:flex; flex-wrap:wrap; gap:0.4rem;">
                                                {[
                                                    "NVIDIA A40",
                                                    "NVIDIA RTX A6000",
                                                    "NVIDIA GeForce RTX 4090",
                                                    "NVIDIA GeForce RTX 3090",
                                                    "NVIDIA L40S",
                                                    "NVIDIA A100 80GB PCIe",
                                                ].into_iter().map(|preset_id| {
                                                    let pid_check = preset_id.to_string();
                                                    let pid_click = preset_id.to_string();
                                                    let pid_lookup = preset_id.to_string();
                                                    view! {
                                                        <button
                                                            type="button"
                                                            style=move || if rp_gpu_type.get() == pid_check {
                                                                "padding:0.35rem 0.65rem; font-size:0.75rem; font-weight:700; border-radius:0.4rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer;"
                                                            } else {
                                                                "padding:0.35rem 0.65rem; font-size:0.75rem; font-weight:500; border-radius:0.4rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer;"
                                                            }
                                                            on:click=move |_| set_rp_gpu_type.set(pid_click.clone())
                                                        >
                                                            {move || {
                                                                if let Some(found) = rp_gpu_catalog.get().into_iter().find(|x| x.id == pid_lookup) {
                                                                    let dot = if found.available { "🟢" } else { "⚪" };
                                                                    let p_txt = match (found.community_price, found.secure_price) {
                                                                        (Some(cp), Some(sp)) => format!("${:.2}–${:.2}/h", cp.min(sp), cp.max(sp)),
                                                                        (Some(cp), None) => format!("${:.2}/h", cp),
                                                                        (None, Some(sp)) => format!("${:.2}/h", sp),
                                                                        (None, None) => String::new(),
                                                                    };
                                                                    format!("{} {} ({} GB · {})", dot, found.display_name, found.memory_gb, p_txt)
                                                                } else {
                                                                    pid_lookup.clone()
                                                                }
                                                            }}
                                                        </button>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        </div>

                                        // Checkbox A: Ollama LLM auf RunPod starten
                                        <div style="background: rgba(30, 41, 59, 0.5); border: 1px solid #334155; border-radius: 0.55rem; padding: 0.85rem; display: flex; flex-direction: column; gap: 0.7rem;">
                                            <div class="toggle-row">
                                                <input
                                                    type="checkbox"
                                                    id="rp-enable-llm"
                                                    prop:checked=move || rp_enable_llm.get()
                                                    on:change=move |ev| {
                                                        let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                        set_rp_enable_llm.set(checked);
                                                    }
                                                />
                                                <label for="rp-enable-llm" style="margin:0; font-size:0.85rem; font-weight:600; color:#f8fafc; cursor:pointer;">
                                                    {move || if is_en() {
                                                        "🤖 Run Local/Open-Weight LLM (Ollama) on RunPod GPU & switch Chat UI automatically"
                                                    } else {
                                                        "🤖 Sprachmodell (Ollama) auf RunPod GPU starten & Chat automatisch umschalten"
                                                    }}
                                                </label>
                                            </div>

                                            {move || rp_enable_llm.get().then(|| view! {
                                                <div style="display:flex; flex-direction:column; gap:0.5rem; padding-left:1.4rem;">
                                                    <label style="font-size:0.76rem; color:#94a3b8;">
                                                        {move || if is_en() {
                                                            "Model to load into RunPod GPU VRAM (click a preset chip or type any Ollama tag):"
                                                        } else {
                                                            "Modell für RunPod GPU auswählen (1-Klick-Vorschlag oder beliebigen Ollama-Namen eintippen):"
                                                        }}
                                                    </label>
                                                    <input
                                                        type="text"
                                                        placeholder="qwen3:8b, qwen3:14b, qwen3:32b, deepseek-r1:14b, deepseek-r1:32b"
                                                        prop:value=move || rp_llm_model.get()
                                                        on:input=move |ev| set_rp_llm_model.set(event_target_value(&ev))
                                                    />
                                                    <div style="display:flex; flex-wrap:wrap; gap:0.4rem;">
                                                        {[
                                                            ("qwen3:8b", "⚡ qwen3:8b (~6 GB)"),
                                                            ("qwen3:14b", "🚀 qwen3:14b (~10 GB)"),
                                                            ("qwen3:32b", "🧠 qwen3:32b (~20 GB · A40)"),
                                                            ("deepseek-r1:14b", "🔬 deepseek-r1:14b (~10 GB)"),
                                                            ("deepseek-r1:32b", "🏛️ deepseek-r1:32b (~20 GB · A40)"),
                                                            ("qwen2.5:14b", "📘 qwen2.5:14b (~10 GB)"),
                                                        ].into_iter().map(|(m_tag, m_desc)| {
                                                            let mt_check = m_tag.to_string();
                                                            let mt_click = m_tag.to_string();
                                                            view! {
                                                                <button
                                                                    type="button"
                                                                    style=move || if rp_llm_model.get().trim().eq_ignore_ascii_case(&mt_check) {
                                                                        "padding:0.3rem 0.65rem; font-size:0.75rem; font-weight:700; border-radius:0.4rem; background:#4f46e5; color:#fff; border:1px solid #38bdf8; cursor:pointer;"
                                                                    } else {
                                                                        "padding:0.3rem 0.65rem; font-size:0.75rem; font-weight:500; border-radius:0.4rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer;"
                                                                    }
                                                                    on:click=move |_| set_rp_llm_model.set(mt_click.clone())
                                                                >
                                                                    {m_desc}
                                                                </button>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            })}
                                        </div>

                                        // Checkbox B: BAAI/bge-m3 GPU-Embedding mitstarten
                                        <div style="background: rgba(30, 41, 59, 0.5); border: 1px solid #334155; border-radius: 0.55rem; padding: 0.85rem;">
                                            <div class="toggle-row">
                                                <input
                                                    type="checkbox"
                                                    id="rp-enable-embed"
                                                    prop:checked=move || rp_enable_embed.get()
                                                    on:change=move |ev| {
                                                        let checked = ev.target().unwrap().unchecked_into::<HtmlInputElement>().checked();
                                                        set_rp_enable_embed.set(checked);
                                                    }
                                                />
                                                <label for="rp-enable-embed" style="margin:0; font-size:0.85rem; font-weight:600; color:#f8fafc; cursor:pointer;">
                                                    {move || if is_en() {
                                                        "⚡ Also run BAAI/bge-m3 (FP16 CUDA) on the same Pod for ultra-fast document uploads (~1.2 GB VRAM)"
                                                    } else {
                                                        "⚡ BAAI/bge-m3 GPU-Embedding auf demselben Pod mitstarten (nur ~1,2 GB VRAM · 20× schnellerer Datei-Upload)"
                                                    }}
                                                </label>
                                            </div>
                                        </div>
                                    </div>

                                    // --- 2. RunPod API-Key & SSH-Key Konfiguration ---
                                    <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 1.05rem; display: flex; flex-direction: column; gap: 0.9rem;">
                                        <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.5rem;">
                                            <div style="font-size: 0.8rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #818cf8;">
                                                {move || if is_en() {
                                                    "2. RunPod API Key & SSH Private Key"
                                                } else {
                                                    "2. RunPod API-Key & SSH-Key (Zugangsdaten)"
                                                }}
                                            </div>
                                            <button
                                                type="button"
                                                class="btn btn-secondary"
                                                style="padding:0.32rem 0.75rem; font-size:0.76rem; border-color:#6366f1; color:#c7d2fe;"
                                                on:click=save_rp_credentials
                                            >
                                                {move || if is_en() { "💾 Verify & Save Credentials" } else { "💾 Zugangsdaten prüfen & speichern" }}
                                            </button>
                                        </div>

                                        // A. RunPod API Key
                                        <div>
                                            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:0.3rem;">
                                                <label style="font-size:0.78rem; color:#94a3b8;">
                                                    "RunPod REST API-Key (rpa_...)"
                                                </label>
                                                {move || if rp_has_api_key.get() {
                                                    view! {
                                                        <span style="font-size:0.72rem; background:rgba(16, 185, 129, 0.15); color:#34d399; border:1px solid rgba(16, 185, 129, 0.35); padding:0.1rem 0.5rem; border-radius:999px;">
                                                            {move || if is_en() { "🟢 API Key stored" } else { "🟢 RunPod API-Key hinterlegt (.env / DB)" }}
                                                        </span>
                                                    }.into_view()
                                                } else {
                                                    view! {
                                                        <span style="font-size:0.72rem; color:#f87171;">
                                                            {move || if is_en() { "⚠️ No API Key set" } else { "⚠️ Noch kein API-Key hinterlegt" }}
                                                        </span>
                                                    }.into_view()
                                                }}
                                            </div>
                                            <div style="display:flex; gap:0.5rem;">
                                                <input
                                                    type=move || if rp_show_api_key.get() { "text" } else { "password" }
                                                    autocomplete="new-password"
                                                    spellcheck="false"
                                                    style="font-family:monospace;"
                                                    placeholder=move || if rp_has_api_key.get() {
                                                        "•••••••••••••••••••••••• (Leer lassen zum Behalten oder neuen rpa_... Key eingeben)"
                                                    } else {
                                                        "rpa_..."
                                                    }
                                                    prop:value=move || rp_api_key_input.get()
                                                    on:input=move |ev| set_rp_api_key_input.set(event_target_value(&ev))
                                                />
                                                <button
                                                    type="button"
                                                    class="btn btn-secondary"
                                                    style="padding:0.55rem 0.75rem; font-size:0.78rem; white-space:nowrap;"
                                                    on:click=move |_| set_rp_show_api_key.update(|v| *v = !*v)
                                                >
                                                    {move || if rp_show_api_key.get() { "🙈" } else { "👁️" }}
                                                </button>
                                            </div>
                                        </div>

                                        // B. SSH Private Key + Datei-Auswahl + optionale Passphrase
                                        <div style="display:flex; flex-direction:column; gap:0.55rem;">
                                            <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.5rem;">
                                                <label style="font-size:0.78rem; color:#94a3b8;">
                                                    "SSH Private Key (z.B. ~/.ssh/id_ed25519_runpod)"
                                                </label>
                                                <div style="display:flex; align-items:center; gap:0.45rem;">
                                                    {move || if rp_has_ssh_key.get() || !rp_ssh_key_input.get().trim().is_empty() {
                                                        view! {
                                                            <span style="font-size:0.72rem; background:rgba(16, 185, 129, 0.15); color:#34d399; border:1px solid rgba(16, 185, 129, 0.35); padding:0.1rem 0.5rem; border-radius:999px;">
                                                                {move || if !rp_ssh_key_filename.get().is_empty() {
                                                                    format!("🟢 Datei geladen: {}", rp_ssh_key_filename.get())
                                                                } else if is_en() {
                                                                    "🟢 SSH Private Key detected".to_string()
                                                                } else {
                                                                    "🟢 SSH-Key erkannt (~/.ssh/id_ed25519_runpod)".to_string()
                                                                }}
                                                            </span>
                                                        }.into_view()
                                                    } else {
                                                        view! {
                                                            <span style="font-size:0.72rem; color:#f87171;">
                                                                "⚠️ Kein SSH-Key gefunden"
                                                            </span>
                                                        }.into_view()
                                                    }}
                                                    <input
                                                        type="file"
                                                        style="display:none;"
                                                        node_ref=rp_ssh_file_input_ref
                                                        on:change=on_ssh_file_change
                                                    />
                                                    <button
                                                        type="button"
                                                        class="btn btn-secondary"
                                                        style="padding:0.25rem 0.65rem; font-size:0.74rem;"
                                                        on:click=move |_| {
                                                            if let Some(el) = rp_ssh_file_input_ref.get() {
                                                                el.click();
                                                            }
                                                        }
                                                    >
                                                        {move || if is_en() { "📂 Select SSH Key File..." } else { "📂 SSH-Key Datei wählen..." }}
                                                    </button>
                                                </div>
                                            </div>

                                            // Passphrase für den SSH-Key (falls verschlüsselt)
                                            <div>
                                                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:0.25rem;">
                                                    <label style="font-size:0.75rem; color:#94a3b8;">
                                                        {move || if is_en() {
                                                            "SSH Key Passphrase (only if your SSH key is password-protected):"
                                                        } else {
                                                            "SSH-Key Passphrase (falls dein SSH-Key wie id_ed25519_runpod mit Passwort geschützt ist):"
                                                        }}
                                                    </label>
                                                    {move || if rp_needs_passphrase.get() {
                                                        view! {
                                                            <span style="font-size:0.72rem; background:rgba(245, 158, 11, 0.18); color:#fde68a; border:1px solid #f59e0b; padding:0.1rem 0.5rem; border-radius:999px;">
                                                                {move || if is_en() { "🔑 Passphrase required for this SSH key" } else { "🔑 Passphrase für diesen SSH-Key erforderlich" }}
                                                            </span>
                                                        }.into_view()
                                                    } else if rp_has_passphrase.get() {
                                                        view! {
                                                            <span style="font-size:0.72rem; background:rgba(16, 185, 129, 0.15); color:#34d399; border:1px solid rgba(16, 185, 129, 0.35); padding:0.1rem 0.5rem; border-radius:999px;">
                                                                {move || if is_en() { "🟢 SSH Key unlocked" } else { "🟢 SSH-Key entsperrt" }}
                                                            </span>
                                                        }.into_view()
                                                    } else {
                                                        view! { <span></span> }.into_view()
                                                    }}
                                                </div>
                                                <div style="display:flex; gap:0.5rem;">
                                                    <input
                                                        type=move || if rp_show_ssh_pass.get() { "text" } else { "password" }
                                                        autocomplete="new-password"
                                                        placeholder=move || if rp_has_passphrase.get() && !rp_needs_passphrase.get() {
                                                            "•••••••••••• (Passphrase hinterlegt & gültig)"
                                                        } else {
                                                            "Passphrase für id_ed25519_runpod eingeben (falls vorhanden)..."
                                                        }
                                                        prop:value=move || rp_ssh_pass_input.get()
                                                        on:input=move |ev| set_rp_ssh_pass_input.set(event_target_value(&ev))
                                                    />
                                                    <button
                                                        type="button"
                                                        class="btn btn-secondary"
                                                        style="padding:0.55rem 0.75rem; font-size:0.78rem;"
                                                        on:click=move |_| set_rp_show_ssh_pass.update(|v| *v = !*v)
                                                    >
                                                        {move || if rp_show_ssh_pass.get() { "🙈" } else { "👁️" }}
                                                    </button>
                                                </div>
                                            </div>

                                            <details style="font-size:0.75rem; color:#94a3b8;">
                                                <summary style="cursor:pointer; color:#a5b4fc;">
                                                    {move || if is_en() {
                                                        "📋 Optional: Paste SSH Private Key as text (-----BEGIN OPENSSH PRIVATE KEY-----)"
                                                    } else {
                                                        "📋 Optional: SSH Private Key direkt als Text einfügen (-----BEGIN OPENSSH PRIVATE KEY-----)"
                                                    }}
                                                </summary>
                                                <textarea
                                                    style="margin-top:0.4rem; min-height:95px; font-family:monospace; font-size:0.74rem;"
                                                    placeholder="-----BEGIN OPENSSH PRIVATE KEY-----\n...\n-----END OPENSSH PRIVATE KEY-----"
                                                    prop:value=move || rp_ssh_key_input.get()
                                                    on:input=move |ev| set_rp_ssh_key_input.set(event_target_value(&ev))
                                                ></textarea>
                                            </details>
                                        </div>
                                    </div>
                                </div>
                            }
                        })}

                        // =========================================================================
                        // TAB 4: SurrealDB Backup (Vollständiger Export & Import inkl. Vektoren & Graph)
                        // =========================================================================
                        {move || (settings_tab.get() == "backup").then(|| {
                            let on_db_file_change = move |ev: leptos::ev::Event| {
                                let Some(target) = ev.target() else { return };
                                let Ok(input_el) = target.dyn_into::<HtmlInputElement>() else { return };
                                let Some(files) = input_el.files() else { return };
                                let Some(file) = files.get(0) else { return };
                                let name = file.name();
                                let blob: &web_sys::Blob = file.as_ref();
                                let size_mb = blob.size() / (1024.0 * 1024.0);
                                let size_str = if size_mb >= 1.0 {
                                    format!("{:.1} MB", size_mb)
                                } else {
                                    format!("{:.0} KB", (blob.size() / 1024.0).max(1.0))
                                };
                                set_db_import_file_label.set(format!("{} ({})", name, size_str));
                                set_db_import_file.set(Some(file));
                                set_db_backup_status.set(String::new());
                                input_el.set_value("");
                            };

                            let run_db_import = move |_| {
                                let Some(file) = db_import_file.get() else {
                                    set_db_backup_status.set(if is_en() {
                                        "⚠️ Please select a .surql backup file first.".to_string()
                                    } else {
                                        "⚠️ Bitte zuerst eine .surql Backup-Datei auswählen.".to_string()
                                    });
                                    return;
                                };
                                let mode = db_import_mode.get();
                                set_is_importing_db.set(true);
                                set_db_backup_status.set(if is_en() {
                                    "⏳ Importing SurrealDB backup (documents, 1024-d HNSW vectors & knowledge graph)...".to_string()
                                } else {
                                    "⏳ Importiere SurrealDB-Backup (Dokumente, 1024-d HNSW-Vektoren & Wissensgraph)...".to_string()
                                });

                                spawn_local(async move {
                                    let form = FormData::new().unwrap();
                                    let _ = form.append_with_str("mode", &mode);
                                    let _ = form.append_with_blob_and_filename("file", file.as_ref(), &file.name());

                                    let req_res = Request::post("/v1/db/import")
                                        .body(form)
                                        .unwrap()
                                        .send()
                                        .await;

                                    match req_res {
                                        Ok(resp) if resp.ok() => {
                                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                let docs = json["total_documents"].as_u64().unwrap_or(0);
                                                let chunks = json["total_chunks"].as_u64().unwrap_or(0);
                                                let batches = json["batches_processed"].as_u64().unwrap_or(1);
                                                set_db_backup_status.set(if is_en() {
                                                    format!(
                                                        "✅ SurrealDB Backup successfully imported ({} batches)! Active knowledge base: {} documents · {} chunks.",
                                                        batches, docs, chunks
                                                    )
                                                } else {
                                                    format!(
                                                        "✅ SurrealDB-Backup erfolgreich importiert ({} Batches)! Aktive Wissensdatenbank: {} Dokumente · {} Chunks.",
                                                        batches, docs, chunks
                                                    )
                                                });
                                                set_db_import_file.set(None);
                                                set_db_import_file_label.set(String::new());
                                                refresh_project_databases();
                                                refresh_documents();
                                                refresh_crm_employees();
                                                refresh_fleet_data();
                                                refresh_webstore_data();
                                            }
                                        }
                                        Ok(resp) => {
                                            let err_txt = resp.text().await.unwrap_or_default();
                                            set_db_backup_status.set(format!("❌ Import-Fehler: {}", err_txt));
                                        }
                                        Err(e) => {
                                            set_db_backup_status.set(format!("❌ Netzwerkfehler beim Import: {}", e));
                                        }
                                    }
                                    set_is_importing_db.set(false);
                                });
                            };

                            let delete_proj_db = move |target_db: String| {
                                spawn_local(async move {
                                    let url = format!("/v1/db/projects/{}", target_db);
                                    if let Ok(resp) = Request::delete(&url).send().await {
                                        if resp.ok() {
                                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                                // Chats der gelöschten Projekt-Datenbank ebenfalls entfernen
                                                set_chat_sessions.update(|sessions| {
                                                    sessions.retain(|s| !session_belongs_to_project(s, &target_db));
                                                    save_sessions_to_storage(sessions);
                                                });
                                                if let Some(act) = json["active_db"].as_str() {
                                                    let act_str = act.to_string();
                                                    set_active_project_db.set(act_str.clone());
                                                    activate_chat_for_project(act_str);
                                                }
                                                let list: Vec<ProjectDatabaseItem> =
                                                    serde_json::from_value(json["databases"].clone()).unwrap_or_default();
                                                set_project_databases.set(list);
                                                set_db_backup_status.set(format!(
                                                    "🗑️ Projekt-Datenbank '{}' und zugehörige Projekt-Chats wurden gelöscht.",
                                                    target_db
                                                ));
                                                refresh_documents();
                                                refresh_crm_employees();
                                                refresh_fleet_data();
                                                refresh_webstore_data();
                                            }
                                        } else if let Ok(json) = resp.json::<serde_json::Value>().await {
                                            let err = json["error"].as_str().unwrap_or("Fehler beim Löschen");
                                            set_db_backup_status.set(format!("⚠️ {}", err));
                                        }
                                    }
                                });
                            };

                            view! {
                                <div style="display:flex; flex-direction:column; gap:1.1rem;">
                                    // --- Aktueller DB-Bestand ---
                                    <div style="background: rgba(30, 41, 59, 0.65); border: 1px solid #475569; border-radius: 0.65rem; padding: 0.9rem 1.15rem; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.75rem;">
                                        <div>
                                            <div style="font-size: 0.84rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #818cf8;">
                                                {move || if is_en() {
                                                    "🗄️ SurrealDB 4.0 Multi-Project Workspaces & Backup"
                                                } else {
                                                    "🗄️ SurrealDB 4.0 Multi-Projekt Datenbanken & Backup"
                                                }}
                                            </div>
                                            <div style="font-size: 0.76rem; color: #94a3b8; margin-top: 0.2rem;">
                                                {move || if is_en() {
                                                    "Maintain separate isolated databases per project (each with its own HNSW vector index, BM25 & Knowledge Graph) and switch in 0 ms."
                                                } else {
                                                    "Nutze getrennte Datenbanken pro Projekt (jeweils mit eigenem HNSW-Vektorindex, BM25 & Wissensgraph) und wechsle ohne Wartezeit."
                                                }}
                                            </div>
                                        </div>
                                        <div style="display:flex; gap:0.5rem; align-items:center;">
                                            <span class="badge" style="background:rgba(16,185,129,0.18); color:#6ee7b7; border:1px solid #10b981; padding:0.3rem 0.7rem; font-size:0.78rem;">
                                                {move || format!("🟢 DB: {}", active_project_db.get())}
                                            </span>
                                            <span class="badge" style="background:#1e1b4b; color:#c7d2fe; border:1px solid #4f46e5; padding:0.3rem 0.7rem; font-size:0.78rem;">
                                                {move || {
                                                    let d = stored_docs.get().len();
                                                    let c: usize = stored_docs.get().iter().map(|x| x.chunk_count).sum();
                                                    if is_en() {
                                                        format!("📄 {} Docs · 🧩 {} Chunks", d, c)
                                                    } else {
                                                        format!("📄 {} Dokumente · 🧩 {} Chunks", d, c)
                                                    }
                                                }}
                                            </span>
                                        </div>
                                    </div>

                                    // --- 0. Multi-Projekt Datenbanken verwalten ---
                                    <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 1.15rem; display: flex; flex-direction: column; gap: 0.9rem;">
                                        <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.75rem;">
                                            <div>
                                                <div style="font-size: 0.9rem; font-weight: 700; color: #f8fafc;">
                                                    {move || if is_en() {
                                                        "🗄️ 1. Project Databases (Parallel SurrealDB Workspaces)"
                                                    } else {
                                                        "🗄️ 1. Projekt-Datenbanken (Parallele SurrealDB-Workspaces)"
                                                    }}
                                                </div>
                                                <div style="font-size: 0.77rem; color: #94a3b8; margin-top: 0.2rem;">
                                                    {move || if is_en() {
                                                        "Create a new project database or switch between existing databases with one click."
                                                    } else {
                                                        "Lege für jedes Thema/Projekt eine eigene Datenbank an (z.B. gesetze, jabber_code, vertraege) und schalte per Klick um."
                                                    }}
                                                </div>
                                            </div>

                                            <div style="display:flex; gap:0.45rem; align-items:center;">
                                                <input
                                                    type="text"
                                                    style="width: 210px; padding: 0.45rem 0.7rem; font-size: 0.8rem;"
                                                    placeholder=move || if is_en() { "New project DB name..." } else { "Neuer Projekt-DB Name..." }
                                                    prop:value=move || new_project_db_input.get()
                                                    on:input=move |ev| set_new_project_db_input.set(event_target_value(&ev))
                                                    on:keydown=move |ev| {
                                                        if ev.key() == "Enter" {
                                                            let name = new_project_db_input.get_untracked();
                                                            if !name.trim().is_empty() {
                                                                switch_or_create_project_db(name);
                                                                set_new_project_db_input.set(String::new());
                                                            }
                                                        }
                                                    }
                                                />
                                                <button
                                                    type="button"
                                                    class="btn"
                                                    style="padding: 0.45rem 0.85rem; font-size: 0.8rem; white-space: nowrap;"
                                                    on:click=move |_| {
                                                        let name = new_project_db_input.get_untracked();
                                                        if !name.trim().is_empty() {
                                                            switch_or_create_project_db(name);
                                                            set_new_project_db_input.set(String::new());
                                                        }
                                                    }
                                                >
                                                    {move || if is_en() { "➕ Create & Switch" } else { "➕ Anlegen & Aktivieren" }}
                                                </button>
                                            </div>
                                        </div>

                                        <div style="display:grid; grid-template-columns:repeat(auto-fill, minmax(260px, 1fr)); gap:0.65rem;">
                                            <For
                                                each=move || project_databases.get()
                                                key=|db| format!("{}_{}_{}_{}", db.db_name, db.is_active, db.document_count, db.chunk_count)
                                                children=move |db| {
                                                    let name_switch = db.db_name.clone();
                                                    let name_del = db.db_name.clone();
                                                    let name_check = db.db_name.clone();
                                                    let is_def = db.is_default;
                                                    view! {
                                                        <div
                                                            style=move || if active_project_db.get() == name_check {
                                                                "background: rgba(79, 70, 229, 0.18); border: 1.5px solid #6366f1; border-radius: 0.55rem; padding: 0.75rem 0.9rem; display: flex; flex-direction: column; justify-content: space-between; gap: 0.6rem;"
                                                            } else {
                                                                "background: #0f172a; border: 1px solid #334155; border-radius: 0.55rem; padding: 0.75rem 0.9rem; display: flex; flex-direction: column; justify-content: space-between; gap: 0.6rem;"
                                                            }
                                                        >
                                                            <div style="display:flex; justify-content:space-between; align-items:center; gap:0.4rem;">
                                                                <span style="font-weight:700; font-size:0.86rem; color:#f8fafc; font-family:monospace;">
                                                                    "🗄️ " {db.db_name.clone()}
                                                                </span>
                                                                {if db.is_active {
                                                                    view! {
                                                                        <span style="font-size:0.68rem; background:rgba(16,185,129,0.2); color:#6ee7b7; border:1px solid #10b981; padding:0.1rem 0.45rem; border-radius:999px; font-weight:700;">
                                                                            {move || if is_en() { "🟢 Active" } else { "🟢 Aktiv" }}
                                                                        </span>
                                                                    }.into_view()
                                                                } else if is_def {
                                                                    view! {
                                                                        <span style="font-size:0.68rem; color:#94a3b8; border:1px solid #334155; padding:0.1rem 0.45rem; border-radius:999px;">
                                                                            "Default"
                                                                        </span>
                                                                    }.into_view()
                                                                } else {
                                                                    view! { <span></span> }.into_view()
                                                                }}
                                                            </div>

                                                            <div style="font-size:0.76rem; color:#94a3b8;">
                                                                {format!("📄 {} Dok. · 🧩 {} Chunks", db.document_count, db.chunk_count)}
                                                            </div>

                                                            <div style="display:flex; justify-content:space-between; align-items:center; gap:0.4rem; padding-top:0.35rem; border-top:1px solid rgba(51,65,85,0.45);">
                                                                {if db.is_active {
                                                                    view! {
                                                                        <span style="font-size:0.74rem; color:#a5b4fc; font-weight:600;">
                                                                            {move || if is_en() { "✓ Currently selected" } else { "✓ Aktuell ausgewählt" }}
                                                                        </span>
                                                                    }.into_view()
                                                                } else {
                                                                    view! {
                                                                        <button
                                                                            type="button"
                                                                            class="btn btn-secondary"
                                                                            style="padding:0.25rem 0.6rem; font-size:0.74rem; border-color:#6366f1; color:#c7d2fe;"
                                                                            on:click=move |_| switch_or_create_project_db(name_switch.clone())
                                                                        >
                                                                            {move || if is_en() { "Switch to DB" } else { "DB aktivieren" }}
                                                                        </button>
                                                                    }.into_view()
                                                                }}

                                                                {(!is_def).then(|| view! {
                                                                    <button
                                                                        type="button"
                                                                        class="btn btn-secondary"
                                                                        style="padding:0.22rem 0.5rem; font-size:0.72rem; color:#fca5a5; border-color:#7f1d1d;"
                                                                        title=move || if is_en() { "Delete this project database" } else { "Diese Projekt-Datenbank löschen" }
                                                                        on:click=move |_| delete_proj_db(name_del.clone())
                                                                    >
                                                                        "🗑️"
                                                                    </button>
                                                                })}
                                                            </div>
                                                        </div>
                                                    }
                                                }
                                            />
                                        </div>
                                    </div>

                                    // --- 2. Export Card ---
                                    <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 1.15rem; display: flex; flex-direction: column; gap: 0.85rem;">
                                        <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:0.75rem;">
                                            <div>
                                                <div style="font-size: 0.9rem; font-weight: 700; color: #f8fafc;">
                                                    {move || if is_en() {
                                                        format!("📥 2. Export Active Database '{}' (.surql)", active_project_db.get())
                                                    } else {
                                                        format!("📥 2. Aktive Datenbank '{}' exportieren (.surql)", active_project_db.get())
                                                    }}
                                                </div>
                                                <div style="font-size: 0.78rem; color: #94a3b8; margin-top: 0.25rem; max-width: 560px;">
                                                    {move || if is_en() {
                                                        "Downloads a complete SurrealQL snapshot (.surql) of the currently selected project database. API keys and credentials are automatically excluded."
                                                    } else {
                                                        "Lädt einen vollständigen SurrealQL-Snapshot (.surql) der aktuell ausgewählten Projekt-Datenbank herunter. API-Keys und Zugangsdaten werden automatisch ausgeschlossen."
                                                    }}
                                                </div>
                                            </div>
                                            <a
                                                href="/v1/db/export"
                                                download=move || format!("surrealdb_backup_{}.surql", active_project_db.get())
                                                class="btn"
                                                style="text-decoration:none; display:inline-flex; align-items:center; gap:0.45rem; background:linear-gradient(135deg, #4f46e5, #0284c7); white-space:nowrap;"
                                            >
                                                <span>"📥"</span>
                                                <span>
                                                    {move || if is_en() {
                                                        format!("Export '{}' (.surql)", active_project_db.get())
                                                    } else {
                                                        format!("'{}' als .surql exportieren", active_project_db.get())
                                                    }}
                                                </span>
                                            </a>
                                        </div>
                                    </div>

                                    // --- 3. Import Card ---
                                    <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 1.15rem; display: flex; flex-direction: column; gap: 0.95rem;">
                                        <div>
                                            <div style="font-size: 0.9rem; font-weight: 700; color: #f8fafc;">
                                                {move || if is_en() {
                                                    format!("📤 3. Import SurrealDB Backup into '{}'", active_project_db.get())
                                                } else {
                                                    format!("📤 3. SurrealDB-Backup in '{}' importieren (.surql)", active_project_db.get())
                                                }}
                                            </div>
                                            <div style="font-size: 0.78rem; color: #94a3b8; margin-top: 0.25rem;">
                                                {move || if is_en() {
                                                    "Restores a previously exported .surql file directly into SurrealDB in seconds (including all precomputed vector embeddings and graph relations)."
                                                } else {
                                                    "Stellt eine zuvor exportierte .surql-Datei in wenigen Sekunden direkt in SurrealDB wieder her (inkl. aller vorberechneten Vektor-Embeddings und Graph-Beziehungen)."
                                                }}
                                            </div>
                                        </div>

                                        // Import-Modus Wahl
                                        <div>
                                            <label style="font-size:0.78rem; color:#94a3b8; display:block; margin-bottom:0.4rem;">
                                                {move || if is_en() { "Import Mode:" } else { "Import-Modus wählen:" }}
                                            </label>
                                            <div style="display:flex; gap:0.5rem; flex-wrap:wrap;">
                                                <button
                                                    type="button"
                                                    style=move || if db_import_mode.get() == "merge" {
                                                        "flex:1; min-width:210px; padding:0.5rem 0.75rem; font-size:0.79rem; font-weight:700; border-radius:0.45rem; background:#4f46e5; color:#fff; border:1px solid #818cf8; cursor:pointer; text-align:left;"
                                                    } else {
                                                        "flex:1; min-width:210px; padding:0.5rem 0.75rem; font-size:0.79rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; text-align:left;"
                                                    }
                                                    on:click=move |_| set_db_import_mode.set("merge".to_string())
                                                >
                                                    {move || if is_en() {
                                                        "➕ Merge (Keep existing documents & add backup)"
                                                    } else {
                                                        "➕ Zusammenführen (Vorhandene Dokumente behalten & Backup ergänzen)"
                                                    }}
                                                </button>
                                                <button
                                                    type="button"
                                                    style=move || if db_import_mode.get() == "replace" {
                                                        "flex:1; min-width:210px; padding:0.5rem 0.75rem; font-size:0.79rem; font-weight:700; border-radius:0.45rem; background:#b91c1c; color:#fff; border:1px solid #f87171; cursor:pointer; text-align:left;"
                                                    } else {
                                                        "flex:1; min-width:210px; padding:0.5rem 0.75rem; font-size:0.79rem; font-weight:500; border-radius:0.45rem; background:#0f172a; color:#cbd5e1; border:1px solid #334155; cursor:pointer; text-align:left;"
                                                    }
                                                    on:click=move |_| set_db_import_mode.set("replace".to_string())
                                                >
                                                    {move || if is_en() {
                                                        "🧹 Clean Restore (Clear database before importing backup)"
                                                    } else {
                                                        "🧹 Komplett ersetzen (Datenbank vor dem Import leeren)"
                                                    }}
                                                </button>
                                            </div>
                                        </div>

                                        // Datei auswählen & Import starten
                                        <div style="display:flex; align-items:center; justify-content:space-between; flex-wrap:wrap; gap:0.65rem; background:#0f172a; border:1px solid #334155; border-radius:0.55rem; padding:0.8rem 1rem;">
                                            <div style="display:flex; align-items:center; gap:0.6rem; flex-wrap:wrap;">
                                                <input
                                                    type="file"
                                                    accept=".surql,.sql"
                                                    style="display:none;"
                                                    node_ref=db_import_input_ref
                                                    on:change=on_db_file_change
                                                />
                                                <button
                                                    type="button"
                                                    class="btn btn-secondary"
                                                    style="padding:0.45rem 0.85rem; font-size:0.8rem;"
                                                    disabled=move || is_importing_db.get()
                                                    on:click=move |_| {
                                                        if let Some(el) = db_import_input_ref.get() {
                                                            el.click();
                                                        }
                                                    }
                                                >
                                                    {move || if is_en() { "📂 Select .surql File..." } else { "📂 .surql Backup-Datei wählen..." }}
                                                </button>
                                                <span style="font-size:0.8rem; color:#cbd5e1;">
                                                    {move || if db_import_file_label.get().is_empty() {
                                                        if is_en() { "No file selected".to_string() } else { "Noch keine Datei ausgewählt".to_string() }
                                                    } else {
                                                        format!("📄 {}", db_import_file_label.get())
                                                    }}
                                                </span>
                                            </div>

                                            <button
                                                type="button"
                                                class="btn"
                                                disabled=move || is_importing_db.get() || db_import_file.get().is_none()
                                                on:click=run_db_import
                                            >
                                                {move || if is_importing_db.get() {
                                                    if is_en() { "⏳ Importing..." } else { "⏳ Importiere Backup..." }
                                                } else if is_en() {
                                                    "🚀 Start SurrealDB Import"
                                                } else {
                                                    "🚀 Backup jetzt in SurrealDB importieren"
                                                }}
                                            </button>
                                        </div>

                                        {move || (!db_backup_status.get().is_empty()).then(|| view! {
                                            <div class="status-banner">{db_backup_status.get()}</div>
                                        })}
                                    </div>
                                </div>
                            }
                        })}

                        // =========================================================================
                        // TAB 5: Allgemein & System (Sprache, Aktive Schnittstellen & Ports)
                        // =========================================================================
                        {move || (settings_tab.get() == "system").then(|| view! {
                            <div style="display:flex; flex-direction:column; gap:1.05rem;">
                                // UI-Sprache (Deutsch / English)
                                <div style="display: flex; justify-content: space-between; align-items: center; background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 0.85rem 1.05rem;">
                                    <div>
                                        <div style="font-size: 0.85rem; font-weight: 600; color: #e2e8f0;">
                                            {move || if is_en() { "🌐 UI Language / Sprache" } else { "🌐 Chat-UI Sprache / Language" }}
                                        </div>
                                        <div style="font-size: 0.74rem; color: #94a3b8; margin-top: 0.15rem;">
                                            {move || if is_en() { "Switch all menus, tooltips, and status messages between German and English." } else { "Wechselt alle Menüs, Erklärungen und Statusanzeigen zwischen Deutsch und Englisch." }}
                                        </div>
                                    </div>
                                    <div style="display: flex; gap: 0.45rem;">
                                        <button
                                            class="btn btn-secondary"
                                            style=move || if !is_en() {
                                                "padding: 0.38rem 0.85rem; font-size: 0.8rem; background: #4f46e5; color: #fff; border-color: #818cf8;"
                                            } else {
                                                "padding: 0.38rem 0.85rem; font-size: 0.8rem;"
                                            }
                                            on:click=move |_| {
                                                set_ui_lang.set("de".to_string());
                                                save_lang_to_storage("de");
                                            }
                                        >
                                            "🇩🇪 Deutsch"
                                        </button>
                                        <button
                                            class="btn btn-secondary"
                                            style=move || if is_en() {
                                                "padding: 0.38rem 0.85rem; font-size: 0.8rem; background: #4f46e5; color: #fff; border-color: #818cf8;"
                                            } else {
                                                "padding: 0.38rem 0.85rem; font-size: 0.8rem;"
                                            }
                                            on:click=move |_| {
                                                set_ui_lang.set("en".to_string());
                                                save_lang_to_storage("en");
                                            }
                                        >
                                            "🇬🇧 English"
                                        </button>
                                    </div>
                                </div>

                                // Aktive Ports & Schnittstellen-Übersicht
                                <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 1.05rem; display: flex; flex-direction: column; gap: 0.65rem;">
                                    <div style="font-size: 0.82rem; font-weight: 700; color: #818cf8; text-transform: uppercase; letter-spacing: 0.04em;">
                                        {move || if is_en() {
                                            "🔌 Active Endpoints & Port Configuration"
                                        } else {
                                            "🔌 Aktive Schnittstellen & Port-Konfiguration"
                                        }}
                                    </div>
                                    <div style="background:#0f172a; border:1px solid #334155; border-radius:0.5rem; padding:0.85rem; font-family:monospace; font-size:0.78rem; line-height:1.7; color:#e2e8f0;">
                                        {
                                            let origin = web_sys::window()
                                                .and_then(|w| w.location().origin().ok())
                                                .unwrap_or_else(|| "http://localhost:8080".to_string());
                                            view! {
                                                <div>"• 🦀 Leptos WASM Chat-UI  : " {origin.clone()} "/"</div>
                                                <div>"• Health-Check            : " {origin.clone()} "/health"</div>
                                                <div>"• Ingestion (JSON)        : POST " {origin.clone()} "/v1/ingest"</div>
                                                <div>"• File Upload (PDF/XML/MD): POST " {origin.clone()} "/v1/ingest/file"</div>
                                                <div>"• Hybrid + Graph Retrieval: POST " {origin.clone()} "/v1/retrieve"</div>
                                                <div>"• OpenAI Chat Completion  : POST " {origin.clone()} "/v1/chat/completions"</div>
                                                <div>"• RunPod Cloud GPU API    : GET/POST " {origin.clone()} "/v1/runpod/{status,start,stop}"</div>
                                                <div>"• MCP Server (Tool-Call)  : POST " {origin} "/mcp"</div>
                                            }
                                        }
                                        <div style="margin-top:0.6rem; padding-top:0.55rem; border-top:1px dashed #334155; color:#38bdf8; font-family:sans-serif; font-size:0.76rem;">
                                            {move || if is_en() {
                                                "💡 If port 8080 is busy, ./setup.sh automatically fails over to the next free port – or specify directly: "
                                            } else {
                                                "💡 Falls Port 8080 belegt ist, weicht ./setup.sh beim Start automatisch auf den nächsten freien Port aus – oder du gibst deinen Wunsch-Port direkt an: "
                                            }}
                                            <code style="background:#1e293b; padding:0.1rem 0.35rem; border-radius:4px; color:#f8fafc;">
                                                "./setup.sh --port 8090"
                                            </code>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        })}
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            })}

            // --- Modal-Dialog: Integrierter Fullscreen Dokument-Viewer (PDF / XML / Markdown / Text) ---
            {move || viewing_pdf_doc.get().map(|(pdf_doc_id, pdf_doc_title)| {
                let pdf_url = format!("/v1/documents/{}/file", pdf_doc_id);
                let pdf_url_tab = pdf_url.clone();
                let (modal_icon, _, _, _) = doc_format_labels(&pdf_doc_title, false);
                let title_for_close = pdf_doc_title.clone();
                view! {
                    <div
                        class="modal-backdrop fullscreen"
                        style="z-index: 1200;"
                        on:click=move |_| set_viewing_pdf_doc.set(None)
                    >
                        <div
                            class="modal-dialog fullscreen"
                            on:click=move |ev| ev.stop_propagation()
                        >
                            <div class="modal-header">
                                <h2 style="display:flex; align-items:center; gap:0.55rem;">
                                    <span>{modal_icon}</span>
                                    <span>{pdf_doc_title}</span>
                                    <span style="font-size:0.75rem; font-family:monospace; color:#64748b; font-weight:400;">
                                        {format!("({})", pdf_doc_id)}
                                    </span>
                                </h2>
                                <div style="display:flex; align-items:center; gap:0.55rem;">
                                    <a
                                        href=pdf_url_tab
                                        target="_blank"
                                        class="btn btn-secondary"
                                        style="padding: 0.35rem 0.75rem; font-size: 0.8rem; text-decoration: none; display: inline-flex; align-items: center; gap: 0.35rem;"
                                    >
                                        {move || if is_en() { "↗️ Open in New Tab" } else { "↗️ In neuem Tab öffnen" }}
                                    </a>
                                    <button
                                        class="btn btn-secondary"
                                        style="padding: 0.35rem 0.75rem; font-size: 0.8rem;"
                                        on:click=move |_| set_viewing_pdf_doc.set(None)
                                    >
                                        {move || doc_format_labels(&title_for_close, is_en()).3}
                                    </button>
                                </div>
                            </div>
                            <div style="flex:1; width:100%; height:100%; background:#0f172a; display:flex; flex-direction:column; overflow:hidden;">
                                <iframe
                                    src=pdf_url
                                    style="width:100%; height:100%; flex:1; border:none; background:#1e293b;"
                                    title="Document Viewer"
                                ></iframe>
                            </div>
                        </div>
                    </div>
                }
            })}

            // --- Modal-Dialog: Integrierter Fullscreen OpenStreetMap GPS-Tour-Viewer ---
            {move || viewing_tour_map.get().map(|(map_tour_id, map_tour_title, gmaps_url)| {
                let osm_url = format!("/v1/fleet/tours/{}/map", map_tour_id);
                let osm_url_tab = osm_url.clone();
                view! {
                    <div
                        class="modal-backdrop fullscreen"
                        style="z-index: 1250;"
                        on:click=move |_| set_viewing_tour_map.set(None)
                    >
                        <div
                            class="modal-dialog fullscreen"
                            on:click=move |ev| ev.stop_propagation()
                        >
                            <div class="modal-header">
                                <h2 style="display:flex; align-items:center; gap:0.55rem;">
                                    <span>"🗺️"</span>
                                    <span>"OpenStreetMap GPS-Tour: " {map_tour_title}</span>
                                </h2>
                                <div style="display:flex; align-items:center; gap:0.55rem;">
                                    {(!gmaps_url.is_empty()).then(|| view! {
                                        <a
                                            href=gmaps_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn"
                                            style="padding: 0.35rem 0.8rem; font-size: 0.8rem; text-decoration: none; display: inline-flex; align-items: center; gap: 0.35rem; background: linear-gradient(135deg, #2563eb 0%, #1d4ed8 100%);"
                                        >
                                            "🌍 In Google Maps öffnen ↗️"
                                        </a>
                                    })}
                                    <a
                                        href=osm_url_tab
                                        target="_blank"
                                        class="btn btn-secondary"
                                        style="padding: 0.35rem 0.75rem; font-size: 0.8rem; text-decoration: none; display: inline-flex; align-items: center; gap: 0.35rem;"
                                    >
                                        "↗️ Karte in neuem Tab"
                                    </a>
                                    <button
                                        class="btn btn-secondary"
                                        style="padding: 0.35rem 0.75rem; font-size: 0.8rem;"
                                        on:click=move |_| set_viewing_tour_map.set(None)
                                    >
                                        "✕ Karte schließen"
                                    </button>
                                </div>
                            </div>
                            <div style="flex:1; width:100%; height:100%; background:#0f172a; display:flex; flex-direction:column; overflow:hidden;">
                                <iframe
                                    src=osm_url
                                    style="width:100%; height:100%; flex:1; border:none; background:#0b0f19;"
                                    title="OpenStreetMap GPS Tour Viewer"
                                ></iframe>
                            </div>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}
