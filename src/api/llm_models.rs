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
use crate::crypto::Crypto;
use crate::generation::types::{ResolvedGenerationSpec, DEFAULT_TEMPERATURE};
use crate::release::{NestedPathModelTag, ReleaseCtx};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmModelRecord {
    pub id: String,
    pub tag: String,
    pub model_name: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub credential_id: Option<String>,
    pub credential_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpsertLlmModelRequest {
    pub tag: String,
    pub model_name: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub credential_id: Option<String>,
}

pub async fn get_llm_models(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<LlmModelRecord>>, ApiError> {
    authorize(&auth, Permission::LlmModelsRead)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(
        load_all_models(&conn, &ctx.release_id)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?,
    ))
}

pub async fn post_llm_models(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<UpsertLlmModelRequest>,
) -> Result<Json<LlmModelRecord>, ApiError> {
    authorize(&auth, Permission::LlmModelsWrite)?;
    validate_model_body(&body)?;
    let id = Uuid::new_v4().to_string();
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    insert_or_update_model(&conn, &ctx.release_id, &id, &body, false)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    load_model_by_id(&conn, &ctx.release_id, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))
        .map(Json)
}

pub async fn put_llm_models(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathModelTag { model_tag, .. }): Path<NestedPathModelTag>,
    Json(body): Json<UpsertLlmModelRequest>,
) -> Result<Json<LlmModelRecord>, ApiError> {
    authorize(&auth, Permission::LlmModelsWrite)?;
    validate_model_body(&body)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let id = find_model_id_by_tag(&conn, &ctx.release_id, &model_tag)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("llm model not found"))?;
    insert_or_update_model(&conn, &ctx.release_id, &id, &body, true)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    load_model_by_id(&conn, &ctx.release_id, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))
        .map(Json)
}

pub async fn delete_llm_models(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathModelTag { model_tag, .. }): Path<NestedPathModelTag>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::LlmModelsDelete)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "DELETE FROM llm_models WHERE tag = ?1 AND release_id = ?2",
        (model_tag.as_str(), ctx.release_id.as_str()),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

#[derive(Debug, Serialize)]
pub struct TestLlmModelResponse {
    pub ok: bool,
    pub message: String,
    pub latency_ms: Option<i64>,
    pub completion_tokens: Option<u32>,
}

/// Minimum output tokens for connectivity tests. Azure Responses API rejects
/// values below 16 (`max_output_tokens`).
const TEST_MAX_OUTPUT_TOKENS: u32 = 16;

const TEST_SYSTEM_PROMPT: &str = "Reply with a single word: pong";

pub async fn test_llm_model(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathModelTag { model_tag, .. }): Path<NestedPathModelTag>,
) -> Result<Json<TestLlmModelResponse>, ApiError> {
    authorize(&auth, Permission::LlmModelsWrite)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let id = find_model_id_by_tag(&conn, &ctx.release_id, &model_tag)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("llm model not found"))?;
    let model = load_model_by_id(&conn, &ctx.release_id, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let api_key = if let Some(cred_id) = &model.credential_id {
        load_credential_key(&conn, &state.crypto, cred_id, &ctx.release_id).await?
    } else {
        String::new()
    };

    let spec = ResolvedGenerationSpec {
        llm_model_id: model.id.clone(),
        llm_model_tag: model.tag.clone(),
        model_name: model.model_name.clone(),
        provider: model.provider.clone(),
        endpoint: model.endpoint.clone(),
        api_key,
        system_prompt: TEST_SYSTEM_PROMPT.to_string(),
        temperature: DEFAULT_TEMPERATURE,
        max_tokens: TEST_MAX_OUTPUT_TOKENS,
        query_text: "ping".to_string(),
        matches: Vec::new(),
    };

    match state.generator.generate(&spec).await {
        Ok(out) => Ok(Json(TestLlmModelResponse {
            ok: true,
            message: "Connection successful".to_string(),
            latency_ms: Some(out.generation_total_ms),
            completion_tokens: out.completion_tokens,
        })),
        Err(err) => Ok(Json(TestLlmModelResponse {
            ok: false,
            message: err.to_string(),
            latency_ms: None,
            completion_tokens: None,
        })),
    }
}

