// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::db::{DbError, DbPool};

pub const DEFAULT_TOP_K: u32 = 10;
pub const DEFAULT_RERANK_CANDIDATES: u32 = 20;
pub const DEFAULT_MIN_SCORE: f32 = 0.0;
pub const DEFAULT_MIN_SEMANTIC_SCORE: f64 = 0.5;
pub const DEFAULT_MIN_RERANK_SCORE: f64 = 0.5;
pub const DEFAULT_RERANK_MAX_LENGTH: u32 = 256;
pub const RERANK_MAX_LENGTH_UNCAPPED: usize = 8192;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PayloadStorage {
    #[default]
    PerRequest,
    Forced,
    Forbidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DedupPolicy {
    Skip,
    Reject,
    #[default]
    Replace,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeSettings {
    pub embedding_model: String,
    pub rerank_model: String,
    pub payload_storage: PayloadStorage,
    pub chunking_strategy: String,
    pub sentence_buffer: u32,
    pub breakpoint_percentile: u32,
    pub min_chunk_tokens: u32,
    pub max_chunk_tokens: u32,
    pub max_upload_size: u64,
    pub max_batch_size: u32,
    pub generation_allowed: bool,
    pub rerank_max_length: u32,
    pub dedup_policy: DedupPolicy,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            embedding_model: "BAAI/bge-m3".to_string(),
            rerank_model: "BAAI/bge-reranker-v2-m3".to_string(),
            payload_storage: PayloadStorage::PerRequest,
            chunking_strategy: "semantic_split".to_string(),
            sentence_buffer: 2,
            breakpoint_percentile: 95,
            min_chunk_tokens: 64,
            max_chunk_tokens: 512,
            max_upload_size: 52_428_800,
            max_batch_size: 100,
            generation_allowed: true,
            rerank_max_length: DEFAULT_RERANK_MAX_LENGTH,
            dedup_policy: DedupPolicy::Replace,
        }
    }
}

#[derive(Clone)]
pub struct SettingsCache {
    inner: Arc<RwLock<HashMap<String, RuntimeSettings>>>,
}

impl Default for SettingsCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn invalidate(&self, release_id: &str) {
        self.inner.write().await.remove(release_id);
    }

    pub async fn clear_all(&self) {
        self.inner.write().await.clear();
    }

    pub async fn get_or_load(
        &self,
        pool: &DbPool,
        release_id: &str,
    ) -> Result<RuntimeSettings, DbError> {
        if let Some(settings) = self.inner.read().await.get(release_id).cloned() {
            return Ok(settings);
        }
        let settings = load_settings(pool, release_id).await?;
        self.inner
            .write()
            .await
            .insert(release_id.to_string(), settings.clone());
        Ok(settings)
    }
}

pub async fn load_settings(pool: &DbPool, release_id: &str) -> Result<RuntimeSettings, DbError> {
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query(
            "SELECT key, value FROM settings WHERE release_id = ?1",
            [release_id],
        )
        .await?;

    let mut settings = RuntimeSettings::default();
    while let Some(row) = rows.next().await? {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        apply_setting(&mut settings, &key, &value)?;
    }
    Ok(settings)
}

