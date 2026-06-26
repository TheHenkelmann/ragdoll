// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed_one(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

#[async_trait]
pub trait Reranker: Send + Sync {
    async fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<f32>>;
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn embedder(&self, model_name: &str) -> Result<Arc<dyn Embedder>>;
    async fn reranker(&self, model_name: &str) -> Result<Arc<dyn Reranker>>;
}
