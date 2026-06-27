// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use axum::routing::{get, patch, post, put};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::analytics::get_analytics;
use crate::api::api_keys::{delete_api_key, get_api_keys, patch_api_key, post_api_keys};
use crate::api::auth::{change_password, get_auth_info, get_status, post_login};
use crate::api::backups::{
    delete_backup_handler, get_backup_download, get_backups, post_backup, post_backup_upload,
    post_restore_backup,
};
use crate::api::chunks::{delete_chunks, get_chunks, patch_chunk, post_chunks};
use crate::api::db_viewer::get_table;
use crate::api::health::health;
use crate::api::ingest_jobs::get_ingest_jobs_status;
use crate::api::llm_credentials::{
    delete_llm_credentials, get_llm_credentials, post_llm_credentials, put_llm_credentials,
};
use crate::api::llm_models::{
    delete_llm_models, get_llm_models, post_llm_models, put_llm_models, test_llm_model,
};
use crate::api::models::{
    add_custom_model_handler, cancel_model_download, delete_custom_model_handler, delete_model,
    delete_model_storage, download_model, get_models, get_models_status, get_models_storage,
    purge_model_memory, purge_unreferenced_models, stream_model_download, test_model,
};
use crate::api::openapi::ApiDoc;
use crate::api::queries::{
    delete_queries, get_queries, get_query_detail, post_playground_queries, post_queries,
};
use crate::api::reindex::{post_reindex, stream_reindex_batch};
use crate::api::releases::{create_release, delete_release, list_releases, rename_release};
use crate::api::settings::{get_settings, patch_settings};
use crate::api::sources::{post_sources, put_sources};
use crate::api::sources_list::{delete_sources, get_sources, patch_source_metadata};
use crate::api::stages::{create_stage, delete_stage, list_stages, update_stage};
use crate::api::system_metrics::get_system_metrics;
use crate::api::users::{delete_user, get_users, post_users, update_user};
use crate::api::webhooks::{
    delete_webhook, get_webhook_secret, get_webhooks, patch_webhook, post_webhooks, test_webhook,
};
use crate::auth::middleware::auth_middleware;
use crate::auth::rate_limit::{rate_limit_middleware, RateLimitStore};
use crate::config::Config;
use crate::crypto::Crypto;
use crate::db::model_guard::EmbeddingMismatch;
use crate::db::DbPool;
use crate::generation::{GenaiGenerator, Generator, MockGenerator};
use crate::models::download::ModelDownloadManager;
use crate::models::{ModelProvider, ModelRegistry};
use crate::release::{inject_playground_ctx, inject_release_ctx, inject_stage_ctx};
use crate::search::SearchPipeline;
use crate::settings::SettingsCache;

pub const API_V1_PREFIX: &str = "/api/v1";

pub struct AppState {
    pub config: Config,
    pub pool: DbPool,
    pub settings_cache: SettingsCache,
    pub search: SearchPipeline,
    pub models: Arc<dyn ModelProvider>,
    pub generator: Arc<dyn Generator>,
    pub crypto: Crypto,
    pub ready: bool,
    pub backup_lock: Arc<tokio::sync::Mutex<()>>,
    pub rate_limits: Arc<RateLimitStore>,
    pub model_downloads: Arc<ModelDownloadManager>,
    pub embedding_mismatches: Vec<EmbeddingMismatch>,
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
        .route("/sources/{id}", patch(patch_source_metadata))
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
        .route("/reindex", post(post_reindex))
        .route("/reindex/{batch_id}/events", get(stream_reindex_batch))
        .route("/ingest_jobs", get(get_ingest_jobs_status))
        .route(
            "/llm_credentials",
            get(get_llm_credentials).post(post_llm_credentials),
        )
        .route(
            "/llm_credentials/{id}",
            put(put_llm_credentials).delete(delete_llm_credentials),
        )
        .route("/llm_models", get(get_llm_models).post(post_llm_models))
        .route(
            "/llm_models/{model_tag}",
            put(put_llm_models).delete(delete_llm_models),
        )
        .route("/llm_models/{model_tag}/test", post(test_llm_model))
        .route("/webhooks", get(get_webhooks).post(post_webhooks))
        .route(
            "/webhooks/{id}",
            patch(patch_webhook).delete(delete_webhook),
        )
        .route("/webhooks/{id}/secret", get(get_webhook_secret))
        .route("/webhooks/{id}/test", post(test_webhook))
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
        .route("/sources/{id}", patch(patch_source_metadata))
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
        .route("/reindex", post(post_reindex))
        .route("/reindex/{batch_id}/events", get(stream_reindex_batch))
        .route(
            "/llm_credentials",
            get(get_llm_credentials).post(post_llm_credentials),
        )
        .route(
            "/llm_credentials/{id}",
            put(put_llm_credentials).delete(delete_llm_credentials),
        )
        .route("/llm_models", get(get_llm_models).post(post_llm_models))
        .route(
            "/llm_models/{model_tag}",
            put(put_llm_models).delete(delete_llm_models),
        )
        .route("/llm_models/{model_tag}/test", post(test_llm_model))
        .route("/webhooks", get(get_webhooks).post(post_webhooks))
        .route(
            "/webhooks/{id}",
            patch(patch_webhook).delete(delete_webhook),
        )
        .route("/webhooks/{id}/secret", get(get_webhook_secret))
        .route("/webhooks/{id}/test", post(test_webhook))
        .route("/db/{table}", get(get_table))
        .layer(from_fn_with_state(state.clone(), inject_stage_ctx));

