// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{
    authorize, encode_api_key_token, parse_and_validate_api_key_permissions, parse_permissions,
    permission_set_to_vec, permissions_to_json, AuthContext, Permission,
};
use crate::crypto::format_api_key_token;

/// Distinguishes "field omitted" (`None`) from "field set to null" (`Some(None)`).
/// Plain `Option<Option<T>>` collapses JSON `null` to the outer `None`, which would
/// make clearing a rate limit indistinguishable from not touching it.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    pub rpm: Option<u32>,
    pub rph: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct PatchApiKeyRequest {
    pub permissions: Option<Vec<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub rpm: Option<Option<u32>>,
    #[serde(default, deserialize_with = "double_option")]
    pub rph: Option<Option<u32>>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyRecord {
    pub id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub rpm: Option<u32>,
    pub rph: Option<u32>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub rpm: Option<u32>,
    pub rph: Option<u32>,
    pub created_at: String,
    pub token: String,
}

fn row_to_record(row: &libsql::Row) -> Result<ApiKeyRecord, ApiError> {
    let perms_raw: String = row.get(2).unwrap_or_else(|_| "[]".to_string());
    let perms = parse_permissions(&perms_raw);
    let rpm: Option<i64> = row.get(3).ok();
    let rph: Option<i64> = row.get(4).ok();
    Ok(ApiKeyRecord {
        id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        permissions: permission_set_to_vec(&perms),
        rpm: rpm.and_then(|v| u32::try_from(v).ok()),
        rph: rph.and_then(|v| u32::try_from(v).ok()),
        created_at: row.get(5).map_err(|e| ApiError::internal(e.to_string()))?,
    })
}

pub async fn get_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ApiKeyRecord>>, ApiError> {
    authorize(&auth, Permission::ApiKeysRead)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, name, permissions, rpm, rph, created_at FROM api_keys ORDER BY created_at DESC",
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
        items.push(row_to_record(&row)?);
    }
    Ok(Json(items))
}

pub async fn post_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, ApiError> {
    authorize(&auth, Permission::ApiKeysWrite)?;
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("name is required"));
    }
    let permissions = parse_and_validate_api_key_permissions(&body.permissions)?;
    let permissions_json = permissions_to_json(&permissions);
    let id = Uuid::new_v4().to_string();
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT 1 FROM api_keys WHERE name = ?1 LIMIT 1",
            [name.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .is_some()
    {
        return Err(ApiError::bad_request(format!(
            "API key name already exists: {name}"
        )));
    }
    conn.execute(
        "INSERT INTO api_keys (id, name, permissions, rpm, rph) VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            id.as_str(),
            name.as_str(),
            permissions_json.as_str(),
            body.rpm.map(i64::from),
            body.rph.map(i64::from),
        ),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let token = encode_api_key_token(&state.config.secret, &id, &name, &created_at)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(CreateApiKeyResponse {
        id,
        name,
        permissions: permission_set_to_vec(&permissions),
        rpm: body.rpm,
        rph: body.rph,
        created_at,
        token: format_api_key_token(&token),
    }))
}

pub async fn patch_api_key(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
    Json(body): Json<PatchApiKeyRequest>,
) -> Result<Json<ApiKeyRecord>, ApiError> {
    authorize(&auth, Permission::ApiKeysWrite)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, name, permissions, rpm, rph, created_at FROM api_keys WHERE id = ?1",
            [id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("api key not found"))?;
    let mut record = row_to_record(&row)?;

    if let Some(perms) = body.permissions {
        let parsed = parse_and_validate_api_key_permissions(&perms)?;
        record.permissions = permission_set_to_vec(&parsed);
        let permissions_json = permissions_to_json(&parsed);
        conn.execute(
            "UPDATE api_keys SET permissions = ?1 WHERE id = ?2",
            (permissions_json.as_str(), id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    if let Some(rpm) = body.rpm {
        record.rpm = rpm;
        conn.execute(
            "UPDATE api_keys SET rpm = ?1 WHERE id = ?2",
            (rpm.map(i64::from), id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    if let Some(rph) = body.rph {
        record.rph = rph;
        conn.execute(
            "UPDATE api_keys SET rph = ?1 WHERE id = ?2",
            (rph.map(i64::from), id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    Ok(Json(record))
}

pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::ApiKeysDelete)?;
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
