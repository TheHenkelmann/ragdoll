// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Extension;
use axum::Json;
use futures_util::stream::Stream;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::config::Config;
use crate::db::model_guard::EmbeddingMismatch;
use crate::models::bootstrap::{
    collect_required_models, delete_local_model, ensure_single_model_public, is_valid_hf_model_name,
    list_local_models, list_supported_models, model_is_complete,
};
use crate::models::catalog::predefined_catalog;
use crate::models::custom_models::{add_custom_model, is_custom_model, load_custom_models, remove_custom_model};
use crate::models::download::test_model_inference;
use crate::models::mapping::{is_supported_embed_model, is_supported_rerank_model};
use crate::models::registry::ModelRegistry;
use crate::settings::RuntimeSettings;

#[derive(serde::Serialize)]
pub struct ModelsResponse {
    pub embedding_dim: usize,
    pub models: Vec<crate::models::bootstrap::ModelInfo>,
}

#[derive(serde::Serialize)]
pub struct RequiredModelInfo {
    pub name: String,
    pub kind: String,
    pub present: bool,
    pub releases: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct CatalogStatusEntry {
    pub name: String,
    pub kind: String,
    pub languages: Vec<String>,
    pub present: bool,
    pub releases: Vec<String>,
    pub loaded: bool,
    pub ram_bytes: Option<u64>,
    pub custom: bool,
}

#[derive(serde::Serialize)]
pub struct ModelsStatusResponse {
    pub embedding_dim: usize,
    pub local: Vec<crate::models::bootstrap::ModelInfo>,
    pub catalog: Vec<CatalogStatusEntry>,
    pub required: Vec<RequiredModelInfo>,
    pub missing: Vec<String>,
    pub mismatches: Vec<EmbeddingMismatch>,
    pub active_downloads: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct PurgeModelsResponse {
    pub purged_embedders: usize,
    pub purged_rerankers: usize,
}

#[derive(serde::Serialize)]
pub struct TestModelResponse {
    pub ok: bool,
    pub name: String,
    pub kind: String,
    pub latency_ms: u64,
}

#[derive(serde::Deserialize)]
pub struct AddCustomModelRequest {
    pub name: String,
}

pub async fn get_models(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<ModelsResponse>, ApiError> {
    authorize(&auth, Permission::ModelsRead)?;
    Ok(Json(ModelsResponse {
        embedding_dim: state.config.embedding_dim,
        models: list_supported_models(&state.config),
    }))
}

pub async fn get_models_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<ModelsStatusResponse>, ApiError> {
    authorize(&auth, Permission::ModelsRead)?;
    Ok(Json(build_models_status(state.as_ref()).await?))
}

async fn build_models_status(state: &AppState) -> Result<ModelsStatusResponse, ApiError> {
    let local = list_local_models(&state.config);
    let local_names: HashSet<String> = local.iter().map(|m| m.name.clone()).collect();
    let required = load_required_models(&state.pool, &state.config).await?;
    let missing: Vec<String> = required
        .iter()
        .filter(|m| !local_names.contains(&m.name))
        .map(|m| m.name.clone())
        .collect();

    let release_map: HashMap<String, Vec<String>> = required
        .iter()
        .map(|m| (m.name.clone(), m.releases.clone()))
        .collect();

    let loaded: HashSet<String> = state.models.list_loaded().await.into_iter().collect();
    let custom_models = load_custom_models(&state.config).unwrap_or_default();
    let custom_set: HashSet<String> = custom_models.into_iter().collect();

    let mut catalog: Vec<CatalogStatusEntry> = predefined_catalog()
        .iter()
        .map(|entry| {
            let present = model_is_complete(&state.config.model_dir_for(entry.name));
            let is_loaded = loaded.contains(entry.name);
            CatalogStatusEntry {
                name: entry.name.to_string(),
                kind: entry.kind.as_str().to_string(),
                languages: entry.languages.iter().map(|l| (*l).to_string()).collect(),
                present,
                releases: release_map.get(entry.name).cloned().unwrap_or_default(),
                loaded: is_loaded,
                ram_bytes: is_loaded.then(|| ModelRegistry::estimate_ram_bytes(&state.config, entry.name)).flatten(),
                custom: false,
            }
        })
        .collect();

    for name in custom_set {
        if catalog.iter().any(|e| e.name == name) {
            continue;
        }
        let kind = if is_supported_embed_model(&name) {
            "embed".to_string()
        } else if is_supported_rerank_model(&name) {
            "rerank".to_string()
        } else {
            local
                .iter()
                .find(|m| m.name == name)
                .map(|m| m.kind.clone())
                .unwrap_or_else(|| "unknown".to_string())
        };
        let present = model_is_complete(&state.config.model_dir_for(&name));
        let is_loaded = loaded.contains(&name);
        catalog.push(CatalogStatusEntry {
            name: name.clone(),
            kind,
            languages: vec![],
            present,
            releases: release_map.get(&name).cloned().unwrap_or_default(),
            loaded: is_loaded,
            ram_bytes: is_loaded
                .then(|| ModelRegistry::estimate_ram_bytes(&state.config, &name))
                .flatten(),
            custom: true,
        });
    }

    catalog.sort_by(|a, b| a.name.cmp(&b.name));

    let mismatches = crate::db::model_guard::check_embedding_mismatches(
        &state.pool,
        state.config.embedding_dim as i64,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(ModelsStatusResponse {
        embedding_dim: state.config.embedding_dim,
        local,
        catalog,
        required,
        missing,
        mismatches,
        active_downloads: state.model_downloads.list_active(),
    })
}

async fn load_required_models(
    pool: &crate::db::DbPool,
    config: &Config,
) -> Result<Vec<RequiredModelInfo>, ApiError> {
    let conn = pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut rows = conn
        .query(
            "SELECT r.tag, s.key, s.value
             FROM settings s
             JOIN releases r ON r.id = s.release_id
             WHERE s.key IN ('embedding_model', 'rerank_model')
             ORDER BY r.tag ASC",
            (),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let defaults = RuntimeSettings::default();
    let mut by_name: HashMap<String, (String, HashSet<String>)> = HashMap::new();

    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let tag: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        let key: String = row.get(1).map_err(|e| ApiError::internal(e.to_string()))?;
        let raw: String = row.get(2).map_err(|e| ApiError::internal(e.to_string()))?;
        let value = serde_json::from_str::<String>(&raw).unwrap_or(raw);
        let kind = if key == "embedding_model" {
            "embed"
        } else {
            "rerank"
        };
        by_name
            .entry(value)
            .or_insert_with(|| (kind.to_string(), HashSet::new()))
            .1
            .insert(tag);
    }

    let local_names: HashSet<String> = list_local_models(config)
        .into_iter()
        .map(|m| m.name)
        .collect();

    let mut required: Vec<RequiredModelInfo> = by_name
        .into_iter()
        .map(|(name, (kind, releases))| {
            let mut tags: Vec<String> = releases.into_iter().collect();
            tags.sort();
            RequiredModelInfo {
                name: name.clone(),
                kind,
                present: local_names.contains(&name),
                releases: tags,
            }
        })
        .collect();

    if !required.iter().any(|m| m.kind == "embed") {
        required.push(RequiredModelInfo {
            name: defaults.embedding_model.clone(),
            kind: "embed".into(),
            present: local_names.contains(&defaults.embedding_model),
            releases: vec![],
        });
    }
    if !required.iter().any(|m| m.kind == "rerank") {
        required.push(RequiredModelInfo {
            name: defaults.rerank_model.clone(),
            kind: "rerank".into(),
            present: local_names.contains(&defaults.rerank_model),
            releases: vec![],
        });
    }

    required.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(required)
}

pub async fn add_custom_model_handler(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<AddCustomModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::ModelsDownload)?;
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("model name is required"));
    }
    add_custom_model(&state.config, &name).map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({ "added": true, "name": name })))
}

