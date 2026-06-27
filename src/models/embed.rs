// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use fastembed::{InitOptionsUserDefined, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::models::mapping::{embedding_model_enum, is_supported_embed_model, query_prefix_for};
use crate::models::traits::Embedder;

pub struct EmbedModel {
    inner: Arc<Mutex<TextEmbedding>>,
    pub model_name: String,
    pub dim: usize,
    pub version: String,
}

impl EmbedModel {
    pub fn new(config: &Config, model_name: &str) -> Result<Self> {
        let embedding = if embedding_model_enum(model_name).is_ok() {
            crate::models::bootstrap::ensure_preset_cache_present(config, model_name)?;
            let model_enum = embedding_model_enum(model_name)?;
            TextEmbedding::try_new(
                fastembed::TextInitOptions::new(model_enum)
                    .with_cache_dir(config.model_cache_dir.clone())
                    .with_show_download_progress(false)
                    .with_intra_threads(config.onnx_num_threads),
            )
            .context("load embedding model from cache")?
        } else if is_supported_embed_model(model_name) {
            load_user_defined_embedder(config, model_name)?
        } else {
            anyhow::bail!("unsupported embedding model: {model_name}");
        };

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
        tokio::task::block_in_place(|| model.embed(refs, None).with_context(|| "embed texts"))
    }

    pub async fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        let prefix = query_prefix_for(&self.model_name);
        let prefixed = if prefix.is_empty() {
            text.to_string()
        } else {
            format!("{prefix}{text}")
        };
        let vectors = self.embed(&[prefixed]).await?;
        vectors.into_iter().next().context("empty embedding result")
    }
}

fn load_user_defined_embedder(config: &Config, model_name: &str) -> Result<TextEmbedding> {
    let model_dir = config.model_dir_for(model_name);
    let onnx_path = model_dir.join("model.onnx");
    if !onnx_path.exists() {
        anyhow::bail!("embedding model missing at {}", model_dir.display());
    }

    let mut user_model = UserDefinedEmbeddingModel::new(
        std::fs::read(&onnx_path).with_context(|| format!("read {}", onnx_path.display()))?,
        read_tokenizer_files(&model_dir)?,
    );

    for name in ["model.onnx_data", "model.onnx.data"] {
        let data_path = model_dir.join(name);
        if data_path.exists() {
            user_model = user_model.with_external_initializer(
                name.to_string(),
                std::fs::read(&data_path)
                    .with_context(|| format!("read {}", data_path.display()))?,
            );
            break;
        }
    }

    TextEmbedding::try_new_from_user_defined(
        user_model,
        InitOptionsUserDefined::default().with_intra_threads(config.onnx_num_threads),
    )
    .context("load user-defined embedding model")
}

fn read_tokenizer_files(model_dir: &std::path::Path) -> Result<TokenizerFiles> {
    Ok(TokenizerFiles {
        tokenizer_file: std::fs::read(model_dir.join("tokenizer.json"))?,
        config_file: std::fs::read(model_dir.join("config.json"))?,
        special_tokens_map_file: std::fs::read(model_dir.join("special_tokens_map.json"))?,
        tokenizer_config_file: std::fs::read(model_dir.join("tokenizer_config.json"))?,
    })
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
