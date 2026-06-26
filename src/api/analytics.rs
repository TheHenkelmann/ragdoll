// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::release::{lookup_release_by_tag, lookup_stage_by_tag};

#[derive(Debug, Deserialize)]
pub struct AnalyticsParams {
    pub lens: String,
    pub tag: String,
    #[serde(default = "default_days")]
    pub days: u32,
    pub start: Option<String>,
    pub end: Option<String>,
    /// Comma-separated status groups: 2xx, 4xx, 5xx
    pub status: Option<String>,
}

fn default_days() -> u32 {
    14
}

#[derive(Debug, Serialize)]
pub struct LatencyStats {
    pub p50: f64,
    pub p95: f64,
}

#[derive(Debug, Serialize)]
pub struct SourceChunkCount {
    pub source_id: String,
    pub name: String,
    pub chunk_count: i64,
}

#[derive(Debug, Serialize)]
pub struct DailyRequestCount {
    pub day: String,
    pub s2xx: i64,
    pub s4xx: i64,
    pub s5xx: i64,
}

#[derive(Debug, Serialize)]
pub struct QueryChunkHit {
    pub chunk_id: String,
    pub source_id: String,
    pub source_name: String,
    pub hit_count: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub request_count: i64,
    pub daily_requests: Vec<DailyRequestCount>,
    pub total_latency: LatencyStats,
    pub embed_latency: LatencyStats,
    pub search_latency: LatencyStats,
    pub rerank_latency: LatencyStats,
    pub store_latency: LatencyStats,
    pub source_count: i64,
    pub chunk_count: i64,
    pub chunks_per_source: Vec<SourceChunkCount>,
    pub metadata_keys: Vec<(String, i64)>,
    pub query_chunk_hits: Vec<QueryChunkHit>,
    pub query_chunk_metadata_keys: Vec<(String, i64)>,
}

struct QueryFilter {
    sql: String,
    bind: Vec<String>,
}

fn parse_status_groups(raw: Option<&str>) -> Vec<&'static str> {
    let Some(raw) = raw else {
        return vec!["2xx", "4xx", "5xx"];
    };
    let mut groups = Vec::new();
    for part in raw.split(',') {
        match part.trim() {
            "2xx" if !groups.contains(&"2xx") => groups.push("2xx"),
            "4xx" if !groups.contains(&"4xx") => groups.push("4xx"),
            "5xx" if !groups.contains(&"5xx") => groups.push("5xx"),
            _ => {}
        }
    }
    if groups.is_empty() {
        vec!["2xx", "4xx", "5xx"]
    } else {
        groups
    }
}

fn status_group_sql(groups: &[&str], col: &str) -> String {
    let mut parts = Vec::new();
    for group in groups {
        match *group {
            "2xx" => parts.push(format!("({col} >= 200 AND {col} < 300)")),
            "4xx" => parts.push(format!("({col} >= 400 AND {col} < 500)")),
            "5xx" => parts.push(format!("({col} >= 500)")),
            _ => {}
        }
    }
    if parts.is_empty() {
        "1=1".to_string()
    } else {
        format!("({})", parts.join(" OR "))
    }
}