pub async fn delete_custom_model_handler(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::ModelsDownload)?;
    if !is_custom_model(&state.config, &name) {
        return Err(ApiError::bad_request(format!(
            "model {name} is not a custom catalog entry"
        )));
    }
    remove_custom_model(&state.config, &name)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({ "removed": true, "name": name })))
}

pub async fn download_model(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::ModelsDownload)?;
    validate_model_name(&state.config, &name)?;
    ensure_single_model_public(&state.config, &name, true)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"downloaded": true, "name": name})))
}

pub async fn delete_model(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::ModelsDelete)?;
    if !is_allowed_model_name(&state.config, &name) {
        return Err(ApiError::bad_request(format!("invalid model name: {name}")));
    }
    delete_local_model(&state.config, &name)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let (purged_embedders, purged_rerankers) = state.models.purge_model(&name).await;
    Ok(Json(serde_json::json!({
        "deleted": true,
        "name": name,
        "purged_embedders": purged_embedders,
        "purged_rerankers": purged_rerankers,
    })))
}

pub async fn purge_unreferenced_models(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<PurgeModelsResponse>, ApiError> {
    authorize(&auth, Permission::ModelsDownload)?;
    let required = collect_required_models(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let (purged_embedders, purged_rerankers) =
        state.models.evict_unreferenced(&required).await;
    Ok(Json(PurgeModelsResponse {
        purged_embedders,
        purged_rerankers,
    }))
}

pub async fn purge_model_memory(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Json<PurgeModelsResponse>, ApiError> {
    authorize(&auth, Permission::ModelsDownload)?;
    validate_model_name(&state.config, &name)?;
    let (purged_embedders, purged_rerankers) = state.models.purge_model(&name).await;
    Ok(Json(PurgeModelsResponse {
        purged_embedders,
        purged_rerankers,
    }))
}

pub async fn cancel_model_download(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::ModelsDownload)?;
    validate_model_name(&state.config, &name)?;
    let cancelled = state.model_downloads.cancel(&name);
    Ok(Json(serde_json::json!({ "cancelled": cancelled, "name": name })))
}

pub async fn stream_model_download(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    authorize(&auth, Permission::ModelsDownload)?;
    validate_model_name(&state.config, &name)?;

    let rx =
        state
            .model_downloads
            .subscribe_or_start(state.config.clone(), state.models.clone(), name);

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let sse = Event::default()
                        .json_data(&event)
                        .unwrap_or_else(|_| Event::default().data("ok"));
                    return Some((Ok(sse), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

pub async fn test_model(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(name): Path<String>,
) -> Result<Json<TestModelResponse>, ApiError> {
    authorize(&auth, Permission::ModelsRead)?;
    validate_model_name(&state.config, &name)?;

    let kind = model_kind(&name);
    let latency_ms = test_model_inference(
        &state.models,
        &name,
        crate::settings::DEFAULT_RERANK_MAX_LENGTH as usize,
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(Json(TestModelResponse {
        ok: true,
        name,
        kind: kind.to_string(),
        latency_ms,
    }))
}

fn validate_model_name(config: &Config, name: &str) -> Result<(), ApiError> {
    if is_allowed_model_name(config, name) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "model name must be org/model (Hugging Face repo id): {name}"
        )))
    }
}

fn is_allowed_model_name(config: &Config, name: &str) -> bool {
    is_whitelisted(name)
        || is_custom_model(config, name)
        || is_valid_hf_model_name(name)
}

fn model_kind(name: &str) -> &'static str {
    if is_supported_embed_model(name) {
        "embed"
    } else if is_supported_rerank_model(name) {
        "rerank"
    } else {
        "unknown"
    }
}

fn is_whitelisted(name: &str) -> bool {
    is_supported_embed_model(name) || is_supported_rerank_model(name)
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
        assert!(is_whitelisted("jinaai/jina-embeddings-v3"));
        assert!(!is_whitelisted("unknown/model"));
    }

    #[test]
    fn allows_hf_repo_ids() {
        let config = Config::for_test(std::env::temp_dir(), "secret");
        assert!(is_allowed_model_name(&config, "org/custom-model"));
    }

    #[test]
    fn catalog_entry_for_predefined_model() {
        use crate::models::catalog::find_catalog_entry;
        let entry = find_catalog_entry("Alibaba-NLP/gte-large-en-v1.5").unwrap();
        assert_eq!(entry.languages, &["en"]);
    }
}
