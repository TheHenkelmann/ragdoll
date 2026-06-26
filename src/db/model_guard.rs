// SPDX-License-Identifier: AGPL-3.0-only

use crate::config::Config;
use crate::db::{DbError, DbPool};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommittedModel {
    pub name: String,
    pub dim: i64,
    pub version: String,
}

fn meta_key(release_id: &str) -> String {
    format!("committed_embedding_model:{release_id}")
}

pub async fn ensure_committed_model(
    pool: &DbPool,
    release_id: &str,
    expected_name: &str,
    expected_dim: i64,
) -> Result<CommittedModel, DbError> {
    let conn = pool.connect_one().await?;
    let key = meta_key(release_id);

    let mut rows = conn
        .query("SELECT value FROM meta WHERE key = ?1", [key.as_str()])
        .await?;

    if let Some(row) = rows.next().await? {
        let value: String = row.get(0)?;
        let committed: CommittedModel = serde_json::from_str(&value).map_err(|e| {
            DbError::ModelGuard(format!("invalid committed model metadata: {e}"))
        })?;

        if committed.name != expected_name || committed.dim != expected_dim {
            return Err(DbError::ModelGuard(format!(
                "embedding model mismatch for release {release_id}: database committed to {} (dim {}), \
                 but expected {} (dim {expected_dim}). Re-embed all chunks or reset the release.",
                committed.name, committed.dim, expected_name
            )));
        }

        return Ok(committed);
    }

    let mut chunk_rows = conn
        .query(
            "SELECT embedding_model, embedding_dim, embedding_version FROM chunks WHERE release_id = ?1 LIMIT 1",
            [release_id],
        )
        .await?;

    if let Some(row) = chunk_rows.next().await? {
        let name: String = row.get(0)?;
        let dim: i64 = row.get(1)?;
        let version: String = row.get(2)?;

        if name != expected_name || dim != expected_dim {
            return Err(DbError::ModelGuard(format!(
                "existing chunks in release {release_id} use {} (dim {}), but expected {} (dim {expected_dim})",
                name, dim, expected_name
            )));
        }

        let committed = CommittedModel {
            name,
            dim,
            version: version.clone(),
        };
        persist_committed_model(pool, release_id, &committed).await?;
        return Ok(committed);
    }

    let committed = CommittedModel {
        name: expected_name.to_string(),
        dim: expected_dim,
        version: "1".to_string(),
    };
    persist_committed_model(pool, release_id, &committed).await?;
    Ok(committed)
}

async fn persist_committed_model(
    pool: &DbPool,
    release_id: &str,
    model: &CommittedModel,
) -> Result<(), DbError> {
    let conn = pool.connect_one().await?;
    let value = serde_json::to_string(model)
        .map_err(|e| DbError::ModelGuard(format!("serialize committed model: {e}")))?;
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
        (meta_key(release_id).as_str(), value.as_str()),
    )
    .await?;
    Ok(())
}

pub async fn run_model_guard(pool: &DbPool, config: &Config) -> Result<(), DbError> {
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query("SELECT id FROM releases", ())
        .await?;

    while let Some(row) = rows.next().await? {
        let release_id: String = row.get(0)?;
        let mut setting_rows = conn
            .query(
                "SELECT value FROM settings WHERE release_id = ?1 AND key = 'embedding_model'",
                [release_id.as_str()],
            )
            .await?;

        let model_name = if let Some(srow) = setting_rows.next().await? {
            let raw: String = srow.get(0)?;
            serde_json::from_str::<String>(&raw).unwrap_or(raw)
        } else {
            config.embedding_model.clone()
        };

        ensure_committed_model(
            pool,
            &release_id,
            &model_name,
            config.embedding_dim as i64,
        )
        .await?;
    }

    tracing::info!(dim = config.embedding_dim, "model guard passed for all releases");
    Ok(())
}
