// SPDX-License-Identifier: AGPL-3.0-only

use std::path::Path;
use std::sync::Arc;

use libsql::{Builder, Connection, Database};

use crate::config::Config;
use crate::db::DbError;

const MAX_LOCKED_ATTEMPTS: u32 = 8;

#[derive(Clone)]
pub struct DbPool {
    db: Arc<Database>,
}

impl DbPool {
    pub async fn connect(config: &Config) -> Result<Self, DbError> {
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DbError::Migration(format!("failed to create db directory: {e}"))
            })?;
        }

        let db = Builder::new_local(&config.db_path).build().await?;

        let pool = Self { db: Arc::new(db) };
        pool.apply_pragmas().await?;
        Ok(pool)
    }

    pub async fn connect_path(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DbError::Migration(format!("failed to create db directory: {e}"))
            })?;
        }

        let db = Builder::new_local(path).build().await?;

        let pool = Self { db: Arc::new(db) };
        pool.apply_pragmas().await?;
        Ok(pool)
    }

    async fn apply_pragmas(&self) -> Result<(), DbError> {
        let conn = self.connect_one().await?;
        run_pragma(&conn, "PRAGMA wal_autocheckpoint = 1000").await?;
        Ok(())
    }

    pub async fn connect_one(&self) -> Result<Connection, DbError> {
        retry_on_locked(|| async {
            let conn = self.db.connect().map_err(DbError::Libsql)?;
            configure_connection(&conn).await?;
            Ok(conn)
        })
        .await
    }
}

pub async fn retry_on_locked<T, F, Fut>(mut operation: F) -> Result<T, DbError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, DbError>>,
{
    let mut last_err: Option<DbError> = None;
    for attempt in 0..MAX_LOCKED_ATTEMPTS {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if err.is_locked() && attempt + 1 < MAX_LOCKED_ATTEMPTS => {
                last_err = Some(err);
                tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt as u64 + 1))).await;
            }
            Err(err) => return Err(err),
        }
    }
    Err(last_err.unwrap_or_else(|| DbError::Migration("database remained locked".to_string())))
}

async fn configure_connection(conn: &Connection) -> Result<(), DbError> {
    run_pragma(conn, "PRAGMA journal_mode = WAL").await?;
    run_pragma(conn, "PRAGMA busy_timeout = 30000").await?;
    run_pragma(conn, "PRAGMA synchronous = NORMAL").await?;
    run_pragma(conn, "PRAGMA foreign_keys = ON").await?;
    Ok(())
}

async fn run_pragma(conn: &Connection, sql: &str) -> Result<(), DbError> {
    let mut rows = conn.query(sql, ()).await?;
    while rows.next().await?.is_some() {}
    Ok(())
}
