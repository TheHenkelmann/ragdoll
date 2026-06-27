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

    /// Transient libsql errors that are safe to retry during connection
    /// acquisition. Besides plain lock contention, libsql can return
    /// `SQLITE_MISUSE` ("bad parameter or other API misuse") when many
    /// connections initialize concurrently (e.g. parallel test setup); this
    /// clears on retry.
    pub fn is_transient(&self) -> bool {
        let msg = self.to_string();
        self.is_locked() || msg.contains("bad parameter or other API misuse")
    }
}
