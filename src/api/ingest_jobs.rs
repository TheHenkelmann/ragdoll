// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::release::ReleaseCtx;

#[derive(Debug, Deserialize)]
pub struct IngestJobsQuery {
    #[serde(default)]
    pub details: Option<bool>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct IngestJobsSummary {
    pub total: u32,
    pub pending: u32,
    pub processing: u32,
    pub completed: u32,
    pub failed: u32,
    pub active: u32,
}

#[derive(Debug, Serialize)]
pub struct IngestJobRecord {
    pub id: String,
    pub source_id: String,
    pub source_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IngestJobsStatusResponse {
    pub summary: IngestJobsSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<IngestJobRecord>>,
}

pub async fn get_ingest_jobs_status(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<IngestJobsQuery>,
) -> Result<Json<IngestJobsStatusResponse>, ApiError> {
    authorize(&auth, Permission::SourcesRead)?;

    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut summary_rows = conn
        .query(
            "SELECT status, COUNT(*) FROM ingest_jobs WHERE release_id = ?1 GROUP BY status",
            [ctx.release_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut pending = 0u32;
    let mut processing = 0u32;
    let mut completed = 0u32;
    let mut failed = 0u32;

    while let Some(row) = summary_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let status: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        let count: i64 = row.get(1).map_err(|e| ApiError::internal(e.to_string()))?;
        let count = u32::try_from(count).unwrap_or(0);
        match status.as_str() {
            "pending" => pending = count,
            "processing" => processing = count,
            "completed" => completed = count,
            "failed" => failed = count,
            _ => {}
        }
    }

    let total = pending + processing + completed + failed;
    let active = pending + processing;

    let jobs = if query.details.unwrap_or(false) {
        let limit = query.limit.unwrap_or(100).min(500);
        let mut rows = conn
            .query(
                "SELECT j.id, j.source_id, COALESCE(s.name, j.source_name), j.status, j.error, j.created_at, j.finished_at
                 FROM ingest_jobs j
                 LEFT JOIN sources s ON s.id = j.source_id
                 WHERE j.release_id = ?1
                 ORDER BY
                   CASE j.status
                     WHEN 'processing' THEN 0
                     WHEN 'pending' THEN 1
                     WHEN 'failed' THEN 2
                     ELSE 3
                   END,
                   j.created_at DESC
                 LIMIT ?2",
                (ctx.release_id.as_str(), limit as i64),
            )
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

        let mut jobs = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
        {
            jobs.push(IngestJobRecord {
                id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
                source_id: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
                source_name: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
                status: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
                error: row.get(4).map_err(|e| ApiError::internal(e.to_string()))?,
                created_at: row.get(5).map_err(|e| ApiError::internal(e.to_string()))?,
                finished_at: row.get(6).map_err(|e| ApiError::internal(e.to_string()))?,
            });
        }
        Some(jobs)
    } else {
        None
    };

    Ok(Json(IngestJobsStatusResponse {
        summary: IngestJobsSummary {
            total,
            pending,
            processing,
            completed,
            failed,
            active,
        },
        jobs,
    }))
}
