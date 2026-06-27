// SPDX-License-Identifier: AGPL-3.0-only

use crate::db::{DbError, DbPool};
use crate::settings::RuntimeSettings;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbeddingMismatch {
    pub release_id: String,
    pub release_tag: String,
    pub settings_model: String,
    pub chunks_model: Option<String>,
    pub message: String,
}

/// Inspect whether stored chunk vectors match the release's configured embedding model.
/// Does not mutate state and never fails on mismatch (only on DB errors).
pub async fn check_embedding_mismatches(
    pool: &DbPool,
    expected_dim: i64,
) -> Result<Vec<EmbeddingMismatch>, DbError> {
    let conn = pool.connect_one().await?;
    let mut release_rows = conn
        .query("SELECT id, tag FROM releases ORDER BY created_at ASC", ())
        .await?;

    let mut mismatches = Vec::new();
    while let Some(row) = release_rows.next().await? {
        let release_id: String = row.get(0)?;
        let release_tag: String = row.get(1)?;

        let settings_model = load_release_embedding_model(&conn, &release_id).await?;
        let chunk_models = load_distinct_chunk_embedding_models(&conn, &release_id).await?;

        if let Some(mismatch) = embedding_mismatch_for_release(
            &release_id,
            &release_tag,
            &settings_model,
            &chunk_models,
            expected_dim,
        ) {
            mismatches.push(mismatch);
        }
    }

    Ok(mismatches)
}

pub async fn run_model_guard(pool: &DbPool, expected_dim: i64) -> Result<(), DbError> {
    let mismatches = check_embedding_mismatches(pool, expected_dim).await?;
    if mismatches.is_empty() {
        tracing::info!(dim = expected_dim, "embedding model guard: no mismatches");
        return Ok(());
    }
    for mismatch in &mismatches {
        tracing::warn!(
            release_id = %mismatch.release_id,
            release_tag = %mismatch.release_tag,
            settings_model = %mismatch.settings_model,
            chunks_model = ?mismatch.chunks_model,
            "embedding model mismatch — queries may be unreliable until re-index completes"
        );
    }
    Ok(())
}

/// Returns a mismatch when any stored chunk embedding model differs from settings.
fn embedding_mismatch_for_release(
    release_id: &str,
    release_tag: &str,
    settings_model: &str,
    chunk_models: &[String],
    expected_dim: i64,
) -> Option<EmbeddingMismatch> {
    if chunk_models.is_empty() {
        return None;
    }

    let wrong_models: Vec<&str> = chunk_models
        .iter()
        .map(String::as_str)
        .filter(|model| *model != settings_model)
        .collect();

    if wrong_models.is_empty() {
        return None;
    }

    let chunks_summary = chunk_models.join(", ");
    let message = if chunk_models.len() == 1 {
        format!(
            "release {release_id} is configured for {settings_model} (dim {expected_dim}), \
             but existing chunks use {}. Re-index all sources before queries are reliable.",
            chunk_models[0]
        )
    } else if wrong_models.len() == chunk_models.len() {
        format!(
            "release {release_id} is configured for {settings_model} (dim {expected_dim}), \
             but existing chunks use multiple embedding models ({chunks_summary}). \
             Re-index all sources before queries are reliable."
        )
    } else {
        format!(
            "release {release_id} is configured for {settings_model} (dim {expected_dim}), \
             but some chunks still use other embedding models ({chunks_summary}). \
             Re-index all sources before queries are reliable."
        )
    };

    Some(EmbeddingMismatch {
        release_id: release_id.to_string(),
        release_tag: release_tag.to_string(),
        settings_model: settings_model.to_string(),
        chunks_model: Some(chunks_summary),
        message,
    })
}

async fn load_release_embedding_model(
    conn: &libsql::Connection,
    release_id: &str,
) -> Result<String, DbError> {
    let mut setting_rows = conn
        .query(
            "SELECT value FROM settings WHERE release_id = ?1 AND key = 'embedding_model'",
            [release_id],
        )
        .await?;

    if let Some(srow) = setting_rows.next().await? {
        let raw: String = srow.get(0)?;
        return Ok(serde_json::from_str::<String>(&raw).unwrap_or(raw));
    }
    Ok(RuntimeSettings::default().embedding_model)
}

async fn load_distinct_chunk_embedding_models(
    conn: &libsql::Connection,
    release_id: &str,
) -> Result<Vec<String>, DbError> {
    let mut rows = conn
        .query(
            "SELECT DISTINCT embedding_model
             FROM chunks
             WHERE release_id = ?1
             ORDER BY embedding_model ASC",
            [release_id],
        )
        .await?;

    let mut models = Vec::new();
    while let Some(row) = rows.next().await? {
        let name: String = row.get(0)?;
        models.push(name);
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_chunks_means_no_mismatch() {
        assert!(embedding_mismatch_for_release(
            "rel-1",
            "v1",
            "BAAI/bge-m3",
            &[],
            1024,
        )
        .is_none());
    }

    #[test]
    fn matching_single_model_means_no_mismatch() {
        assert!(embedding_mismatch_for_release(
            "rel-1",
            "v1",
            "BAAI/bge-m3",
            &["BAAI/bge-m3".into()],
            1024,
        )
        .is_none());
    }

    #[test]
    fn single_wrong_model_reports_mismatch() {
        let mismatch = embedding_mismatch_for_release(
            "rel-1",
            "v1",
            "mixedbread-ai/mxbai-embed-large-v1",
            &["BAAI/bge-m3".into()],
            1024,
        )
        .expect("mismatch");

        assert_eq!(mismatch.chunks_model.as_deref(), Some("BAAI/bge-m3"));
        assert!(mismatch.message.contains("existing chunks use BAAI/bge-m3"));
    }

    #[test]
    fn multiple_distinct_models_report_mismatch_even_if_one_matches_settings() {
        let mismatch = embedding_mismatch_for_release(
            "rel-1",
            "first-release",
            "mixedbread-ai/mxbai-embed-large-v1",
            &[
                "BAAI/bge-m3".into(),
                "mixedbread-ai/mxbai-embed-large-v1".into(),
            ],
            1024,
        )
        .expect("mismatch");

        assert_eq!(
            mismatch.chunks_model.as_deref(),
            Some("BAAI/bge-m3, mixedbread-ai/mxbai-embed-large-v1")
        );
        assert!(mismatch.message.contains("some chunks still use other embedding models"));
    }

    #[test]
    fn multiple_wrong_models_report_all_models() {
        let mismatch = embedding_mismatch_for_release(
            "rel-1",
            "v1",
            "mixedbread-ai/mxbai-embed-large-v1",
            &["BAAI/bge-m3".into(), "intfloat/multilingual-e5-large".into()],
            1024,
        )
        .expect("mismatch");

        assert_eq!(
            mismatch.chunks_model.as_deref(),
            Some("BAAI/bge-m3, intfloat/multilingual-e5-large")
        );
        assert!(mismatch.message.contains("multiple embedding models"));
    }
}
