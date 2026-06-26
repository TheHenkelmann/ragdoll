// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::api::build_router;
use crate::auth::ensure_superadmin;
use crate::config::Config;
use crate::db::{migrations, model_guard, DbPool};
use crate::models;

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
        Commands::ModelsEnsure => models::ensure_models(&config).await,
        Commands::ModelGuard => {
            let pool = DbPool::connect(&config).await?;
            model_guard::run_model_guard(&pool, &config)
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
    model_guard::run_model_guard(&pool, &config).await?;
    models::ensure_models(&config).await?;

    let state = crate::api::router::build_state(config.clone(), pool).await?;
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port))
        .await
        .with_context(|| format!("bind port {}", config.port))?;
    tracing::info!(port = config.port, "ragdoll listening");
    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn doctor(config: Config) -> anyhow::Result<()> {
    config.ensure_directories()?;
    println!("RAGDOLL_DATA_DIR={}", config.data_dir.display());
    println!("RAGDOLL_DB_PATH={}", config.db_path.display());
    println!(
        "RAGDOLL_MODEL_CACHE_DIR={}",
        config.model_cache_dir.display()
    );
    println!("RAGDOLL_STAGING_DIR={}", config.staging_dir.display());
    println!("RAGDOLL_EMBEDDING_MODEL={}", config.embedding_model);
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
