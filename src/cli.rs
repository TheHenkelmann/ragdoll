// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::api::build_router;
use crate::auth::ensure_superadmin;
use crate::backup::{self, BackupTrigger};
use crate::config::Config;
use crate::db::{migrations, model_guard, DbPool};
use crate::models;
use crate::models::mapping::{is_supported_embed_model, is_supported_rerank_model};

#[derive(Parser)]
#[command(name = "ragdoll", about = "One-stop local RAG pipeline")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Serve,
    Migrate,
    ModelsEnsure,
    ModelGuard,
    Doctor,
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let config = Config::from_env()?;
    match cli.command {
        Commands::Serve => serve(config).await,
        Commands::Migrate => migrations::migrate(&config).await.map_err(Into::into),
        Commands::ModelsEnsure => {
            let pool = DbPool::connect(&config).await?;
            models::ensure_models(&config, &pool).await
        }
        Commands::ModelGuard => {
            let pool = DbPool::connect(&config).await?;
            model_guard::run_model_guard(&pool, config.embedding_dim as i64)
                .await
                .map_err(Into::into)
        }
        Commands::Doctor => doctor(config).await,
    }
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    config.ensure_directories()?;
    let pool = DbPool::connect(&config).await?;
    migrations::run_migrations(&pool, &config.migrations_dir).await?;
    ensure_superadmin(&pool, &config).await?;

    let mismatches =
        model_guard::check_embedding_mismatches(&pool, config.embedding_dim as i64).await?;
    model_guard::run_model_guard(&pool, config.embedding_dim as i64).await?;
    models::ensure_models(&config, &pool).await?;

    let state = crate::api::router::build_state(config.clone(), pool.clone(), mismatches).await?;
    spawn_backup_scheduler(state.clone());
    spawn_model_warmup(state.clone());
    spawn_model_cache_eviction(state.clone());
    spawn_model_artifact_cleanup(state.clone());
    crate::system_metrics::spawn_sampler(state.pool.clone());

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port))
        .await
        .with_context(|| format!("bind port {}", config.port))?;
    tracing::info!(port = config.port, "ragdoll listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_backup_scheduler(state: std::sync::Arc<crate::api::router::AppState>) {
    let startup_state = state.clone();
    tokio::spawn(async move {
        run_daily_backup_if_needed(&startup_state).await;
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        interval.tick().await;
        loop {
            interval.tick().await;
            run_daily_backup_if_needed(&state).await;
        }
    });
}

fn spawn_model_warmup(state: std::sync::Arc<crate::api::router::AppState>) {
    tokio::spawn(async move {
        let required = match models::collect_required_models(&state.pool).await {
            Ok(names) => names,
            Err(err) => {
                tracing::warn!(error = %err, "model warm-up skipped: could not load release settings");
                return;
            }
        };

        let max_len = crate::settings::DEFAULT_RERANK_MAX_LENGTH as usize;
        // #region agent log
        {
            let required_names: Vec<String> = required.iter().cloned().collect();
            dbg_log("D", "cli.rs:spawn_model_warmup", "startup warmup begins (will load each required model, holding registry lock)", serde_json::json!({"required": required_names, "count": required.len()}));
        }
        // #endregion
        for name in required {
            // #region agent log
            dbg_log("D", "cli.rs:spawn_model_warmup", "warmup loading model", serde_json::json!({"model": name.as_str()}));
            // #endregion
            if is_supported_embed_model(&name) {
                match state.models.embedder(&name).await {
                    Ok(embedder) => {
                        if let Err(err) = embedder.embed_one("warmup").await {
                            tracing::warn!(model = %name, error = %err, "model warm-up embed failed");
                        }
                    }
                    Err(err) => {
                        tracing::warn!(model = %name, error = %err, "model warm-up embedder load failed");
                    }
                }
            } else if is_supported_rerank_model(&name) {
                match state.models.reranker(&name, max_len).await {
                    Ok(reranker) => {
                        if let Err(err) = reranker
                            .rerank("warmup", &[String::from("warmup document")])
                            .await
                        {
                            tracing::warn!(model = %name, error = %err, "model warm-up rerank failed");
                        }
                    }
                    Err(err) => {
                        tracing::warn!(model = %name, error = %err, "model warm-up reranker load failed");
                    }
                }
            }
        }

        tracing::info!("model warm-up completed");
    });
}

