// SPDX-License-Identifier: AGPL-3.0-only

use std::path::Path;

use crate::config::Config;
use crate::db::{DbError, DbPool};

pub async fn run_migrations(pool: &DbPool, migrations_dir: &Path) -> Result<(), DbError> {
    let conn = pool.connect_one().await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        (),
    )
    .await?;

    let mut paths: Vec<_> = std::fs::read_dir(migrations_dir)
        .map_err(|e| DbError::Migration(format!("read migrations dir: {e}")))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "sql"))
        .collect();

    paths.sort_by_key(|entry| entry.path());

    for entry in paths {
        let path = entry.path();
        let file_name = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            DbError::Migration(format!("invalid migration file: {}", path.display()))
        })?;

        let version: i64 = file_name
            .split('_')
            .next()
            .ok_or_else(|| DbError::Migration(format!("invalid migration name: {file_name}")))?
            .parse()
            .map_err(|_| DbError::Migration(format!("invalid migration version: {file_name}")))?;

        let mut rows = conn
            .query(
                "SELECT version FROM schema_migrations WHERE version = ?1",
                [version],
            )
            .await?;

        if rows.next().await?.is_some() {
            continue;
        }

        let sql = std::fs::read_to_string(&path)
            .map_err(|e| DbError::Migration(format!("read {}: {e}", path.display())))?;

        conn.execute("BEGIN IMMEDIATE", ()).await?;
        let apply_result = conn.execute_batch(&sql).await;
        if let Err(err) = apply_result {
            let _ = conn.execute("ROLLBACK", ()).await;
            return Err(DbError::Migration(format!(
                "apply migration {version}: {err}"
            )));
        }

        conn.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [version],
        )
        .await?;
        conn.execute("COMMIT", ()).await?;
        tracing::info!(version, file = %path.display(), "applied migration");
    }

    Ok(())
}

pub async fn migrate(config: &Config) -> Result<(), DbError> {
    let pool = DbPool::connect(config).await?;
    run_migrations(&pool, &config.migrations_dir).await
}
