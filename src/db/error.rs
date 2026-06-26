// SPDX-License-Identifier: AGPL-3.0-only

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite failure: `{0}`")]
    Libsql(#[from] libsql::Error),
    #[error("migration error: {0}")]
    Migration(String),
    #[error("model guard error: {0}")]
    ModelGuard(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl DbError {
    pub fn is_locked(&self) -> bool {
        self.to_string().contains("database is locked")
    }
}
