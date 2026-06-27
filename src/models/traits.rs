// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashSet;
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
    async fn reranker(&self, model_name: &str, max_length: usize) -> Result<Arc<dyn Reranker>>;

    /// Release in-memory model sessions that are not in `keep`. No-op for providers without a cache.
    async fn evict_unreferenced(&self, keep: &HashSet<String>) -> (usize, usize) {
        let _ = keep;
        (0, 0)
    }

    /// Drop a single model from the in-memory cache. No-op for providers without a cache.
    async fn purge_model(&self, name: &str) -> (usize, usize) {
        let _ = name;
        (0, 0)
    }

    /// Names of models currently held in memory. Empty for providers without a cache.
    async fn list_loaded(&self) -> Vec<String> {
        Vec::new()
    }
}
