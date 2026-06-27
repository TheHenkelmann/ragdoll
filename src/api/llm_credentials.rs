// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::release::ReleaseCtx;

#[derive(Debug, Serialize)]
pub struct LlmCredentialRecord {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateLlmCredentialRequest {
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub service_account_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLlmCredentialRequest {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub service_account_json: Option<String>,
}

fn extract_credential_secret(
    provider: &str,
    api_key: Option<&str>,
    service_account_json: Option<&str>,
) -> Result<String, ApiError> {
    let provider = provider.trim().to_lowercase();
    if provider == "vertex" {
        if let Some(json) = service_account_json.filter(|s| !s.trim().is_empty()) {
            crate::generation::vertex::validate_service_account_json(json)
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            return Ok(json.trim().to_string());
        }
        if let Some(key) = api_key.filter(|s| !s.trim().is_empty()) {
            return Ok(key.trim().to_string());
        }
        return Err(ApiError::bad_request(
            "vertex credentials require service_account_json (GCP service account key JSON)",
        ));
    }
    let key = api_key
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("api_key is required"))?;
    Ok(key.trim().to_string())
}

pub async fn get_llm_credentials(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<LlmCredentialRecord>>, ApiError> {
    authorize(&auth, Permission::LlmCredentialsRead)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, name, provider, created_at, updated_at FROM llm_credentials
             WHERE release_id = ?1 ORDER BY name ASC",
            [ctx.release_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        items.push(LlmCredentialRecord {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            provider: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            created_at: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
            updated_at: row.get(4).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }
    Ok(Json(items))
}

pub async fn post_llm_credentials(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateLlmCredentialRequest>,
) -> Result<Json<LlmCredentialRecord>, ApiError> {
    authorize(&auth, Permission::LlmCredentialsWrite)?;
    let name = body.name.trim().to_string();
    let provider = body.provider.trim().to_string();
    if name.is_empty() || provider.is_empty() {
        return Err(ApiError::bad_request("name and provider are required"));
    }
    let secret = extract_credential_secret(
        &provider,
        body.api_key.as_deref(),
        body.service_account_json.as_deref(),
    )?;

    let (nonce, ciphertext) = state
        .crypto
        .encrypt(&secret)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let id = Uuid::new_v4().to_string();
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "INSERT INTO llm_credentials (id, release_id, name, provider, nonce, ciphertext)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (
            id.as_str(),
            ctx.release_id.as_str(),
            name.as_str(),
            provider.as_str(),
            nonce.as_str(),
            ciphertext.as_str(),
        ),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut rows = conn
        .query(
            "SELECT id, name, provider, created_at, updated_at FROM llm_credentials
             WHERE id = ?1 AND release_id = ?2",
            (id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("credential not found after insert"))?;

    Ok(Json(LlmCredentialRecord {
        id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        provider: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
        created_at: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
        updated_at: row.get(4).map_err(|e| ApiError::internal(e.to_string()))?,
    }))
}

pub async fn put_llm_credentials(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateLlmCredentialRequest>,
) -> Result<Json<LlmCredentialRecord>, ApiError> {
    authorize(&auth, Permission::LlmCredentialsWrite)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT provider FROM llm_credentials WHERE id = ?1 AND release_id = ?2",
            (id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let provider: String = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("credential not found"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let secret = extract_credential_secret(
        &provider,
        body.api_key.as_deref(),
        body.service_account_json.as_deref(),
    )?;
    let (nonce, ciphertext) = state
        .crypto
        .encrypt(&secret)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    conn.execute(
        "UPDATE llm_credentials SET nonce = ?1, ciphertext = ?2, updated_at = datetime('now')
         WHERE id = ?3 AND release_id = ?4",
        (
            nonce.as_str(),
            ciphertext.as_str(),
            id.as_str(),
            ctx.release_id.as_str(),
        ),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut rows = conn
        .query(
            "SELECT id, name, provider, created_at, updated_at FROM llm_credentials
             WHERE id = ?1 AND release_id = ?2",
            (id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("credential not found"))?;

    Ok(Json(LlmCredentialRecord {
        id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
        name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
        provider: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
        created_at: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
        updated_at: row.get(4).map_err(|e| ApiError::internal(e.to_string()))?,
    }))
}

pub async fn delete_llm_credentials(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::LlmCredentialsDelete)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "DELETE FROM llm_credentials WHERE id = ?1 AND release_id = ?2",
        (id.as_str(), ctx.release_id.as_str()),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}
