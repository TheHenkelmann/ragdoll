// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::batch::BatchItemResult;
use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::api::sources::{enqueue_ingest_job, IngestJobPayload};
use crate::auth::{authorize, AuthContext, Permission};
use crate::filter::{compile_filter, FilterExpr};
use crate::release::ReleaseCtx;

#[derive(Debug, Deserialize)]
pub struct ReindexRequest {
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub filter: Option<FilterExpr>,
}

#[derive(Debug, Deserialize)]
pub struct ReindexBatchPath {
    #[serde(rename = "tag")]
    pub _tag: String,
    pub batch_id: String,
}

#[derive(Debug, Serialize)]
pub struct ReindexResult {
    pub source_id: String,
    pub job_id: String,
}

#[derive(Debug, Serialize)]
pub struct ReindexBatchResponse {
    pub batch_id: String,
    pub items: Vec<BatchItemResult<ReindexResult>>,
}

impl ReindexBatchResponse {
    fn http_status(&self) -> axum::http::StatusCode {
        let all_ok = self
            .items
            .iter()
            .all(|item| (200..300).contains(&item.status));
        let all_failed = self.items.iter().all(|item| item.status >= 400);
        if all_ok {
            axum::http::StatusCode::OK
        } else if all_failed {
            axum::http::StatusCode::BAD_REQUEST
        } else {
            axum::http::StatusCode::MULTI_STATUS
        }
    }
}

impl IntoResponse for ReindexBatchResponse {
    fn into_response(self) -> Response {
        (self.http_status(), Json(self)).into_response()
    }
}

#[derive(Debug, Serialize)]
pub struct ReindexBatchSummary {
    pub batch_id: String,
    pub total: u32,
    pub pending: u32,
    pub processing: u32,
    pub completed: u32,
    pub failed: u32,
    pub active: u32,
}

#[derive(Debug, Serialize)]
pub struct ReindexBatchJob {
    pub id: String,
    pub source_id: String,
    pub source_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReindexBatchEvent {
    pub batch_id: String,
    pub summary: ReindexBatchSummary,
    pub jobs: Vec<ReindexBatchJob>,
}

pub async fn post_reindex(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<ReindexRequest>,
) -> Result<ReindexBatchResponse, ApiError> {
    authorize(&auth, Permission::SourcesWrite)?;

    let settings = state
        .settings_cache
        .get_or_load(&state.pool, &ctx.release_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let source_ids = resolve_reindex_sources(&conn, &ctx, &body).await?;

    let batch_id = Uuid::new_v4().to_string();

    let batch_size = settings.max_batch_size.max(1) as usize;
    let mut items = Vec::with_capacity(source_ids.len());
    for (index, source_id) in source_ids.into_iter().enumerate() {
        if index > 0 && index % batch_size == 0 {
            // Yield between batches so large reindex waves do not monopolize the runtime.
            tokio::task::yield_now().await;
        }
        items.push(reindex_one_source(&state, &ctx, &batch_id, index, &source_id).await);
    }

    Ok(ReindexBatchResponse { batch_id, items })
}

async fn resolve_reindex_sources(
    conn: &libsql::Connection,
    ctx: &ReleaseCtx,
    body: &ReindexRequest,
) -> Result<Vec<String>, ApiError> {
    if let Some(source_id) = &body.source_id {
        let mut rows = conn
            .query(
                "SELECT id FROM sources WHERE id = ?1 AND release_id = ?2",
                (source_id.as_str(), ctx.release_id.as_str()),
            )
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        if rows
            .next()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .is_none()
        {
            return Err(ApiError::not_found("source not found"));
        }
        return Ok(vec![source_id.clone()]);
    }

    let mut where_parts = vec![format!("s.release_id = '{}'", ctx.release_id)];
    let mut params: Vec<String> = Vec::new();
    if let Some(filter) = &body.filter {
        let compiled =
            compile_filter(filter, "s").map_err(|e| ApiError::bad_request(e.to_string()))?;
        where_parts.push(compiled.sql);
        params.extend(compiled.params);
    }
    let where_clause = where_parts.join(" AND ");
    let sql = format!("SELECT s.id FROM sources s WHERE {where_clause} ORDER BY s.created_at ASC");

    let mut rows = conn
        .query(&sql, params)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut ids = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        ids.push(row.get(0).map_err(|e| ApiError::internal(e.to_string()))?);
    }
    Ok(ids)
}

async fn reindex_one_source(
    state: &AppState,
    ctx: &ReleaseCtx,
    batch_id: &str,
    index: usize,
    source_id: &str,
) -> BatchItemResult<ReindexResult> {
    match reindex_source_internal(state, ctx, batch_id, source_id).await {
        Ok(result) => BatchItemResult::ok(index, result),
        Err(err) => {
            BatchItemResult::err(index, axum::http::StatusCode::BAD_REQUEST, err.to_string())
        }
    }
}

async fn reindex_source_internal(
    state: &AppState,
    ctx: &ReleaseCtx,
    batch_id: &str,
    source_id: &str,
) -> anyhow::Result<ReindexResult> {
    let conn = state.pool.connect_one().await?;

    let mut source_rows = conn
        .query(
            "SELECT name, type, uri, content_hash, config, metadata
             FROM sources WHERE id = ?1 AND release_id = ?2",
            (source_id, ctx.release_id.as_str()),
        )
        .await?;
    let source_row = source_rows
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("source not found"))?;
    let source_name: String = source_row.get(0)?;
    let source_type: String = source_row.get(1)?;
    let source_uri: Option<String> = source_row.get(2).ok();
    let content_hash: Option<String> = source_row.get(3).ok();
    let config: String = source_row.get(4)?;
    let metadata: String = source_row.get(5)?;

