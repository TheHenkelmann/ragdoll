// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::crypto::generate_webhook_secret;
use crate::release::{NestedPathId, ReleaseCtx};
use crate::webhooks::{is_host_event, validate_known_events, HOST_UTILIZATION_TYPE};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookRecord {
    pub id: String,
    pub release_id: String,
    #[serde(rename = "type")]
    pub webhook_type: String,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    #[serde(default = "default_webhook_type")]
    #[serde(rename = "type")]
    pub webhook_type: String,
    pub url: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_webhook_type() -> String {
    "ingest_status".to_string()
}

fn default_active() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct PatchWebhookRequest {
    pub url: Option<String>,
    pub events: Option<Vec<String>>,
    pub active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TestWebhookResponse {
    pub status_code: Option<u16>,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct WebhookSecretResponse {
    pub secret: String,
}

pub async fn get_webhooks(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<WebhookRecord>>, ApiError> {
    authorize(&auth, Permission::WebhooksRead)?;
    let conn = state.pool.connect_one().await.map_err(internal)?;
    let mut rows = conn
        .query(
            "SELECT id, release_id, type, url, events, active, created_at
             FROM webhooks WHERE release_id = ?1 ORDER BY created_at DESC",
            [ctx.release_id.as_str()],
        )
        .await
        .map_err(internal)?;
    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(internal)? {
        items.push(row_to_webhook(&row, false)?);
    }
    Ok(Json(items))
}

pub async fn post_webhooks(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateWebhookRequest>,
) -> Result<Json<WebhookRecord>, ApiError> {
    authorize(&auth, Permission::WebhooksWrite)?;
    let url = body.url.trim().to_string();
    if url.is_empty() {
        return Err(ApiError::bad_request("url is required"));
    }
    let id = Uuid::new_v4().to_string();
    let secret = generate_webhook_secret();
    let events = body.events.clone();
    validate_known_events(&events).map_err(ApiError::bad_request)?;
    let events_json =
        serde_json::to_string(&events).map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = state.pool.connect_one().await.map_err(internal)?;
    ensure_webhook_url_available(&conn, &ctx.release_id, &url, None).await?;
    conn.execute(
        "INSERT INTO webhooks (id, release_id, type, url, secret, events, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            id.as_str(),
            ctx.release_id.as_str(),
            body.webhook_type.as_str(),
            url.as_str(),
            secret.as_str(),
            events_json.as_str(),
            if body.active { 1i64 } else { 0i64 },
        ),
    )
    .await
    .map_err(internal)?;
    Ok(Json(WebhookRecord {
        id,
        release_id: ctx.release_id,
        webhook_type: body.webhook_type,
        url,
        events,
        active: body.active,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    }))
}

pub async fn patch_webhook(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathId { id, .. }): Path<NestedPathId>,
    Json(body): Json<PatchWebhookRequest>,
) -> Result<Json<WebhookRecord>, ApiError> {
    authorize(&auth, Permission::WebhooksWrite)?;
    let conn = state.pool.connect_one().await.map_err(internal)?;
    let mut rows = conn
        .query(
            "SELECT id, release_id, type, url, events, active, created_at
             FROM webhooks WHERE id = ?1 AND release_id = ?2",
            (id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(internal)?;
    let row = rows
        .next()
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("webhook not found"))?;
    let current = row_to_webhook(&row, false)?;
    let url = body.url.unwrap_or(current.url).trim().to_string();
    if url.is_empty() {
        return Err(ApiError::bad_request("url is required"));
    }
    let events = body.events.unwrap_or(current.events);
    validate_known_events(&events).map_err(ApiError::bad_request)?;
    let active = body.active.unwrap_or(current.active);
    let events_json =
        serde_json::to_string(&events).map_err(|e| ApiError::internal(e.to_string()))?;
    ensure_webhook_url_available(&conn, &ctx.release_id, &url, Some(&id)).await?;
    conn.execute(
        "UPDATE webhooks SET url = ?1, events = ?2, active = ?3 WHERE id = ?4",
        (
            url.as_str(),
            events_json.as_str(),
            if active { 1i64 } else { 0i64 },
            id.as_str(),
        ),
    )
    .await
    .map_err(internal)?;
    Ok(Json(WebhookRecord {
        id,
        release_id: ctx.release_id,
        webhook_type: current.webhook_type,
        url,
        events,
        active,
        created_at: current.created_at,
    }))
}

pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathId { id, .. }): Path<NestedPathId>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::WebhooksDelete)?;
    let conn = state.pool.connect_one().await.map_err(internal)?;
    conn.execute(
        "DELETE FROM webhooks WHERE id = ?1 AND release_id = ?2",
        (id.as_str(), ctx.release_id.as_str()),
    )
    .await
    .map_err(internal)?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

pub async fn get_webhook_secret(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathId { id, .. }): Path<NestedPathId>,
) -> Result<Json<WebhookSecretResponse>, ApiError> {
    authorize(&auth, Permission::WebhooksRead)?;
    let conn = state.pool.connect_one().await.map_err(internal)?;
    let mut rows = conn
        .query(
            "SELECT secret FROM webhooks WHERE id = ?1 AND release_id = ?2",
            (id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(internal)?;
    let row = rows
        .next()
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("webhook not found"))?;
    let secret: String = row.get(0).map_err(internal)?;
    Ok(Json(WebhookSecretResponse { secret }))
}

pub async fn test_webhook(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathId { id, .. }): Path<NestedPathId>,
) -> Result<Json<TestWebhookResponse>, ApiError> {
    authorize(&auth, Permission::WebhooksWrite)?;
    let conn = state.pool.connect_one().await.map_err(internal)?;
    let mut rows = conn
        .query(
            "SELECT id, type, url, secret, events FROM webhooks WHERE id = ?1 AND release_id = ?2",
            (id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(internal)?;
    let row = rows
        .next()
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("webhook not found"))?;
    let webhook_type: String = row.get(1).map_err(internal)?;
    let url: String = row.get(2).map_err(internal)?;
    let secret: String = row.get(3).map_err(internal)?;
    let events_raw: String = row.get(4).map_err(internal)?;
    let events: Vec<String> = serde_json::from_str(&events_raw).unwrap_or_default();
    let _ = webhook_type;
    let payload = if events.iter().any(|e| is_host_event(e)) {
        serde_json::json!({
            "type": HOST_UTILIZATION_TYPE,
            "event": "test",
            "scope": "host",
            "note": "Host-wide utilization, not scoped to a release, stage, or Ragdoll process.",
            "release_id": ctx.release_id,
            "cpu_percent": 42.0,
            "memory_used_bytes": 8_000_000_000_u64,
            "memory_total_bytes": 16_000_000_000_u64,
            "memory_available_bytes": 8_000_000_000_u64,
            "memory_used_percent": 50.0,
            "cpu_cores": 8,
            "ts": time::OffsetDateTime::now_utc().unix_timestamp(),
        })
    } else {
        serde_json::json!({
            "type": "ingest_status",
            "event": "test",
            "source_id": "00000000-0000-0000-0000-000000000000",
            "status": "completed",
            "release_id": ctx.release_id,
            "stage_id": ctx.stage_id,
            "chunk_count": 0,
            "error": null,
            "ts": time::OffsetDateTime::now_utc().unix_timestamp(),
        })
    };
    let body = payload.to_string();
    let ts = time::OffsetDateTime::now_utc().unix_timestamp().to_string();
    let signature =
        crate::webhooks::sign_payload(&secret, &ts, &body).map_err(ApiError::internal)?;
    let client = Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-Ragdoll-Signature", format!("sha256={signature}"))
        .header("X-Ragdoll-Timestamp", &ts)
        .body(body.clone())
        .send()
        .await
        .map_err(|e| ApiError::bad_request(format!("webhook request failed: {e}")))?;
    let status_code = response.status().as_u16();
    let response_body = response.text().await.unwrap_or_default();
    Ok(Json(TestWebhookResponse {
        status_code: Some(status_code),
        body: response_body,
    }))
}

async fn ensure_webhook_url_available(
    conn: &libsql::Connection,
    release_id: &str,
    url: &str,
    exclude_id: Option<&str>,
) -> Result<(), ApiError> {
    let mut rows = conn
        .query(
            "SELECT id FROM webhooks WHERE release_id = ?1 AND url = ?2 LIMIT 1",
            (release_id, url),
        )
        .await
        .map_err(internal)?;
    if let Some(row) = rows.next().await.map_err(internal)? {
        let existing_id: String = row.get(0).map_err(internal)?;
        if exclude_id != Some(existing_id.as_str()) {
            return Err(ApiError::bad_request(
                "webhook url already exists for this release",
            ));
        }
    }
    Ok(())
}

fn row_to_webhook(row: &libsql::Row, include_secret: bool) -> Result<WebhookRecord, ApiError> {
    let _ = include_secret;
    let events_raw: String = row.get(4).map_err(internal)?;
    let events: Vec<String> = serde_json::from_str(&events_raw).unwrap_or_default();
    let active: i64 = row.get(5).map_err(internal)?;
    Ok(WebhookRecord {
        id: row.get(0).map_err(internal)?,
        release_id: row.get(1).map_err(internal)?,
        webhook_type: row.get(2).map_err(internal)?,
        url: row.get(3).map_err(internal)?,
        events,
        active: active != 0,
        created_at: row.get(6).map_err(internal)?,
    })
}

fn internal<E: std::fmt::Display>(err: E) -> ApiError {
    ApiError::internal(err.to_string())
}

#[cfg(test)]
mod tests {
    use crate::webhooks::sign_payload;

    #[test]
    fn sign_payload_is_deterministic() {
        let sig = match sign_payload("secret", "1710000000", r#"{"event":"test"}"#) {
            Ok(v) => v,
            Err(_) => panic!("sign failed"),
        };
        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64);
        let again = match sign_payload("secret", "1710000000", r#"{"event":"test"}"#) {
            Ok(v) => v,
            Err(_) => panic!("sign again failed"),
        };
        assert_eq!(sig, again);
        let different_ts = match sign_payload("secret", "1710000001", r#"{"event":"test"}"#) {
            Ok(v) => v,
            Err(_) => panic!("different ts sign failed"),
        };
        assert_ne!(sig, different_ts);
    }
}
