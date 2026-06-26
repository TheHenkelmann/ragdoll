// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{encode_api_key_token, require_superadmin, AuthContext};

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyRecord {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub token: String,
}

pub async fn get_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ApiKeyRecord>>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, name, created_at FROM api_keys ORDER BY created_at DESC",
            (),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        items.push(ApiKeyRecord {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            created_at: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }
    Ok(Json(items))
}

pub async fn post_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, ApiError> {
    require_superadmin(&auth)?;
    let id = Uuid::new_v4().to_string();
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "INSERT INTO api_keys (id, name) VALUES (?1, ?2)",
        (id.as_str(), body.name.as_str()),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let token = encode_api_key_token(&state.config.jwt_secret, &id, &body.name, &created_at)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(CreateApiKeyResponse {
        id,
        name: body.name,
        created_at,
        token,
    }))
}

pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_superadmin(&auth)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute("DELETE FROM api_keys WHERE id = ?1", [id.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}
