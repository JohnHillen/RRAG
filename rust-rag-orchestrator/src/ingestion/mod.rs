pub mod attachments;
pub mod chunker;
pub mod documents;
pub mod pdf;
pub mod pdf_layout;
pub mod pipelines;
pub mod xml;

use crate::config::AppConfig;
use crate::db::RetrievedChunk;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct InferenceClient {
    pub(crate) http: Client,
    pub(crate) config: AppConfig,
    pub(crate) runtime_embed_url: Arc<RwLock<String>>,
}

#[derive(Debug, Deserialize)]
struct TeiRerankItem {
    index: usize,
    score: f64,
}

impl InferenceClient {
    pub fn new(http: Client, config: AppConfig) -> Self {
        let initial_embed_url = config.tei_embed_url.clone();
        Self {
            http,
            config,
            runtime_embed_url: Arc::new(RwLock::new(initial_embed_url)),
        }
    }

    pub fn get_embed_url(&self) -> String {
        self.runtime_embed_url
            .read()
            .map(|u| u.clone())
            .unwrap_or_else(|_| self.config.tei_embed_url.clone())
    }

    pub fn set_embed_url(&self, url: String) {
        if let Ok(mut guard) = self.runtime_embed_url.write() {
            *guard = url;
        }
    }

    /// Generates dense vector embeddings for a slice of texts via Hugging Face TEI (`/embed`).
    /// Sorts texts by length before batching (and restores original order afterward) to minimize
    /// Transformer sequence-padding overhead in TEI.
    pub async fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let current_embed_url = self.get_embed_url();
        let url = format!("{}/embed", current_embed_url.trim_end_matches('/'));
        let mut indexed: Vec<(usize, &String)> = texts.iter().enumerate().collect();
        indexed.sort_by_key(|(_, s)| s.len());

        let mut all_embeddings: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
        let batch_size = if current_embed_url.contains(":18081") {
            64
        } else {
            16
        };

        for batch in indexed.chunks(batch_size) {
            let batch_inputs: Vec<&str> = batch.iter().map(|(_, s)| s.as_str()).collect();
            let payload = json!({
                "inputs": batch_inputs,
                "truncate": true
            });

            let resp = self
                .http
                .post(&url)
                .json(&payload)
                .send()
                .await
                .context("Failed to communicate with TEI embedding server")?;

            let status = resp.status();
            if !status.is_success() {
                let err_txt = resp.text().await.unwrap_or_default();
                anyhow::bail!("TEI Embed HTTP {}: {}", status, err_txt);
            }

            let batch_vecs: Vec<Vec<f32>> = resp
                .json()
                .await
                .context("Failed to parse TEI embedding response")?;
            for ((orig_idx, _), vec) in batch.iter().zip(batch_vecs.into_iter()) {
                all_embeddings[*orig_idx] = Some(vec);
            }
        }

        Ok(all_embeddings.into_iter().flatten().collect())
    }

    /// Reranks retrieved candidate chunks using the selected Cross-Encoder model
    /// (`fast` = mmarco-mMiniLMv2-L12, `high` = bge-reranker-v2-m3, `none` = SurrealDB RRF only).
    pub async fn rerank_chunks(
        &self,
        query: &str,
        mut candidates: Vec<RetrievedChunk>,
        top_n: usize,
        reranker_mode: &str,
    ) -> Result<(Vec<RetrievedChunk>, String)> {
        if candidates.is_empty() {
            return Ok((vec![], "No candidates".to_string()));
        }

        if reranker_mode.eq_ignore_ascii_case("none") {
            candidates.truncate(top_n);
            return Ok((candidates, "SurrealDB RRF (No Reranker)".to_string()));
        }

        let (primary_url, primary_label, fallback_url, fallback_label) =
            if reranker_mode.eq_ignore_ascii_case("fast") {
                (
                    format!(
                        "{}/rerank",
                        self.config.tei_rerank_fast_url.trim_end_matches('/')
                    ),
                    self.config.reranker_fast_model.clone(),
                    format!("{}/rerank", self.config.tei_rerank_url.trim_end_matches('/')),
                    self.config.reranker_model.clone(),
                )
            } else {
                (
                    format!("{}/rerank", self.config.tei_rerank_url.trim_end_matches('/')),
                    self.config.reranker_model.clone(),
                    format!(
                        "{}/rerank",
                        self.config.tei_rerank_fast_url.trim_end_matches('/')
                    ),
                    self.config.reranker_fast_model.clone(),
                )
            };

        let texts: Vec<String> = candidates
            .iter()
            .map(|c| format!("[Document: {}]\n{}", c.doc_title, c.content))
            .collect();
        let payload = json!({
            "query": query,
            "texts": texts,
            "truncate": true
        });

        let mut active_label = primary_label;
        let mut resp = self.http.post(&primary_url).json(&payload).send().await;

        if resp
            .as_ref()
            .map(|r| !r.status().is_success())
            .unwrap_or(true)
        {
            if let Ok(fallback_resp) = self.http.post(&fallback_url).json(&payload).send().await {
                if fallback_resp.status().is_success() {
                    resp = Ok(fallback_resp);
                    active_label = format!("{} (Fallback)", fallback_label);
                }
            }
        }

        match resp {
            Ok(r) if r.status().is_success() => {
                let scores: Vec<TeiRerankItem> = r.json().await.unwrap_or_default();

                let max_rrf = candidates
                    .iter()
                    .map(|c| c.rrf_score)
                    .fold(0.0_f64, f64::max)
                    .max(0.001);

                let mut blended: Vec<(f64, RetrievedChunk)> = Vec::with_capacity(scores.len());
                for item in scores {
                    if let Some(chunk) = candidates.get(item.index) {
                        let mut c = chunk.clone();
                        let norm_rrf = (c.rrf_score / max_rrf).clamp(0.0, 1.0);
                        let combined_score = (item.score * 0.72) + (norm_rrf * 0.28);
                        c.rerank_score = Some(combined_score);
                        blended.push((combined_score, c));
                    }
                }

                blended.sort_by(|a, b| {
                    b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
                });

                // Document diversity: prevent a single multi-chunk document from monopolizing all `top_n` slots
                // when another relevant document is also among the top reranked candidates.
                let max_per_doc = (top_n / 2).max(2);
                let mut selected: Vec<RetrievedChunk> = Vec::with_capacity(top_n);
                let mut overflow: Vec<RetrievedChunk> = Vec::new();
                let mut doc_counts: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();

                for (_, c) in blended {
                    let cnt = doc_counts.entry(c.doc_id.clone()).or_insert(0);
                    if *cnt < max_per_doc && selected.len() < top_n {
                        *cnt += 1;
                        selected.push(c);
                    } else {
                        overflow.push(c);
                    }
                }
                for c in overflow {
                    if selected.len() >= top_n {
                        break;
                    }
                    selected.push(c);
                }

                Ok((selected, active_label))
            }
            _ => {
                candidates.truncate(top_n);
                Ok((candidates, "SurrealDB RRF (Fallback)".to_string()))
            }
        }
    }
}
