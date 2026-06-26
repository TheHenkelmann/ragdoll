// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::batch::{BatchItemResult, BatchResponse};
use crate::api::error::ApiError;
use crate::api::queries::ListQueryParams;
use crate::api::router::AppState;
use crate::auth::{require_superadmin, AuthContext};
use crate::filter::decode_filter_param;
use crate::release::{NestedPathId, ReleaseCtx};

#[derive(Debug, Deserialize)]
pub struct ChunkInput {
    pub source_id: String,
    pub content: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub provenance: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ChunkEnqueueResult {
    pub chunk_id: String,
    pub job_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub id: String,
    pub source_id: String,
    pub ordinal: i64,
    pub content: String,
    pub metadata: serde_json::Value,
    pub provenance: serde_json::Value,
    pub token_count: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ChunkPatch {
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

pub async fn get_chunks(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<Vec<ChunkRecord>>, ApiError> {
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);
    let mut where_clause = format!("c.release_id = '{}'", ctx.release_id);
    let mut bind = Vec::new();

    if let Some(filter_raw) = params.filter {
        let filter =
            decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
        let compiled = crate::filter::compile_filter(&filter, "c")
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        where_clause = format!("c.release_id = '{}' AND {}", ctx.release_id, compiled.sql);
        bind.extend(compiled.params);
    }

    bind.push(limit.to_string());
    bind.push(offset.to_string());

    let sql = format!(
        "SELECT id, source_id, ordinal, content, metadata, provenance, token_count, created_at
         FROM chunks c WHERE {where_clause}
         ORDER BY created_at DESC LIMIT ?{} OFFSET ?{}",
        bind.len() - 1,
        bind.len()
    );

    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(&sql, bind)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        items.push(ChunkRecord {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            source_id: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            ordinal: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            content: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
            metadata: serde_json::from_str(
                &row.get::<String>(4)
                    .map_err(|e| ApiError::internal(e.to_string()))?,
            )
            .unwrap_or(serde_json::json!({})),
            provenance: serde_json::from_str(
                &row.get::<String>(5)
                    .map_err(|e| ApiError::internal(e.to_string()))?,
            )
            .unwrap_or(serde_json::json!([])),
            token_count: row.get(6).ok(),
            created_at: row.get(7).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }
    Ok(Json(items))
}

pub async fn post_chunks(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(items): Json<Vec<ChunkInput>>,
) -> Result<BatchResponse<ChunkEnqueueResult>, ApiError> {
    require_superadmin(&auth)?;
    let settings = state
        .settings_cache
        .get_or_load(&state.pool, &ctx.release_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut results = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        let chunk_id = Uuid::new_v4().to_string();
        let job_id = Uuid::new_v4().to_string();
        let metadata = serde_json::to_string(&item.metadata).unwrap_or_else(|_| "{}".to_string());
        let staging = state
            .config
            .staging_dir
            .join(format!("chunk-{chunk_id}.json"));
        let payload = serde_json::json!({
            "chunk_id": chunk_id,
            "source_id": item.source_id,
            "content": item.content,
            "metadata": item.metadata,
            "provenance": item.provenance,
        });
        if let Err(err) = std::fs::write(&staging, payload.to_string()) {
            results.push(BatchItemResult::err(
                index,
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
            continue;
        }

        let conn = match state.pool.connect_one().await {
            Ok(c) => c,
            Err(err) => {
                results.push(BatchItemResult::err(
                    index,
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    err.to_string(),
                ));
                continue;
            }
        };

        if let Err(err) = conn
            .execute(
                "INSERT INTO sources (id, release_id, name, type, config, metadata, status)
                 VALUES (?1, ?2, ?3, 'text', '{}', ?4, 'pending')
                 ON CONFLICT(id) DO NOTHING",
                (
                    item.source_id.as_str(),
                    ctx.release_id.as_str(),
                    item.source_id.as_str(),
                    metadata.as_str(),
                ),
            )
            .await
        {
            results.push(BatchItemResult::err(
                index,
                axum::http::StatusCode::BAD_REQUEST,
                err.to_string(),
            ));
            continue;
        }

        if let Err(err) = conn
            .execute(
                "INSERT INTO ingest_jobs (id, release_id, stage_id, source_id, status, max_attempts)
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
                (
                    job_id.as_str(),
                    ctx.release_id.as_str(),
                    ctx.stage_id.as_deref(),
                    item.source_id.as_str(),
                    state.config.max_attempts as i64,
                ),
            )
            .await
        {
            results.push(BatchItemResult::err(
                index,
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
            continue;
        }

        let _ = settings;
        results.push(BatchItemResult::ok(
            index,
            ChunkEnqueueResult { chunk_id, job_id },
        ));
    }
    Ok(BatchResponse { items: results })
}

pub async fn patch_chunk(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathId { id, .. }): Path<NestedPathId>,
    Json(patch): Json<ChunkPatch>,
) -> Result<Json<ChunkRecord>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if let Some(content) = &patch.content {
        conn.execute(
            "UPDATE chunks SET content = ?1 WHERE id = ?2 AND release_id = ?3",
            (content.as_str(), id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    if let Some(metadata) = &patch.metadata {
        let serialized =
            serde_json::to_string(metadata).map_err(|e| ApiError::bad_request(e.to_string()))?;
        conn.execute(
            "UPDATE chunks SET metadata = ?1 WHERE id = ?2 AND release_id = ?3",
            (serialized.as_str(), id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    let mut rows = conn
        .query(
            "SELECT id, source_id, ordinal, content, metadata, provenance, token_count, created_at
             FROM chunks WHERE id = ?1 AND release_id = ?2",
            (id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("chunk not found"))?;

    Ok(Json(ChunkRecord {
        id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        source_id: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        ordinal: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
        content: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
        metadata: serde_json::from_str(
            &row.get::<String>(4)
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .unwrap_or(serde_json::json!({})),
        provenance: serde_json::from_str(
            &row.get::<String>(5)
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .unwrap_or(serde_json::json!([])),
        token_count: row.get(6).ok(),
        created_at: row.get(7).map_err(|e| ApiError::internal(e.to_string()))?,
    }))
}

pub async fn delete_chunks(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_superadmin(&auth)?;
    let filter_raw = params
        .filter
        .ok_or_else(|| ApiError::bad_request("filter query param required"))?;
    let filter =
        decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let compiled = crate::filter::compile_filter(&filter, "c")
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let sql = format!(
        "DELETE FROM chunks c WHERE c.release_id = '{}' AND {}",
        ctx.release_id, compiled.sql
    );
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(&sql, compiled.params)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}
