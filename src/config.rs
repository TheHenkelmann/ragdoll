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
    pub embedding_model: String,
    pub embedding_dim: usize,
    pub distance_metric: String,
    pub port: u16,
    pub onnx_num_threads: usize,
    pub hf_token: Option<String>,
    pub hf_hub_offline: bool,
    pub worker_poll_interval_ms: u64,
    pub job_lease_seconds: u64,
    pub max_attempts: u32,
    pub jwt_secret: String,
    pub superadmin_email: String,
    pub superadmin_password: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir: PathBuf = std::env::var("RAGDOLL_DATA_DIR")
            .context("RAGDOLL_DATA_DIR is required")?
            .into();

        let db_path = env_path("RAGDOLL_DB_PATH")
            .unwrap_or_else(|| data_dir.join("db").join("ragdoll.db"));
        let model_cache_dir =
            env_path("RAGDOLL_MODEL_CACHE_DIR").unwrap_or_else(|| data_dir.join("models"));
        let staging_dir =
            env_path("RAGDOLL_STAGING_DIR").unwrap_or_else(|| data_dir.join("staging"));

        let migrations_dir = std::env::var("RAGDOLL_MIGRATIONS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("migrations"));

        let static_dir = std::env::var("RAGDOLL_STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("frontend/dist"));

        let jwt_secret = std::env::var("RAGDOLL_JWT_SECRET")
            .context("RAGDOLL_JWT_SECRET is required")?;

        Ok(Self {
            data_dir: data_dir.clone(),
            db_path,
            model_cache_dir,
            staging_dir,
            migrations_dir,
            static_dir,
            embedding_model: std::env::var("RAGDOLL_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "BAAI/bge-m3".to_string()),
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
            jwt_secret,
            superadmin_email: std::env::var("RAGDOLL_SUPERADMIN_EMAIL")
                .unwrap_or_else(|_| "admin@ragdoll.ai".to_string()),
            superadmin_password: std::env::var("RAGDOLL_SUPERADMIN_PW").ok(),
        })
    }

    pub fn for_test(data_dir: PathBuf, jwt_secret: impl Into<String>) -> Self {
        let jwt_secret = jwt_secret.into();
        Self {
            data_dir: data_dir.clone(),
            db_path: data_dir.join("db").join("ragdoll.db"),
            model_cache_dir: data_dir.join("models"),
            staging_dir: data_dir.join("staging"),
            migrations_dir: PathBuf::from("migrations"),
            static_dir: data_dir.join("static"),
            embedding_model: "BAAI/bge-m3".to_string(),
            embedding_dim: 1024,
            distance_metric: "cosine".to_string(),
            port: 8080,
            onnx_num_threads: 1,
            hf_token: None,
            hf_hub_offline: true,
            worker_poll_interval_ms: 1000,
            job_lease_seconds: 300,
            max_attempts: 3,
            jwt_secret,
            superadmin_email: "admin@ragdoll.ai".to_string(),
            superadmin_password: Some("admin".to_string()),
        }
    }

    pub fn ensure_directories(&self) -> Result<()> {
        for dir in [
            &self.data_dir,
            self.db_path.parent().unwrap_or(Path::new(".")),
            &self.model_cache_dir,
            &self.staging_dir,
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
