// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::{bail, Context, Result};
use fastembed::{InitOptions, RerankInitOptions, TextEmbedding, TextRerank};

use crate::config::Config;
use crate::db::{DbError, DbPool};
use crate::models::catalog::{find_catalog_entry, LoadStrategy};
use crate::models::download::DownloadNotifier;
use crate::models::download_io::copy_file_with_limits;
use crate::models::mapping::{
    all_supported_model_names, embedding_model_enum, is_supported_embed_model,
    is_supported_rerank_model, reranker_model_enum,
};
use crate::settings::RuntimeSettings;

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
    all_supported_model_names()
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

pub async fn ensure_models(config: &Config, pool: &DbPool) -> Result<()> {
    ensure_models_for_releases(config, pool).await
}

pub async fn collect_required_models(pool: &DbPool) -> Result<HashSet<String>, DbError> {
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query(
            "SELECT key, value FROM settings WHERE key IN ('embedding_model', 'rerank_model')",
            (),
        )
        .await?;

    let defaults = RuntimeSettings::default();
    let mut required = HashSet::new();
    let mut embed_by_release: HashSet<String> = HashSet::new();
    let mut rerank_by_release: HashSet<String> = HashSet::new();

    while let Some(row) = rows.next().await? {
        let key: String = row.get(0)?;
        let raw: String = row.get(1)?;
        let value = serde_json::from_str::<String>(&raw).unwrap_or(raw);
        if key == "embedding_model" {
            embed_by_release.insert(value);
        } else {
            rerank_by_release.insert(value);
        }
    }

    if embed_by_release.is_empty() {
        required.insert(defaults.embedding_model);
    } else {
        required.extend(embed_by_release);
    }
    if rerank_by_release.is_empty() {
        required.insert(defaults.rerank_model);
    } else {
        required.extend(rerank_by_release);
    }

    Ok(required)
}

pub async fn ensure_models_for_releases(config: &Config, pool: &DbPool) -> Result<()> {
    let required = collect_required_models(pool)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    for name in required {
        let required_model = is_supported_embed_model(&name);
        ensure_single_model(config, &name, required_model).await?;
    }
    Ok(())
}

pub fn list_local_models(config: &Config) -> Vec<ModelInfo> {
    let mut models = Vec::new();
    let Ok(entries) = std::fs::read_dir(&config.model_cache_dir) else {
        return models;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !model_is_complete(&path) {
            continue;
        }
        let name = read_model_id_file(&path).unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .replace("__", "/")
        });
        if name.is_empty() {
            continue;
        }
        let kind = if is_supported_embed_model(&name) {
            "embed"
        } else if is_supported_rerank_model(&name) {
            "rerank"
        } else {
            "unknown"
        };
        models.push(ModelInfo {
            name,
            kind: kind.to_string(),
            present: true,
            path,
        });
    }
    models.sort_by(|a, b| a.name.cmp(&b.name));
    models
}

