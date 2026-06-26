// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::filter::{FilterExpr, compile_filter};
use crate::models::ModelProvider;
use crate::release::ReleaseCtx;
use crate::search::score::{cosine_similarity_from_distance, normalize_rerank_scores};
use crate::settings::{
    DEFAULT_RERANK_CANDIDATES, DEFAULT_TOP_K, RuntimeSettings, effective_store_payload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub text: String,
    #[serde(default)]
    pub filter: Option<FilterExpr>,
    pub top_k: Option<u32>,
    pub rerank: Option<bool>,
    pub rerank_candidates: Option<u32>,
    #[serde(default)]
    pub min_semantic_score: Option<f64>,
    #[serde(default)]
    pub min_rerank_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMatch {
    pub chunk_id: String,
    pub source_id: String,
    pub source_name: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub provenance: serde_json::Value,
    pub semantic_score: f64,
    pub rerank_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub query_id: String,
    pub matches: Vec<QueryMatch>,
    pub latency: QueryLatency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryLatency {
    pub upstream_ms: Option<i64>,
    pub embed_ms: i64,
    pub search_ms: i64,
    pub rerank_ms: Option<i64>,
    pub store_ms: i64,
    pub total_ms: i64,
    pub candidate_count: usize,
    pub result_count: usize,
}

pub struct QueryOptions {
    pub ts_start: Option<i64>,
    pub store_payload: bool,
    pub playground: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            ts_start: None,
            store_payload: false,
            playground: false,
        }
    }
}

pub struct SearchPipeline {
    pub pool: DbPool,
    pub models: Arc<dyn ModelProvider>,
}

