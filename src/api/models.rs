// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{require_superadmin, AuthContext};
use crate::config::Config;
use crate::models::bootstrap::{ensure_single_model_public, list_supported_models};

#[derive(serde::Serialize)]
pub struct ModelsResponse {
    pub embedding_dim: usize,
    pub models: Vec<crate::models::bootstrap::ModelInfo>,
}

pub async fn get_models(State(state): State<Arc<AppState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        embedding_dim: state.config.embedding_dim,
        models: list_supported_models(&state.config),
    })
}

pub async fn download_model(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_superadmin(&auth)?;
    if !is_whitelisted(&name) {
        return Err(ApiError::bad_request(
            "model not in dim-1024 whitelist; only BAAI/bge-m3 and BAAI/bge-reranker-v2-m3 supported",
        ));
    }
    ensure_single_model_public(&state.config, &name, true)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"downloaded": true, "name": name})))
}

fn is_whitelisted(name: &str) -> bool {
    matches!(name, "BAAI/bge-m3" | "BAAI/bge-reranker-v2-m3")
}

pub fn fixed_model_info(config: &Config) -> ModelsResponse {
    ModelsResponse {
        embedding_dim: config.embedding_dim,
        models: list_supported_models(config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_whitelisted_accepts_supported_models() {
        assert!(is_whitelisted("BAAI/bge-m3"));
        assert!(!is_whitelisted("unknown/model"));
    }
}
