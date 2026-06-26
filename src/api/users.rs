// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::Json;
use axum::Extension;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{AuthContext, hash_password, require_superadmin, validate_email};

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub is_superadmin: bool,
    pub created_at: String,
}

pub async fn get_users(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<UserRecord>>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state.pool.connect_one().await.map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, email, is_superadmin, created_at FROM users ORDER BY created_at DESC",
            (),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| ApiError::internal(e.to_string()))? {
        let is_superadmin: i64 = row.get(2).map_err(|e| ApiError::internal(e.to_string()))?;
        items.push(UserRecord {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            email: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            is_superadmin: is_superadmin != 0,
            created_at: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }
    Ok(Json(items))
}

pub async fn post_users(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<UserRecord>, ApiError> {
    require_superadmin(&auth)?;
    if !validate_email(&body.email) {
        return Err(ApiError::bad_request("invalid email"));
    }
    let id = Uuid::new_v4().to_string();
    let hash = hash_password(&body.password).map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = state.pool.connect_one().await.map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "INSERT INTO users (id, email, password_hash, is_superadmin, password_is_default)
         VALUES (?1, ?2, ?3, 0, 0)",
        (id.as_str(), body.email.as_str(), hash.as_str()),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(UserRecord {
        id,
        email: body.email,
        is_superadmin: false,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    }))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state.pool.connect_one().await.map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query("SELECT is_superadmin FROM users WHERE id = ?1", [id.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let is_superadmin: i64 = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
    if is_superadmin != 0 {
        return Err(ApiError::bad_request("cannot delete superadmin"));
    }
    conn.execute("DELETE FROM users WHERE id = ?1", [id.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}
