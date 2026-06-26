// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::models::traits::{Embedder, ModelProvider, Reranker};
use crate::models::{EmbedModel, RerankModel};

pub struct ModelRegistry {
    config: Config,
    embedders: Mutex<HashMap<String, Arc<dyn Embedder>>>,
    rerankers: Mutex<HashMap<String, Arc<dyn Reranker>>>,
}

impl ModelRegistry {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            embedders: Mutex::new(HashMap::new()),
            rerankers: Mutex::new(HashMap::new()),
        }
    }

    pub async fn embedder(&self, model_name: &str) -> Result<Arc<dyn Embedder>> {
        let mut map = self.embedders.lock().await;
        if let Some(model) = map.get(model_name) {
            return Ok(model.clone());
        }
        let model: Arc<dyn Embedder> = Arc::new(EmbedModel::new(&self.config, model_name)?);
        map.insert(model_name.to_string(), model.clone());
        Ok(model)
    }

    pub async fn reranker(&self, model_name: &str) -> Result<Arc<dyn Reranker>> {
        let mut map = self.rerankers.lock().await;
        if let Some(model) = map.get(model_name) {
            return Ok(model.clone());
        }
        let model: Arc<dyn Reranker> = Arc::new(RerankModel::new(&self.config, model_name)?);
        map.insert(model_name.to_string(), model.clone());
        Ok(model)
    }
}

#[async_trait]
impl ModelProvider for ModelRegistry {
    async fn embedder(&self, model_name: &str) -> Result<Arc<dyn Embedder>> {
        ModelRegistry::embedder(self, model_name).await
    }

    async fn reranker(&self, model_name: &str) -> Result<Arc<dyn Reranker>> {
        ModelRegistry::reranker(self, model_name).await
    }
}