impl SearchPipeline {
    pub async fn execute(
        &self,
        ctx: &ReleaseCtx,
        settings: &RuntimeSettings,
        request: &QueryRequest,
        options: &QueryOptions,
    ) -> Result<QueryResult> {
        let started = Instant::now();
        let top_k = request.top_k.unwrap_or(DEFAULT_TOP_K);
        let rerank_enabled = request.rerank.unwrap_or(true);
        let rerank_candidates = request
            .rerank_candidates
            .unwrap_or(DEFAULT_RERANK_CANDIDATES)
            .max(top_k);
        let min_semantic = request.min_semantic_score.unwrap_or(0.0);
        let min_rerank = request.min_rerank_score.unwrap_or(0.0);
        let store_payload = effective_store_payload(
            settings,
            options.store_payload,
            options.playground,
        );

        let upstream_ms = options.ts_start.map(|ts| {
            let now = time::OffsetDateTime::now_utc().unix_timestamp() * 1000;
            (now - ts).max(0)
        });

        let embedder = self
            .models
            .embedder(&settings.embedding_model)
            .await
            .context("load embedder")?;
        let embed_start = Instant::now();
        let query_vector = embedder.embed_one(&request.text).await?;
        let embed_ms = embed_start.elapsed().as_millis() as i64;
        let vector_json = serde_json::to_string(&query_vector)?;

        // Embed release_id in SQL so filter placeholders stay at ?1, ?2, ...
        // (compile_filter always numbers from 1; mixing bound params breaks json_extract paths).
        let mut where_parts = vec![format!("c.release_id = '{}'", ctx.release_id)];
        let mut params: Vec<String> = Vec::new();
        if let Some(filter) = &request.filter {
            let compiled = compile_filter(filter, "c")?;
            where_parts.push(compiled.sql);
            params.extend(compiled.params);
        }
        let where_clause = where_parts.join(" AND ");

        params.push(vector_json.clone());
        let vector_param_index = params.len();
        params.push(rerank_candidates.to_string());

        let sql = format!(
            "SELECT c.id, c.source_id, s.name, c.content, c.metadata, c.provenance,
                    vector_distance_cos(c.embedding, vector32(?{vector_param_index})) AS distance
             FROM chunks c
             JOIN sources s ON s.id = c.source_id
             WHERE {where_clause}
             ORDER BY distance ASC
             LIMIT ?{}",
            params.len()
        );

        let search_start = Instant::now();
        let conn = self.pool.connect_one().await?;
        let mut rows = conn.query(&sql, params).await?;
        let mut all_semantic = Vec::new();
        while let Some(row) = rows.next().await? {
            let distance: f64 = row.get(6)?;
            let semantic_score = cosine_similarity_from_distance(distance);
            all_semantic.push(QueryMatch {
                chunk_id: row.get(0)?,
                source_id: row.get(1)?,
                source_name: row.get(2)?,
                content: row.get(3)?,
                metadata: serde_json::from_str(&row.get::<String>(4)?)
                    .unwrap_or(serde_json::json!({})),
                provenance: serde_json::from_str(&row.get::<String>(5)?)
                    .unwrap_or(serde_json::json!([])),
                semantic_score,
                rerank_score: None,
            });
        }
        let search_ms = search_start.elapsed().as_millis() as i64;
        let candidate_count = all_semantic.len();

        let mut rerank_pool: Vec<QueryMatch> = all_semantic
            .iter()
            .filter(|c| c.semantic_score >= min_semantic)
            .cloned()
            .collect();

        let rerank_ms = if rerank_enabled && !rerank_pool.is_empty() {
            let reranker = self
                .models
                .reranker(&settings.rerank_model)
                .await
                .context("load reranker")?;
            let rerank_start = Instant::now();
            let docs: Vec<String> = rerank_pool.iter().map(|c| c.content.clone()).collect();
            let scores = reranker.rerank(&request.text, &docs).await?;
            let normalized = normalize_rerank_scores(&scores);
            for (candidate, score) in rerank_pool.iter_mut().zip(normalized) {
                candidate.rerank_score = Some(score as f64);
            }
            rerank_pool.sort_by(|a, b| {
                b.rerank_score
                    .partial_cmp(&a.rerank_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            Some(rerank_start.elapsed().as_millis() as i64)
        } else {
            rerank_pool.sort_by(|a, b| {
                b.semantic_score
                    .partial_cmp(&a.semantic_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            None
        };

        let all_reranked = rerank_pool;

        let playground_semantic = if options.playground {
            Some(all_semantic.clone())
        } else {
            None
        };
        let playground_reranked = if options.playground && rerank_enabled {
            Some(all_reranked.clone())
        } else {
            None
        };

        let response_matches: Vec<QueryMatch> = if options.playground {
            let rerank_by_id: std::collections::HashMap<&str, Option<f64>> = all_reranked
                .iter()
                .map(|c| (c.chunk_id.as_str(), c.rerank_score))
                .collect();
            all_semantic
                .iter()
                .map(|c| {
                    let mut out = c.clone();
                    out.rerank_score = rerank_by_id.get(c.chunk_id.as_str()).copied().flatten();
                    out
                })
                .collect()
        } else {
            let mut filtered: Vec<QueryMatch> = if rerank_enabled {
                all_reranked
                    .into_iter()
                    .filter(|c| c.rerank_score.unwrap_or(0.0) >= min_rerank)
                    .collect()
            } else {
                all_semantic
                    .into_iter()
                    .filter(|c| c.semantic_score >= min_semantic)
                    .collect()
            };
            filtered.truncate(top_k as usize);
            filtered
        };
        let result_count = response_matches.len();

        let query_id = Uuid::new_v4().to_string();
        let params_json = serde_json::json!({
            "top_k": top_k,
            "rerank": rerank_enabled,
            "rerank_candidates": rerank_candidates,
            "min_semantic_score": min_semantic,
            "min_rerank_score": min_rerank,
        });
        let filter_json = request
            .filter
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());

        let store_start = Instant::now();
        let text_to_store = if store_payload {
            Some(request.text.as_str())
        } else {
            None
        };

        conn.execute(
            "INSERT INTO queries (
                id, release_id, stage_id, text, filters, params, playground,
                upstream_ms, embed_ms, search_ms, rerank_ms, store_ms, total_ms,
                candidate_count, result_count, response_status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            (
                query_id.as_str(),
                ctx.release_id.as_str(),
                ctx.stage_id.as_deref(),
                text_to_store,
                filter_json.as_str(),
                params_json.to_string().as_str(),
                if options.playground { 1i64 } else { 0i64 },
                upstream_ms,
                embed_ms,
                search_ms,
                rerank_ms,
                0i64,
                started.elapsed().as_millis() as i64,
                candidate_count as i64,
                result_count as i64,
                200i64,
            ),
        )
        .await?;

        let (semantic_store, rerank_store): (Vec<&QueryMatch>, Option<Vec<&QueryMatch>>) =
            if options.playground {
                (
                    playground_semantic.as_ref().unwrap().iter().collect(),
                    playground_reranked
                        .as_ref()
                        .map(|rows| rows.iter().collect()),
                )
            } else {
                (
                    response_matches.iter().collect(),
                    if rerank_enabled {
                        Some(response_matches.iter().collect())
                    } else {
                        None
                    },
                )
            };

        for (rank, candidate) in semantic_store.iter().enumerate() {
            let metadata = serde_json::to_string(&candidate.metadata)?;
            let content = if store_payload {
                Some(candidate.content.as_str())
            } else {
                None
            };
            conn.execute(
                "INSERT INTO query_chunks (
                    query_id, release_id, stage_id, step, rank, chunk_id, source_id, score, metadata, content
                 ) VALUES (?1, ?2, ?3, 'semantic', ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    query_id.as_str(),
                    ctx.release_id.as_str(),
                    ctx.stage_id.as_deref(),
                    rank as i64,
                    candidate.chunk_id.as_str(),
                    candidate.source_id.as_str(),
                    candidate.semantic_score,
                    metadata.as_str(),
                    content,
                ),
            )
            .await?;
        }

        if let Some(rerank_rows) = rerank_store {
            for (rank, candidate) in rerank_rows.iter().enumerate() {
                let score = candidate.rerank_score.unwrap_or(candidate.semantic_score);
                let metadata = serde_json::to_string(&candidate.metadata)?;
                let content = if store_payload {
                    Some(candidate.content.as_str())
                } else {
                    None
                };
                conn.execute(
                    "INSERT INTO query_chunks (
                        query_id, release_id, stage_id, step, rank, chunk_id, source_id, score, metadata, content
                     ) VALUES (?1, ?2, ?3, 'rerank', ?4, ?5, ?6, ?7, ?8, ?9)",
                    (
                        query_id.as_str(),
                        ctx.release_id.as_str(),
                        ctx.stage_id.as_deref(),
                        rank as i64,
                        candidate.chunk_id.as_str(),
                        candidate.source_id.as_str(),
                        score,
                        metadata.as_str(),
                        content,
                    ),
                )
                .await?;
            }
        }

        let store_ms = store_start.elapsed().as_millis() as i64;
        let total_ms = started.elapsed().as_millis() as i64;

        conn.execute(
            "UPDATE queries SET store_ms = ?1, total_ms = ?2 WHERE id = ?3",
            (store_ms, total_ms, query_id.as_str()),
        )
        .await?;

        Ok(QueryResult {
            query_id,
            matches: response_matches,
            latency: QueryLatency {
                upstream_ms,
                embed_ms,
                search_ms,
                rerank_ms,
                store_ms,
                total_ms,
                candidate_count,
                result_count,
            },
        })
    }

    pub async fn record_failure(
        &self,
        ctx: &ReleaseCtx,
        settings: &RuntimeSettings,
        request: &QueryRequest,
        options: &QueryOptions,
        response_status: u16,
    ) -> Result<String> {
        let query_id = Uuid::new_v4().to_string();
        let top_k = request.top_k.unwrap_or(DEFAULT_TOP_K);
        let rerank_enabled = request.rerank.unwrap_or(true);
        let rerank_candidates = request
            .rerank_candidates
            .unwrap_or(DEFAULT_RERANK_CANDIDATES)
            .max(top_k);
        let store_payload = effective_store_payload(
            settings,
            options.store_payload,
            options.playground,
        );

        let filter_json = request
            .filter
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());
        let min_semantic = request.min_semantic_score.unwrap_or(0.0);
        let min_rerank = request.min_rerank_score.unwrap_or(0.0);
        let params_json = serde_json::json!({
            "top_k": top_k,
            "rerank": rerank_enabled,
            "rerank_candidates": rerank_candidates,
            "min_semantic_score": min_semantic,
            "min_rerank_score": min_rerank,
        });
        let text_to_store = if store_payload {
            Some(request.text.as_str())
        } else {
            None
        };

        let conn = self.pool.connect_one().await?;
        conn.execute(
            "INSERT INTO queries (
                id, release_id, stage_id, text, filters, params, playground, response_status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                query_id.as_str(),
                ctx.release_id.as_str(),
                ctx.stage_id.as_deref(),
                text_to_store,
                filter_json.as_str(),
                params_json.to_string().as_str(),
                if options.playground { 1i64 } else { 0i64 },
                i64::from(response_status),
            ),
        )
        .await?;

        Ok(query_id)
    }
}
