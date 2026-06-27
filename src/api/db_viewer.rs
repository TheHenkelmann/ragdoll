// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use libsql::Row;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::release::{NestedPathTable, ReleaseCtx};

const FACET_MAX: usize = 50;

#[derive(Debug, Deserialize)]
pub struct DbViewParams {
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ColumnFilter {
    column: String,
    op: String,
    value: Value,
}

#[derive(Debug, Serialize)]
pub struct ColumnFacet {
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
pub struct DbTableResponse {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub facets: HashMap<String, ColumnFacet>,
}

pub async fn get_table(
    State(state): State<Arc<AppState>>,
    ctx: ReleaseCtx,
    Extension(auth): Extension<AuthContext>,
    Path(NestedPathTable { table, .. }): Path<NestedPathTable>,
    Query(params): Query<DbViewParams>,
) -> Result<Json<DbTableResponse>, ApiError> {
    authorize(&auth, Permission::DbRead)?;
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
    } else if table == "webhook_deliveries" {
        where_parts.push(
            "EXISTS (SELECT 1 FROM webhooks wh WHERE wh.id = wd.webhook_id AND wh.release_id = ?1)"
                .to_string(),
        );
        bind.push(ctx.release_id.clone());
    }
    if table == "queries" {
        where_parts.push("COALESCE(q.playground, 0) = 0".to_string());
    }
    if let Some(filter_raw) = params.filter {
        let filters: Vec<ColumnFilter> =
            serde_json::from_str(&filter_raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
        for f in filters {
            let compiled = compile_column_filter(&columns, alias, &f)?;
            where_parts.push(compiled.0);
            bind.extend(compiled.1);
        }
    }
    let where_clause = if where_parts.is_empty() {
        "1=1".to_string()
    } else {
        where_parts.join(" AND ")
    };

    let sort_col = params.sort.as_deref().unwrap_or("");
    let sort_dir = match params.dir.as_deref().unwrap_or("asc") {
        "desc" => "DESC",
        _ => "ASC",
    };
    let order_clause = if sort_col.is_empty() || !columns.iter().any(|c| c == sort_col) {
        format!("{alias}.rowid")
    } else {
        format!("{alias}.{sort_col} {sort_dir}")
    };

    bind.push(limit.to_string());
    bind.push(offset.to_string());
    let select_cols = columns
        .iter()
        .map(|c| format!("{alias}.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {select_cols} FROM {table} {alias} WHERE {where_clause} ORDER BY {order_clause} LIMIT ?{} OFFSET ?{}",
        bind.len() - 1,
        bind.len()
    );
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(&sql, bind.clone())
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

    let facets =
        compute_facets(&state, &table, alias, &columns, &where_clause, &bind, &ctx).await?;

    Ok(Json(DbTableResponse {
        columns,
        rows: items,
        facets,
    }))
}

async fn compute_facets(
    state: &AppState,
    table: &str,
    alias: &str,
    columns: &[String],
    where_clause: &str,
    bind: &[String],
    ctx: &ReleaseCtx,
) -> Result<HashMap<String, ColumnFacet>, ApiError> {
    let mut facets = HashMap::new();
    let facet_bind: Vec<String> = bind[..bind.len().saturating_sub(2)].to_vec();
    for col in columns {
        if skip_facet_column(col) {
            continue;
        }
        let sql = format!(
            "SELECT DISTINCT {alias}.{col} FROM {table} {alias} WHERE {where_clause} ORDER BY {alias}.{col} LIMIT {}",
            FACET_MAX + 1
        );
        let conn = state
            .pool
            .connect_one()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let mut rows = conn
            .query(&sql, facet_bind.clone())
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let mut values = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
        {
            values.push(row_cell(&row, 0)?);
        }
        let truncated = values.len() > FACET_MAX;
        if truncated {
            values.truncate(FACET_MAX);
        }
        facets.insert(
            col.clone(),
            ColumnFacet {
                truncated,
                values: if truncated { None } else { Some(values) },
            },
        );
    }
    let _ = ctx;
    Ok(facets)
}

fn compile_column_filter(
    columns: &[String],
    alias: &str,
    filter: &ColumnFilter,
) -> Result<(String, Vec<String>), ApiError> {
    if !columns.iter().any(|c| c == &filter.column) {
        return Err(ApiError::bad_request(format!(
            "unknown column: {}",
            filter.column
        )));
    }
    let col_ref = format!("{alias}.{}", filter.column);
    let op = filter.op.as_str();
    match op {
        "eq" => {
            let v = scalar_param(&filter.value)?;
            Ok((format!("{col_ref} = ?"), vec![v]))
        }
        "ne" => {
            let v = scalar_param(&filter.value)?;
            Ok((format!("{col_ref} != ?"), vec![v]))
        }
        "contains" => {
            let v = scalar_param(&filter.value)?;
            Ok((format!("{col_ref} LIKE ?"), vec![format!("%{v}%")]))
        }
        "gt" | "gte" | "lt" | "lte" => {
            let sql_op = match op {
                "gt" => ">",
                "gte" => ">=",
                "lt" => "<",
                _ => "<=",
            };
            let v = scalar_param(&filter.value)?;
            Ok((format!("{col_ref} {sql_op} ?"), vec![v]))
        }
        "in" => {
            let arr = filter
                .value
                .as_array()
                .ok_or_else(|| ApiError::bad_request("in requires array value"))?;
            if arr.is_empty() {
                return Err(ApiError::bad_request("in requires non-empty array"));
            }
            let placeholders: Vec<String> =
                arr.iter().enumerate().map(|_| "?".to_string()).collect();
            let params: Vec<String> = arr.iter().map(scalar_param).collect::<Result<_, _>>()?;
            Ok((
                format!("{col_ref} IN ({})", placeholders.join(", ")),
                params,
            ))
        }
        _ => Err(ApiError::bad_request(format!("unsupported op: {op}"))),
    }
}

fn scalar_param(value: &Value) -> Result<String, ApiError> {
    match value {
        Value::Null => Ok(String::new()),
        Value::Bool(b) => Ok(if *b { "1".into() } else { "0".into() }),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(s.clone()),
        _ => Err(ApiError::bad_request("filter value must be scalar")),
    }
}

fn skip_facet_column(col: &str) -> bool {
    matches!(
        col,
        "content"
            | "metadata"
            | "provenance"
            | "config"
            | "filters"
            | "params"
            | "text"
            | "error"
            | "payload"
            | "password_hash"
            | "response"
            | "uri"
            | "message"
            | "value"
    ) || col.ends_with("_id")
        || col == "id"
        || col.contains("embedding")
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
        "sources" | "chunks" | "queries" | "query_chunks" | "ingest_jobs" | "settings" | "webhooks"
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
        "webhooks" => "w",
        "webhook_deliveries" => "wd",
        _ => "t",
    }
}

fn table_columns(table: &str) -> Option<Vec<String>> {
    let raw: Vec<&str> = match table {
        "sources" => vec![
            "name",
            "type",
            "uri",
            "status",
            "error",
            "content_hash",
            "config",
            "metadata",
            "release_id",
            "id",
            "created_at",
            "updated_at",
        ],
        "chunks" => vec![
            "ordinal",
            "content",
            "token_count",
            "embedding_model",
            "embedding_dim",
            "embedding_version",
            "provenance",
            "metadata",
            "source_id",
            "release_id",
            "id",
            "created_at",
        ],
        "queries" => vec![
            "text",
            "playground",
            "upstream_ms",
            "embed_ms",
            "search_ms",
            "rerank_ms",
            "store_ms",
            "total_ragdoll_ms",
            "candidate_count",
            "result_count",
            "response_status",
            "filters",
            "params",
            "stage_id",
            "release_id",
            "id",
            "created_at",
        ],
        "query_chunks" => vec![
            "step",
            "rank",
            "score",
            "content",
            "metadata",
            "chunk_id",
            "source_id",
            "stage_id",
            "release_id",
            "query_id",
        ],
        "ingest_jobs" => vec![
            "status",
            "attempts",
            "max_attempts",
            "worker_id",
            "heartbeat_at",
            "error",
            "queue_ms",
            "extract_ms",
            "chunk_ms",
            "embed_ms",
            "db_write_ms",
            "total_ms",
            "chunk_count",
            "char_count",
            "started_at",
            "finished_at",
            "source_name",
            "source_type",
            "source_uri",
            "content_hash",
            "config",
            "metadata",
            "source_id",
            "stage_id",
            "release_id",
            "id",
            "created_at",
        ],
        "settings" => vec!["key", "value", "release_id"],
        "webhooks" => vec![
            "type",
            "url",
            "events",
            "active",
            "release_id",
            "id",
            "created_at",
        ],
        "webhook_deliveries" => vec![
            "event",
            "status_code",
            "error",
            "response",
            "payload",
            "webhook_id",
            "id",
            "created_at",
        ],
        "models" => vec![
            "name",
            "kind",
            "runtime",
            "dim",
            "uri",
            "status",
            "is_default",
        ],
        "releases" => vec!["tag", "message", "id", "created_at"],
        "stages" => vec!["tag", "release_id", "id", "created_at"],
        "users" => vec![
            "email",
            "is_superadmin",
            "password_is_default",
            "id",
            "created_at",
            "password_hash",
        ],
        _ => return None,
    };
    Some(raw.into_iter().map(String::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_columns_rejects_unknown_table() {
        assert!(table_columns("injected;drop").is_none());
    }

    #[test]
    fn table_columns_puts_ids_at_end_for_sources() {
        let cols = table_columns("sources").unwrap();
        assert_eq!(cols.last(), Some(&"updated_at".to_string()));
        assert!(
            cols.iter().position(|c| c == "name").unwrap()
                < cols.iter().position(|c| c == "id").unwrap()
        );
    }

    #[test]
    fn table_columns_includes_webhook_tables_without_secret() {
        let webhooks = table_columns("webhooks").unwrap();
        assert!(webhooks.contains(&"events".to_string()));
        assert!(!webhooks.contains(&"secret".to_string()));

        let deliveries = table_columns("webhook_deliveries").unwrap();
        assert!(deliveries.contains(&"event".to_string()));
        assert!(deliveries.contains(&"payload".to_string()));
    }

    #[test]
    fn is_release_scoped_marks_expected_tables() {
        assert!(is_release_scoped("sources"));
        assert!(is_release_scoped("webhooks"));
        assert!(!is_release_scoped("webhook_deliveries"));
        assert!(!is_release_scoped("releases"));
    }

    #[test]
    fn skip_facet_column_skips_high_cardinality() {
        assert!(skip_facet_column("content"));
        assert!(skip_facet_column("source_id"));
        assert!(!skip_facet_column("status"));
    }
}
