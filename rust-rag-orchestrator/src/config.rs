use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub surreal_url: String,
    pub surreal_user: String,
    pub surreal_pass: String,
    pub surreal_ns: String,
    pub surreal_db: String,
    pub tei_embed_url: String,
    pub tei_rerank_url: String,
    pub tei_rerank_fast_url: String,
    pub reranker_model: String,
    pub reranker_fast_model: String,
    pub llm_provider: String,
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub enable_graph_extraction: bool,
    pub orchestrator_port: u16,
    pub chunk_max_chars: usize,
    pub retrieval_top_k_candidates: usize,
    pub retrieval_final_top_n: usize,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            surreal_url: env::var("SURREAL_URL").unwrap_or_else(|_| "http://localhost:8000".into()),
            surreal_user: env::var("SURREAL_USER").unwrap_or_else(|_| "root".into()),
            surreal_pass: env::var("SURREAL_PASS")
                .unwrap_or_else(|_| "root_secret_password_change_me".into()),
            surreal_ns: env::var("SURREAL_NS").unwrap_or_else(|_| "rag_namespace".into()),
            surreal_db: env::var("SURREAL_DB").unwrap_or_else(|_| "knowledge_base".into()),
            tei_embed_url: env::var("TEI_EMBED_URL")
                .unwrap_or_else(|_| "http://localhost:8081".into()),
            tei_rerank_url: env::var("TEI_RERANK_URL")
                .unwrap_or_else(|_| "http://localhost:8082".into()),
            tei_rerank_fast_url: env::var("TEI_RERANK_FAST_URL")
                .unwrap_or_else(|_| "http://localhost:8083".into()),
            reranker_model: env::var("RERANKER_MODEL")
                .unwrap_or_else(|_| "BAAI/bge-reranker-v2-m3".into()),
            reranker_fast_model: env::var("RERANKER_FAST_MODEL")
                .unwrap_or_else(|_| "cross-encoder/mmarco-mMiniLMv2-L12-H384-v1".into()),
            llm_provider: env::var("LLM_PROVIDER").unwrap_or_else(|_| "openai_compatible".into()),
            llm_base_url: env::var("LLM_BASE_URL")
                .unwrap_or_else(|_| "http://host.docker.internal:11434/v1".into()),
            llm_api_key: env::var("LLM_API_KEY").unwrap_or_else(|_| "ollama".into()),
            llm_model: env::var("LLM_MODEL").unwrap_or_else(|_| "llama3.1:8b".into()),
            enable_graph_extraction: env::var("ENABLE_GRAPH_EXTRACTION")
                .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
                .unwrap_or(false),
            orchestrator_port: env::var("ORCHESTRATOR_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            chunk_max_chars: env::var("CHUNK_MAX_CHARS")
                .ok()
                .and_then(|c| c.parse().ok())
                .unwrap_or(1000),
            retrieval_top_k_candidates: env::var("RETRIEVAL_TOP_K_CANDIDATES")
                .ok()
                .and_then(|k| k.parse().ok())
                .unwrap_or(15),
            retrieval_final_top_n: env::var("RETRIEVAL_FINAL_TOP_N")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(4),
        }
    }
}

