// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::State;
use axum::Extension;
use axum::Json;
use serde_json::{Map, Value};

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::release::ReleaseCtx;
use crate::settings::RuntimeSettings;

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<RuntimeSettings>, ApiError> {
    authorize(&auth, Permission::SettingsRead)?;
    let settings = state
        .settings_cache
        .get_or_load(&state.pool, &ctx.release_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(settings))
}

pub async fn patch_settings(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<Map<String, Value>>,
) -> Result<Json<RuntimeSettings>, ApiError> {
    authorize(&auth, Permission::SettingsWrite)?;
    let updated =
        crate::settings::patch_settings(&state.pool, &state.config, &ctx.release_id, &body)
            .await
            .map_err(|e| match e {
                crate::db::DbError::InvalidInput(msg) => ApiError::bad_request(msg),
                other => ApiError::internal(other.to_string()),
            })?;
    state.settings_cache.invalidate(&ctx.release_id).await;
    Ok(Json(updated))
}
