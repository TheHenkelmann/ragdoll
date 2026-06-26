// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use anyhow::Context;
use axum::extract::State;
use axum::Extension;
use axum::Json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::api::batch::{BatchItemResult, BatchResponse};
use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{require_superadmin, AuthContext};
use crate::release::ReleaseCtx;

#[derive(Debug, serde::Deserialize)]
pub struct SourceInput {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub source_type: String,
    pub content: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
pub struct SourceEnqueueResult {
    pub source_id: String,
    pub job_id: String,
}

pub async fn post_sources(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(items): Json<Vec<SourceInput>>,
) -> Result<BatchResponse<SourceEnqueueResult>, ApiError> {
    require_superadmin(&auth)?;
    let settings = state
        .settings_cache
        .get_or_load(&state.pool, &ctx.release_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if items.len() as u32 > settings.max_batch_size {
        return Err(ApiError::bad_request(format!(
            "batch size {} exceeds max {}",
            items.len(),
            settings.max_batch_size
        )));
    }

    let mut results = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        results.push(create_source(&state, &ctx, index, item, &settings).await);
    }
    Ok(BatchResponse { items: results })
}

pub async fn put_sources(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(items): Json<Vec<SourceInput>>,
) -> Result<BatchResponse<SourceEnqueueResult>, ApiError> {
    require_superadmin(&auth)?;
    let settings = state
        .settings_cache
        .get_or_load(&state.pool, &ctx.release_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if items.len() as u32 > settings.max_batch_size {
        return Err(ApiError::bad_request("batch too large"));
    }

    let mut results = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        if item.id.is_none() {
            results.push(BatchItemResult::err(
                index,
                axum::http::StatusCode::BAD_REQUEST,
                "PUT requires id",
            ));
            continue;
        }
        let source_id = item.id.clone().unwrap();
        if let Err(err) = delete_source_internal(&state, &ctx, &source_id).await {
            results.push(BatchItemResult::err(
                index,
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
            continue;
        }
        results.push(create_source(&state, &ctx, index, item, &settings).await);
    }
    Ok(BatchResponse { items: results })
}

async fn create_source(
    state: &AppState,
    ctx: &ReleaseCtx,
    index: usize,
    item: SourceInput,
    settings: &crate::settings::RuntimeSettings,
) -> BatchItemResult<SourceEnqueueResult> {
    match create_source_internal(state, ctx, item, settings).await {
        Ok(result) => BatchItemResult::ok(index, result),
        Err(err) => {
            BatchItemResult::err(index, axum::http::StatusCode::BAD_REQUEST, err.to_string())
        }
    }
}

const SUPPORTED_FILE_EXTENSIONS: &[&str] = &[
    ".txt", ".md", ".csv", ".json", ".pdf", ".docx", ".xlsx", ".xlsm", ".pptx",
];

fn file_extension_from_name(name: &str) -> anyhow::Result<&'static str> {
    let lower = name.to_lowercase();
    SUPPORTED_FILE_EXTENSIONS
        .iter()
        .find(|ext| lower.ends_with(**ext))
        .copied()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unsupported file type; supported extensions: {}",
                SUPPORTED_FILE_EXTENSIONS.join(", ")
            )
        })
}

fn validate_file_extension(name: &str) -> anyhow::Result<()> {
    file_extension_from_name(name).map(|_| ())
}

async fn create_source_internal(
    state: &AppState,
    ctx: &ReleaseCtx,
    item: SourceInput,
    settings: &crate::settings::RuntimeSettings,
) -> anyhow::Result<SourceEnqueueResult> {
    if !matches!(item.source_type.as_str(), "text" | "file" | "url") {
        anyhow::bail!("invalid source type");
    }

    let source_id = item.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = item.name.unwrap_or_else(|| source_id.clone());
    let job_id = Uuid::new_v4().to_string();

    if item.source_type == "file" {
        validate_file_extension(&name)?;
    }

    let (uri, content_hash, text_content) = match item.source_type.as_str() {
        "text" => {
            let content = item.content.context("text source requires content")?;
            if content.len() as u64 > settings.max_upload_size {
                anyhow::bail!("content exceeds max upload size");
            }
            let hash = hash_text(&content);
            (None, Some(hash), Some(content))
        }
        "file" => {
            let content = item
                .content
                .context("file source requires base64 content")?;
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(content.trim())
                .context("invalid base64 content")?;
            if bytes.len() as u64 > settings.max_upload_size {
                anyhow::bail!("file exceeds max upload size");
            }
            let ext = file_extension_from_name(&name)?;
            let staging_path = state.config.staging_dir.join(format!("{source_id}{ext}"));
            std::fs::write(&staging_path, &bytes)?;
            let hash = format!("{:x}", Sha256::digest(&bytes));
            (
                Some(staging_path.to_string_lossy().to_string()),
                Some(hash),
                None,
            )
        }
        "url" => {
            let url = item.url.context("url source requires url")?;
            (Some(url.clone()), None, None)
        }
        _ => anyhow::bail!("unsupported source type"),
    };

    let config = serde_json::to_string(&item.config)?;
    let metadata = serde_json::to_string(&item.metadata)?;

    let conn = state.pool.connect_one().await?;
    conn.execute(
        "INSERT INTO sources (id, release_id, name, type, uri, content_hash, config, metadata, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending')",
        (
            source_id.as_str(),
            ctx.release_id.as_str(),
            name.as_str(),
            item.source_type.as_str(),
            uri.as_deref(),
            content_hash.as_deref(),
            config.as_str(),
            metadata.as_str(),
        ),
    )
    .await?;

    conn.execute(
        "INSERT INTO ingest_jobs (id, release_id, stage_id, source_id, status, max_attempts)
         VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
        (
            job_id.as_str(),
            ctx.release_id.as_str(),
            ctx.stage_id.as_deref(),
            source_id.as_str(),
            state.config.max_attempts as i64,
        ),
    )
    .await?;

    if let Some(content) = text_content {
        let text_path = state.config.staging_dir.join(format!("{source_id}.txt"));
        std::fs::write(text_path, content)?;
    }

    Ok(SourceEnqueueResult { source_id, job_id })
}

async fn delete_source_internal(
    state: &AppState,
    ctx: &ReleaseCtx,
    source_id: &str,
) -> anyhow::Result<()> {
    let conn = state.pool.connect_one().await?;
    conn.execute(
        "DELETE FROM sources WHERE id = ?1 AND release_id = ?2",
        (source_id, ctx.release_id.as_str()),
    )
    .await?;
    Ok(())
}

fn hash_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_extension_from_name_accepts_supported_types() {
        assert_eq!(file_extension_from_name("notes.md").unwrap(), ".md");
        assert_eq!(file_extension_from_name("DATA.PDF").unwrap(), ".pdf");
    }

    #[test]
    fn file_extension_from_name_rejects_unknown_types() {
        assert!(file_extension_from_name("archive.zip").is_err());
    }

    #[test]
    fn hash_text_is_deterministic() {
        let a = hash_text("hello");
        let b = hash_text("hello");
        assert_eq!(a, b);
        assert_ne!(a, hash_text("world"));
    }
}
