// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Query, State};
use axum::http::{header, HeaderValue};
use axum::response::Response;
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::backup::{self, BackupInfo, BackupTrigger, BackupsListResponse, RestoreInfo};

#[derive(Debug, Deserialize)]
pub struct BackupFileRequest {
    pub file_name: String,
}

#[derive(Debug, Deserialize)]
pub struct RestoreBackupRequest {
    pub file_name: String,
    #[serde(default)]
    pub safety_backup: bool,
}

#[derive(Debug, Deserialize)]
pub struct DownloadBackupQuery {
    pub file_name: String,
}

pub async fn get_backups(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<BackupsListResponse>, ApiError> {
    authorize(&auth, Permission::BackupsRead)?;
    backup::backups_list_response(&state.config)
        .map(Json)
        .map_err(|e| ApiError::internal(e.to_string()))
}

pub async fn post_backup(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<BackupInfo>, ApiError> {
    authorize(&auth, Permission::BackupsCreate)?;
    let _guard = state.backup_lock.lock().await;
    backup::create_backup(&state.pool, &state.config, BackupTrigger::Manual)
        .await
        .map(Json)
        .map_err(|e| ApiError::internal(e.to_string()))
}

pub async fn post_restore_backup(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<RestoreBackupRequest>,
) -> Result<Json<RestoreInfo>, ApiError> {
    authorize(&auth, Permission::BackupsRestore)?;
    let _guard = state.backup_lock.lock().await;
    let info = backup::restore_backup(
        &state.pool,
        &state.config,
        &body.file_name,
        body.safety_backup,
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    state.settings_cache.clear_all().await;
    Ok(Json(info))
}

pub async fn get_backup_download(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<DownloadBackupQuery>,
) -> Result<Response, ApiError> {
    authorize(&auth, Permission::BackupsDownload)?;
    let path = backup::resolve_backup_path(&state.config, &query.file_name)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let disposition = format!("attachment; filename=\"{}\"", query.file_name);
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition).map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .body(Body::from(bytes))
        .map_err(|e| ApiError::internal(e.to_string()))
}

pub async fn post_backup_upload(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    mut multipart: Multipart,
) -> Result<Json<BackupInfo>, ApiError> {
    authorize(&auth, Permission::BackupsUpload)?;

    let mut file_name: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?
    {
        if field.name() == Some("file") {
            file_name = field
                .file_name()
                .map(|name| name.to_string())
                .filter(|name| !name.is_empty());
            file_data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::bad_request(e.to_string()))?
                    .to_vec(),
            );
            break;
        }
    }

    let file_name = file_name.ok_or_else(|| {
        ApiError::bad_request("upload file name must match ragdoll-<timestamp>-<daily|manual>.db")
    })?;
    backup::validate_backup_file_name(&file_name)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let data = file_data.ok_or_else(|| ApiError::bad_request("missing file field"))?;
    if data.is_empty() {
        return Err(ApiError::bad_request("empty upload"));
    }

    let _guard = state.backup_lock.lock().await;
    backup::import_backup_bytes(&state.config, &file_name, &data)
        .await
        .map(Json)
        .map_err(|e| ApiError::bad_request(e.to_string()))
}

pub async fn delete_backup_handler(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<BackupFileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::BackupsDelete)?;
    let _guard = state.backup_lock.lock().await;
    backup::delete_backup(&state.config, &body.file_name)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "deleted": true,
        "file_name": body.file_name,
    })))
}
