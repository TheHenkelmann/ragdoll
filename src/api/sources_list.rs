// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::queries::ListQueryParams;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::filter::decode_filter_param;
use crate::release::{NestedPathId, ReleaseCtx};
use crate::staging::cleanup_staging_artifacts;

const CHUNK_DERIVED_METADATA_KEYS: &[&str] = &["section_path", "unit_kinds"];

#[derive(Debug, Serialize)]
pub struct SourceRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: String,
    pub uri: Option<String>,
    pub content_hash: Option<String>,
    pub config: serde_json::Value,
    pub metadata: serde_json::Value,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub chunk_count: i64,
}

pub async fn get_sources(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<Vec<SourceRecord>>, ApiError> {
    authorize(&auth, Permission::SourcesRead)?;
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);
    let mut where_clause = format!("s.release_id = '{}'", ctx.release_id);
    let mut bind = Vec::new();

    if let Some(filter_raw) = params.filter {
        let filter =
            decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
        let compiled = crate::filter::compile_filter(&filter, "s")
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        where_clause = format!("s.release_id = '{}' AND {}", ctx.release_id, compiled.sql);
        bind.extend(compiled.params);
    }

    bind.push(limit.to_string());
    bind.push(offset.to_string());

    let sql = format!(
        "SELECT s.id, s.name, s.type, s.uri, s.content_hash, s.config, s.metadata, s.status, s.error, s.created_at, s.updated_at,
                (SELECT COUNT(*) FROM chunks c WHERE c.source_id = s.id) AS chunk_count
         FROM sources s WHERE {where_clause}
         ORDER BY s.created_at DESC LIMIT ?{} OFFSET ?{}",
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
        items.push(SourceRecord {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            source_type: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            uri: row.get(3).ok(),
            content_hash: row.get(4).ok(),
            config: serde_json::from_str(
                &row.get::<String>(5)
                    .map_err(|e| ApiError::internal(e.to_string()))?,
            )
            .unwrap_or(serde_json::json!({})),
            metadata: serde_json::from_str(
                &row.get::<String>(6)
                    .map_err(|e| ApiError::internal(e.to_string()))?,
            )
            .unwrap_or(serde_json::json!({})),
            status: row.get(7).map_err(|e| ApiError::internal(e.to_string()))?,
            error: row.get(8).ok(),
            created_at: row.get(9).map_err(|e| ApiError::internal(e.to_string()))?,
            updated_at: row.get(10).map_err(|e| ApiError::internal(e.to_string()))?,
            chunk_count: row.get(11).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
pub struct PatchSourceMetadataRequest {
    pub metadata: serde_json::Value,
}

pub async fn patch_source_metadata(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathId { id: source_id, .. }): Path<NestedPathId>,
    Json(body): Json<PatchSourceMetadataRequest>,
) -> Result<Json<SourceRecord>, ApiError> {
    authorize(&auth, Permission::SourcesWrite)?;
    if !body.metadata.is_object() {
        return Err(ApiError::bad_request("metadata must be a JSON object"));
    }

    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut rows = conn
        .query(
            "SELECT id, name, type, uri, content_hash, config, metadata, status, error, created_at, updated_at
             FROM sources WHERE id = ?1 AND release_id = ?2",
            (source_id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("source not found"))?;

    let new_metadata_json =
        serde_json::to_string(&body.metadata).map_err(|e| ApiError::bad_request(e.to_string()))?;

    conn.execute(
        "UPDATE sources SET metadata = ?1, updated_at = datetime('now') WHERE id = ?2",
        (new_metadata_json.as_str(), source_id.as_str()),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut chunk_rows = conn
        .query(
            "SELECT id, metadata FROM chunks WHERE source_id = ?1",
            [source_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    while let Some(chunk_row) = chunk_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let chunk_id: String = chunk_row
            .get(0)
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let old_meta: serde_json::Value = serde_json::from_str(
            &chunk_row
                .get::<String>(1)
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .unwrap_or(serde_json::json!({}));

        let mut merged: serde_json::Value = body.metadata.clone();
        if let Some(obj) = merged.as_object_mut() {
            for key in CHUNK_DERIVED_METADATA_KEYS {
                if let Some(val) = old_meta.get(*key) {
                    obj.insert(key.to_string(), val.clone());
                }
            }
        }
        let merged_json =
            serde_json::to_string(&merged).map_err(|e| ApiError::internal(e.to_string()))?;
        conn.execute(
            "UPDATE chunks SET metadata = ?1 WHERE id = ?2",
            (merged_json.as_str(), chunk_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    let mut count_rows = conn
        .query(
            "SELECT COUNT(*) FROM chunks WHERE source_id = ?1",
            [source_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let chunk_count: i64 = count_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("count failed"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(SourceRecord {
        id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        source_type: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
        uri: row.get(3).ok(),
        content_hash: row.get(4).ok(),
        config: serde_json::from_str(
            &row.get::<String>(5)
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .unwrap_or(serde_json::json!({})),
        metadata: body.metadata,
        status: row.get(7).map_err(|e| ApiError::internal(e.to_string()))?,
        error: row.get(8).ok(),
        created_at: row.get(9).map_err(|e| ApiError::internal(e.to_string()))?,
        updated_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        chunk_count,
    }))
}

pub async fn delete_sources(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::SourcesDelete)?;
    let filter_raw = params
        .filter
        .ok_or_else(|| ApiError::bad_request("filter query param required"))?;
    let filter =
        decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let compiled = crate::filter::compile_filter(&filter, "sources")
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let select_sql = format!(
        "SELECT id, type, uri FROM sources WHERE sources.release_id = '{}' AND {}",
        ctx.release_id, compiled.sql
    );
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(&select_sql, compiled.params.clone())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let id: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        let source_type: String = row.get(1).map_err(|e| ApiError::internal(e.to_string()))?;
        let uri: Option<String> = row.get(2).ok();
        cleanup_staging_artifacts(&state.config, &id, &source_type, uri.as_deref());
    }
    let sql = format!(
        "DELETE FROM sources WHERE sources.release_id = '{}' AND {}",
        ctx.release_id, compiled.sql
    );
    conn.execute(&sql, compiled.params)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}
