// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::models::traits::{Embedder, ModelProvider, Reranker};

const EMBED_DIM: usize = 1024;

fn deterministic_vector(text: &str) -> Vec<f32> {
    let mut out = vec![0.0_f32; EMBED_DIM];
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let seed = hasher.finish();
    for (idx, value) in out.iter_mut().enumerate() {
        let mixed = seed.wrapping_add((idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        *value = ((mixed % 10_000) as f32 / 10_000.0) - 0.5;
    }
    let norm: f32 = out.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut out {
            *value /= norm;
        }
    }
    out
}

pub struct MockEmbedder;

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        Ok(deterministic_vector(text))
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| deterministic_vector(text))
            .collect())
    }
}

pub struct MockReranker;

#[async_trait]
impl Reranker for MockReranker {
    async fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        Ok(documents
            .iter()
            .enumerate()
            .map(|(idx, doc)| {
                let overlap = query
                    .split_whitespace()
                    .filter(|word| doc.contains(word))
                    .count() as f32;
                overlap + (documents.len() - idx) as f32 * 0.001
            })
            .collect())
    }
}

pub struct MockModelProvider;

#[async_trait]
impl ModelProvider for MockModelProvider {
    async fn embedder(&self, _model_name: &str) -> Result<Arc<dyn Embedder>> {
        Ok(Arc::new(MockEmbedder))
    }

    async fn reranker(&self, _model_name: &str, _max_length: usize) -> Result<Arc<dyn Reranker>> {
        Ok(Arc::new(MockReranker))
    }
}