fn apply_setting(settings: &mut RuntimeSettings, key: &str, raw: &str) -> Result<(), DbError> {
    let value: Value = serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()));

    match key {
        "embedding_model" => settings.embedding_model = parse_string(value)?,
        "rerank_model" => settings.rerank_model = parse_string(value)?,
        "payload_storage" => {
            settings.payload_storage = match parse_string(value)?.as_str() {
                "forced" => PayloadStorage::Forced,
                "forbidden" => PayloadStorage::Forbidden,
                _ => PayloadStorage::PerRequest,
            }
        }
        "chunking_strategy" => settings.chunking_strategy = parse_string(value)?,
        "sentence_buffer" => settings.sentence_buffer = parse_u32(value)?,
        "breakpoint_percentile" => settings.breakpoint_percentile = parse_u32(value)?,
        "min_chunk_tokens" => settings.min_chunk_tokens = parse_u32(value)?,
        "max_chunk_tokens" => settings.max_chunk_tokens = parse_u32(value)?,
        "max_upload_size" => settings.max_upload_size = parse_u64(value)?,
        "max_batch_size" => settings.max_batch_size = parse_u32(value)?,
        "generation_allowed" => settings.generation_allowed = parse_bool(value)?,
        "rerank_max_length" => settings.rerank_max_length = parse_rerank_max_length(value)?,
        "dedup_policy" => {
            settings.dedup_policy = match parse_string(value)?.as_str() {
                "reject" => DedupPolicy::Reject,
                "replace" => DedupPolicy::Replace,
                _ => DedupPolicy::Skip,
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_rerank_max_length(value: Value) -> Result<u32, DbError> {
    let raw = parse_u32(value)?;
    match raw {
        0 | 128 | 256 | 512 | 1024 => Ok(raw),
        _ => Ok(DEFAULT_RERANK_MAX_LENGTH),
    }
}

pub fn effective_rerank_max_length(settings: &RuntimeSettings) -> usize {
    match settings.rerank_max_length {
        0 => RERANK_MAX_LENGTH_UNCAPPED,
        n => n as usize,
    }
}

pub async fn patch_settings(
    pool: &DbPool,
    config: &Config,
    release_id: &str,
    updates: &serde_json::Map<String, Value>,
) -> Result<RuntimeSettings, DbError> {
    let conn = pool.connect_one().await?;
    for (key, value) in updates {
        let serialized = serde_json::to_string(value)
            .map_err(|e| DbError::InvalidInput(format!("serialize setting: {e}")))?;
        conn.execute(
            "INSERT INTO settings (release_id, key, value) VALUES (?1, ?2, ?3)
             ON CONFLICT(release_id, key) DO UPDATE SET value = excluded.value",
            (release_id, key.as_str(), serialized.as_str()),
        )
        .await?;
    }
    let settings = load_settings(pool, release_id).await?;
    let require_models_present =
        updates.contains_key("embedding_model") || updates.contains_key("rerank_model");
    validate_runtime_settings(&settings, config, require_models_present)?;
    Ok(settings)
}

pub fn validate_runtime_settings(
    settings: &RuntimeSettings,
    config: &Config,
    require_models_present: bool,
) -> Result<(), DbError> {
    use crate::models::bootstrap::model_is_complete;
    use crate::models::mapping::{is_supported_embed_model, is_supported_rerank_model};

    if !is_supported_embed_model(&settings.embedding_model) {
        return Err(DbError::InvalidInput(format!(
            "unsupported embedding_model: {}",
            settings.embedding_model
        )));
    }
    if !is_supported_rerank_model(&settings.rerank_model) {
        return Err(DbError::InvalidInput(format!(
            "unsupported rerank_model: {}",
            settings.rerank_model
        )));
    }

    if !require_models_present {
        return Ok(());
    }

    let embed_dir = config.model_dir_for(&settings.embedding_model);
    if !model_is_complete(&embed_dir) {
        return Err(DbError::InvalidInput(format!(
            "embedding model {} is not downloaded; download it from the Models page first",
            settings.embedding_model
        )));
    }
    let rerank_dir = config.model_dir_for(&settings.rerank_model);
    if !model_is_complete(&rerank_dir) {
        return Err(DbError::InvalidInput(format!(
            "rerank model {} is not downloaded; download it from the Models page first",
            settings.rerank_model
        )));
    }
    Ok(())
}

fn parse_string(value: Value) -> Result<String, DbError> {
    match value {
        Value::String(s) => Ok(s),
        other => Ok(other.to_string()),
    }
}

fn parse_u32(value: Value) -> Result<u32, DbError> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .context("invalid u32")
            .map_err(|e| DbError::InvalidInput(e.to_string())),
        Value::String(s) => s
            .parse()
            .map_err(|_| DbError::InvalidInput(format!("invalid u32: {s}"))),
        _ => Err(DbError::InvalidInput("invalid u32".into())),
    }
}

fn parse_bool(value: Value) -> Result<bool, DbError> {
    match value {
        Value::Bool(b) => Ok(b),
        Value::String(s) => match s.to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            other => Err(DbError::InvalidInput(format!("invalid bool: {other}"))),
        },
        Value::Number(n) => Ok(n.as_i64().unwrap_or(0) != 0),
        _ => Err(DbError::InvalidInput("invalid bool".into())),
    }
}

fn parse_u64(value: Value) -> Result<u64, DbError> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .context("invalid u64")
            .map_err(|e| DbError::InvalidInput(e.to_string())),
        Value::String(s) => s
            .parse()
            .map_err(|_| DbError::InvalidInput(format!("invalid u64: {s}"))),
        _ => Err(DbError::InvalidInput("invalid u64".into())),
    }
}

pub fn effective_store_payload(
    settings: &RuntimeSettings,
    store_payload_param: bool,
    playground: bool,
) -> bool {
    if playground {
        return true;
    }
    match settings.payload_storage {
        PayloadStorage::Forced => true,
        PayloadStorage::Forbidden => false,
        PayloadStorage::PerRequest => store_payload_param,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn validate_requires_downloaded_models() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        let settings = RuntimeSettings::default();
        assert!(validate_runtime_settings(&settings, &config, true).is_err());
        assert!(validate_runtime_settings(&settings, &config, false).is_ok());
    }

    #[test]
    fn effective_store_payload_respects_policy() {
        let settings = RuntimeSettings {
            payload_storage: PayloadStorage::Forbidden,
            ..RuntimeSettings::default()
        };
        assert!(!effective_store_payload(&settings, true, false));
        assert!(effective_store_payload(&settings, false, true));

        let settings = RuntimeSettings {
            payload_storage: PayloadStorage::Forced,
            ..RuntimeSettings::default()
        };
        assert!(effective_store_payload(&settings, false, false));

        let settings = RuntimeSettings {
            payload_storage: PayloadStorage::PerRequest,
            ..RuntimeSettings::default()
        };
        assert!(effective_store_payload(&settings, true, false));
        assert!(!effective_store_payload(&settings, false, false));
    }

    #[test]
    fn effective_rerank_max_length_maps_zero_to_uncapped() {
        let settings = RuntimeSettings {
            rerank_max_length: 0,
            ..RuntimeSettings::default()
        };
        assert_eq!(
            effective_rerank_max_length(&settings),
            RERANK_MAX_LENGTH_UNCAPPED
        );

        let settings = RuntimeSettings::default();
        assert_eq!(effective_rerank_max_length(&settings), 256);
    }
}