fn read_model_id_file(dir: &Path) -> Option<String> {
    let id_path = dir.join(".model-id");
    let content = std::fs::read_to_string(id_path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn delete_local_model(config: &Config, model_name: &str) -> Result<()> {
    let dir = config.model_dir_for(model_name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .with_context(|| format!("remove model dir {}", dir.display()))?;
    }
    for slug in hf_cache_slugs(model_name) {
        let cache = config.model_cache_dir.join(slug);
        if cache.exists() {
            std::fs::remove_dir_all(&cache)
                .with_context(|| format!("remove model cache {}", cache.display()))?;
        }
    }
    Ok(())
}

/// Remove the partial download directory and any Hugging Face cache trees for a
/// model. Intended for cancelled/failed downloads; callers must ensure the model
/// is not a complete, in-use model before calling.
pub fn remove_incomplete_model_artifacts(config: &Config, model_name: &str) -> Result<()> {
    let dir = config.model_dir_for(model_name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .with_context(|| format!("remove partial model dir {}", dir.display()))?;
    }
    for slug in hf_cache_slugs(model_name) {
        let cache = config.model_cache_dir.join(slug);
        if cache.exists() {
            std::fs::remove_dir_all(&cache)
                .with_context(|| format!("remove partial model cache {}", cache.display()))?;
        }
    }
    Ok(())
}

pub fn is_valid_hf_model_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split('/').collect();
    parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty()
}

pub async fn ensure_single_model_public(
    config: &Config,
    model_name: &str,
    required: bool,
) -> Result<()> {
    ensure_single_model(config, model_name, required).await
}

async fn ensure_single_model(config: &Config, model_name: &str, required: bool) -> Result<()> {
    ensure_single_model_blocking(config, model_name, required)
}

pub fn ensure_single_model_blocking(
    config: &Config,
    model_name: &str,
    required: bool,
) -> Result<()> {
    let never_cancel = AtomicBool::new(false);
    ensure_single_model_blocking_cancellable(
        config,
        model_name,
        required,
        &never_cancel,
        None,
        None,
    )
}

/// Like [`ensure_single_model_blocking`], but aborts as soon as `cancel` is set.
/// Cancellation is honoured between individual file downloads for user-defined
/// models; fastembed preset downloads are atomic and only checked afterwards.
pub fn ensure_single_model_blocking_cancellable(
    config: &Config,
    model_name: &str,
    required: bool,
    cancel: &AtomicBool,
    progress: Option<&AtomicU64>,
    notifier: Option<&DownloadNotifier>,
) -> Result<()> {
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
        tracing::warn!(
            model = model_name,
            "model missing but optional in offline mode"
        );
        return Ok(());
    }

    tracing::info!(
        model = model_name,
        "downloading model via fastembed cache bootstrap"
    );
    bootstrap_download(config, model_name, &dir, cancel, progress, notifier)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    if !model_is_complete(&dir) {
        bail!(
            "model download incomplete for {model_name} at {}",
            dir.display()
        );
    }
    Ok(())
}

fn bootstrap_download(
    config: &Config,
    model_name: &str,
    target_dir: &Path,
    cancel: &AtomicBool,
    progress: Option<&AtomicU64>,
    notifier: Option<&DownloadNotifier>,
) -> Result<()> {
    let cache_dir = config.model_cache_dir.clone();
    if embedding_model_enum(model_name).is_ok() {
        if let Some(n) = notifier {
            n.set_cancellable(false);
        }
        let embed = embedding_model_enum(model_name)?;
        let _ = TextEmbedding::try_new(
            InitOptions::new(embed)
                .with_cache_dir(cache_dir.clone())
                .with_show_download_progress(true)
                .with_intra_threads(config.onnx_num_threads.min(2)),
        )?;
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        if let Some(n) = notifier {
            n.set_cancellable(true);
        }
        materialize_canonical_model(config, model_name, target_dir, cancel, progress)?;
        if let Some(n) = notifier {
            n.set_cancellable(false);
        }
    } else if reranker_model_enum(model_name).is_ok() {
        if let Some(n) = notifier {
            n.set_cancellable(false);
        }
        let rerank = reranker_model_enum(model_name)?;
        let _ = TextRerank::try_new(
            RerankInitOptions::new(rerank)
                .with_cache_dir(cache_dir.clone())
                .with_show_download_progress(true)
                .with_intra_threads(config.onnx_num_threads.min(2)),
        )?;
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        if let Some(n) = notifier {
            n.set_cancellable(true);
        }
        materialize_canonical_model(config, model_name, target_dir, cancel, progress)?;
        if let Some(n) = notifier {
            n.set_cancellable(false);
        }
    } else if is_user_defined_model(model_name) || is_valid_hf_model_name(model_name) {
        if let Some(n) = notifier {
            n.set_cancellable(true);
        }
        download_user_defined_from_hf(config, model_name, target_dir, cancel, progress)?;
        if let Some(n) = notifier {
            n.set_cancellable(false);
        }
    } else {
        bail!("unsupported bootstrap model: {model_name}");
    }
    Ok(())
}

/// Self-heal for preset models: they load from the fastembed download cache
/// (`models--<repo>`). If that cache is missing (e.g. it was deleted) repopulate
/// it through the normal download path *before* the loader runs, so a load never
/// silently fails or blocks on an ad-hoc fastembed re-download. No-op for
/// user-defined models (they load from the canonical dir) and when offline.
pub fn ensure_preset_cache_present(config: &Config, model_name: &str) -> Result<()> {
    let is_preset =
        embedding_model_enum(model_name).is_ok() || reranker_model_enum(model_name).is_ok();
    if !is_preset {
        return Ok(());
    }
    if find_fastembed_snapshot(&config.model_cache_dir, model_name).is_some() {
        return Ok(());
    }
    if config.hf_hub_offline {
        // Can't repopulate offline; let the loader surface a clear error.
        return Ok(());
    }
    tracing::info!(
        model = model_name,
        "preset load cache missing; repopulating via download before load"
    );
    let never_cancel = AtomicBool::new(false);
    let dir = config.model_dir_for(model_name);
    std::fs::create_dir_all(&dir).with_context(|| format!("create model dir {}", dir.display()))?;
    bootstrap_download(config, model_name, &dir, &never_cancel, None, None)
}

