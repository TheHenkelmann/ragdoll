// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::queries::ListQueryParams;
use crate::api::router::AppState;
use crate::auth::{require_superadmin, AuthContext};
use crate::filter::decode_filter_param;
use crate::release::ReleaseCtx;

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
    Query(params): Query<ListQueryParams>,
) -> Result<Json<Vec<SourceRecord>>, ApiError> {
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

pub async fn delete_sources(
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
    let compiled = crate::filter::compile_filter(&filter, "s")
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let sql = format!(
        "DELETE FROM sources s WHERE s.release_id = '{}' AND {}",
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
