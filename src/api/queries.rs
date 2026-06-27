// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::api::batch::{BatchItemResult, BatchResponse};
use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::filter::decode_filter_param;
use crate::generation::{
    attach_generation, generate_answer, persist_generation_metrics, resolve_generation_spec,
    GenerationOutput, StreamEvent, MAX_GENERATION_CONCURRENCY,
};
use crate::release::{NestedPathId, ReleaseCtx};
use crate::search::{QueryLatency, QueryOptions, QueryRequest, QueryResult};

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
    pub total_ragdoll_ms: Option<i64>,
    pub generation_ms: Option<i64>,
    pub generation_total_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub llm_model_id: Option<String>,
    pub candidate_count: Option<i64>,
    pub result_count: Option<i64>,
    pub response_status: i64,
    pub created_at: String,
}

pub async fn post_queries(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<QueryParams>,
    Json(items): Json<Vec<QueryRequest>>,
) -> Result<Response, ApiError> {
    authorize(&auth, Permission::QueriesRun)?;
    execute_queries(state, ctx, params, items).await
}

async fn execute_queries(
    state: Arc<AppState>,
    ctx: ReleaseCtx,
    params: QueryParams,
    items: Vec<QueryRequest>,
) -> Result<Response, ApiError> {
    let settings = state
        .settings_cache
        .get_or_load(&state.pool, &ctx.release_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if items.len() as u32 > settings.max_batch_size {
        return Err(ApiError::bad_request("batch too large"));
    }

    if items
        .iter()
        .any(|item| item.generation.as_ref().is_some_and(|g| g.stream))
    {
        if items.len() != 1 {
            return Err(ApiError::bad_request(
                "streaming generation requires exactly one query item",
            ));
        }
        let item = items.into_iter().next().expect("one item");
        return handle_streaming_query(state, ctx, params, item, &settings).await;
    }

    let options = QueryOptions {
        ts_start: params.ts_start,
        store_payload: params.store_payload.unwrap_or(false),
        playground: params.playground.unwrap_or(false),
    };

    let has_generation = items.iter().any(|item| item.generation.is_some());
    if has_generation {
        let indexed: Vec<(usize, QueryRequest)> = items.into_iter().enumerate().collect();
        let results: Vec<BatchItemResult<QueryResult>> =
            stream::iter(indexed)
                .map(|(index, item)| {
                    let state = state.clone();
                    let ctx = ctx.clone();
                    let settings = settings.clone();
                    let options = options.clone();
                    async move {
                        process_query_item(&state, &ctx, &settings, &options, index, item).await
                    }
                })
                .buffer_unordered(MAX_GENERATION_CONCURRENCY)
                .collect()
                .await;
        Ok(BatchResponse { items: results }.into_response())
    } else {
        let mut results = Vec::with_capacity(items.len());
        for (index, item) in items.into_iter().enumerate() {
            results.push(process_query_item(&state, &ctx, &settings, &options, index, item).await);
        }
        Ok(BatchResponse { items: results }.into_response())
    }
}

async fn process_query_item(
    state: &AppState,
    ctx: &ReleaseCtx,
    settings: &crate::settings::RuntimeSettings,
    options: &QueryOptions,
    index: usize,
    item: QueryRequest,
) -> BatchItemResult<QueryResult> {
    if item.text.trim().is_empty() {
        return BatchItemResult::err(index, StatusCode::BAD_REQUEST, "query text required");
    }

    let generation = item.generation.clone();
    match state.search.execute(ctx, settings, &item, options).await {
        Ok(mut result) => {
            if let Some(gen) = generation {
                match generate_answer(
                    &state.pool,
                    &state.crypto,
                    state.generator.as_ref(),
                    ctx,
                    settings,
                    &gen,
                    &item.text,
                    result.matches.clone(),
                )
                .await
                {
                    Ok((answer, output, spec)) => {
                        let total_ragdoll_ms =
                            result.latency.total_ragdoll_ms + output.generation_total_ms;
                        if let Err(err) = persist_generation_metrics(
                            &state.pool,
                            &result.query_id,
                            &output,
                            &spec.llm_model_id,
                            total_ragdoll_ms,
                        )
                        .await
                        {
                            return BatchItemResult::err(
                                index,
                                StatusCode::INTERNAL_SERVER_ERROR,
                                err.to_string(),
                            );
                        }
                        result = attach_generation(result, answer, &output);
                    }
                    Err(err) => {
                        let _ = state
                            .search
                            .record_failure(ctx, settings, &item, options, 500)
                            .await;
                        return BatchItemResult::err(
                            index,
                            StatusCode::BAD_REQUEST,
                            err.to_string(),
                        );
                    }
                }
            }
            BatchItemResult::ok(index, result)
        }
        Err(err) => {
            let _ = state
                .search
                .record_failure(ctx, settings, &item, options, 500)
                .await;
            BatchItemResult::err(index, StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        }
    }
}

async fn handle_streaming_query(
    state: Arc<AppState>,
    ctx: ReleaseCtx,
    params: QueryParams,
    item: QueryRequest,
    settings: &crate::settings::RuntimeSettings,
) -> Result<Response, ApiError> {
    if item.text.trim().is_empty() {
        return Err(ApiError::bad_request("query text required"));
    }
    let generation = item
        .generation
        .clone()
        .ok_or_else(|| ApiError::bad_request("generation object required for streaming"))?;
    if !generation.stream {
        return Err(ApiError::bad_request("generation.stream must be true"));
    }

    let options = QueryOptions {
        ts_start: params.ts_start,
        store_payload: params.store_payload.unwrap_or(false),
        playground: params.playground.unwrap_or(false),
    };

    let result = state
        .search
        .execute(&ctx, settings, &item, &options)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let spec = resolve_generation_spec(
        &state.pool,
        &state.crypto,
        &ctx,
        settings,
        &generation,
        &item.text,
        result.matches.clone(),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (tx, rx) =
        tokio::sync::mpsc::unbounded_channel::<Result<Event, std::convert::Infallible>>();
    let query_id = result.query_id.clone();
    let matches = result.matches.clone();
    let pool = state.pool.clone();
    let generator = state.generator.clone();
    let search_latency = result.latency.clone();
    let base_total_ragdoll_ms = result.latency.total_ragdoll_ms;

    tokio::spawn(async move {
        let send = |event: Event| tx.send(Ok(event));
        if send(
            Event::default()
                .event("sources")
                .data(serde_json::to_string(&matches).unwrap_or_else(|_| "[]".into())),
        )
        .is_err()
        {
            return;
        }

        for event in search_latency_events(&search_latency) {
            if send(event).is_err() {
                return;
            }
        }

        match generator.generate_stream(&spec).await {
            Ok(mut gen_stream) => {
                let mut answer = String::new();
                let mut done_metrics = None;
                while let Some(event) = gen_stream.next().await {
                    match event {
                        Ok(StreamEvent::Token { delta }) => {
                            answer.push_str(&delta);
                            if send(
                                Event::default()
                                    .event("token")
                                    .data(serde_json::json!({"delta": delta}).to_string()),
                            )
                            .is_err()
                            {
                                return;
                            }
                        }
                        Ok(StreamEvent::Done {
                            generation_ms,
                            generation_total_ms,
                            prompt_tokens,
                            completion_tokens,
                        }) => {
                            done_metrics = Some((
                                generation_ms,
                                generation_total_ms,
                                prompt_tokens,
                                completion_tokens,
                            ));
                        }
                        Err(err) => {
                            let _ = send(Event::default().event("error").data(err.to_string()));
                            return;
                        }
                    }
                }

                if let Some((
                    generation_ms,
                    generation_total_ms,
                    prompt_tokens,
                    completion_tokens,
                )) = done_metrics
                {
                    let output = GenerationOutput {
                        text: answer,
                        generation_ms,
                        generation_total_ms,
                        prompt_tokens,
                        completion_tokens,
                    };
                    let total_ragdoll_ms = base_total_ragdoll_ms + generation_total_ms;
                    let _ = persist_generation_metrics(
                        &pool,
                        &query_id,
                        &output,
                        &spec.llm_model_id,
                        total_ragdoll_ms,
                    )
                    .await;
                    let _ = send(
                        Event::default().event("done").data(
                            serde_json::json!({
                                "query_id": query_id,
                                "text": output.text,
                                "final": true,
                                "latency": {
                                    "upstream_ms": search_latency.upstream_ms,
                                    "embed_ms": search_latency.embed_ms,
                                    "search_ms": search_latency.search_ms,
                                    "rerank_ms": search_latency.rerank_ms,
                                    "store_ms": search_latency.store_ms,
                                    "generation_ms": generation_ms,
                                    "generation_total_ms": generation_total_ms,
                                    "total_ragdoll_ms": total_ragdoll_ms,
                                },
                                "usage": {
                                    "prompt_tokens": prompt_tokens,
                                    "completion_tokens": completion_tokens,
                                },
                                "llm_model_id": spec.llm_model_id,
                                "llm_model_tag": spec.llm_model_tag,
                            })
                            .to_string(),
                        ),
                    );
                }
            }
            Err(err) => {
                let _ = send(Event::default().event("error").data(err.to_string()));
            }
        }
    });

    let sse_stream = stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });

    Ok(Sse::new(sse_stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

fn search_latency_events(latency: &QueryLatency) -> Vec<Event> {
    let mut events = Vec::new();
    let mut segments: Vec<(&str, i64)> = vec![
        ("upstream_ms", latency.upstream_ms.unwrap_or(0)),
        ("embed_ms", latency.embed_ms),
        ("search_ms", latency.search_ms),
    ];
    if let Some(rerank_ms) = latency.rerank_ms {
        segments.push(("rerank_ms", rerank_ms));
    }
    segments.push(("store_ms", latency.store_ms));
    for (segment, ms) in segments {
        events.push(
            Event::default().event("latency").data(
                serde_json::json!({ "segment": segment, "ms": ms, "final": false }).to_string(),
            ),
        );
    }
    events.push(
        Event::default().event("latency").data(
            serde_json::json!({
                "final": false,
                "upstream_ms": latency.upstream_ms,
                "embed_ms": latency.embed_ms,
                "search_ms": latency.search_ms,
                "rerank_ms": latency.rerank_ms,
                "store_ms": latency.store_ms,
                "total_ragdoll_ms": latency.total_ragdoll_ms,
            })
            .to_string(),
        ),
    );
    events
}

pub async fn post_playground_queries(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<QueryParams>,
    Json(items): Json<Vec<QueryRequest>>,
) -> Result<Response, ApiError> {
    authorize(&auth, Permission::PlaygroundRun)?;
    let forced = QueryParams {
        ts_start: params.ts_start,
        store_payload: Some(true),
        playground: Some(true),
    };
    execute_queries(state, ctx, forced, items).await
}

pub async fn get_queries(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<Vec<StoredQuery>>, ApiError> {
    authorize(&auth, Permission::QueriesRead)?;
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
                store_ms, total_ragdoll_ms, generation_ms, generation_total_ms, prompt_tokens,
                completion_tokens, llm_model_id, candidate_count, result_count, response_status, created_at
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
            total_ragdoll_ms: row.get(10).ok(),
            generation_ms: row.get(11).ok(),
            generation_total_ms: row.get(12).ok(),
            prompt_tokens: row.get(13).ok(),
            completion_tokens: row.get(14).ok(),
            llm_model_id: row.get(15).ok(),
            candidate_count: row.get(16).ok(),
            result_count: row.get(17).ok(),
            response_status: row.get(18).map_err(|e| ApiError::internal(e.to_string()))?,
            created_at: row.get(19).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }

    Ok(Json(items))
}

pub async fn delete_queries(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::QueriesDelete)?;
    let filter_raw = params
        .filter
        .ok_or_else(|| ApiError::bad_request("filter query param required"))?;
    let filter =
        decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let compiled = crate::filter::compile_filter(&filter, "queries")
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let sql = format!(
        "DELETE FROM queries WHERE queries.release_id = '{}' AND {}",
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
    Extension(auth): Extension<AuthContext>,
    axum::extract::Path(NestedPathId { id: query_id, .. }): axum::extract::Path<NestedPathId>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::QueriesRead)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, text, filters, params, playground, upstream_ms, embed_ms, search_ms, rerank_ms,
                    store_ms, total_ragdoll_ms, generation_ms, generation_total_ms, prompt_tokens,
                    completion_tokens, llm_model_id, candidate_count, result_count, response_status, created_at
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
        "total_ragdoll_ms": row.get::<Option<i64>>(10).ok().flatten(),
        "generation_ms": row.get::<Option<i64>>(11).ok().flatten(),
        "generation_total_ms": row.get::<Option<i64>>(12).ok().flatten(),
        "prompt_tokens": row.get::<Option<i64>>(13).ok().flatten(),
        "completion_tokens": row.get::<Option<i64>>(14).ok().flatten(),
        "llm_model_id": row.get::<Option<String>>(15).ok().flatten(),
        "candidate_count": row.get::<Option<i64>>(16).ok().flatten(),
        "result_count": row.get::<Option<i64>>(17).ok().flatten(),
        "response_status": row.get::<i64>(18).map_err(|e| ApiError::internal(e.to_string()))?,
        "created_at": row.get::<String>(19).map_err(|e| ApiError::internal(e.to_string()))?,
        "chunks": chunks,
    })))
}
