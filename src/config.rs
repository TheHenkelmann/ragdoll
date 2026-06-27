// SPDX-License-Identifier: AGPL-3.0-only

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub model_cache_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub migrations_dir: PathBuf,
    pub static_dir: PathBuf,
    pub embedding_dim: usize,
    pub distance_metric: String,
    pub port: u16,
    pub onnx_num_threads: usize,
    pub rerank_pool_size: usize,
    pub hf_token: Option<String>,
    pub hf_hub_offline: bool,
    pub worker_poll_interval_ms: u64,
    pub job_lease_seconds: u64,
    pub max_attempts: u32,
    pub secret: String,
    pub superadmin_email: String,
    pub superadmin_password: Option<String>,
    pub backup_dir: PathBuf,
    pub backup_keep_daily: u32,
    pub backup_keep_manual: u32,
    /// Maximum number of model downloads that may run at the same time.
    pub model_download_max_concurrent: usize,
    /// Optional download bandwidth cap in bytes per second (`None` = unlimited).
    pub model_download_bandwidth_bps: Option<u64>,
    /// Read/write chunk size for throttled model downloads and materialization.
    pub model_download_write_chunk_bytes: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir: PathBuf = std::env::var("RAGDOLL_DATA_DIR")
            .context("RAGDOLL_DATA_DIR is required")?
            .into();

        let db_path =
            env_path("RAGDOLL_DB_PATH").unwrap_or_else(|| data_dir.join("db").join("ragdoll.db"));
        let model_cache_dir =
            env_path("RAGDOLL_MODEL_CACHE_DIR").unwrap_or_else(|| data_dir.join("models"));
        let staging_dir =
            env_path("RAGDOLL_STAGING_DIR").unwrap_or_else(|| data_dir.join("staging"));
        let backup_dir = env_path("RAGDOLL_BACKUP_DIR").unwrap_or_else(|| data_dir.join("backups"));

        let migrations_dir = std::env::var("RAGDOLL_MIGRATIONS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("migrations"));

        let static_dir = std::env::var("RAGDOLL_STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("frontend/dist"));

        let secret = std::env::var("RAGDOLL_SECRET").context("RAGDOLL_SECRET is required")?;

        Ok(Self {
            data_dir: data_dir.clone(),
            db_path,
            model_cache_dir,
            staging_dir,
            migrations_dir,
            static_dir,
            embedding_dim: std::env::var("RAGDOLL_EMBEDDING_DIM")
                .unwrap_or_else(|_| "1024".to_string())
                .parse()
                .context("invalid RAGDOLL_EMBEDDING_DIM")?,
            distance_metric: std::env::var("RAGDOLL_DISTANCE_METRIC")
                .unwrap_or_else(|_| "cosine".to_string()),
            port: std::env::var("RAGDOLL_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .context("invalid RAGDOLL_PORT")?,
            onnx_num_threads: std::env::var("RAGDOLL_ONNX_NUM_THREADS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .context("invalid RAGDOLL_ONNX_NUM_THREADS")?,
            rerank_pool_size: std::env::var("RAGDOLL_RERANK_POOL_SIZE")
                .unwrap_or_else(|_| "1".to_string())
                .parse::<usize>()
                .context("invalid RAGDOLL_RERANK_POOL_SIZE")?
                .max(1),
            hf_token: std::env::var("RAGDOLL_HF_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            hf_hub_offline: std::env::var("RAGDOLL_HF_HUB_OFFLINE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            worker_poll_interval_ms: std::env::var("RAGDOLL_WORKER_POLL_INTERVAL_MS")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .context("invalid RAGDOLL_WORKER_POLL_INTERVAL_MS")?,
            job_lease_seconds: std::env::var("RAGDOLL_JOB_LEASE_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .context("invalid RAGDOLL_JOB_LEASE_SECONDS")?,
            max_attempts: std::env::var("RAGDOLL_MAX_ATTEMPTS")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .context("invalid RAGDOLL_MAX_ATTEMPTS")?,
            secret,
            superadmin_email: std::env::var("RAGDOLL_SUPERADMIN_EMAIL")
                .unwrap_or_else(|_| "admin@ragdoll.ai".to_string()),
            superadmin_password: std::env::var("RAGDOLL_SUPERADMIN_PW").ok(),
            backup_dir,
            backup_keep_daily: std::env::var("RAGDOLL_BACKUP_KEEP_DAILY")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .context("invalid RAGDOLL_BACKUP_KEEP_DAILY")?,
            backup_keep_manual: std::env::var("RAGDOLL_BACKUP_KEEP_MANUAL")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("invalid RAGDOLL_BACKUP_KEEP_MANUAL")?,
            model_download_max_concurrent: std::env::var("RAGDOLL_MODEL_DOWNLOAD_MAX_CONCURRENT")
                .unwrap_or_else(|_| "1".to_string())
                .parse::<usize>()
                .context("invalid RAGDOLL_MODEL_DOWNLOAD_MAX_CONCURRENT")?
                .max(1),
            model_download_bandwidth_bps: std::env::var("RAGDOLL_MODEL_DOWNLOAD_BANDWIDTH_BPS")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| s.parse::<u64>())
                .transpose()
                .context("invalid RAGDOLL_MODEL_DOWNLOAD_BANDWIDTH_BPS")?,
            model_download_write_chunk_bytes: std::env::var(
                "RAGDOLL_MODEL_DOWNLOAD_WRITE_CHUNK_BYTES",
            )
            .unwrap_or_else(|_| (256 * 1024).to_string())
            .parse::<usize>()
            .context("invalid RAGDOLL_MODEL_DOWNLOAD_WRITE_CHUNK_BYTES")?
            .clamp(16 * 1024, 4 * 1024 * 1024),
        })
    }

    pub fn for_test(data_dir: PathBuf, secret: impl Into<String>) -> Self {
        let secret = secret.into();
        Self {
            data_dir: data_dir.clone(),
            db_path: data_dir.join("db").join("ragdoll.db"),
            model_cache_dir: data_dir.join("models"),
            staging_dir: data_dir.join("staging"),
            migrations_dir: PathBuf::from("migrations"),
            static_dir: data_dir.join("static"),
            embedding_dim: 1024,
            distance_metric: "cosine".to_string(),
            port: 8080,
            onnx_num_threads: 1,
            rerank_pool_size: 1,
            hf_token: None,
            hf_hub_offline: true,
            worker_poll_interval_ms: 1000,
            job_lease_seconds: 300,
            max_attempts: 3,
            secret,
            superadmin_email: "admin@ragdoll.ai".to_string(),
            superadmin_password: Some("admin".to_string()),
            backup_dir: data_dir.join("backups"),
            backup_keep_daily: 7,
            backup_keep_manual: 10,
            model_download_max_concurrent: 1,
            model_download_bandwidth_bps: None,
            model_download_write_chunk_bytes: 256 * 1024,
        }
    }

    pub fn ensure_directories(&self) -> Result<()> {
        for dir in [
            &self.data_dir,
            self.db_path.parent().unwrap_or(Path::new(".")),
            &self.model_cache_dir,
            &self.staging_dir,
            &self.backup_dir,
        ] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("failed to create directory {}", dir.display()))?;
        }
        Ok(())
    }

    pub fn model_dir_for(&self, model_name: &str) -> PathBuf {
        self.model_cache_dir.join(sanitize_model_name(model_name))
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

pub fn sanitize_model_name(model_name: &str) -> String {
    model_name.replace('/', "__")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_model_name_replaces_slashes() {
        assert_eq!(sanitize_model_name("BAAI/bge-m3"), "BAAI__bge-m3");
    }

    #[test]
    fn for_test_config_has_required_paths() {
        let dir = std::env::temp_dir().join("ragdoll-test-config");
        let config = Config::for_test(dir.clone(), "secret");
        assert_eq!(config.db_path, dir.join("db").join("ragdoll.db"));
        assert_eq!(config.embedding_dim, 1024);
    }
}
