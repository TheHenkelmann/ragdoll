// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{require_superadmin, AuthContext};

#[derive(Debug, Serialize)]
pub struct StageRecord {
    pub id: String,
    pub tag: String,
    pub release_id: String,
    pub release_tag: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateStageRequest {
    pub tag: String,
    #[serde(default)]
    pub release_tag: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStageRequest {
    #[serde(default)]
    pub release_tag: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
}

fn row_to_stage(row: &libsql::Row) -> Result<StageRecord, ApiError> {
    Ok(StageRecord {
        id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        tag: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        release_id: optional_string(row, 2)?,
        release_tag: optional_string(row, 3)?,
        created_at: row.get(4).map_err(|e| ApiError::internal(e.to_string()))?,
    })
}

fn optional_string(row: &libsql::Row, idx: i32) -> Result<String, ApiError> {
    let value: Option<String> = row
        .get(idx)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(value.unwrap_or_default())
}

const STAGE_SELECT: &str = "SELECT s.id, s.tag, s.release_id, r.tag, s.created_at
             FROM stages s LEFT JOIN releases r ON r.id = s.release_id";

pub async fn list_stages(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<StageRecord>>, ApiError> {
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(&format!("{STAGE_SELECT} ORDER BY s.created_at DESC"), ())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        items.push(row_to_stage(&row)?);
    }
    Ok(Json(items))
}

async fn lookup_release_id(
    conn: &libsql::Connection,
    release_tag: &str,
) -> Result<String, ApiError> {
    let mut rows = conn
        .query("SELECT id FROM releases WHERE tag = ?1", [release_tag])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    rows.next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("release not found"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))
}

pub async fn create_stage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateStageRequest>,
) -> Result<Json<StageRecord>, ApiError> {
    require_superadmin(&auth)?;
    validate_stage_tag(&body.tag)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let release_id: Option<String> = if body.release_tag.is_empty() {
        None
    } else {
        Some(lookup_release_id(&conn, &body.release_tag).await?)
    };
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO stages (id, tag, release_id) VALUES (?1, ?2, ?3)",
        (id.as_str(), body.tag.as_str(), release_id.as_deref()),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(StageRecord {
        id,
        tag: body.tag,
        release_id: release_id.unwrap_or_default(),
        release_tag: body.release_tag,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    }))
}

pub async fn update_stage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tag): Path<String>,
    Json(body): Json<UpdateStageRequest>,
) -> Result<Json<StageRecord>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if let Some(release_tag) = body.release_tag.as_deref() {
        if release_tag.is_empty() {
            conn.execute(
                "UPDATE stages SET release_id = NULL WHERE tag = ?1",
                [tag.as_str()],
            )
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        } else {
            let release_id = lookup_release_id(&conn, release_tag).await?;
            conn.execute(
                "UPDATE stages SET release_id = ?1 WHERE tag = ?2",
                (release_id.as_str(), tag.as_str()),
            )
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        }
    }
    let current_tag = if let Some(new_tag) = body.tag.as_deref() {
        validate_stage_tag(new_tag)?;
        conn.execute(
            "UPDATE stages SET tag = ?1 WHERE tag = ?2",
            (new_tag, tag.as_str()),
        )
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
        new_tag.to_string()
    } else {
        tag.clone()
    };
    let mut rows = conn
        .query(
            &format!("{STAGE_SELECT} WHERE s.tag = ?1"),
            [current_tag.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("stage not found"))?;
    row_to_stage(&row).map(Json)
}

pub async fn delete_stage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tag): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute("DELETE FROM stages WHERE tag = ?1", [tag.as_str()])
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

fn validate_stage_tag(tag: &str) -> Result<(), ApiError> {
    if tag.is_empty() || tag.len() > 12 || !tag.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(ApiError::bad_request("invalid stage tag"));
    }
    Ok(())
}
