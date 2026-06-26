// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use fastembed::{RerankInitOptions, RerankerModel, TextRerank};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::models::traits::Reranker;

pub struct RerankModel {
    inner: Arc<Mutex<TextRerank>>,
    pub model_name: String,
}

impl RerankModel {
    pub fn new(config: &Config, model_name: &str) -> Result<Self> {
        let rerank = TextRerank::try_new(
            RerankInitOptions::new(RerankerModel::BGERerankerV2M3)
                .with_cache_dir(config.model_cache_dir.clone())
                .with_show_download_progress(false),
        )
        .context("load rerank model from cache")?;

        Ok(Self {
            inner: Arc::new(Mutex::new(rerank)),
            model_name: model_name.to_string(),
        })
    }

    pub async fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        let mut model = self.inner.lock().await;
        let refs: Vec<&str> = documents.iter().map(String::as_str).collect();
        let results = model
            .rerank(query, refs, false, None)
            .with_context(|| "rerank documents")?;
        Ok(results.into_iter().map(|r| r.score).collect())
    }
}

#[async_trait]
impl Reranker for RerankModel {
    async fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        RerankModel::rerank(self, query, documents).await
    }
}
