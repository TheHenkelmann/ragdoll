// SPDX-License-Identifier: AGPL-3.0-only

use uuid::Uuid;

use crate::auth::{hash_password, verify_password};
use crate::config::Config;
use crate::db::{DbError, DbPool};

pub async fn ensure_superadmin(pool: &DbPool, config: &Config) -> Result<(), DbError> {
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query(
            "SELECT id, password_hash, password_is_default FROM users WHERE is_superadmin = 1 LIMIT 1",
            (),
        )
        .await?;

    let password = config
        .superadmin_password
        .clone()
        .unwrap_or_else(|| "admin".to_string());
    let password_is_default = config.superadmin_password.is_none();

    if let Some(row) = rows.next().await? {
        let id: String = row.get(0)?;
        if config.superadmin_password.is_some() {
            let hash = hash_password(&password)
                .map_err(|e| DbError::InvalidInput(format!("hash password: {e}")))?;
            conn.execute(
                "UPDATE users SET password_hash = ?1, password_is_default = 0 WHERE id = ?2",
                (hash.as_str(), id.as_str()),
            )
            .await?;
        } else {
            conn.execute(
                "UPDATE users SET password_is_default = 1 WHERE id = ?1",
                [id.as_str()],
            )
            .await?;
        }
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    let hash = hash_password(&password)
        .map_err(|e| DbError::InvalidInput(format!("hash password: {e}")))?;
    conn.execute(
        "INSERT INTO users (id, email, password_hash, is_superadmin, password_is_default)
         VALUES (?1, ?2, ?3, 1, ?4)",
        (
            id.as_str(),
            config.superadmin_email.as_str(),
            hash.as_str(),
            if password_is_default { 1i64 } else { 0i64 },
        ),
    )
    .await?;
    tracing::info!(email = %config.superadmin_email, "seeded superadmin user");
    Ok(())
}

pub async fn authenticate_user(
    pool: &DbPool,
    email: &str,
    password: &str,
) -> Result<(String, bool, bool), DbError> {
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query(
            "SELECT id, password_hash, is_superadmin, password_is_default FROM users WHERE email = ?1",
            [email],
        )
        .await?;

    let row = rows
        .next()
        .await?
        .ok_or_else(|| DbError::InvalidInput("invalid credentials".into()))?;

    let id: String = row.get(0)?;
    let hash: String = row.get(1)?;
    let is_superadmin: i64 = row.get(2)?;
    let password_is_default: i64 = row.get(3)?;

    let ok = verify_password(password, &hash)
        .map_err(|e| DbError::InvalidInput(format!("verify password: {e}")))?;
    if !ok {
        return Err(DbError::InvalidInput("invalid credentials".into()));
    }

    Ok((id, is_superadmin != 0, password_is_default != 0))
}

pub fn validate_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_email_accepts_common_addresses() {
        assert!(validate_email("admin@ragdoll.ai"));
        assert!(validate_email("user@example.com"));
    }

    #[test]
    fn validate_email_rejects_invalid_addresses() {
        assert!(!validate_email("not-an-email"));
        assert!(!validate_email("@missing-local.com"));
        assert!(!validate_email("missing-domain@localhost"));
    }
}
