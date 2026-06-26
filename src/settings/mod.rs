// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::db::{DbError, DbPool};

pub const DEFAULT_TOP_K: u32 = 10;
pub const DEFAULT_RERANK_CANDIDATES: u32 = 50;
pub const DEFAULT_MIN_SCORE: f32 = 0.0;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PayloadStorage {
    #[default]
    PerRequest,
    Forced,
    Forbidden,
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
        _ => {}
    }
    Ok(())
}

pub async fn patch_settings(
    pool: &DbPool,
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
    load_settings(pool, release_id).await
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
}