async fn load_credential_key(
    conn: &libsql::Connection,
    crypto: &Crypto,
    credential_id: &str,
    release_id: &str,
) -> Result<String, ApiError> {
    let mut rows = conn
        .query(
            "SELECT nonce, ciphertext FROM llm_credentials WHERE id = ?1 AND release_id = ?2",
            (credential_id, release_id),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("llm credential not found"))?;
    let nonce: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
    let ciphertext: String = row.get(1).map_err(|e| ApiError::internal(e.to_string()))?;
    crypto
        .decrypt(&nonce, &ciphertext)
        .map_err(|e| ApiError::internal(e.to_string()))
}

fn validate_model_body(body: &UpsertLlmModelRequest) -> Result<(), ApiError> {
    if body.tag.trim().is_empty() || body.provider.trim().is_empty() {
        return Err(ApiError::bad_request("tag and provider are required"));
    }
    if body.provider.eq_ignore_ascii_case("openai_compat")
        && body.endpoint.as_deref().is_none_or(|s| s.trim().is_empty())
    {
        return Err(ApiError::bad_request(
            "openai_compat provider requires an API base URL endpoint",
        ));
    }
    if body.provider.eq_ignore_ascii_case("azure")
        && body.endpoint.as_deref().is_none_or(|s| s.trim().is_empty())
    {
        return Err(ApiError::bad_request(
            "azure provider requires the full deployment URL",
        ));
    }
    crate::generation::endpoints::resolve_model_name(
        &body.provider,
        &body.model_name,
        body.endpoint.as_deref(),
    )
    .map_err(|e| ApiError::bad_request(&e.to_string()))?;
    Ok(())
}

async fn find_model_id_by_tag(
    conn: &libsql::Connection,
    release_id: &str,
    tag: &str,
) -> Result<Option<String>, libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT id FROM llm_models WHERE tag = ?1 AND release_id = ?2",
            (tag, release_id),
        )
        .await?;
    Ok(rows.next().await?.map(|row| row.get(0)).transpose()?)
}

async fn insert_or_update_model(
    conn: &libsql::Connection,
    release_id: &str,
    id: &str,
    body: &UpsertLlmModelRequest,
    is_update: bool,
) -> Result<(), libsql::Error> {
    if is_update {
        conn.execute(
            "UPDATE llm_models SET tag = ?3, model_name = ?4, provider = ?5, endpoint = ?6,
                    credential_id = ?7, updated_at = datetime('now')
             WHERE id = ?1 AND release_id = ?2",
            (
                id,
                release_id,
                body.tag.trim(),
                body.model_name.trim(),
                body.provider.trim(),
                body.endpoint.as_deref(),
                body.credential_id.as_deref(),
            ),
        )
        .await?;
    } else {
        conn.execute(
            "INSERT INTO llm_models (
                id, release_id, tag, model_name, provider, endpoint, credential_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                id,
                release_id,
                body.tag.trim(),
                body.model_name.trim(),
                body.provider.trim(),
                body.endpoint.as_deref(),
                body.credential_id.as_deref(),
            ),
        )
        .await?;
    }
    Ok(())
}

async fn load_model_by_id(
    conn: &libsql::Connection,
    release_id: &str,
    id: &str,
) -> Result<LlmModelRecord, libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT m.id, m.tag, m.model_name, m.provider, m.endpoint, m.credential_id, c.name,
                    m.created_at, m.updated_at
             FROM llm_models m
             LEFT JOIN llm_credentials c ON c.id = m.credential_id
             WHERE m.id = ?1 AND m.release_id = ?2",
            (id, release_id),
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| libsql::Error::Misuse("model not found".into()))?;
    read_model_record(row)
}

async fn load_all_models(
    conn: &libsql::Connection,
    release_id: &str,
) -> Result<Vec<LlmModelRecord>, libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT m.id, m.tag, m.model_name, m.provider, m.endpoint, m.credential_id, c.name,
                    m.created_at, m.updated_at
             FROM llm_models m
             LEFT JOIN llm_credentials c ON c.id = m.credential_id
             WHERE m.release_id = ?1
             ORDER BY m.tag ASC",
            [release_id],
        )
        .await?;
    let mut items = Vec::new();
    while let Some(row) = rows.next().await? {
        items.push(read_model_record(row)?);
    }
    Ok(items)
}

fn read_model_record(row: libsql::Row) -> Result<LlmModelRecord, libsql::Error> {
    Ok(LlmModelRecord {
        id: row.get(0)?,
        tag: row.get(1)?,
        model_name: row.get(2)?,
        provider: row.get(3)?,
        endpoint: row.get(4).ok(),
        credential_id: row.get(5).ok(),
        credential_name: row.get(6).ok(),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}
