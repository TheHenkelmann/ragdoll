// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::Json;
use axum::Extension;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{AuthContext, require_superadmin};

#[derive(Debug, Serialize)]
pub struct ReleaseRecord {
    pub id: String,
    pub tag: String,
    pub message: String,
    pub created_at: String,
    pub stage_tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReleaseInit {
    New,
    Fork { source: String },
    Template { source: String },
}

#[derive(Debug, Deserialize)]
pub struct CreateReleaseRequest {
    pub tag: String,
    #[serde(default)]
    pub message: String,
    pub init: ReleaseInit,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReleaseRequest {
    pub tag: String,
}

pub async fn list_releases(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ReleaseRecord>>, ApiError> {
    let conn = state.pool.connect_one().await.map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, tag, message, created_at FROM releases ORDER BY created_at DESC",
            (),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| ApiError::internal(e.to_string()))? {
        let id: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        let mut stage_rows = conn
            .query("SELECT tag FROM stages WHERE release_id = ?1", [id.as_str()])
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let mut stage_tags = Vec::new();
        while let Some(srow) = stage_rows.next().await.map_err(|e| ApiError::internal(e.to_string()))? {
            stage_tags.push(srow.get(0).map_err(|e| ApiError::internal(e.to_string()))?);
        }
        items.push(ReleaseRecord {
            id,
            tag: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            message: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            created_at: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
            stage_tags,
        });
    }
    Ok(Json(items))
}

pub async fn create_release(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateReleaseRequest>,
) -> Result<Json<ReleaseRecord>, ApiError> {
    require_superadmin(&auth)?;
    validate_tag(&body.tag)?;
    let id = Uuid::new_v4().to_string();
    let conn = state.pool.connect_one().await.map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "INSERT INTO releases (id, tag, message) VALUES (?1, ?2, ?3)",
        (id.as_str(), body.tag.as_str(), body.message.as_str()),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;

    match body.init {
        ReleaseInit::New => seed_default_settings(&conn, &id).await?,
        ReleaseInit::Fork { source } => fork_release(&conn, &source, &id).await?,
        ReleaseInit::Template { .. } => {
            return Err(ApiError::bad_request("template init not implemented yet"));
        }
    }

    Ok(Json(ReleaseRecord {
        id,
        tag: body.tag,
        message: body.message,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        stage_tags: vec![],
    }))
}

pub async fn rename_release(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tag): Path<String>,
    Json(body): Json<UpdateReleaseRequest>,
) -> Result<Json<ReleaseRecord>, ApiError> {
    require_superadmin(&auth)?;
    validate_tag(&body.tag)?;
    let conn = state.pool.connect_one().await.map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query("SELECT id FROM releases WHERE tag = ?1", [tag.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let release_id: String = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("release not found"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "UPDATE releases SET tag = ?1 WHERE id = ?2",
        (body.tag.as_str(), release_id.as_str()),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let mut info_rows = conn
        .query(
            "SELECT message, created_at FROM releases WHERE id = ?1",
            [release_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let info = info_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("release not found"))?;
    let mut stage_rows = conn
        .query("SELECT tag FROM stages WHERE release_id = ?1", [release_id.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut stage_tags = Vec::new();
    while let Some(srow) = stage_rows.next().await.map_err(|e| ApiError::internal(e.to_string()))? {
        stage_tags.push(srow.get(0).map_err(|e| ApiError::internal(e.to_string()))?);
    }

    Ok(Json(ReleaseRecord {
        id: release_id,
        tag: body.tag,
        message: info.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        created_at: info.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        stage_tags,
    }))
}

pub async fn delete_release(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tag): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state.pool.connect_one().await.map_err(|e| ApiError::internal(e.to_string()))?;
    let mut count_rows = conn
        .query("SELECT COUNT(*) FROM releases", ())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let count: i64 = count_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("count failed"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if count <= 1 {
        return Err(ApiError::bad_request("at least one release must remain"));
    }
    let mut rows = conn
        .query("SELECT id FROM releases WHERE tag = ?1", [tag.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let release_id: String = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("release not found"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut stage_rows = conn
        .query("SELECT COUNT(*) FROM stages WHERE release_id = ?1", [release_id.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let stage_count: i64 = stage_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("count failed"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if stage_count > 0 {
        return Err(ApiError::bad_request("release is bound to a stage"));
    }
    conn.execute("DELETE FROM releases WHERE id = ?1", [release_id.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

fn validate_tag(tag: &str) -> Result<(), ApiError> {
    if tag.is_empty() || tag.len() > 50 {
        return Err(ApiError::bad_request("invalid release tag"));
    }
    if !tag
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError::bad_request("invalid release tag characters"));
    }
    Ok(())
}

async fn seed_default_settings(
    conn: &libsql::Connection,
    release_id: &str,
) -> Result<(), ApiError> {
    let defaults = [
        ("embedding_model", "\"BAAI/bge-m3\""),
        ("rerank_model", "\"BAAI/bge-reranker-v2-m3\""),
        ("payload_storage", "\"per_request\""),
        ("chunking_strategy", "\"semantic_split\""),
        ("sentence_buffer", "2"),
        ("breakpoint_percentile", "95"),
        ("min_chunk_tokens", "64"),
        ("max_chunk_tokens", "512"),
        ("max_upload_size", "52428800"),
        ("max_batch_size", "100"),
    ];
    for (key, value) in defaults {
        conn.execute(
            "INSERT INTO settings (release_id, key, value) VALUES (?1, ?2, ?3)",
            (release_id, key, value),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    Ok(())
}

async fn fork_release(
    conn: &libsql::Connection,
    source_tag: &str,
    target_release_id: &str,
) -> Result<(), ApiError> {
    let mut rows = conn
        .query("SELECT id FROM releases WHERE tag = ?1", [source_tag])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let source_id: String = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("fork source release not found"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut settings = conn
        .query(
            "SELECT key, value FROM settings WHERE release_id = ?1",
            [source_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    while let Some(row) = settings.next().await.map_err(|e| ApiError::internal(e.to_string()))? {
        conn.execute(
            "INSERT INTO settings (release_id, key, value) VALUES (?1, ?2, ?3)",
            (
                target_release_id,
                row.get::<String>(0).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
                row.get::<String>(1).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
            ),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    let mut sources = conn
        .query(
            "SELECT id, name, type, uri, content_hash, config, metadata, status, error, created_at, updated_at
             FROM sources WHERE release_id = ?1",
            [source_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut source_map = std::collections::HashMap::new();
    while let Some(row) = sources.next().await.map_err(|e| ApiError::internal(e.to_string()))? {
        let old_id: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        let new_id = Uuid::new_v4().to_string();
        source_map.insert(old_id.clone(), new_id.clone());
        conn.execute(
            "INSERT INTO sources (id, release_id, name, type, uri, content_hash, config, metadata, status, error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            (
                new_id.as_str(),
                target_release_id,
                row.get::<String>(1).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
                row.get::<String>(2).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
                row.get::<Option<String>>(3).ok().flatten().as_deref(),
                row.get::<Option<String>>(4).ok().flatten().as_deref(),
                row.get::<String>(5).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
                row.get::<String>(6).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
                row.get::<String>(7).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
                row.get::<Option<String>>(8).ok().flatten().as_deref(),
                row.get::<String>(9).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
                row.get::<String>(10).map_err(|e| ApiError::internal(e.to_string()))?.as_str(),
            ),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    let mut chunks = conn
        .query(
            "SELECT id, source_id, ordinal, content, provenance, metadata, token_count, embedding, embedding_model, embedding_dim, embedding_version, created_at
             FROM chunks WHERE release_id = ?1",
            [source_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    while let Some(row) = chunks.next().await.map_err(|e| ApiError::internal(e.to_string()))? {
        let old_source: String = row.get(1).map_err(|e| ApiError::internal(e.to_string()))?;
        let new_source = source_map
            .get(&old_source)
            .ok_or_else(|| ApiError::internal("missing source mapping"))?;
        let old_chunk_id: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        let new_chunk_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO chunks (
                id, release_id, source_id, ordinal, content, provenance, metadata, token_count,
                embedding, embedding_model, embedding_dim, embedding_version, created_at
             )
             SELECT ?1, ?2, ?3, ordinal, content, provenance, metadata, token_count,
                    embedding, embedding_model, embedding_dim, embedding_version, created_at
             FROM chunks WHERE id = ?4",
            (
                new_chunk_id.as_str(),
                target_release_id,
                new_source.as_str(),
                old_chunk_id.as_str(),
            ),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    Ok(())
}