    let playground_routes = Router::new()
        .route("/queries", post(post_playground_queries))
        .route("/queries/{id}", get(get_query_detail))
        .layer(from_fn_with_state(state.clone(), inject_playground_ctx));

    let api_v1 = Router::new()
        .route("/health", get(health))
        .route("/auth/login", post(post_login))
        .route("/auth/info", get(get_auth_info))
        .route("/auth/status", get(get_status))
        .route("/auth/password", patch(change_password))
        .route("/users", get(get_users).post(post_users))
        .route("/users/{id}", patch(update_user).delete(delete_user))
        .route("/api_keys", get(get_api_keys).post(post_api_keys))
        .route(
            "/api_keys/{id}",
            patch(patch_api_key).delete(delete_api_key),
        )
        .route("/releases", get(list_releases).post(create_release))
        .route(
            "/releases/{tag}",
            patch(rename_release).delete(delete_release),
        )
        .route("/stages", get(list_stages).post(create_stage))
        .route("/stages/{tag}", patch(update_stage).delete(delete_stage))
        .route("/models", get(get_models))
        .route("/models/status", get(get_models_status))
        .route("/models/storage", get(get_models_storage))
        .route(
            "/models/storage/{dir_name}",
            axum::routing::delete(delete_model_storage),
        )
        .route("/models/custom", post(add_custom_model_handler))
        .route(
            "/models/custom/{name}",
            axum::routing::delete(delete_custom_model_handler),
        )
        .route("/models/purge", post(purge_unreferenced_models))
        .route("/models/{name}", axum::routing::delete(delete_model))
        .route("/models/{name}/download", post(download_model))
        .route("/models/{name}/download/stream", get(stream_model_download))
        .route(
            "/models/{name}/download/cancel",
            post(cancel_model_download),
        )
        .route("/models/{name}/purge", post(purge_model_memory))
        .route("/models/{name}/test", post(test_model))
        .route("/analytics", get(get_analytics))
        .route("/system-metrics", get(get_system_metrics))
        .route("/backups", get(get_backups).post(post_backup))
        .route("/backups/restore", post(post_restore_backup))
        .route("/backups/download", get(get_backup_download))
        .route("/backups/upload", post(post_backup_upload))
        .route(
            "/backups/delete",
            axum::routing::delete(delete_backup_handler),
        )
        .nest("/releases/{tag}", release_routes)
        .nest("/stages/{tag}", stage_routes)
        .nest("/playground/{tag}", playground_routes)
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", ApiDoc::openapi()));

    Router::new()
        .nest(API_V1_PREFIX, api_v1)
        .route_service(
            "/favicon.ico",
            ServeFile::new(static_dir.join("assets/favicon.ico")),
        )
        .nest_service("/assets", ServeDir::new(static_dir.join("assets")))
        .fallback_service(ServeFile::new(index))
        // Order matters: the last `.layer()` is the outermost (runs first).
        // auth_middleware must run BEFORE rate_limit_middleware so the AuthContext
        // is populated when the rate limiter inspects it.
        .layer(from_fn_with_state(state.clone(), rate_limit_middleware))
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

pub async fn build_state(
    config: Config,
    pool: DbPool,
    embedding_mismatches: Vec<EmbeddingMismatch>,
) -> anyhow::Result<Arc<AppState>> {
    let models: Arc<dyn ModelProvider> = Arc::new(ModelRegistry::new(config.clone()));
    let generator: Arc<dyn Generator> = if std::env::var("RAGDOLL_USE_MOCK_GENERATOR")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        Arc::new(MockGenerator::default())
    } else {
        Arc::new(GenaiGenerator::new())
    };
    build_state_with_provider(config, pool, models, generator, embedding_mismatches).await
}

pub async fn build_state_with_provider(
    config: Config,
    pool: DbPool,
    models: Arc<dyn ModelProvider>,
    generator: Arc<dyn Generator>,
    embedding_mismatches: Vec<EmbeddingMismatch>,
) -> anyhow::Result<Arc<AppState>> {
    let registry = models.clone();
    let search = SearchPipeline {
        pool: pool.clone(),
        models,
    };
    let crypto = Crypto::from_secret(&config.secret)?;
    let model_download_max_concurrent = config.model_download_max_concurrent;

    Ok(Arc::new(AppState {
        config,
        pool,
        settings_cache: SettingsCache::new(),
        search,
        models: registry,
        generator,
        crypto,
        ready: true,
        backup_lock: Arc::new(tokio::sync::Mutex::new(())),
        rate_limits: Arc::new(RateLimitStore::new()),
        model_downloads: Arc::new(ModelDownloadManager::new(model_download_max_concurrent)),
        embedding_mismatches,
    }))
}
