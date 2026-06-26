// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::batch::{BatchItemResult, BatchResponse};
use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::filter::decode_filter_param;
use crate::release::{NestedPathId, ReleaseCtx};
use crate::search::{QueryOptions, QueryRequest};

#[derive(Debug, Deserialize)]
pub struct ListQueryParams {
    pub filter: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub ts_start: Option<i64>,
    #[serde(default)]
    pub store_payload: Option<bool>,
    #[serde(default)]
    pub playground: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredQuery {
    pub id: String,
    pub text: Option<String>,
    pub filters: serde_json::Value,
    pub params: serde_json::Value,
    pub playground: bool,
    pub upstream_ms: Option<i64>,
    pub embed_ms: Option<i64>,
    pub search_ms: Option<i64>,
    pub rerank_ms: Option<i64>,
    pub store_ms: Option<i64>,
    pub total_ms: Option<i64>,
    pub candidate_count: Option<i64>,
    pub result_count: Option<i64>,
    pub response_status: i64,
    pub created_at: String,
}

pub async fn post_queries(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Query(params): Query<QueryParams>,
    Json(items): Json<Vec<QueryRequest>>,
) -> Result<BatchResponse<crate::search::QueryResult>, ApiError> {
    let settings = state
        .settings_cache
        .get_or_load(&state.pool, &ctx.release_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if items.len() as u32 > settings.max_batch_size {
        return Err(ApiError::bad_request("batch too large"));
    }

    let options = QueryOptions {
        ts_start: params.ts_start,
        store_payload: params.store_payload.unwrap_or(false),
        playground: params.playground.unwrap_or(false),
    };

    let mut results = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        match state.search.execute(&ctx, &settings, &item, &options).await {
            Ok(result) => results.push(BatchItemResult::ok(index, result)),
            Err(err) => {
                let _ = state
                    .search
                    .record_failure(&ctx, &settings, &item, &options, 500)
                    .await;
                results.push(BatchItemResult::err(
                    index,
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    err.to_string(),
                ));
            }
        }
    }
    Ok(BatchResponse { items: results })
}

pub async fn get_queries(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<Vec<StoredQuery>>, ApiError> {
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);

    let mut where_clause = format!("q.release_id = '{}'", ctx.release_id);
    let mut bind: Vec<String> = Vec::new();

    if let Some(filter_raw) = params.filter {
        let filter =
            decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
        let compiled = crate::filter::compile_filter(&filter, "q")
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        where_clause = format!("q.release_id = '{}' AND {}", ctx.release_id, compiled.sql);
        bind.extend(compiled.params);
    }

    bind.push(limit.to_string());
    bind.push(offset.to_string());

    let sql = format!(
        "SELECT id, text, filters, params, playground, upstream_ms, embed_ms, search_ms, rerank_ms,
                store_ms, total_ms, candidate_count, result_count, response_status, created_at
         FROM queries q
         WHERE {where_clause}
         ORDER BY created_at DESC
         LIMIT ?{} OFFSET ?{}",
        bind.len() - 1,
        bind.len()
    );

    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(&sql, bind)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let playground: i64 = row.get(4).map_err(|e| ApiError::internal(e.to_string()))?;
        items.push(StoredQuery {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            text: row.get(1).ok(),
            filters: serde_json::from_str(
                &row.get::<String>(2)
                    .map_err(|e| ApiError::internal(e.to_string()))?,
            )
            .unwrap_or(serde_json::json!({})),
            params: serde_json::from_str(
                &row.get::<String>(3)
                    .map_err(|e| ApiError::internal(e.to_string()))?,
            )
            .unwrap_or(serde_json::json!({})),
            playground: playground != 0,
            upstream_ms: row.get(5).ok(),
            embed_ms: row.get(6).ok(),
            search_ms: row.get(7).ok(),
            rerank_ms: row.get(8).ok(),
            store_ms: row.get(9).ok(),
            total_ms: row.get(10).ok(),
            candidate_count: row.get(11).ok(),
            result_count: row.get(12).ok(),
            response_status: row.get(13).map_err(|e| ApiError::internal(e.to_string()))?,
            created_at: row.get(14).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }

    Ok(Json(items))
}

pub async fn delete_queries(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let filter_raw = params
        .filter
        .ok_or_else(|| ApiError::bad_request("filter query param required"))?;
    let filter =
        decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let compiled = crate::filter::compile_filter(&filter, "q")
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let sql = format!(
        "DELETE FROM queries q WHERE q.release_id = '{}' AND {}",
        ctx.release_id, compiled.sql
    );
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(&sql, compiled.params)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

pub async fn get_query_detail(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    axum::extract::Path(NestedPathId { id: query_id, .. }): axum::extract::Path<NestedPathId>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, text, filters, params, playground, upstream_ms, embed_ms, search_ms, rerank_ms,
                    store_ms, total_ms, candidate_count, result_count, response_status, created_at
             FROM queries WHERE id = ?1 AND release_id = ?2",
            (query_id.as_str(), ctx.release_id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("query not found"))?;

    let mut chunk_rows = conn
        .query(
            "SELECT qc.step, qc.rank, qc.chunk_id, qc.source_id, COALESCE(s.name, qc.source_id), qc.score, qc.metadata, qc.content
             FROM query_chunks qc
             LEFT JOIN sources s ON s.id = qc.source_id AND s.release_id = qc.release_id
             WHERE qc.query_id = ?1 ORDER BY qc.step, qc.rank",
            [query_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut chunks = Vec::new();
    while let Some(crow) = chunk_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        chunks.push(serde_json::json!({
            "step": crow.get::<String>(0).map_err(|e| ApiError::internal(e.to_string()))?,
            "rank": crow.get::<i64>(1).map_err(|e| ApiError::internal(e.to_string()))?,
            "chunk_id": crow.get::<String>(2).map_err(|e| ApiError::internal(e.to_string()))?,
            "source_id": crow.get::<String>(3).map_err(|e| ApiError::internal(e.to_string()))?,
            "source_name": crow.get::<String>(4).map_err(|e| ApiError::internal(e.to_string()))?,
            "score": crow.get::<f64>(5).map_err(|e| ApiError::internal(e.to_string()))?,
            "metadata": serde_json::from_str(&crow.get::<String>(6).map_err(|e| ApiError::internal(e.to_string()))?).unwrap_or(serde_json::json!({})),
            "content": crow.get::<Option<String>>(7).ok().flatten(),
        }));
    }

    let playground: i64 = row.get(4).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "id": row.get::<String>(0).map_err(|e| ApiError::internal(e.to_string()))?,
        "text": row.get::<Option<String>>(1).ok().flatten(),
        "filters": serde_json::from_str(&row.get::<String>(2).map_err(|e| ApiError::internal(e.to_string()))?).unwrap_or(serde_json::json!({})),
        "params": serde_json::from_str(&row.get::<String>(3).map_err(|e| ApiError::internal(e.to_string()))?).unwrap_or(serde_json::json!({})),
        "playground": playground != 0,
        "upstream_ms": row.get::<Option<i64>>(5).ok().flatten(),
        "embed_ms": row.get::<Option<i64>>(6).ok().flatten(),
        "search_ms": row.get::<Option<i64>>(7).ok().flatten(),
        "rerank_ms": row.get::<Option<i64>>(8).ok().flatten(),
        "store_ms": row.get::<Option<i64>>(9).ok().flatten(),
        "total_ms": row.get::<Option<i64>>(10).ok().flatten(),
        "candidate_count": row.get::<Option<i64>>(11).ok().flatten(),
        "result_count": row.get::<Option<i64>>(12).ok().flatten(),
        "response_status": row.get::<i64>(13).map_err(|e| ApiError::internal(e.to_string()))?,
        "created_at": row.get::<String>(14).map_err(|e| ApiError::internal(e.to_string()))?,
        "chunks": chunks,
    })))
}
