// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{FromRequestParts, OriginalUri, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::api::router::API_V1_PREFIX;

#[derive(Debug, Clone)]
pub struct ReleaseCtx {
    pub release_id: String,
    pub release_tag: String,
    pub stage_id: Option<String>,
    pub stage_tag: Option<String>,
}

impl FromRequestParts<Arc<AppState>> for ReleaseCtx {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        if let Some(ctx) = parts.extensions.get::<ReleaseCtx>() {
            return Ok(ctx.clone());
        }
        let path = parts.uri.path();
        if let Some(tag) = extract_segment(path, &format!("{API_V1_PREFIX}/releases/")) {
            return resolve_release(state, &tag, None).await;
        }
        if let Some(tag) = extract_segment(path, &format!("{API_V1_PREFIX}/stages/")) {
            return resolve_stage(state, &tag).await;
        }
        Err(ApiError::bad_request(
            "release or stage tag required in path",
        ))
    }
}

pub async fn inject_release_ctx(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    // Nested routers strip the mount prefix from req.uri(); OriginalUri keeps the full path.
    let path = uri.path();
    let tag = extract_segment(path, &format!("{API_V1_PREFIX}/releases/"))
        .ok_or_else(|| ApiError::bad_request("release tag missing in path"))?;
    let ctx = lookup_release_by_tag(&state, &tag).await?;
    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}

pub async fn inject_stage_ctx(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let path = uri.path();
    let tag = extract_segment(path, &format!("{API_V1_PREFIX}/stages/"))
        .ok_or_else(|| ApiError::bad_request("stage tag missing in path"))?;
    let ctx = lookup_stage_by_tag(&state, &tag).await?;
    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}

#[derive(Debug, Deserialize)]
pub struct NestedPathId {
    pub tag: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct NestedPathTable {
    pub tag: String,
    pub table: String,
}

fn extract_segment(path: &str, prefix: &str) -> Option<String> {
    path.strip_prefix(prefix)
        .and_then(|rest| rest.split('/').next())
        .map(str::to_string)
}

async fn resolve_release(
    state: &AppState,
    tag: &str,
    stage: Option<(String, String)>,
) -> Result<ReleaseCtx, ApiError> {
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query("SELECT id, tag FROM releases WHERE tag = ?1", [tag])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found(format!("release not found: {tag}")))?;

    Ok(ReleaseCtx {
        release_id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        release_tag: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        stage_id: stage.as_ref().map(|(id, _)| id.clone()),
        stage_tag: stage.map(|(_, t)| t),
    })
}

async fn resolve_stage(state: &AppState, tag: &str) -> Result<ReleaseCtx, ApiError> {
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT s.id, s.tag, s.release_id, r.tag FROM stages s LEFT JOIN releases r ON r.id = s.release_id WHERE s.tag = ?1",
            [tag],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found(format!("stage not found: {tag}")))?;

    let release_id: Option<String> = row.get(2).map_err(|e| ApiError::internal(e.to_string()))?;
    let release_tag: Option<String> = row.get(3).map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(ReleaseCtx {
        stage_id: Some(row.get(0).map_err(|e| ApiError::internal(e.to_string()))?),
        stage_tag: Some(row.get(1).map_err(|e| ApiError::internal(e.to_string()))?),
        release_id: release_id.unwrap_or_default(),
        release_tag: release_tag.unwrap_or_default(),
    })
}

pub async fn lookup_release_by_tag(state: &AppState, tag: &str) -> Result<ReleaseCtx, ApiError> {
    resolve_release(state, tag, None).await
}

pub async fn lookup_stage_by_tag(state: &AppState, tag: &str) -> Result<ReleaseCtx, ApiError> {
    resolve_stage(state, tag).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_segment_parses_release_tag() {
        let tag = extract_segment(
            "/api/v1/releases/first-release/settings",
            "/api/v1/releases/",
        );
        assert_eq!(tag.as_deref(), Some("first-release"));
    }

    #[test]
    fn extract_segment_returns_none_for_unrelated_path() {
        assert!(extract_segment("/api/v1/health", "/api/v1/releases/").is_none());
    }
}
