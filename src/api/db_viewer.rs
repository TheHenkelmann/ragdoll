// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use libsql::Row;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::filter::decode_filter_param;
use crate::release::{NestedPathTable, ReleaseCtx};

#[derive(Debug, Deserialize)]
pub struct DbViewParams {
    pub filter: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

pub async fn get_table(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Path(NestedPathTable { table, .. }): Path<NestedPathTable>,
    Query(params): Query<DbViewParams>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    if table == "api_keys" {
        return Err(ApiError::forbidden("table not available"));
    }
    let columns = table_columns(&table).ok_or_else(|| ApiError::bad_request("unknown table"))?;
    let alias = table_alias(&table);
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);
    let mut bind = Vec::new();
    let mut where_parts = Vec::new();

    if is_release_scoped(&table) {
        where_parts.push(format!("{alias}.release_id = ?1"));
        bind.push(ctx.release_id.clone());
    }
    if table == "queries" {
        where_parts.push("COALESCE(q.playground, 0) = 0".to_string());
    }
    if let Some(filter_raw) = params.filter {
        let filter =
            decode_filter_param(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
        let compiled = crate::filter::compile_filter(&filter, alias)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        where_parts.push(compiled.sql);
        bind.extend(compiled.params);
    }
    let where_clause = if where_parts.is_empty() {
        "1=1".to_string()
    } else {
        where_parts.join(" AND ")
    };
    bind.push(limit.to_string());
    bind.push(offset.to_string());
    let select_cols = columns
        .iter()
        .map(|c| format!("{alias}.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {select_cols} FROM {table} {alias} WHERE {where_clause} LIMIT ?{} OFFSET ?{}",
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
        let mut obj = serde_json::Map::new();
        for (idx, col) in columns.iter().enumerate() {
            obj.insert(col.clone(), row_cell(&row, idx as i32)?);
        }
        items.push(Value::Object(obj));
    }
    Ok(Json(items))
}

fn row_cell(row: &Row, idx: i32) -> Result<Value, ApiError> {
    let value = row
        .get_value(idx)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(match value {
        libsql::Value::Null => Value::Null,
        libsql::Value::Integer(n) => json!(n),
        libsql::Value::Real(f) => json!(f),
        libsql::Value::Text(s) => Value::String(s),
        libsql::Value::Blob(b) => Value::String(format!("<blob {} bytes>", b.len())),
    })
}

fn is_release_scoped(table: &str) -> bool {
    matches!(
        table,
        "sources" | "chunks" | "queries" | "query_chunks" | "ingest_jobs" | "settings"
    )
}

fn table_alias(table: &str) -> &str {
    match table {
        "sources" => "s",
        "chunks" => "c",
        "queries" => "q",
        "query_chunks" => "qc",
        "ingest_jobs" => "j",
        "settings" => "st",
        _ => "t",
    }
}

fn table_columns(table: &str) -> Option<Vec<String>> {
    Some(
        match table {
            "sources" => vec![
                "id",
                "release_id",
                "name",
                "type",
                "uri",
                "content_hash",
                "config",
                "metadata",
                "status",
                "error",
                "created_at",
                "updated_at",
            ],
            "chunks" => vec![
                "id",
                "release_id",
                "source_id",
                "ordinal",
                "content",
                "provenance",
                "metadata",
                "token_count",
                "embedding_model",
                "embedding_dim",
                "embedding_version",
                "created_at",
            ],
            "queries" => vec![
                "id",
                "release_id",
                "stage_id",
                "text",
                "filters",
                "params",
                "playground",
                "upstream_ms",
                "embed_ms",
                "search_ms",
                "rerank_ms",
                "store_ms",
                "total_ms",
                "candidate_count",
                "result_count",
                "response_status",
                "created_at",
            ],
            "query_chunks" => vec![
                "query_id",
                "release_id",
                "stage_id",
                "step",
                "rank",
                "chunk_id",
                "source_id",
                "score",
                "metadata",
                "content",
            ],
            "ingest_jobs" => vec![
                "id",
                "release_id",
                "stage_id",
                "source_id",
                "status",
                "attempts",
                "max_attempts",
                "worker_id",
                "heartbeat_at",
                "error",
                "created_at",
                "started_at",
                "finished_at",
                "queue_ms",
                "extract_ms",
                "chunk_ms",
                "embed_ms",
                "db_write_ms",
                "total_ms",
                "chunk_count",
                "char_count",
            ],
            "settings" => vec!["release_id", "key", "value"],
            "models" => vec![
                "name",
                "kind",
                "runtime",
                "dim",
                "uri",
                "status",
                "is_default",
            ],
            "releases" => vec!["id", "tag", "message", "created_at"],
            "stages" => vec!["id", "tag", "release_id", "created_at"],
            "users" => vec![
                "id",
                "email",
                "password_hash",
                "is_superadmin",
                "password_is_default",
                "created_at",
            ],
            _ => return None,
        }
        .into_iter()
        .map(String::from)
        .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_columns_rejects_unknown_table() {
        assert!(table_columns("injected;drop").is_none());
    }

    #[test]
    fn is_release_scoped_marks_expected_tables() {
        assert!(is_release_scoped("sources"));
        assert!(is_release_scoped("chunks"));
        assert!(!is_release_scoped("releases"));
    }

    #[test]
    fn table_alias_maps_known_tables() {
        assert_eq!(table_alias("sources"), "s");
        assert_eq!(table_alias("queries"), "q");
    }
}
