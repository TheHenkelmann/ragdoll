// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use fastembed::{
    OnnxSource, RerankInitOptions, RerankInitOptionsUserDefined, TextRerank, TokenizerFiles,
    UserDefinedRerankingModel,
};
use tokio::sync::{Mutex, Semaphore};

use crate::config::Config;
use crate::models::mapping::reranker_model_enum;
use crate::models::traits::Reranker;

pub struct RerankModel {
    permits: Arc<Semaphore>,
    instances: Arc<Mutex<Vec<TextRerank>>>,
    pub model_name: String,
}

impl RerankModel {
    pub fn new(config: &Config, model_name: &str, max_length: usize) -> Result<Self> {
        let pool_size = config.rerank_pool_size;
        let mut instances = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            let rerank = if let Ok(model_enum) = reranker_model_enum(model_name) {
                TextRerank::try_new(
                    RerankInitOptions::new(model_enum)
                        .with_cache_dir(config.model_cache_dir.clone())
                        .with_show_download_progress(false)
                        .with_max_length(max_length)
                        .with_intra_threads(config.onnx_num_threads),
                )
                .context("load rerank model from cache")?
            } else {
                load_user_defined_reranker(config, model_name, max_length)?
            };
            instances.push(rerank);
        }

        Ok(Self {
            permits: Arc::new(Semaphore::new(pool_size)),
            instances: Arc::new(Mutex::new(instances)),
            model_name: model_name.to_string(),
        })
    }

    pub async fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        let _permit = self
            .permits
            .acquire()
            .await
            .context("acquire rerank pool permit")?;

        let mut instance = {
            let mut pool = self.instances.lock().await;
            pool.pop()
                .context("rerank pool empty despite acquired permit")?
        };

        let query = query.to_string();
        let documents = documents.to_vec();
        let (instance, results) = tokio::task::spawn_blocking(move || {
            let refs: Vec<&str> = documents.iter().map(String::as_str).collect();
            let results = instance
                .rerank(query.as_str(), refs, false, None)
                .with_context(|| "rerank documents")?;
            Ok::<_, anyhow::Error>((instance, results))
        })
        .await
        .context("rerank blocking task panicked")??;

        {
            let mut pool = self.instances.lock().await;
            pool.push(instance);
        }

        Ok(results.into_iter().map(|r| r.score).collect())
    }
}

fn load_user_defined_reranker(
    config: &Config,
    model_name: &str,
    max_length: usize,
) -> Result<TextRerank> {
    let model_dir = config.model_dir_for(model_name);
    let onnx_path = model_dir.join("model.onnx");
    if !onnx_path.exists() {
        anyhow::bail!("rerank model missing at {}", model_dir.display());
    }
    let tokenizer_files = TokenizerFiles {
        tokenizer_file: std::fs::read(model_dir.join("tokenizer.json"))?,
        config_file: std::fs::read(model_dir.join("config.json"))?,
        special_tokens_map_file: std::fs::read(model_dir.join("special_tokens_map.json"))?,
        tokenizer_config_file: std::fs::read(model_dir.join("tokenizer_config.json"))?,
    };
    let user_model = UserDefinedRerankingModel::new(OnnxSource::File(onnx_path), tokenizer_files);
    TextRerank::try_new_from_user_defined(
        user_model,
        RerankInitOptionsUserDefined::default().with_max_length(max_length),
    )
    .context("load user-defined rerank model")
}

#[async_trait]
impl Reranker for RerankModel {
    async fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        RerankModel::rerank(self, query, documents).await
    }
}