fn is_user_defined_model(model_name: &str) -> bool {
    find_catalog_entry(model_name).is_some_and(|e| e.load_strategy == LoadStrategy::UserDefined)
        || (is_supported_embed_model(model_name) && embedding_model_enum(model_name).is_err())
        || (is_supported_rerank_model(model_name) && reranker_model_enum(model_name).is_err())
}

#[derive(serde::Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
}

fn hf_download_client(config: &Config) -> Result<reqwest::blocking::Client> {
    // Model artifacts can be multiple GB and stream for several minutes, so the
    // previous 120s total timeout aborted large downloads mid-transfer. Use a
    // short connect timeout to fail fast on dead hosts and a generous overall
    // timeout that still bounds a stalled transfer.
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(1800));
    if let Some(token) = &config.hf_token {
        builder = builder.default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {token}").parse().unwrap(),
            );
            headers
        });
    }
    builder.build().context("build HF download client")
}

pub fn download_user_defined_from_hf(
    config: &Config,
    model_name: &str,
    target_dir: &Path,
    cancel: &AtomicBool,
    progress: Option<&AtomicU64>,
) -> Result<()> {
    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("create model dir {}", target_dir.display()))?;

    let client = hf_download_client(config)?;
    let url = format!("https://huggingface.co/api/models/{model_name}/tree/main?recursive=true");
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("fetch HF tree for {model_name}"))?;
    if !resp.status().is_success() {
        bail!("HF tree API returned {} for {model_name}", resp.status());
    }
    let body = resp.text().context("read HF tree response")?;
    let entries: Vec<HfTreeEntry> =
        serde_json::from_str(&body).context("parse HF tree response")?;
    let files: Vec<&HfTreeEntry> = entries.iter().filter(|e| e.kind == "file").collect();

    let onnx_path = files
        .iter()
        .find(|e| e.path == "onnx/model.onnx")
        .or_else(|| files.iter().find(|e| e.path == "model.onnx"))
        .map(|e| e.path.as_str())
        .context(format!(
            "no ONNX model.onnx found in Hugging Face repo {model_name}"
        ))?;

    let onnx_prefix = if onnx_path.starts_with("onnx/") {
        "onnx/"
    } else {
        ""
    };

    let mut to_download = vec![onnx_path.to_string()];
    for data_name in ["model.onnx_data", "model.onnx.data"] {
        let candidate = format!("{onnx_prefix}{data_name}");
        if files.iter().any(|e| e.path == candidate) {
            to_download.push(candidate);
            break;
        }
    }
    for aux in TOKENIZER_FILES {
        let found = files
            .iter()
            .find(|e| e.path.rsplit('/').next() == Some(aux))
            .map(|e| e.path.clone());
        if let Some(path) = found {
            to_download.push(path);
        } else {
            bail!("missing tokenizer artifact {aux} in Hugging Face repo {model_name}");
        }
    }

    for remote_path in &to_download {
        if cancel.load(Ordering::Relaxed) {
            tracing::info!(model = model_name, "user-defined download cancelled");
            return Ok(());
        }
        let local_name = remote_path.rsplit('/').next().unwrap_or(remote_path);
        let local_path = target_dir.join(local_name);
        let download_url =
            format!("https://huggingface.co/{model_name}/resolve/main/{remote_path}");
        // Stream the response body straight to disk instead of buffering the
        // whole (potentially multi-GB) file in memory. Writing incrementally
        // also lets the directory-size progress poller observe real progress.
        let mut resp = client
            .get(&download_url)
            .send()
            .with_context(|| format!("download {download_url}"))?
            .error_for_status()
            .with_context(|| format!("HF download failed for {download_url}"))?;
        let mut file = std::fs::File::create(&local_path)
            .with_context(|| format!("create {}", local_path.display()))?;
        crate::models::download_io::copy_with_limits(
            &mut resp,
            &mut file,
            cancel,
            config.model_download_write_chunk_bytes,
            config.model_download_bandwidth_bps,
            progress,
        )
        .with_context(|| format!("stream {download_url} to {}", local_path.display()))?;
    }

    std::fs::write(target_dir.join(".model-id"), model_name)?;
    tracing::info!(
        model = model_name,
        target = %target_dir.display(),
        "downloaded user-defined model from Hugging Face"
    );
    Ok(())
}

