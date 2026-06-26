// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::Json;
use axum::Extension;
use axum::extract::State;
use serde_json::{Map, Value};

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{AuthContext, require_superadmin};
use crate::release::ReleaseCtx;
use crate::settings::RuntimeSettings;

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
) -> Result<Json<RuntimeSettings>, ApiError> {
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
    require_superadmin(&auth)?;
    let updated = crate::settings::patch_settings(&state.pool, &ctx.release_id, &body)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    state.settings_cache.invalidate(&ctx.release_id).await;
    Ok(Json(updated))
}
