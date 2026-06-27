// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::models::traits::{Embedder, ModelProvider, Reranker};
use crate::models::{EmbedModel, RerankModel};

type RerankerKey = (String, usize);

pub struct ModelRegistry {
    config: Config,
    embedders: Mutex<HashMap<String, Arc<dyn Embedder>>>,
    rerankers: Mutex<HashMap<RerankerKey, Arc<dyn Reranker>>>,
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
        // Fast path: brief lock just to read the cache. Never hold the map lock
        // across the (heavy, blocking) model load.
        {
            let map = self.embedders.lock().await;
            if let Some(model) = map.get(model_name) {
                // #region agent log
                dbg_log("A", "registry.rs:embedder", "cache hit (lock released immediately)", serde_json::json!({"model": model_name}));
                // #endregion
                return Ok(model.clone());
            }
        }
        // #region agent log
        let _t0 = std::time::Instant::now();
        let _inflight = LOADS_INFLIGHT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        dbg_log("F", "registry.rs:embedder", "loading EmbedModel in spawn_blocking (lock NOT held)", serde_json::json!({"model": model_name, "inflight_loads": _inflight, "thread": format!("{:?}", std::thread::current().id())}));
        // #endregion
        let config = self.config.clone();
        let name = model_name.to_string();
        let model: Arc<dyn Embedder> = tokio::task::spawn_blocking(move || {
            Ok::<Arc<dyn Embedder>, anyhow::Error>(Arc::new(EmbedModel::new(&config, &name)?))
        })
        .await
        .context("embedder load task panicked")??;
        // #region agent log
        LOADS_INFLIGHT.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        dbg_log("F", "registry.rs:embedder", "EmbedModel load finished (lock acquired only now to insert)", serde_json::json!({"model": model_name, "load_ms": _t0.elapsed().as_millis() as u64, "thread": format!("{:?}", std::thread::current().id())}));
        // #endregion
        let mut map = self.embedders.lock().await;
        let stored = map
            .entry(model_name.to_string())
            .or_insert_with(|| model.clone());
        Ok(stored.clone())
    }

    pub async fn reranker(&self, model_name: &str, max_length: usize) -> Result<Arc<dyn Reranker>> {
        let key = (model_name.to_string(), max_length);
        {
            let map = self.rerankers.lock().await;
            if let Some(model) = map.get(&key) {
                return Ok(model.clone());
            }
        }
        // #region agent log
        let _t0 = std::time::Instant::now();
        let _inflight = LOADS_INFLIGHT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        dbg_log("F", "registry.rs:reranker", "loading RerankModel in spawn_blocking (lock NOT held)", serde_json::json!({"model": model_name, "inflight_loads": _inflight, "thread": format!("{:?}", std::thread::current().id())}));
        // #endregion
        let config = self.config.clone();
        let name = model_name.to_string();
        let model: Arc<dyn Reranker> = tokio::task::spawn_blocking(move || {
            Ok::<Arc<dyn Reranker>, anyhow::Error>(Arc::new(RerankModel::new(&config, &name, max_length)?))
        })
        .await
        .context("reranker load task panicked")??;
        // #region agent log
        LOADS_INFLIGHT.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        dbg_log("F", "registry.rs:reranker", "RerankModel load finished (lock acquired only now to insert)", serde_json::json!({"model": model_name, "load_ms": _t0.elapsed().as_millis() as u64}));
        // #endregion
        let mut map = self.rerankers.lock().await;
        let stored = map.entry(key).or_insert_with(|| model.clone());
        Ok(stored.clone())
    }

    /// Drop in-memory ONNX sessions for models no longer referenced by any release.
    pub async fn evict_unreferenced(&self, keep: &HashSet<String>) -> (usize, usize) {
        let mut embedders = self.embedders.lock().await;
        let embed_before = embedders.len();
        embedders.retain(|name, _| keep.contains(name));
        let embed_evicted = embed_before.saturating_sub(embedders.len());

        let mut rerankers = self.rerankers.lock().await;
        let rerank_before = rerankers.len();
        rerankers.retain(|(name, _), _| keep.contains(name));
        let rerank_evicted = rerank_before.saturating_sub(rerankers.len());

        (embed_evicted, rerank_evicted)
    }

    /// Drop a single model from the in-memory cache (disk artifacts are untouched).
    pub async fn purge_model(&self, name: &str) -> (usize, usize) {
        // #region agent log
        dbg_log("A", "registry.rs:purge_model", "waiting for embedders lock (unload/delete clicked)", serde_json::json!({"model": name, "thread": format!("{:?}", std::thread::current().id())}));
        // #endregion
        let mut embedders = self.embedders.lock().await;
        // #region agent log
        dbg_log("A", "registry.rs:purge_model", "embedders lock acquired (unload/delete proceeding)", serde_json::json!({"model": name}));
        // #endregion
        let embed_evicted = usize::from(embedders.remove(name).is_some());

        let mut rerankers = self.rerankers.lock().await;
        let rerank_before = rerankers.len();
        rerankers.retain(|(model_name, _), _| model_name != name);
        let rerank_evicted = rerank_before.saturating_sub(rerankers.len());

        (embed_evicted, rerank_evicted)
    }

    /// Names of models currently loaded in gateway RAM.
    pub async fn list_loaded(&self) -> Vec<String> {
        // #region agent log
        dbg_log("A", "registry.rs:list_loaded", "waiting for embedders lock (models/status page load)", serde_json::json!({"thread": format!("{:?}", std::thread::current().id())}));
        // #endregion
        let embedders = self.embedders.lock().await;
        // #region agent log
        dbg_log("A", "registry.rs:list_loaded", "embedders lock acquired (models/status page proceeding)", serde_json::json!({}));
        // #endregion
        let rerankers = self.rerankers.lock().await;
        let mut names: Vec<String> = embedders.keys().cloned().collect();
        for (name, _) in rerankers.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        names.sort();
        names
    }

    /// Estimate RAM footprint from on-disk ONNX artifacts when a model is loaded.
    pub fn estimate_ram_bytes(config: &Config, model_name: &str) -> Option<u64> {
        let dir = config.model_dir_for(model_name);
        if !dir.exists() {
            return None;
        }
        let mut total = file_size(&dir.join("model.onnx"));
        for name in ["model.onnx_data", "model.onnx.data"] {
            let data = dir.join(name);
            if data.exists() {
                total = total.saturating_add(file_size(&data));
                break;
            }
        }
        (total > 0).then_some(total)
    }
}

