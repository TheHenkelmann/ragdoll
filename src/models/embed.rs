// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::models::traits::Embedder;

pub struct EmbedModel {
    inner: Arc<Mutex<TextEmbedding>>,
    pub model_name: String,
    pub dim: usize,
    pub version: String,
}

impl EmbedModel {
    pub fn new(config: &Config, model_name: &str) -> Result<Self> {
        let embedding = TextEmbedding::try_new(
            TextInitOptions::new(EmbeddingModel::BGEM3)
                .with_cache_dir(config.model_cache_dir.clone())
                .with_show_download_progress(false),
        )
        .context("load embedding model from cache")?;

        Ok(Self {
            inner: Arc::new(Mutex::new(embedding)),
            model_name: model_name.to_string(),
            dim: config.embedding_dim,
            version: "1".to_string(),
        })
    }

    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut model = self.inner.lock().await;
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        model
            .embed(refs, None)
            .with_context(|| "embed texts")
    }

    pub async fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        let vectors = self.embed(&[text.to_string()]).await?;
        vectors
            .into_iter()
            .next()
            .context("empty embedding result")
    }
}

#[async_trait]
impl Embedder for EmbedModel {
    async fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        EmbedModel::embed_one(self, text).await
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        EmbedModel::embed(self, texts).await
    }
}