    let mut text_rows = conn
        .query(
            "SELECT 1 FROM source_texts WHERE source_id = ?1",
            [source_id],
        )
        .await?;
    if text_rows.next().await?.is_none() {
        anyhow::bail!("source has no stored extracted text; ingest it first");
    }

    let mut active = conn
        .query(
            "SELECT 1 FROM ingest_jobs
             WHERE source_id = ?1 AND status IN ('pending', 'processing')
             LIMIT 1",
            [source_id],
        )
        .await?;
    if active.next().await?.is_some() {
        anyhow::bail!("source already has a pending or processing job");
    }

    let job_id = enqueue_ingest_job(
        &conn,
        state,
        ctx,
        Some(batch_id),
        &IngestJobPayload {
            source_id: source_id.to_string(),
            source_name,
            source_type,
            source_uri,
            content_hash,
            config,
            metadata,
        },
    )
    .await?;

    Ok(ReindexResult {
        source_id: source_id.to_string(),
        job_id,
    })
}

pub async fn stream_reindex_batch(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(ReindexBatchPath { batch_id, .. }): Path<ReindexBatchPath>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, ApiError> {
    authorize(&auth, Permission::SourcesRead)?;

    let initial = load_batch_event(&state, &ctx.release_id, &batch_id).await?;
    if initial.summary.total == 0 {
        return Err(ApiError::not_found("reindex batch not found"));
    }

    let stream = futures_util::stream::unfold(
        (state, ctx.release_id, batch_id, false),
        |(state, release_id, batch_id, sent_final)| async move {
            if sent_final {
                return None;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
            let event = match load_batch_event(&state, &release_id, &batch_id).await {
                Ok(event) => event,
                Err(err) => {
                    let fallback = serde_json::json!({
                        "batch_id": batch_id,
                        "error": err.problem.detail,
                    });
                    let sse = Event::default()
                        .event("error")
                        .json_data(fallback)
                        .unwrap_or_else(|_| {
                            Event::default().event("error").data("batch poll failed")
                        });
                    return Some((Ok(sse), (state, release_id, batch_id, true)));
                }
            };

            let finished = event.summary.active == 0;
            let sse = Event::default()
                .event(if finished { "complete" } else { "progress" })
                .json_data(&event)
                .unwrap_or_else(|_| Event::default().data("ok"));
            Some((Ok(sse), (state, release_id, batch_id, finished)))
        },
    );

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

async fn load_batch_event(
    state: &AppState,
    release_id: &str,
    batch_id: &str,
) -> Result<ReindexBatchEvent, ApiError> {
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut summary_rows = conn
        .query(
            "SELECT status, COUNT(*)
             FROM ingest_jobs
             WHERE release_id = ?1 AND batch_id = ?2
             GROUP BY status",
            (release_id, batch_id),
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

    let mut rows = conn
        .query(
            "SELECT j.id, j.source_id, COALESCE(s.name, j.source_name), j.status, j.error, j.created_at, j.finished_at
             FROM ingest_jobs j
             LEFT JOIN sources s ON s.id = j.source_id
             WHERE j.release_id = ?1 AND j.batch_id = ?2
             ORDER BY
               CASE j.status
                 WHEN 'processing' THEN 0
                 WHEN 'pending' THEN 1
                 WHEN 'failed' THEN 2
                 ELSE 3
               END,
               j.created_at DESC",
            (release_id, batch_id),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut jobs = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        jobs.push(ReindexBatchJob {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            source_id: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            source_name: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            status: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
            error: row.get(4).map_err(|e| ApiError::internal(e.to_string()))?,
            created_at: row.get(5).map_err(|e| ApiError::internal(e.to_string()))?,
            finished_at: row.get(6).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }

    let total = pending + processing + completed + failed;
    let active = pending + processing;
    Ok(ReindexBatchEvent {
        batch_id: batch_id.to_string(),
        summary: ReindexBatchSummary {
            batch_id: batch_id.to_string(),
            total,
            pending,
            processing,
            completed,
            failed,
            active,
        },
        jobs,
    })
}
