// SPDX-License-Identifier: AGPL-3.0-only

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use fastembed::{
    EmbeddingModel, InitOptions, RerankInitOptions, RerankerModel, TextEmbedding, TextRerank,
};

use crate::config::Config;

const TOKENIZER_FILES: [&str; 4] = [
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub kind: String,
    pub path: std::path::PathBuf,
    pub present: bool,
}

pub fn list_supported_models(config: &Config) -> Vec<ModelInfo> {
    [
        ("BAAI/bge-m3", "embed"),
        ("BAAI/bge-reranker-v2-m3", "rerank"),
    ]
    .into_iter()
    .map(|(name, kind)| {
        let path = config.model_dir_for(name);
        ModelInfo {
            name: name.to_string(),
            kind: kind.to_string(),
            present: model_is_complete(&path),
            path,
        }
    })
    .collect()
}

pub async fn ensure_models(config: &Config) -> Result<()> {
    ensure_single_model(config, &config.embedding_model, true).await?;
    let rerank_model = read_rerank_model_name(config).await?;
    ensure_single_model(config, &rerank_model, false).await?;
    Ok(())
}

async fn read_rerank_model_name(config: &Config) -> Result<String> {
    if !config.db_path.exists() {
        return Ok("BAAI/bge-reranker-v2-m3".to_string());
    }
    let pool = crate::db::DbPool::connect(config).await?;
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query(
            "SELECT value FROM settings WHERE key = 'rerank_model' LIMIT 1",
            (),
        )
        .await?;
    if let Some(row) = rows.next().await? {
        let value: String = row.get(0)?;
        let parsed: String = serde_json::from_str(&value).unwrap_or(value);
        return Ok(parsed);
    }
    Ok("BAAI/bge-reranker-v2-m3".to_string())
}

pub async fn ensure_single_model_public(
    config: &Config,
    model_name: &str,
    required: bool,
) -> Result<()> {
    ensure_single_model(config, model_name, required).await
}

async fn ensure_single_model(config: &Config, model_name: &str, required: bool) -> Result<()> {
    let dir = config.model_dir_for(model_name);
    std::fs::create_dir_all(&dir).with_context(|| format!("create model dir {}", dir.display()))?;

    if model_is_complete(&dir) {
        tracing::info!(model = model_name, path = %dir.display(), "model already present");
        return Ok(());
    }

    if config.hf_hub_offline {
        if required {
            bail!(
                "model {model_name} missing at {} and HF_HUB_OFFLINE is enabled",
                dir.display()
            );
        }
        tracing::warn!(model = model_name, "model missing but optional in offline mode");
        return Ok(());
    }

    tracing::info!(model = model_name, "downloading model via fastembed cache bootstrap");
    bootstrap_download(config, model_name, &dir)?;
    if !model_is_complete(&dir) {
        bail!("model download incomplete for {model_name} at {}", dir.display());
    }
    Ok(())
}

fn bootstrap_download(config: &Config, model_name: &str, target_dir: &Path) -> Result<()> {
    let cache_dir = config.model_cache_dir.clone();
    match model_name {
        "BAAI/bge-m3" => {
            let _ = TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::BGEM3)
                    .with_cache_dir(cache_dir.clone())
                    .with_show_download_progress(true),
            )?;
        }
        "BAAI/bge-reranker-v2-m3" => {
            let _ = TextRerank::try_new(
                RerankInitOptions::new(RerankerModel::BGERerankerV2M3)
                    .with_cache_dir(cache_dir.clone())
                    .with_show_download_progress(true),
            )?;
        }
        other => bail!("unsupported bootstrap model: {other}"),
    }

    materialize_canonical_model(config, model_name, target_dir)
}

fn hf_cache_slug(model_name: &str) -> String {
    format!("models--{}", model_name.replace('/', "--"))
}

fn hf_cache_slugs(model_name: &str) -> Vec<String> {
    match model_name {
        // fastembed downloads this logical model from rozgo's HF repo.
        "BAAI/bge-reranker-v2-m3" => vec![
            hf_cache_slug("rozgo/bge-reranker-v2-m3"),
            hf_cache_slug(model_name),
        ],
        other => vec![hf_cache_slug(other)],
    }
}

fn materialize_canonical_model(
    config: &Config,
    model_name: &str,
    target_dir: &Path,
) -> Result<()> {
    let snapshot = find_fastembed_snapshot(&config.model_cache_dir, model_name)
        .with_context(|| format!("could not locate downloaded artifacts for {model_name}"))?;

    let onnx_src = if snapshot.join("onnx/model.onnx").exists() {
        snapshot.join("onnx/model.onnx")
    } else {
        snapshot.join("model.onnx")
    };

    if !onnx_src.exists() {
        bail!("onnx model file missing in {}", snapshot.display());
    }

    std::fs::copy(&onnx_src, target_dir.join("model.onnx"))
        .with_context(|| format!("copy {}", onnx_src.display()))?;

    let onnx_dir = onnx_src.parent().unwrap_or(&snapshot);
    for name in ["model.onnx_data", "model.onnx.data"] {
        let external_data = onnx_dir.join(name);
        if external_data.exists() {
            std::fs::copy(&external_data, target_dir.join(name))
                .with_context(|| format!("copy {}", external_data.display()))?;
            break;
        }
    }

    for file in TOKENIZER_FILES {
        let source = snapshot.join(file);
        if !source.exists() {
            bail!("missing tokenizer artifact {file} in {}", snapshot.display());
        }
        std::fs::copy(&source, target_dir.join(file))
            .with_context(|| format!("copy {}", source.display()))?;
    }

    std::fs::write(target_dir.join(".model-id"), model_name)?;
    tracing::info!(
        model = model_name,
        source = %snapshot.display(),
        target = %target_dir.display(),
        "materialized canonical model artifacts"
    );
    Ok(())
}

fn find_fastembed_snapshot(cache_dir: &Path, model_name: &str) -> Option<PathBuf> {
    for slug in hf_cache_slugs(model_name) {
        let slug_dir = cache_dir.join(slug);
        if slug_dir.is_dir() {
            if let Some(found) = find_snapshot_recursive(&slug_dir) {
                return Some(found);
            }
        }
    }
    None
}

fn find_snapshot_recursive(dir: &Path) -> Option<PathBuf> {
    if is_canonical_model_dir(dir) {
        return None;
    }

    if dir.join("tokenizer.json").exists()
        && (dir.join("model.onnx").exists() || dir.join("onnx/model.onnx").exists())
    {
        return Some(dir.to_path_buf());
    }

    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_snapshot_recursive(&path) {
                return Some(found);
            }
        }
    }
    None
}

fn is_canonical_model_dir(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains("__") && !name.starts_with("models--"))
}

pub fn model_is_complete(dir: &Path) -> bool {
    dir.join("model.onnx").exists()
        && dir.join("tokenizer.json").exists()
        && dir.join("config.json").exists()
        && dir.join("special_tokens_map.json").exists()
        && dir.join("tokenizer_config.json").exists()
}

pub fn embedding_model_dir(config: &Config) -> std::path::PathBuf {
    config.model_dir_for(&config.embedding_model)
}

pub fn rerank_model_dir(config: &Config, model_name: &str) -> std::path::PathBuf {
    config.model_dir_for(model_name)
}
