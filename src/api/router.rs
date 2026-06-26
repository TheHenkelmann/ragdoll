// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use axum::routing::{delete, get, patch, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::analytics::get_analytics;
use crate::api::api_keys::{delete_api_key, get_api_keys, post_api_keys};
use crate::api::auth::{get_auth_info, get_status, post_login};
use crate::api::chunks::{delete_chunks, get_chunks, patch_chunk, post_chunks};
use crate::api::db_viewer::get_table;
use crate::api::health::health;
use crate::api::models::{download_model, get_models};
use crate::api::openapi::ApiDoc;
use crate::api::queries::{delete_queries, get_queries, get_query_detail, post_queries};
use crate::api::releases::{create_release, delete_release, list_releases, rename_release};
use crate::api::settings::{get_settings, patch_settings};
use crate::api::sources::{post_sources, put_sources};
use crate::api::sources_list::{delete_sources, get_sources};
use crate::api::stages::{create_stage, delete_stage, list_stages, update_stage};
use crate::api::users::{delete_user, get_users, post_users};
use crate::auth::middleware::auth_middleware;
use crate::config::Config;
use crate::db::DbPool;
use crate::models::ModelProvider;
use crate::models::ModelRegistry;
use crate::release::{inject_release_ctx, inject_stage_ctx};
use crate::search::SearchPipeline;
use crate::settings::SettingsCache;

pub const API_V1_PREFIX: &str = "/api/v1";

pub struct AppState {
    pub config: Config,
    pub pool: DbPool,
    pub settings_cache: SettingsCache,
    pub search: SearchPipeline,
    pub models: Arc<dyn ModelProvider>,
    pub ready: bool,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let limit = 52_428_800usize;
    let static_dir = state.config.static_dir.clone();
    let index = static_dir.join("index.html");

    let release_routes = Router::new()
        .route(
            "/sources",
            get(get_sources)
                .post(post_sources)
                .put(put_sources)
                .delete(delete_sources),
        )
        .route(
            "/queries",
            post(post_queries).get(get_queries).delete(delete_queries),
        )
        .route("/queries/{id}", get(get_query_detail))
        .route(
            "/chunks",
            get(get_chunks).post(post_chunks).delete(delete_chunks),
        )
        .route("/chunks/{id}", patch(patch_chunk))
        .route("/settings", get(get_settings).patch(patch_settings))
        .route("/db/{table}", get(get_table))
        .layer(from_fn_with_state(state.clone(), inject_release_ctx));

    let stage_routes = Router::new()
        .route(
            "/sources",
            get(get_sources)
                .post(post_sources)
                .put(put_sources)
                .delete(delete_sources),
        )
        .route(
            "/queries",
            post(post_queries).get(get_queries).delete(delete_queries),
        )
        .route("/queries/{id}", get(get_query_detail))
        .route(
            "/chunks",
            get(get_chunks).post(post_chunks).delete(delete_chunks),
        )
        .route("/chunks/{id}", patch(patch_chunk))
        .route("/settings", get(get_settings).patch(patch_settings))
        .route("/db/{table}", get(get_table))
        .layer(from_fn_with_state(state.clone(), inject_stage_ctx));

    let api_v1 = Router::new()
        .route("/health", get(health))
        .route("/auth/login", post(post_login))
        .route("/auth/info", get(get_auth_info))
        .route("/auth/status", get(get_status))
        .route("/users", get(get_users).post(post_users))
        .route("/users/{id}", delete(delete_user))
        .route("/api_keys", get(get_api_keys).post(post_api_keys))
        .route("/api_keys/{id}", delete(delete_api_key))
        .route("/releases", get(list_releases).post(create_release))
        .route(
            "/releases/{tag}",
            patch(rename_release).delete(delete_release),
        )
        .route("/stages", get(list_stages).post(create_stage))
        .route("/stages/{tag}", patch(update_stage).delete(delete_stage))
        .route("/models", get(get_models))
        .route("/models/{name}/download", post(download_model))
        .route("/analytics", get(get_analytics))
        .nest("/releases/{tag}", release_routes)
        .nest("/stages/{tag}", stage_routes)
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", ApiDoc::openapi()));

    Router::new()
        .nest(API_V1_PREFIX, api_v1)
        .route_service(
            "/favicon.ico",
            ServeFile::new(static_dir.join("assets/favicon.ico")),
        )
        .nest_service("/assets", ServeDir::new(static_dir.join("assets")))
        .fallback_service(ServeFile::new(index))
        .layer(from_fn_with_state(state.clone(), auth_middleware))
        .layer(RequestBodyLimitLayer::new(limit))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn build_state(config: Config, pool: DbPool) -> anyhow::Result<Arc<AppState>> {
    let models: Arc<dyn ModelProvider> = Arc::new(ModelRegistry::new(config.clone()));
    build_state_with_provider(config, pool, models).await
}

pub async fn build_state_with_provider(
    config: Config,
    pool: DbPool,
    models: Arc<dyn ModelProvider>,
) -> anyhow::Result<Arc<AppState>> {
    let registry = models.clone();
    let search = SearchPipeline {
        pool: pool.clone(),
        models,
    };

    Ok(Arc::new(AppState {
        config,
        pool,
        settings_cache: SettingsCache::new(),
        search,
        models: registry,
        ready: true,
    }))
}