fn hf_cache_slug(model_name: &str) -> String {
    format!("models--{}", model_name.replace('/', "--"))
}

pub fn hf_cache_slugs(model_name: &str) -> Vec<String> {
    // Several fastembed presets are downloaded from a mirror/ONNX repo whose id
    // differs from the logical catalog name. The actual fastembed download repo
    // must come first so snapshot discovery, progress polling, and size
    // estimation all look at the directory fastembed actually populates.
    match model_name {
        "BAAI/bge-reranker-v2-m3" => vec![
            hf_cache_slug("rozgo/bge-reranker-v2-m3"),
            hf_cache_slug(model_name),
        ],
        "BAAI/bge-large-en-v1.5" => vec![
            hf_cache_slug("Xenova/bge-large-en-v1.5"),
            hf_cache_slug(model_name),
        ],
        "intfloat/multilingual-e5-large" => vec![
            hf_cache_slug("Qdrant/multilingual-e5-large-onnx"),
            hf_cache_slug(model_name),
        ],
        other => vec![hf_cache_slug(other)],
    }
}

fn materialize_canonical_model(
    config: &Config,
    model_name: &str,
    target_dir: &Path,
    cancel: &AtomicBool,
    progress: Option<&AtomicU64>,
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

    copy_file_with_limits(
        config,
        &onnx_src,
        &target_dir.join("model.onnx"),
        cancel,
        progress,
    )?;

    let onnx_dir = onnx_src.parent().unwrap_or(&snapshot);
    for name in ["model.onnx_data", "model.onnx.data"] {
        let external_data = onnx_dir.join(name);
        if external_data.exists() {
            copy_file_with_limits(
                config,
                &external_data,
                &target_dir.join(name),
                cancel,
                progress,
            )?;
            break;
        }
    }

    for file in TOKENIZER_FILES {
        let source = snapshot.join(file);
        if !source.exists() {
            bail!(
                "missing tokenizer artifact {file} in {}",
                snapshot.display()
            );
        }
        copy_file_with_limits(config, &source, &target_dir.join(file), cancel, progress)?;
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

pub fn embedding_model_dir(config: &Config, model_name: &str) -> std::path::PathBuf {
    config.model_dir_for(model_name)
}

pub fn rerank_model_dir(config: &Config, model_name: &str) -> std::path::PathBuf {
    config.model_dir_for(model_name)
}

/// Map a subdirectory name under `model_cache_dir` back to a Hugging Face model id.
pub fn model_name_from_cache_dir_name(dir_name: &str) -> Option<String> {
    if let Some(rest) = dir_name.strip_prefix("models--") {
        let (org, repo) = rest.rsplit_once("--")?;
        if org.is_empty() || repo.is_empty() {
            return None;
        }
        Some(format!("{org}/{repo}"))
    } else if dir_name.contains("__") {
        Some(dir_name.replace("__", "/"))
    } else {
        None
    }
}

fn is_hf_cache_dir_name(dir_name: &str) -> bool {
    dir_name.starts_with("models--")
}

fn is_canonical_cache_dir_name(dir_name: &str) -> bool {
    dir_name.contains("__") && !dir_name.starts_with("models--")
}

/// A single directory entry under `model_cache_dir`, for the manual storage UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageEntry {
    /// Directory name on disk (e.g. `BAAI__bge-m3` or `models--rozgo--bge-reranker-v2-m3`).
    pub dir_name: String,
    /// Best-effort Hugging Face model id this directory belongs to, if recognizable.
    pub model_name: Option<String>,
    /// `canonical` (materialized model), `hf_cache` (fastembed/HF download cache), or `other`.
    pub kind: String,
    /// Total size on disk in bytes.
    pub size_bytes: u64,
    /// Whether this is a complete, loadable canonical model directory.
    pub complete: bool,
    /// Whether the model is currently referenced by a release or loaded in RAM.
    pub in_use: bool,
}

/// List every directory under `model_cache_dir` so the user can manage disk usage
/// manually. `in_use` reflects models that are release-referenced or loaded in RAM.
pub fn list_storage_entries(config: &Config, in_use: &HashSet<String>) -> Vec<StorageEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(&config.model_cache_dir) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if dir_name.is_empty() {
            continue;
        }
        let model_name =
            read_model_id_file(&path).or_else(|| model_name_from_cache_dir_name(&dir_name));
        let kind = if is_hf_cache_dir_name(&dir_name) {
            "hf_cache"
        } else if is_canonical_cache_dir_name(&dir_name) {
            "canonical"
        } else {
            "other"
        };
        let complete = model_is_complete(&path);
        let entry_in_use = model_name
            .as_ref()
            .is_some_and(|name| in_use.contains(name));
        entries.push(StorageEntry {
            dir_name,
            model_name,
            kind: kind.to_string(),
            size_bytes: crate::models::download::directory_size(&path),
            complete,
            in_use: entry_in_use,
        });
    }
    entries.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    entries
}