async fn build_query_filter(
    state: &AppState,
    params: &AnalyticsParams,
) -> Result<QueryFilter, ApiError> {
    let days = params.days.max(1).min(365);
    let status_groups = parse_status_groups(params.status.as_deref());
    let status_sql = status_group_sql(&status_groups, "q.response_status");

    let (scope_col, mut bind) = match params.lens.as_str() {
        "stage" => {
            let ctx = lookup_stage_by_tag(state, &params.tag).await?;
            ("q.stage_id", vec![ctx.stage_id.unwrap_or_default()])
        }
        "release" => {
            let ctx = lookup_release_by_tag(state, &params.tag).await?;
            ("q.release_id", vec![ctx.release_id])
        }
        _ => return Err(ApiError::bad_request("lens must be stage or release")),
    };

    let date_sql = if params.start.is_some() || params.end.is_some() {
        let start = params
            .start
            .clone()
            .unwrap_or_else(|| "1970-01-01".to_string());
        let end = params
            .end
            .clone()
            .unwrap_or_else(|| "9999-12-31".to_string());
        bind.push(start);
        bind.push(end);
        format!(
            "q.created_at >= ?{} AND q.created_at < date(?{}, '+1 day')",
            bind.len() - 1,
            bind.len()
        )
    } else {
        bind.push(format!("-{days} days"));
        format!("q.created_at >= datetime('now', ?{})", bind.len())
    };

    Ok(QueryFilter {
        sql: format!(
            "{scope_col} = ?1 AND COALESCE(q.playground,0)=0 AND {date_sql} AND {status_sql}"
        ),
        bind,
    })
}

