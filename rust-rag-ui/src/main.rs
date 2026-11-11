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
}

const STORAGE_KEY_SESSIONS: &str = "rust_rag_chat_sessions_v1";
const STORAGE_KEY_LANG: &str = "rust_rag_ui_lang_v1";
const STORAGE_KEY_RERANKER: &str = "rust_rag_reranker_v1";
const STORAGE_KEY_TOP_N: &str = "rust_rag_top_n_v1";
const STORAGE_KEY_USE_RAG: &str = "rust_rag_use_rag_v1";
const STORAGE_KEY_NUM_CTX: &str = "rust_rag_num_ctx_v1";
const STORAGE_KEY_THINK: &str = "rust_rag_think_mode_v1";
const STORAGE_KEY_LLM_MODEL: &str = "rust_rag_llm_model_v1";
const STORAGE_KEY_LLM_PROVIDER: &str = "rust_rag_llm_provider_v1";
const STORAGE_KEY_LLM_URL: &str = "rust_rag_llm_url_v1";

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

    // Speichert die Nachrichten des aktuell geöffneten Chats in `chat_sessions` + `localStorage`
    let persist_current_chat = move |custom_title: Option<String>| {
        let cur_id = active_session_id.get_untracked();
        let stored_msgs = ui_to_stored_messages(&messages.get_untracked());
        set_chat_sessions.update(|sessions| {
            if let Some(sess) = sessions.iter_mut().find(|s| s.id == cur_id) {
                sess.messages = stored_msgs;
                if let Some(new_title) = custom_title {
                    if sess.title == "Neuer Chat" || sess.title.starts_with("Chat #") {
                        sess.title = new_title;
                    }
                }
            }
            save_sessions_to_storage(sessions);
        });
    };

    let (user_input, set_user_input) = create_signal(String::new());
    let (use_rag, set_use_rag) = create_signal(load_use_rag_from_storage());
    let (reranker_mode, set_reranker_mode) = create_signal(load_reranker_from_storage());
    let (top_n, set_top_n) = create_signal(load_top_n_from_storage());
    let (num_ctx_sig, set_num_ctx_sig) = create_signal(load_num_ctx_from_storage());
    let (think_mode_sig, set_think_mode_sig) = create_signal(load_think_from_storage());
    let (is_loading, set_is_loading) = create_signal(false);

    // --- Temporärer Chat-Anhang (📎 ohne Datenbank-Speicherung) ---
    let (chat_attachment, set_chat_attachment) = create_signal(None::<AttachedChatDoc>);
    let (attachment_only, set_attachment_only) = create_signal(true);
    let (is_extracting_attachment, set_is_extracting_attachment) = create_signal(false);
    let (attachment_error, set_attachment_error) = create_signal(None::<String>);
    let chat_attach_input_ref = create_node_ref::<leptos::html::Input>();

    // --- Modal & Ingestion Signals ---
    let (show_ingest_modal, set_show_ingest_modal) = create_signal(false);
    let (is_modal_fullscreen, set_is_modal_fullscreen) = create_signal(false);
    let (doc_search_term, set_doc_search_term) = create_signal(String::new());
    let (selected_category_filter, set_selected_category_filter) = create_signal("all".to_string());
    let (is_filter_expanded, set_is_filter_expanded) = create_signal(false);
    // Tab innerhalb des Dialogs: "files" (Multi-File Upload), "text" (Manueller Text) oder "docs" (Dokumenten-Verwaltung)
    let (modal_tab, set_modal_tab) = create_signal("files".to_string());
    let (selected_file_names, set_selected_file_names) = create_signal(Vec::<(String, String)>::new());
    let (selected_files, set_selected_files) = create_signal(Vec::<web_sys::File>::new());
    let (is_uploading, set_is_uploading) = create_signal(false);

    let (doc_category, set_doc_category) = create_signal(String::new());
    let (replace_revision, set_replace_revision) = create_signal(true);
    let (extract_graph, set_extract_graph) = create_signal(false);
    let (stored_docs, set_stored_docs) = create_signal(Vec::<DocumentItem>::new());

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
        });
    };

    let saved_llm_pref = load_llm_pref_from_storage();
    let initial_llm_model = saved_llm_pref.as_ref().map(|p| p.2.clone()).unwrap_or_else(|| "qwen2.5:7b".to_string());
    let initial_llm_prov = saved_llm_pref.as_ref().map(|p| p.0.clone()).unwrap_or_else(|| "openai_compatible".to_string());
    let initial_llm_url = saved_llm_pref.as_ref().map(|p| p.1.clone()).unwrap_or_else(|| "http://host.docker.internal:11434/v1".to_string());

    let (backend_info, set_backend_info) = create_signal("Verbinde...".to_string());
    let (show_llm_modal, set_show_llm_modal) = create_signal(false);
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

                    // Check if local storage has a preference that differs from server default (e.g. after container recreate)
                    let current_storage_model = load_llm_pref_from_storage();
                    let (eff_prov, eff_url, eff_mod) = match current_storage_model {
                        Some((p, u, m)) if m != model => {
                            // Automatically sync back to server so the server uses the user's preferred model
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

    create_effect(move |_| {
        load_llm_config();
    });

    // --- Handler: Dateiauswahl (Dateien oder ganzer Ordner) geändert ---
    let on_files_selected = move |ev: leptos::ev::Event| {
        let Some(target) = ev.target() else { return };
        let Ok(dom_input) = target.dyn_into::<HtmlInputElement>() else { return };
        let Some(files) = dom_input.files() else { return };

        let mut list = Vec::new();
        let mut file_vec = Vec::new();
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
                list.push((display_name, format!("{} KB", size_kb)));
                file_vec.push(f);
            }
        }
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

        set_is_uploading.set(true);
        let category_val = doc_category.get();
        let replace_rev_val = replace_revision.get();
        let extract_graph_val = extract_graph.get();
        let is_en_val = is_en();

        spawn_local(async move {
            let total_files = file_vec.len();
            let mut success_files = 0usize;
            let mut skipped_files = 0usize;
            let mut replaced_files = 0usize;
            let mut total_chunks = 0u64;
            let mut total_entities = 0u64;
            let mut total_relations = 0u64;
            let mut error_messages: Vec<String> = Vec::new();

            for (idx, file) in file_vec.into_iter().enumerate() {
                let filename = file.name();
                let rel_path = get_file_relative_path(&file);
                let auto_folder = extract_category_from_path(&rel_path);
                let effective_category = match (category_val.trim(), auto_folder) {
                    (cat, Some(folder)) if !cat.is_empty() => format!("{} / {}", cat, folder),
                    ("", Some(folder)) => folder,
                    (cat, None) if !cat.is_empty() => cat.to_string(),
                    _ => String::new(),
                };

                let step_desc = if is_en_val {
                    if extract_graph_val {
                        "Extracting text (incl. OCR), embeddings & 🕸️ Knowledge Graph relations..."
                    } else {
                        "Checking content hash, extracting text (incl. OCR) & computing embeddings..."
                    }
                } else {
                    if extract_graph_val {
                        "Extrahiere Text (inkl. OCR), Embeddings & 🕸️ Wissensgraph-Beziehungen..."
                    } else {
                        "Prüfe Inhalts-Hash, extrahiere Text (inkl. OCR) & berechne Embeddings..."
                    }
                };
                let cat_prefix = if !effective_category.is_empty() {
                    format!("[{}] ", effective_category)
                } else {
                    String::new()
                };
                let file_prefix = if is_en_val { "File" } else { "Datei" };
                set_ingest_status.set(format!(
                    "⏳ {} {}/{} ('{}{}'): {}",
                    file_prefix,
                    idx + 1,
                    total_files,
                    cat_prefix,
                    filename,
                    step_desc
                ));

                let form_data = FormData::new().unwrap();
                if !effective_category.is_empty() {
                    let _ = form_data.append_with_str("category", &effective_category);
                }
                let _ = form_data.append_with_str(
                    "replace_revision",
                    if replace_rev_val { "true" } else { "false" },
                );
                let _ = form_data.append_with_str(
                    "extract_graph",
                    if extract_graph_val { "true" } else { "false" },
                );
                let _ = form_data.append_with_blob_and_filename("files", &file, &filename);

                match Request::post("/v1/ingest/file")
                    .body(form_data)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        let data: serde_json::Value = resp.json().await.unwrap_or_default();
                        let skipped = data["skipped_duplicates"].as_u64().unwrap_or(0) as usize;
                        let replaced = data["replaced_revisions"].as_u64().unwrap_or(0) as usize;
                        let chunks = data["chunks_indexed"].as_u64().unwrap_or(0);
                        let g_ent = data["graph_entities_extracted"].as_u64().unwrap_or(0);
                        let g_rel = data["graph_relations_extracted"].as_u64().unwrap_or(0);
                        total_entities += g_ent;
                        total_relations += g_rel;
                        replaced_files += replaced;
                        if skipped > 0 {
                            skipped_files += skipped;
                        } else {
                            total_chunks += chunks;
                            success_files += 1;
                        }
                    }
                    Ok(resp) => {
                        let err = resp.text().await.unwrap_or_default();
                        error_messages.push(format!("{}: {}", filename, err));
                    }
                    Err(e) => {
                        let net_lbl = if is_en_val { "Network" } else { "Netzwerk" };
                        error_messages.push(format!("{} ({}: {})", filename, net_lbl, e));
                    }
                }
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

    // --- Handler: Temporäre Datei (📎) im Chat-Fenster auswählen (nur im RAM, ohne DB-Speicherung) ---
    let on_chat_attachment_selected = move |ev: leptos::ev::Event| {
        let Some(target) = ev.target() else { return };
        let Ok(dom_input) = target.dyn_into::<HtmlInputElement>() else { return };
        let Some(files) = dom_input.files() else { return };
        let Some(file) = files.get(0) else { return };

        let filename = file.name();
        set_is_extracting_attachment.set(true);
        set_attachment_error.set(None);

        spawn_local(async move {
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
                    set_chat_attachment.set(Some(AttachedChatDoc {
                        filename,
                        text,
                        char_count,
                        chunk_count,
                    }));
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
            if let Some(el) = chat_attach_input_ref.get() {
                el.set_value("");
            }
            set_is_extracting_attachment.set(false);
        });
    };

    // --- Handler: Chat-Frage absenden (Echtzeit-Streaming) ---
    let send_message = move || {
        let prompt = user_input.get();
        if prompt.trim().is_empty() || is_loading.get() || is_extracting_attachment.get() {
            return;
        }

        set_user_input.set(String::new());
        set_is_loading.set(true);

        let active_attachment = chat_attachment.get();
        let attach_only_mode = active_attachment.is_some() && attachment_only.get();

        let user_msg_id = next_msg_id.get();
        let assistant_msg_id = user_msg_id + 1;
        next_msg_id.set(assistant_msg_id + 1);

        let displayed_user_prompt = if let Some(ref att) = active_attachment {
            let mode_badge = if is_en() {
                if attach_only_mode { "Attachment only" } else { "Attachment + DB" }
            } else {
                if attach_only_mode { "Nur Anhang" } else { "Anhang + DB" }
            };
            format!("📎 [{} · {}] {}", att.filename, mode_badge, prompt)
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
            if active_attachment.is_some() && attach_only_mode {
                "📎 Analyzing attached file in RAM...".to_string()
            } else {
                "🔍 Searching in SurrealDB & starting reranking...".to_string()
            }
        } else {
            if active_attachment.is_some() && attach_only_mode {
                "📎 Analysiere angehängte Datei im Arbeitsspeicher...".to_string()
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
        let active_llm_model = llm_model_sig.get();

        spawn_local(async move {
            let attachment_json = active_attachment.map(|att| {
                serde_json::json!({
                    "filename": att.filename,
                    "text": att.text
                })
            });

            let body = serde_json::json!({
                "model": active_llm_model,
                "use_rag": rag_enabled,
                "reranker": selected_reranker,
                "top_n": k,
                "num_ctx": current_ctx,
                "think": current_think,
                "attachment": attachment_json,
                "attachment_only": attach_only_mode,
                "messages": history_payload
            });

            match Request::post("/v1/chat/stream")
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .unwrap()
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    if let Some(body_stream) = resp.body() {
                        let reader: web_sys::ReadableStreamDefaultReader =
                            body_stream.get_reader().unchecked_into();

                        let mut raw_bytes: Vec<u8> = Vec::new();
                        let mut first_token = true;
                        let mut base_metrics_str = String::new();

                        loop {
                            let read_promise = reader.read();
                            let Ok(js_result) =
                                wasm_bindgen_futures::JsFuture::from(read_promise).await
                            else {
                                break;
                            };

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
                                                assistant_content
                                                    .set(format!("❌ Fehler: {}", err));
                                                first_token = false;
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
                    let err_txt = resp.text().await.unwrap_or_default();
                    assistant_content.set(format!("❌ Fehler vom Server: {}", err_txt));
                }
                Err(e) => {
                    assistant_content.set(format!("❌ Verbindungsfehler: {}", e));
                }
            }
            persist_current_chat(None);
            set_is_loading.set(false);
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

        let new_session = ChatSession {
            id: new_id,
            title: "Neuer Chat".to_string(),
            messages: vec![default_welcome_stored()],
        };
        let ui_msgs = stored_to_ui_messages(&new_session.messages, next_msg_id);

        set_chat_sessions.update(|sessions| {
            sessions.insert(0, new_session);
            save_sessions_to_storage(sessions);
        });
        set_active_session_id.set(new_id);
        set_messages.set(ui_msgs);
        set_chat_attachment.set(None);
        set_attachment_error.set(None);
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
            set_active_session_id.set(target_id);
            set_messages.set(ui_msgs);
            set_chat_attachment.set(None);
            set_attachment_error.set(None);
        }
    };

    // --- Handler: Einen Chat aus der Seitenleiste löschen ---
    let delete_chat_session = move |delete_id: usize| {
        if is_loading.get() {
            return;
        }
        persist_current_chat(None);
        let mut sessions = chat_sessions.get();
        sessions.retain(|s| s.id != delete_id);

        if sessions.is_empty() {
            let fresh_id = next_session_id.get();
            set_next_session_id.set(fresh_id + 1);
            sessions.push(ChatSession {
                id: fresh_id,
                title: "Neuer Chat".to_string(),
                messages: vec![default_welcome_stored()],
            });
        }

        let current_active = active_session_id.get();
        if current_active == delete_id || !sessions.iter().any(|s| s.id == current_active) {
            let next_active = sessions.first().unwrap().clone();
            let ui_msgs = stored_to_ui_messages(&next_active.messages, next_msg_id);
            set_active_session_id.set(next_active.id);
            set_messages.set(ui_msgs);
            set_chat_attachment.set(None);
            set_attachment_error.set(None);
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

                    <nav class="nav-menu">
                        <button
                            class="nav-btn nav-btn-primary"
                            on:click=move |_| create_new_chat()
                        >
                            <span>"💬"</span>
                            <span>{move || if is_en() { "+ New Chat" } else { "+ Neuer Chat" }}</span>
                        </button>
                    </nav>

                    // --- Multi-Chat Sitzungs-Panel ---
                    <div class="chat-sessions-section">
                        <div class="chat-sessions-header">
                            <span>
                                {move || if is_en() {
                                    format!("Your Chats ({})", chat_sessions.get().len())
                                } else {
                                    format!("Deine Chats ({})", chat_sessions.get().len())
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
                                    set_chat_attachment.set(None);
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
                                each=move || chat_sessions.get()
                                key=|sess| format!("{}_{}_{}", sess.id, sess.title, sess.messages.len())
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
                                set_show_ingest_modal.set(true);
                            }
                        >
                            <span>"🗂️"</span>
                            <span class="icon-caption">{move || if is_en() { "Docs" } else { "Dokumente" }}</span>
                        </button>

                        <button
                            class="sidebar-icon-btn"
                            title=move || if is_en() { "Settings (Language, RAG, Reranker, LLM & Cloud)" } else { "Einstellungen (Sprache, RAG, Reranker, LLM & Cloud)" }
                            on:click=move |_| {
                                set_llm_config_status.set(String::new());
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

                    <div style="display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0;">
                        <button
                            class="btn btn-secondary"
                            style="padding: 0.35rem 0.75rem; font-size: 0.76rem; display: flex; align-items: center; gap: 0.5rem; border-radius: 999px;"
                            title=move || if is_en() { "Click to configure RAG, Reranker, Context Window & LLM settings" } else { "Klicken, um RAG-, Reranker-, Kontextfenster- und LLM-Einstellungen anzupassen" }
                            on:click=move |_| {
                                set_llm_config_status.set(String::new());
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
                            <span style="color:#a5b4fc; font-weight:600;">
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
                                    format!("{} · Top-{} · {} · {}", r, top_n.get(), ctx_str, think_ico)
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
                                                        view! {
                                                            <div class="source-item">
                                                                <div>
                                                                    <span class="badge">
                                                                        {move || if is_en() {
                                                                            format!("Source #{}", i + 1)
                                                                        } else {
                                                                            format!("Quelle #{}", i + 1)
                                                                        }}
                                                                    </span>
                                                                    <strong>{src.doc_title}</strong>
                                                                    <span style="color:#94a3b8; margin-left:8px;">
                                                                        {format!("(Chunk #{}, Score: {:.4})", src.chunk_index, score)}
                                                                    </span>
                                                                </div>
                                                                <div style="margin-top:6px; color:#cbd5e1;">{src.content}</div>
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
                    } else if let Some(att) = chat_attachment.get() {
                        view! {
                            <div style="margin: 0 1.25rem 0.5rem 1.25rem; padding: 0.55rem 0.9rem; background: #0f172a; border: 1px solid #6366f1; border-radius: 8px; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.6rem; font-size: 0.82rem;">
                                <div style="display: flex; align-items: center; gap: 0.5rem;">
                                    <span class="badge" style="background: #4f46e5; color: #fff;">
                                        {move || if is_en() { "📎 In-Memory Attachment" } else { "📎 Temporär im RAM" }}
                                    </span>
                                    <strong style="color: #f8fafc;">{att.filename}</strong>
                                    <span style="color: #94a3b8;">
                                        {move || if is_en() {
                                            format!("({} chars · {} chunks · not in DB)", att.char_count, att.chunk_count)
                                        } else {
                                            format!("({} Zeichen · {} Chunks · nicht in DB)", att.char_count, att.chunk_count)
                                        }}
                                    </span>
                                </div>
                                <div style="display: flex; align-items: center; gap: 0.65rem;">
                                    <select
                                        style="width: 245px; padding: 0.28rem 0.5rem; font-size: 0.78rem;"
                                        prop:value=move || if attachment_only.get() { "only" } else { "combine" }
                                        on:change=move |ev| {
                                            set_attachment_only.set(event_target_value(&ev) == "only");
                                        }
                                    >
                                        <option value="only" selected>{move || if is_en() { "📄 Query this attachment only" } else { "📄 Nur diesen Anhang befragen" }}</option>
                                        <option value="combine">{move || if is_en() { "🔗 Combine attachment + SurrealDB" } else { "🔗 Anhang + SurrealDB kombinieren" }}</option>
                                    </select>
                                    <button
                                        class="btn btn-secondary"
                                        style="padding: 0.25rem 0.6rem; font-size: 0.76rem; background: #7f1d1d; color: #fecaca;"
                                        title=move || if is_en() { "Remove attachment from chat" } else { "Anhang aus dem Chat entfernen" }
                                        on:click=move |_| set_chat_attachment.set(None)
                                    >
                                        {move || if is_en() { "✕ Remove attachment" } else { "✕ Anhang entfernen" }}
                                    </button>
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
                        accept=".pdf,.md,.txt,.html,.htm,.csv,.json"
                        style="display: none;"
                        node_ref=chat_attach_input_ref
                        on:change=on_chat_attachment_selected
                    />
                    <button
                        class="btn btn-secondary"
                        style="padding: 0.65rem 0.85rem; font-size: 1.05rem;"
                        title="PDF oder Textdatei nur für diesen Chat anhängen (ohne Speicherung in SurrealDB)"
                        disabled=move || is_loading.get() || is_extracting_attachment.get()
                        on:click=move |_| {
                            if let Some(input_el) = chat_attach_input_ref.get() {
                                input_el.click();
                            }
                        }
                    >
                        "📎"
                    </button>
                    <input
                        type="text"
                        placeholder=move || {
                            if let Some(att) = chat_attachment.get() {
                                if is_en() {
                                    format!("Ask a question about '{}'...", att.filename)
                                } else {
                                    format!("Frage zu '{}' stellen...", att.filename)
                                }
                            } else if is_en() {
                                "Ask your knowledge base & LLM a question (or attach a PDF via 📎)...".to_string()
                            } else {
                                "Frage an deine Wissensdatenbank & LLM stellen (oder 📎 PDF anhängen)...".to_string()
                            }
                        }
                        prop:value=move || user_input.get()
                        on:input=move |ev| set_user_input.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                send_message();
                            }
                        }
                    />
                    <button
                        class="btn"
                        disabled=move || is_loading.get() || is_extracting_attachment.get()
                        on:click=move |_| send_message()
                    >
                        {move || if is_en() { "Send" } else { "Senden" }}
                    </button>
                </div>
            </main>

            // --- Modal-Dialog für Wissen einspeisen & Multi-File Upload ---
            {move || show_ingest_modal.get().then(|| {
                let is_fs = is_modal_fullscreen.get();
                let is_docs_tab = modal_tab.get() == "docs";
                let backdrop_class = if is_fs { "modal-backdrop fullscreen" } else { "modal-backdrop" };
                let dialog_class = if is_fs {
                    "modal-dialog fullscreen"
                } else if is_docs_tab {
                    "modal-dialog wide"
                } else {
                    "modal-dialog"
                };

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
                                    } else if is_en() {
                                        "📚 Ingest Knowledge into SurrealDB"
                                    } else {
                                        "📚 Wissen in SurrealDB einspeisen"
                                    }}
                                </h2>
                                <div style="display:flex; align-items:center; gap:0.5rem;">
                                    <button
                                        class="btn btn-secondary"
                                        style="padding:0.35rem 0.65rem; font-size:0.8rem;"
                                        title=move || if is_en() { "Toggle Fullscreen" } else { "Vollbild umschalten" }
                                        on:click=move |_| set_is_modal_fullscreen.update(|fs| *fs = !*fs)
                                    >
                                        {move || if is_modal_fullscreen.get() {
                                            if is_en() { "🗗 Restore" } else { "🗗 Fenster" }
                                        } else {
                                            if is_en() { "⛶ Fullscreen" } else { "⛶ Vollbild" }
                                        }}
                                    </button>
                                    <button
                                        class="btn btn-secondary"
                                        style="padding: 0.35rem 0.75rem; font-size:0.8rem;"
                                        on:click=move |_| {
                                            set_show_ingest_modal.set(false);
                                            set_is_modal_fullscreen.set(false);
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
                        </div>

                        <div class="modal-body">
                            {move || if modal_tab.get() == "files" {
                                view! {
                                    <div style="display:flex; flex-direction:column; gap:1rem;">
                                        // Revisions- & Projekt-Einstellungen
                                        <div style="display:flex; flex-direction:column; gap:0.6rem; background:#0f172a; padding:0.85rem; border-radius:8px; border:1px solid #334155;">
                                            <div>
                                                <label style="font-size:0.78rem; color:#94a3b8; display:block; margin-bottom:0.25rem;">
                                                    {move || if is_en() {
                                                        "Project / Folder / Category (optional, groups documents & namespaces identical filenames)"
                                                    } else {
                                                        "Projekt / Akte / Kategorie (optional, trennt gleichnamige Dateien verschiedener Projekte)"
                                                    }}
                                                </label>
                                                <input
                                                    type="text"
                                                    placeholder=move || if is_en() { "e.g. Case 53-20, Project North, or Directives" } else { "z.B. Akte 53-20, Objekt Dichtervillen oder Dienstanweisungen" }
                                                    prop:value=move || doc_category.get()
                                                    on:input=move |ev| set_doc_category.set(event_target_value(&ev))
                                                />
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
                                            accept=".pdf,.md,.txt,.html,.htm,.json,.csv"
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

                                        // Liste der ausgewählten Dateien als moderne Badges/Pills
                                        {move || (!selected_file_names.get().is_empty()).then(|| view! {
                                            <div class="file-pill-list">
                                                {selected_file_names.get().into_iter().map(|(name, size)| view! {
                                                    <div class="file-pill">
                                                        <span>"📄 " {name}</span>
                                                        <span style="color:#94a3b8;">{size}</span>
                                                    </div>
                                                }).collect_view()}
                                            </div>
                                        })}

                                        <button
                                            class="btn"
                                            disabled=move || is_uploading.get() || selected_file_names.get().is_empty()
                                            on:click=on_upload_files
                                        >
                                            {move || if is_uploading.get() {
                                                if is_en() { "Processing files...".to_string() } else { "Verarbeite Dateien...".to_string() }
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
                            } else {
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
                                                                                let (_, clean_title) = parse_category_and_title(&doc.doc_title);
                                                                                let chunk_badge = format!("🧩 {} Chunks", doc.chunk_count);
                                                                                let date_badge = format!("🕒 {}", doc.created_at);
                                                                                let id_badge = format!("🏷️ {}", doc.doc_id);
                                                                                let rel_count = doc.relations.len();
                                                                                let rels_for_view = doc.relations.clone();

                                                                                view! {
                                                                                    <div class="doc-card">
                                                                                        <div class="doc-card-header">
                                                                                            <span class="doc-card-icon">"📄"</span>
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

                                                                                        <div class="doc-card-actions">
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
                            }}

                            {move || (!ingest_status.get().is_empty()).then(|| view! {
                                <div class="status-banner">{ingest_status.get()}</div>
                            })}
                        </div>
                    </div>
                </div>
            }
        })}

            // --- Modal-Dialog: Generische LLM-, Remote-GPU- (RunPod) & Cloud-Konfiguration ---
            {move || show_llm_modal.get().then(|| view! {
                <div class="modal-backdrop" on:click=move |_| set_show_llm_modal.set(false)>
                    <div
                        class="modal-dialog"
                        style="max-width: 660px;"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="modal-header">
                            <h2>
                                {move || if is_en() {
                                    "⚙️ Settings (Language, RAG Pipeline, Reranker & LLM)"
                                } else {
                                    "⚙️ Einstellungen (Sprache, RAG-Pipeline, Reranker & LLM)"
                                }}
                            </h2>
                            <button
                                class="btn btn-secondary"
                                style="padding: 0.3rem 0.65rem; font-size: 0.8rem;"
                                on:click=move |_| set_show_llm_modal.set(false)
                            >
                                {move || if is_en() { "✕ Close" } else { "✕ Schließen" }}
                            </button>
                        </div>

                        <div class="modal-body" style="gap: 1rem;">
                            // --- Sektion 0: UI-Sprache (Deutsch / English) ---
                            <div style="display: flex; justify-content: space-between; align-items: center; background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 0.65rem 0.9rem;">
                                <span style="font-size: 0.82rem; font-weight: 600; color: #e2e8f0;">
                                    {move || if is_en() { "🌐 UI Language / Sprache" } else { "🌐 Chat-UI Sprache / Language" }}
                                </span>
                                <div style="display: flex; gap: 0.45rem;">
                                    <button
                                        class="btn btn-secondary"
                                        style=move || if !is_en() {
                                            "padding: 0.3rem 0.75rem; font-size: 0.78rem; background: #4f46e5; color: #fff; border-color: #818cf8;"
                                        } else {
                                            "padding: 0.3rem 0.75rem; font-size: 0.78rem;"
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
                                            "padding: 0.3rem 0.75rem; font-size: 0.78rem; background: #4f46e5; color: #fff; border-color: #818cf8;"
                                        } else {
                                            "padding: 0.3rem 0.75rem; font-size: 0.78rem;"
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

                            // --- Sektion A: RAG & Retrieval-Pipeline ---
                            <div style="background: rgba(15, 23, 42, 0.55); border: 1px solid #334155; border-radius: 0.65rem; padding: 0.9rem; display: flex; flex-direction: column; gap: 0.85rem;">
                                <div style="font-size: 0.78rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #818cf8;">
                                    {move || if is_en() {
                                        "🔍 RAG & Retrieval Pipeline (SurrealDB + TEI)"
                                    } else {
                                        "🔍 RAG & Retrieval-Pipeline (SurrealDB + TEI)"
                                    }}
                                </div>

                                <div class="toggle-row">
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
                                </div>

                                // 1. Cross-Encoder Reranker Wahl (Sleek Buttons)
                                <div>
                                    <div style="font-size:0.76rem; color:#94a3b8; margin-bottom:0.35rem; display:flex; justify-content:space-between; align-items:center;">
                                        <span>"Cross-Encoder Reranker"</span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || match reranker_mode.get().as_str() {
                                                "high" => "🎯 High (bge-reranker-v2-m3)",
                                                "none" => "🚀 Aus (Nur SurrealDB RRF)",
                                                _ => "⚡ Fast (mmarco-mMiniLMv2-L12)",
                                            }}
                                        </span>
                                    </div>
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
                                        <span>{move || if is_en() { "Top-N Context Chunks to LLM" } else { "Top-N Dokument-Chunks für LLM" }}</span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || format!("Top-{} Chunks", top_n.get())}
                                        </span>
                                    </div>
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
                                        <span>{move || if is_en() { "LLM Context Window (num_ctx in tokens)" } else { "LLM Kontext-Fenster (num_ctx in Tokens)" }}</span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || format!("{} Tokens", num_ctx_sig.get())}
                                        </span>
                                    </div>
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
                                        <span>{move || if is_en() { "Thinking / Reasoning Mode (DeepSeek-R1, QwQ)" } else { "Denkprozess / Reasoning (DeepSeek-R1, QwQ)" }}</span>
                                        <span style="color:#a5b4fc; font-weight:600; font-size:0.75rem;">
                                            {move || if think_mode_sig.get() {
                                                if is_en() { "🧠 Thinking visible" } else { "🧠 Denken aktiv" }
                                            } else {
                                                if is_en() { "⚡ Direct Answer" } else { "⚡ Direkte Antwort" }
                                            }}
                                        </span>
                                    </div>
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
                            </div>

                            // --- Sektion B: LLM-Server, Remote-GPU & Cloud ---
                            <div style="font-size: 0.78rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: #818cf8; margin-top: 0.15rem;">
                                {move || if is_en() {
                                    "🤖 Language Model (Local, RunPod GPU or Cloud API)"
                                } else {
                                    "🤖 Sprachmodell (Lokal, RunPod GPU oder Cloud-API)"
                                }}
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
                                        style="padding:0.35rem 0.65rem; font-size:0.76rem;"
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
                                        style="padding:0.35rem 0.65rem; font-size:0.76rem;"
                                        on:click=move |_| {
                                            set_llm_provider_sig.set("openai_compatible".to_string());
                                            set_llm_base_url_sig.set("https://DEINE-POD-ID-8000.proxy.runpod.net/v1".to_string());
                                            set_llm_config_status.set(if is_en() {
                                                "Preset 'RunPod / Remote vLLM / Ollama' loaded. Enter your RunPod proxy URL or server IP below.".to_string()
                                            } else {
                                                "Vorlage 'RunPod / Remote vLLM / Ollama' geladen. Trage deine RunPod-URL oder Server-IP (z.B. http://192.168.1.50:11434/v1) ein.".to_string()
                                            });
                                        }
                                    >
                                        "🚀 Remote GPU: RunPod / vLLM"
                                    </button>
                                    <button
                                        class="btn btn-secondary"
                                        style="padding:0.35rem 0.65rem; font-size:0.76rem;"
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
                                        style="padding:0.35rem 0.65rem; font-size:0.76rem;"
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
                                        style="padding:0.35rem 0.65rem; font-size:0.76rem;"
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
                                        style="padding:0.35rem 0.65rem; font-size:0.76rem;"
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
                                                            set_llm_model_sig.set(models[0].clone());
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
                                <div style="display:flex; flex-wrap:wrap; gap:0.35rem; margin-top:0.5rem; max-height:110px; overflow-y:auto;">
                                    <For
                                        each=move || available_models.get()
                                        key=|m| m.clone()
                                        children=move |m| {
                                            let m_click = m.clone();
                                            let m_check = m.clone();
                                            let m_label = m.clone();
                                            view! {
                                                <button
                                                    style=move || if llm_model_sig.get() == m_check {
                                                        "background:#4f46e5; color:#fff; border:1px solid #818cf8; border-radius:0.35rem; padding:0.2rem 0.55rem; font-size:0.75rem; cursor:pointer; font-weight:600;"
                                                    } else {
                                                        "background:#0f172a; color:#cbd5e1; border:1px solid #334155; border-radius:0.35rem; padding:0.2rem 0.55rem; font-size:0.75rem; cursor:pointer;"
                                                    }
                                                    on:click=move |_| set_llm_model_sig.set(m_click.clone())
                                                >
                                                    {m_label}
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

                            // --- Aktive Ports & Schnittstellen-Übersicht ---
                            <details style="margin-top:0.2rem; border-top:1px solid #334155; padding-top:0.75rem; font-size:0.76rem; color:#94a3b8;">
                                <summary style="cursor:pointer; color:#cbd5e1; font-weight:600;">
                                    {move || if is_en() {
                                        "🔌 View active endpoints & port configuration"
                                    } else {
                                        "🔌 Aktive Schnittstellen & Port-Konfiguration anzeigen"
                                    }}
                                </summary>
                                <div style="margin-top:0.55rem; background:#0f172a; border:1px solid #334155; border-radius:0.5rem; padding:0.7rem; font-family:monospace; line-height:1.6; color:#e2e8f0;">
                                    {
                                        let origin = web_sys::window()
                                            .and_then(|w| w.location().origin().ok())
                                            .unwrap_or_else(|| "http://localhost:8080".to_string());
                                        view! {
                                            <div>"• 🦀 Leptos WASM Chat-UI  : " {origin.clone()} "/"</div>
                                            <div>"• Health-Check            : " {origin.clone()} "/health"</div>
                                            <div>"• Ingestion (JSON)        : POST " {origin.clone()} "/v1/ingest"</div>
                                            <div>"• File Upload (PDF/MD/TXT): POST " {origin.clone()} "/v1/ingest/file"</div>
                                            <div>"• Hybrid + Graph Retrieval: POST " {origin.clone()} "/v1/retrieve"</div>
                                            <div>"• OpenAI Chat Completion  : POST " {origin.clone()} "/v1/chat/completions"</div>
                                            <div>"• MCP Server (Tool-Call)  : POST " {origin} "/mcp"</div>
                                        }
                                    }
                                    <div style="margin-top:0.5rem; padding-top:0.45rem; border-top:1px dashed #334155; color:#38bdf8; font-family:sans-serif;">
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
                            </details>
                        </div>
                    </div>
                </div>
            })}
        </div>
    }
}
