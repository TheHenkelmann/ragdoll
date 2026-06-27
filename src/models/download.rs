// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use serde::Serialize;
use tokio::sync::{broadcast, Semaphore};

use crate::config::Config;
use crate::models::bootstrap::{
    ensure_single_model_blocking_cancellable, hf_cache_slugs, model_is_complete,
};
use crate::models::mapping::{
    embedding_model_enum, is_supported_embed_model, is_supported_rerank_model, reranker_model_enum,
};
use crate::models::traits::ModelProvider;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ModelDownloadEvent {
    Started {
        name: String,
    },
    Progress {
        name: String,
        bytes: u64,
        total: Option<u64>,
        message: String,
    },
    Materializing {
        name: String,
    },
    Testing {
        name: String,
    },
    Complete {
        name: String,
        latency_ms: u64,
    },
    Error {
        name: String,
        message: String,
    },
    Cancelled {
        name: String,
    },
    /// Whether the client should offer a cancel button for this download.
    Cancellable {
        name: String,
        cancellable: bool,
    },
}

#[derive(Clone)]
pub struct DownloadNotifier {
    tx: broadcast::Sender<ModelDownloadEvent>,
    name: String,
    cancellable: Arc<AtomicBool>,
}

impl DownloadNotifier {
    pub fn new(
        tx: broadcast::Sender<ModelDownloadEvent>,
        name: String,
        cancellable: Arc<AtomicBool>,
    ) -> Self {
        Self {
            tx,
            name,
            cancellable,
        }
    }

    pub fn set_cancellable(&self, cancellable: bool) {
        self.cancellable.store(cancellable, Ordering::Relaxed);
        let _ = self.tx.send(ModelDownloadEvent::Cancellable {
            name: self.name.clone(),
            cancellable,
        });
    }
}

struct DownloadJob {
    tx: broadcast::Sender<ModelDownloadEvent>,
    done: AtomicBool,
    cancel: Arc<AtomicBool>,
    cancellable: Arc<AtomicBool>,
}

pub struct ModelDownloadManager {
    jobs: Mutex<HashMap<String, Arc<DownloadJob>>>,
    concurrency: Arc<Semaphore>,
}