pub async fn get_analytics(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<AnalyticsResponse>, ApiError> {
    let filter = build_query_filter(&state, &params).await?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let count_sql = format!("SELECT COUNT(*) FROM queries q WHERE {}", filter.sql);
    let mut count_rows = conn
        .query(&count_sql, filter.bind.clone())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let request_count: i64 = count_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("count failed"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let daily_sql = format!(
        "SELECT date(q.created_at) d,
                SUM(CASE WHEN q.response_status >= 200 AND q.response_status < 300 THEN 1 ELSE 0 END),
                SUM(CASE WHEN q.response_status >= 400 AND q.response_status < 500 THEN 1 ELSE 0 END),
                SUM(CASE WHEN q.response_status >= 500 THEN 1 ELSE 0 END)
         FROM queries q WHERE {} GROUP BY d ORDER BY d",
        filter.sql
    );
    let mut daily_rows = conn
        .query(&daily_sql, filter.bind.clone())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut daily_requests = Vec::new();
    while let Some(row) = daily_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        daily_requests.push(DailyRequestCount {
            day: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            s2xx: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            s4xx: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            s5xx: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }

    let latency_filter = format!(
        "{} AND q.response_status >= 200 AND q.response_status < 300",
        filter.sql
    );
    let total_latency = latency_stats(&conn, "total_ms", &latency_filter, &filter.bind).await?;
    let embed_latency = latency_stats(&conn, "embed_ms", &latency_filter, &filter.bind).await?;
    let search_latency = latency_stats(&conn, "search_ms", &latency_filter, &filter.bind).await?;
    let rerank_latency = latency_stats(&conn, "rerank_ms", &latency_filter, &filter.bind).await?;
    let store_latency = latency_stats(&conn, "store_ms", &latency_filter, &filter.bind).await?;

    let release_id = if params.lens == "release" {
        lookup_release_by_tag(&state, &params.tag)
            .await?
            .release_id
    } else {
        lookup_stage_by_tag(&state, &params.tag)
            .await?
            .release_id
    };

    let mut source_rows = conn
        .query(
            "SELECT COUNT(*) FROM sources WHERE release_id = ?1",
            [release_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let source_count: i64 = source_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("count failed"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut chunk_rows = conn
        .query(
            "SELECT COUNT(*) FROM chunks WHERE release_id = ?1",
            [release_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let chunk_count: i64 = chunk_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("count failed"))?
        .get(0)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut per_source_rows = conn
        .query(
            "SELECT s.id, s.name, COUNT(c.id) FROM sources s LEFT JOIN chunks c ON c.source_id = s.id WHERE s.release_id = ?1 GROUP BY s.id, s.name ORDER BY COUNT(c.id) DESC",
            [release_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut chunks_per_source = Vec::new();
    while let Some(row) = per_source_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        chunks_per_source.push(SourceChunkCount {
            source_id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            name: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            chunk_count: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }

    let mut meta_rows = conn
        .query(
            "SELECT metadata FROM sources WHERE release_id = ?1 AND metadata IS NOT NULL",
            [release_id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut metadata_keys: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    while let Some(row) = meta_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let raw: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(obj) = value.as_object() {
                for key in obj.keys() {
                    *metadata_keys.entry(key.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    let metadata_keys: Vec<(String, i64)> = metadata_keys.into_iter().collect();

    let chunk_hits_sql = format!(
        "SELECT qc.chunk_id, qc.source_id, COALESCE(s.name, qc.source_id),
                COUNT(DISTINCT qc.query_id)
         FROM query_chunks qc
         INNER JOIN queries q ON q.id = qc.query_id
         LEFT JOIN sources s ON s.id = qc.source_id AND s.release_id = qc.release_id
         WHERE {}
         GROUP BY qc.chunk_id, qc.source_id, s.name
         ORDER BY 4 DESC",
        filter.sql
    );
    let mut chunk_hit_rows = conn
        .query(&chunk_hits_sql, filter.bind.clone())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut query_chunk_hits = Vec::new();
    while let Some(row) = chunk_hit_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        query_chunk_hits.push(QueryChunkHit {
            chunk_id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            source_id: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            source_name: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            hit_count: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }

    let qc_meta_sql = format!(
        "SELECT qc.metadata
         FROM query_chunks qc
         INNER JOIN queries q ON q.id = qc.query_id
         WHERE {}",
        filter.sql
    );
    let mut qc_meta_rows = conn
        .query(&qc_meta_sql, filter.bind.clone())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut query_chunk_metadata_keys: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    while let Some(row) = qc_meta_rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let raw: String = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(obj) = value.as_object() {
                for key in obj.keys() {
                    *query_chunk_metadata_keys.entry(key.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    let query_chunk_metadata_keys: Vec<(String, i64)> =
        query_chunk_metadata_keys.into_iter().collect();

    Ok(Json(AnalyticsResponse {
        request_count,
        daily_requests,
        total_latency,
        embed_latency,
        search_latency,
        rerank_latency,
        store_latency,
        source_count,
        chunk_count,
        chunks_per_source,
        metadata_keys,
        query_chunk_hits,
        query_chunk_metadata_keys,
    }))
}

async fn latency_stats(
    conn: &libsql::Connection,
    column: &str,
    filter_sql: &str,
    bind: &[String],
) -> Result<LatencyStats, ApiError> {
    let sql = format!(
        "SELECT {column} FROM queries q WHERE {filter_sql} AND q.{column} IS NOT NULL ORDER BY q.{column}"
    );
    let mut rows = conn
        .query(&sql, bind.to_vec())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut values = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        values.push(row.get::<i64>(0).map_err(|e| ApiError::internal(e.to_string()))? as f64);
    }
    if values.is_empty() {
        return Ok(LatencyStats { p50: 0.0, p95: 0.0 });
    }
    Ok(LatencyStats {
        p50: percentile(&values, 0.50),
        p95: percentile(&values, 0.95),
    })
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let idx = ((values.len() as f64 - 1.0) * p).round() as usize;
    values[idx.min(values.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_status_groups_defaults_to_all() {
        assert_eq!(
            parse_status_groups(None),
            vec!["2xx", "4xx", "5xx"]
        );
    }

    #[test]
    fn parse_status_groups_filters_unknown_values() {
        assert_eq!(parse_status_groups(Some("2xx,garbage")), vec!["2xx"]);
    }

    #[test]
    fn status_group_sql_builds_or_expression() {
        let sql = status_group_sql(&["2xx", "4xx"], "q.response_status");
        assert!(sql.contains("q.response_status >= 200"));
        assert!(sql.contains("OR"));
    }

    #[test]
    fn percentile_picks_expected_index() {
        let values = vec![10.0, 20.0, 30.0, 40.0];
        assert_eq!(percentile(&values, 0.50), 30.0);
        assert_eq!(percentile(&values, 0.95), 40.0);
        assert_eq!(percentile(&[], 0.50), 0.0);
    }
}