fn spawn_model_cache_eviction(state: std::sync::Arc<crate::api::router::AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        interval.tick().await;
        loop {
            interval.tick().await;
            match models::collect_required_models(&state.pool).await {
                Ok(required) => {
                    let (embedders, rerankers) = state.models.evict_unreferenced(&required).await;
                    if embedders > 0 || rerankers > 0 {
                        tracing::info!(
                            embedders,
                            rerankers,
                            "evicted unreferenced models from gateway memory"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(error = %err, "model cache eviction skipped");
                }
            }
        }
    });
}

fn spawn_model_artifact_cleanup(state: std::sync::Arc<crate::api::router::AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        interval.tick().await;
        loop {
            interval.tick().await;
            let mut protected = match models::collect_required_models(&state.pool).await {
                Ok(required) => required,
                Err(err) => {
                    tracing::warn!(error = %err, "model artifact cleanup skipped");
                    continue;
                }
            };
            for name in state.model_downloads.list_active() {
                protected.insert(name);
            }
            for name in state.models.list_loaded().await {
                protected.insert(name);
            }

            match models::bootstrap::cleanup_stale_model_artifacts(&state.config, &protected) {
                Ok(removed) if !removed.is_empty() => {
                    tracing::info!(count = removed.len(), "cleaned stale model artifacts");
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "model artifact cleanup failed");
                }
            }
        }
    });
}

async fn run_daily_backup_if_needed(state: &std::sync::Arc<crate::api::router::AppState>) {
    let now = time::OffsetDateTime::now_utc();
    match backup::has_daily_for_today(&state.config, now) {
        Ok(true) => {}
        Ok(false) => {
            let _guard = state.backup_lock.lock().await;
            match backup::has_daily_for_today(&state.config, now) {
                Ok(true) => return,
                Ok(false) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "failed to check daily backup status");
                    return;
                }
            }
            match backup::create_backup(&state.pool, &state.config, BackupTrigger::Daily).await {
                Ok(info) => tracing::info!(file = %info.file_name, "daily backup created"),
                Err(err) => tracing::warn!(error = %err, "daily backup failed"),
            }
        }
        Err(err) => tracing::warn!(error = %err, "failed to check daily backup status"),
    }
}

// #region agent log
fn dbg_log(hyp: &str, location: &str, message: &str, data: serde_json::Value) {
    use std::io::Write;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = serde_json::json!({
        "sessionId": "a04a75",
        "runId": "pre-fix",
        "hypothesisId": hyp,
        "location": location,
        "message": message,
        "data": data,
        "timestamp": ts,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/Users/henkelmann/Documents/PRIVAT/henley/.cursor/debug-a04a75.log")
    {
        let _ = writeln!(f, "{line}");
    }
}
// #endregion

pub async fn doctor(config: Config) -> anyhow::Result<()> {
    config.ensure_directories()?;
    println!("RAGDOLL_DATA_DIR={}", config.data_dir.display());
    println!("RAGDOLL_DB_PATH={}", config.db_path.display());
    println!(
        "RAGDOLL_MODEL_CACHE_DIR={}",
        config.model_cache_dir.display()
    );
    println!("RAGDOLL_STAGING_DIR={}", config.staging_dir.display());
    println!("RAGDOLL_EMBEDDING_DIM={}", config.embedding_dim);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_serve_command() {
        let cli = Cli::try_parse_from(["ragdoll", "serve"]).expect("parse serve");
        assert!(matches!(cli.command, Commands::Serve));
    }

    #[test]
    fn cli_parses_doctor_command() {
        let cli = Cli::try_parse_from(["ragdoll", "doctor"]).expect("parse doctor");
        assert!(matches!(cli.command, Commands::Doctor));
    }
}