fn file_size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

// #region agent log
static LOADS_INFLIGHT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

#[async_trait]
impl ModelProvider for ModelRegistry {
    async fn embedder(&self, model_name: &str) -> Result<Arc<dyn Embedder>> {
        ModelRegistry::embedder(self, model_name).await
    }

    async fn reranker(&self, model_name: &str, max_length: usize) -> Result<Arc<dyn Reranker>> {
        ModelRegistry::reranker(self, model_name, max_length).await
    }

    async fn evict_unreferenced(&self, keep: &HashSet<String>) -> (usize, usize) {
        ModelRegistry::evict_unreferenced(self, keep).await
    }

    async fn purge_model(&self, name: &str) -> (usize, usize) {
        ModelRegistry::purge_model(self, name).await
    }

    async fn list_loaded(&self) -> Vec<String> {
        ModelRegistry::list_loaded(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn evict_unreferenced_on_empty_cache_is_noop() {
        let config = Config::for_test(std::env::temp_dir(), "secret");
        let registry = ModelRegistry::new(config);
        let keep: HashSet<String> = HashSet::from(["BAAI/bge-m3".into()]);
        let (embedders, rerankers) = registry.evict_unreferenced(&keep).await;
        assert_eq!((embedders, rerankers), (0, 0));
    }

    #[tokio::test]
    async fn purge_model_reports_eviction_counts() {
        let config = Config::for_test(std::env::temp_dir(), "secret");
        let registry = ModelRegistry::new(config);
        let (e, r) = registry.purge_model("BAAI/bge-m3").await;
        assert_eq!((e, r), (0, 0));
    }
}