impl ModelDownloadManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            concurrency: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    pub fn list_active(&self) -> Vec<String> {
        let guard = self.jobs.lock().unwrap();
        guard
            .iter()
            .filter(|(_, job)| {
                !job.done.load(Ordering::Relaxed) && !job.cancel.load(Ordering::Relaxed)
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Request cancellation of an in-flight download. Returns `true` if a running
    /// job was found and signalled. The job stops at the next safe checkpoint and
    /// cleans up its partial artifacts.
    pub fn cancel(&self, name: &str) -> bool {
        let guard = self.jobs.lock().unwrap();
        match guard.get(name) {
            Some(job)
                if !job.done.load(Ordering::Relaxed) && job.cancellable.load(Ordering::Relaxed) =>
            {
                job.cancel.store(true, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    pub fn subscribe_or_start(
        &self,
        config: Config,
        models: Arc<dyn ModelProvider>,
        name: String,
    ) -> broadcast::Receiver<ModelDownloadEvent> {
        let mut guard = self.jobs.lock().unwrap();
        // Never start a second job for the same model while one is still running
        // (including a cancelled job that is still winding down).
        if let Some(job) = guard.get(&name) {
            if !job.done.load(Ordering::Relaxed) {
                let rx = job.tx.subscribe();
                let _ = job.tx.send(ModelDownloadEvent::Cancellable {
                    name: name.clone(),
                    cancellable: job.cancellable.load(Ordering::Relaxed),
                });
                return rx;
            }
        }

        let (tx, rx) = broadcast::channel(64);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancellable = Arc::new(AtomicBool::new(false));
        let job = Arc::new(DownloadJob {
            tx: tx.clone(),
            done: AtomicBool::new(false),
            cancel: cancel.clone(),
            cancellable: cancellable.clone(),
        });
        guard.insert(name.clone(), job.clone());
        let concurrency = self.concurrency.clone();
        drop(guard);

        let notifier = DownloadNotifier::new(tx.clone(), name.clone(), cancellable);

        tokio::spawn(async move {
            let _permit = concurrency.acquire().await;
            run_download_job(config, models, name, tx, cancel, notifier).await;
            job.done.store(true, Ordering::Relaxed);
        });

        rx
    }
}

pub async fn test_model_inference(
    models: &Arc<dyn ModelProvider>,
    model_name: &str,
    rerank_max_length: usize,
) -> Result<u64> {
    let start = Instant::now();
    if embedding_model_enum(model_name).is_ok() || is_supported_embed_model(model_name) {
        let embedder = models.embedder(model_name).await?;
        embedder
            .embed_one("ragdoll model test")
            .await
            .context("embedding test inference failed")?;
    } else if reranker_model_enum(model_name).is_ok() || is_supported_rerank_model(model_name) {
        let reranker = models.reranker(model_name, rerank_max_length).await?;
        reranker
            .rerank("test query", &[String::from("test document")])
            .await
            .context("rerank test inference failed")?;
    } else {
        anyhow::bail!("unsupported model: {model_name}");
    }
    Ok(start.elapsed().as_millis() as u64)
}

async fn run_download_job(
    config: Config,
    models: Arc<dyn ModelProvider>,
    name: String,
    tx: broadcast::Sender<ModelDownloadEvent>,
    cancel: Arc<AtomicBool>,
    notifier: DownloadNotifier,
) {
    let _ = tx.send(ModelDownloadEvent::Started { name: name.clone() });

    let is_fastembed_preset =
        embedding_model_enum(&name).is_ok() || reranker_model_enum(&name).is_ok();
    // User-defined HF downloads are cancellable between files; fastembed preset
    // downloads are not until the blocking try_new call finishes.
    notifier.set_cancellable(!is_fastembed_preset);

    let target_dir = config.model_dir_for(&name);
    if !model_is_complete(&target_dir) {
        if config.hf_hub_offline {
            let _ = tx.send(ModelDownloadEvent::Error {
                name: name.clone(),
                message: "model missing and HF_HUB_OFFLINE is enabled".to_string(),
            });
            return;
        }

        let cache_dirs: Vec<_> = hf_cache_slugs(&name)
            .into_iter()
            .map(|slug| config.model_cache_dir.join(slug))
            .chain(std::iter::once(target_dir.clone()))
            .collect();
        let initial_size: u64 = cache_dirs.iter().map(|d| directory_size(d)).sum();
        let progress_bytes = Arc::new(AtomicU64::new(initial_size));

        let total = fetch_expected_download_size(&name).await;
        let _ = tx.send(ModelDownloadEvent::Progress {
            name: name.clone(),
            bytes: 0,
            total,
            message: progress_message(0, total),
        });

        let running = Arc::new(AtomicBool::new(true));
        let running_poll = running.clone();
        let cancel_poll = cancel.clone();
        let name_poll = name.clone();
        let tx_poll = tx.clone();
        let progress_poll = progress_bytes.clone();
        let cache_dirs_poll = cache_dirs.clone();
        let poller = tokio::spawn(async move {
            while running_poll.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if cancel_poll.load(Ordering::Relaxed) {
                    break;
                }
                // Prefer the cheap atomic counter maintained by throttled writes.
                // Fall back to a blocking directory walk for fastembed downloads
                // that write outside our copy path (only when no progress yet).
                let tracked = progress_poll.load(Ordering::Relaxed);
                let downloaded = if tracked > initial_size {
                    tracked.saturating_sub(initial_size)
                } else {
                    let cache_dirs_blocking = cache_dirs_poll.clone();
                    let initial_blocking = initial_size;
                    tokio::task::spawn_blocking(move || {
                        let size: u64 = cache_dirs_blocking.iter().map(|d| directory_size(d)).sum();
                        size.saturating_sub(initial_blocking)
                    })
                    .await
                    .unwrap_or_default()
                };
                let _ = tx_poll.send(ModelDownloadEvent::Progress {
                    name: name_poll.clone(),
                    bytes: downloaded,
                    total,
                    message: progress_message(downloaded, total),
                });
            }
        });

        let config_dl = config.clone();
        let name_dl = name.clone();
        let cancel_dl = cancel.clone();
        let progress_dl = progress_bytes.clone();
        let notifier_dl = notifier.clone();
        let download_result = tokio::task::spawn_blocking(move || {
            ensure_single_model_blocking_cancellable(
                &config_dl,
                &name_dl,
                true,
                &cancel_dl,
                Some(&progress_dl),
                Some(&notifier_dl),
            )
        })
        .await;

        running.store(false, Ordering::Relaxed);
        let _ = poller.await;

        // A cancel may land while the blocking download is still finishing; treat
        // any post-download cancel flag as a cancellation regardless of result.
        if cancel.load(Ordering::Relaxed) {
            cleanup_after_cancel(&config, &name);
            let _ = tx.send(ModelDownloadEvent::Cancelled { name });
            return;
        }

        match download_result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                let _ = tx.send(ModelDownloadEvent::Error {
                    name: name.clone(),
                    message: err.to_string(),
                });
                return;
            }
            Err(err) => {
                let _ = tx.send(ModelDownloadEvent::Error {
                    name: name.clone(),
                    message: err.to_string(),
                });
                return;
            }
        }

        let _ = tx.send(ModelDownloadEvent::Materializing { name: name.clone() });

        if !model_is_complete(&target_dir) {
            let _ = tx.send(ModelDownloadEvent::Error {
                name: name.clone(),
                message: "model download finished but artifacts are incomplete".into(),
            });
            return;
        }
    }

    if cancel.load(Ordering::Relaxed) {
        cleanup_after_cancel(&config, &name);
        let _ = tx.send(ModelDownloadEvent::Cancelled { name });
        return;
    }

    notifier.set_cancellable(false);
    let _ = tx.send(ModelDownloadEvent::Testing { name: name.clone() });

    let max_len = crate::settings::DEFAULT_RERANK_MAX_LENGTH as usize;
    // #region agent log
    dbg_log("B", "download.rs:run_download_job", "download finished -> calling test_model_inference (will load model into registry, holds registry lock)", serde_json::json!({"model": name, "thread": format!("{:?}", std::thread::current().id())}));
    // #endregion
    match test_model_inference(&models, &name, max_len).await {
        Ok(latency_ms) => {
            let _ = tx.send(ModelDownloadEvent::Complete { name, latency_ms });
        }
        Err(err) => {
            let _ = tx.send(ModelDownloadEvent::Error {
                name,
                message: err.to_string(),
            });
        }
    }
}

/// Remove partial artifacts left behind by a cancelled download. Only removes
/// the model when it is not a complete, usable model on disk.
fn cleanup_after_cancel(config: &Config, name: &str) {
    let target_dir = config.model_dir_for(name);
    if model_is_complete(&target_dir) {
        return;
    }
    if let Err(err) = crate::models::bootstrap::remove_incomplete_model_artifacts(config, name) {
        tracing::warn!(model = name, error = %err, "failed to clean up cancelled download");
    }
}

pub fn directory_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0u64;
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total = total.saturating_add(directory_size(&path));
        } else if let Ok(meta) = entry.metadata() {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

/// Tokenizer/config artifacts fastembed materializes alongside the ONNX graph.
const EXPECTED_AUX_FILES: [&str; 4] = [
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

#[derive(serde::Deserialize)]
struct TreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    #[serde(default)]
    size: u64,
}

/// Best-effort estimate of how many bytes will be downloaded for a model by
/// querying the Hugging Face tree API and summing only the files fastembed
/// actually materializes (the ONNX graph, its optional external data blob, and
/// the tokenizer/config files). Returns `None` when the size cannot be
/// determined (offline, network error, unexpected repo layout).
async fn fetch_expected_download_size(model_name: &str) -> Option<u64> {
    let slug = hf_cache_slugs(model_name).into_iter().next()?;
    let repo = slug.strip_prefix("models--")?.replace("--", "/");
    let url = format!("https://huggingface.co/api/models/{repo}/tree/main?recursive=true");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    let entries: Vec<TreeEntry> = serde_json::from_str(&body).ok()?;
    let files: Vec<&TreeEntry> = entries.iter().filter(|e| e.kind == "file").collect();

    let onnx = files
        .iter()
        .find(|e| e.path == "onnx/model.onnx")
        .or_else(|| files.iter().find(|e| e.path == "model.onnx"))?;
    let mut total = onnx.size;

    let onnx_prefix = if onnx.path.starts_with("onnx/") {
        "onnx/"
    } else {
        ""
    };
    for data_name in ["model.onnx_data", "model.onnx.data"] {
        let candidate = format!("{onnx_prefix}{data_name}");
        if let Some(entry) = files.iter().find(|e| e.path == candidate) {
            total = total.saturating_add(entry.size);
            break;
        }
    }

    for aux in EXPECTED_AUX_FILES {
        if let Some(entry) = files
            .iter()
            .find(|e| e.path.rsplit('/').next() == Some(aux))
        {
            total = total.saturating_add(entry.size);
        }
    }

    (total > 0).then_some(total)
}

// #region agent log
fn dbg_log(hyp: &str, location: &str, message: &str, data: serde_json::Value) {
    use std::io::Write;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = serde_json::json!({
        "sessionId": "a04a75",
        "runId": "pre-fix",
        "hypothesisId": hyp,
        "location": location,
        "message": message,
        "data": data,
        "timestamp": ts,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/Users/henkelmann/Documents/PRIVAT/henley/.cursor/debug-a04a75.log")
    {
        let _ = writeln!(f, "{line}");
    }
}
// #endregion

fn progress_message(downloaded: u64, total: Option<u64>) -> String {
    match total {
        Some(total) if total > 0 => {
            format!(
                "{} / {} downloaded",
                format_size(downloaded),
                format_size(total)
            )
        }
        _ => format!("{} downloaded", format_size(downloaded)),
    }
}

fn format_size(bytes: u64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    if bytes as f64 >= GB {
        format!("{:.1} GB", bytes as f64 / GB)
    } else if bytes as f64 >= MB {
        format!("{:.1} MB", bytes as f64 / MB)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn directory_size_sums_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), vec![0u8; 100]).unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("b.txt"), vec![0u8; 50]).unwrap();
        assert_eq!(directory_size(dir.path()), 150);
    }

    #[test]
    fn format_size_uses_mb_for_medium_sizes() {
        assert!(format_size(2 * 1024 * 1024).contains("MB"));
    }

    #[test]
    fn progress_message_includes_total_when_known() {
        let msg = progress_message(1024 * 1024, Some(4 * 1024 * 1024));
        assert!(msg.contains('/'));
        assert!(msg.contains("MB"));
    }

    #[test]
    fn progress_message_omits_total_when_unknown() {
        let msg = progress_message(1024 * 1024, None);
        assert!(!msg.contains('/'));
    }

    #[test]
    fn list_active_empty_when_no_jobs() {
        let mgr = ModelDownloadManager::new(1);
        assert!(mgr.list_active().is_empty());
    }
}