/// Delete a single directory under `model_cache_dir` by its on-disk name. The name
/// is validated to be a direct child of the cache dir (no path traversal).
pub fn delete_storage_entry(config: &Config, dir_name: &str) -> Result<()> {
    if dir_name.is_empty()
        || dir_name.contains('/')
        || dir_name.contains('\\')
        || dir_name.contains("..")
    {
        bail!("invalid storage entry name: {dir_name}");
    }
    let path = config.model_cache_dir.join(dir_name);
    if !path.is_dir() {
        bail!("storage entry not found: {dir_name}");
    }
    std::fs::remove_dir_all(&path)
        .with_context(|| format!("remove storage entry {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use std::fs;

    fn test_config(root: &Path) -> Config {
        let mut config = Config::for_test(root.to_path_buf(), "secret");
        config.model_cache_dir = root.join("models");
        config
    }

    #[test]
    fn model_name_from_hf_cache_slug() {
        assert_eq!(
            model_name_from_cache_dir_name("models--BAAI--bge-m3").as_deref(),
            Some("BAAI/bge-m3")
        );
        assert_eq!(
            model_name_from_cache_dir_name("Alibaba-NLP__gte-large-en-v1.5").as_deref(),
            Some("Alibaba-NLP/gte-large-en-v1.5")
        );
    }

    #[test]
    fn list_storage_marks_in_use_and_kind() {
        let root = tempfile::tempdir().unwrap();
        let config = test_config(root.path());
        let canonical = config.model_dir_for("BAAI/bge-m3");
        fs::create_dir_all(&canonical).unwrap();
        fs::write(canonical.join("model.onnx"), b"onnx").unwrap();
        let hf = config
            .model_cache_dir
            .join("models--rozgo--bge-reranker-v2-m3");
        fs::create_dir_all(&hf).unwrap();

        let mut in_use = HashSet::new();
        in_use.insert("BAAI/bge-m3".to_string());
        let entries = list_storage_entries(&config, &in_use);

        let canonical_entry = entries
            .iter()
            .find(|e| e.dir_name == "BAAI__bge-m3")
            .unwrap();
        assert_eq!(canonical_entry.kind, "canonical");
        assert!(canonical_entry.in_use);
        let hf_entry = entries
            .iter()
            .find(|e| e.dir_name == "models--rozgo--bge-reranker-v2-m3")
            .unwrap();
        assert_eq!(hf_entry.kind, "hf_cache");
        assert!(!hf_entry.in_use);
    }

    #[test]
    fn delete_storage_entry_rejects_traversal() {
        let root = tempfile::tempdir().unwrap();
        let config = test_config(root.path());
        fs::create_dir_all(&config.model_cache_dir).unwrap();
        assert!(delete_storage_entry(&config, "../secrets").is_err());
        assert!(delete_storage_entry(&config, "missing").is_err());
    }

    #[test]
    fn delete_storage_entry_removes_dir() {
        let root = tempfile::tempdir().unwrap();
        let config = test_config(root.path());
        let dir = config.model_cache_dir.join("models--org--x");
        fs::create_dir_all(&dir).unwrap();
        assert!(delete_storage_entry(&config, "models--org--x").is_ok());
        assert!(!dir.exists());
    }
}
