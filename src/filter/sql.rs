// SPDX-License-Identifier: AGPL-3.0-only

use crate::filter::dsl::{Condition, FilterError, FilterExpr, FilterOp, ResolvedField, resolve_field};

#[derive(Debug, Clone)]
pub struct SqlFilter {
    pub sql: String,
    pub params: Vec<String>,
}

pub fn compile_filter(expr: &FilterExpr, table_alias: &str) -> Result<SqlFilter, FilterError> {
    let mut params = Vec::new();
    let sql = compile_expr(expr, table_alias, &mut params)?;
    Ok(SqlFilter { sql, params })
}

fn compile_expr(expr: &FilterExpr, alias: &str, params: &mut Vec<String>) -> Result<String, FilterError> {
    match expr {
        FilterExpr::And { and: items } => {
            let parts: Result<Vec<_>, _> =
                items.iter().map(|item| compile_expr(item, alias, params)).collect();
            Ok(format!("({})", parts?.join(" AND ")))
        }
        FilterExpr::Or { or: items } => {
            let parts: Result<Vec<_>, _> =
                items.iter().map(|item| compile_expr(item, alias, params)).collect();
            Ok(format!("({})", parts?.join(" OR ")))
        }
        FilterExpr::Not { not: inner } => Ok(format!("NOT ({})", compile_expr(inner, alias, params)?)),
        FilterExpr::Condition(cond) => compile_condition(cond, alias, params),
    }
}

fn compile_condition(cond: &Condition, alias: &str, params: &mut Vec<String>) -> Result<String, FilterError> {
    let field = resolve_field(&cond.field)?;
    let sql_field = match &field {
        ResolvedField::Column(name) => format!("{alias}.{name}"),
        ResolvedField::Metadata(path) => {
            params.push(path.clone());
            format!("json_extract({alias}.metadata, ?{})", params.len())
        }
    };

    match cond.op {
        FilterOp::Exists => {
            if let ResolvedField::Metadata(path) = field {
                params.push(path);
                Ok(format!("json_extract({alias}.metadata, ?{}) IS NOT NULL", params.len()))
            } else {
                Ok(format!("{sql_field} IS NOT NULL"))
            }
        }
        FilterOp::Contains => {
            let value = scalar_to_string(&cond.value)?;
            params.push(format!("%{value}%"));
            Ok(format!("{sql_field} LIKE ?{}", params.len()))
        }
        FilterOp::In | FilterOp::Nin => {
            let values = cond
                .value
                .as_array()
                .ok_or_else(|| FilterError::Validation("in/nin requires array".into()))?;
            if values.is_empty() {
                return Err(FilterError::Validation("in/nin requires non-empty array".into()));
            }
            let placeholders: Vec<String> = values
                .iter()
                .map(|value| {
                    params.push(scalar_to_string(value)?);
                    Ok(format!("?{}", params.len()))
                })
                .collect::<Result<_, FilterError>>()?;
            let op = if matches!(cond.op, FilterOp::In) { "IN" } else { "NOT IN" };
            Ok(format!("{sql_field} {op} ({})", placeholders.join(", ")))
        }
        FilterOp::Eq => {
            params.push(scalar_to_string(&cond.value)?);
            Ok(format!("{sql_field} = ?{}", params.len()))
        }
        FilterOp::Ne => {
            params.push(scalar_to_string(&cond.value)?);
            Ok(format!("{sql_field} != ?{}", params.len()))
        }
        FilterOp::Gt => {
            params.push(scalar_to_string(&cond.value)?);
            Ok(format!("{sql_field} > ?{}", params.len()))
        }
        FilterOp::Gte => {
            params.push(scalar_to_string(&cond.value)?);
            Ok(format!("{sql_field} >= ?{}", params.len()))
        }
        FilterOp::Lt => {
            params.push(scalar_to_string(&cond.value)?);
            Ok(format!("{sql_field} < ?{}", params.len()))
        }
        FilterOp::Lte => {
            params.push(scalar_to_string(&cond.value)?);
            Ok(format!("{sql_field} <= ?{}", params.len()))
        }
    }
}

fn scalar_to_string(value: &serde_json::Value) -> Result<String, FilterError> {
    match value {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        _ => Err(FilterError::Validation(format!(
            "unsupported scalar value: {value}"
        ))),
    }
}

pub fn bind_params<'a>(sql_filter: &'a SqlFilter) -> Vec<libsql::Value> {
    sql_filter
        .params
        .iter()
        .map(|p| libsql::Value::Text(p.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::dsl::{Condition, FilterExpr, FilterOp};

    #[test]
    fn compiles_source_id_in() {
        let expr = FilterExpr::Condition(Condition {
            field: "source_id".to_string(),
            op: FilterOp::In,
            value: serde_json::json!(["s1", "s2"]),
        });
        let compiled = compile_filter(&expr, "c").unwrap();
        assert!(compiled.sql.contains("c.source_id IN"));
        assert_eq!(compiled.params, vec!["s1", "s2"]);
    }

    #[test]
    fn compiles_metadata_eq() {
        let expr = FilterExpr::Condition(Condition {
            field: "meta.department".to_string(),
            op: FilterOp::Eq,
            value: serde_json::json!("hr"),
        });
        let compiled = compile_filter(&expr, "c").unwrap();
        assert!(compiled.sql.contains("json_extract(c.metadata"));
        assert_eq!(compiled.params, vec!["$.department", "hr"]);
    }

    #[test]
    fn compiles_exists_on_metadata() {
        let expr = FilterExpr::Condition(Condition {
            field: "meta.department".to_string(),
            op: FilterOp::Exists,
            value: serde_json::Value::Null,
        });
        let compiled = compile_filter(&expr, "c").unwrap();
        assert!(compiled.sql.contains("json_extract(c.metadata"));
        assert!(compiled.sql.contains("IS NOT NULL"));
    }

    #[test]
    fn compiles_nin_operator() {
        let expr = FilterExpr::Condition(Condition {
            field: "source_id".to_string(),
            op: FilterOp::Nin,
            value: serde_json::json!(["a"]),
        });
        let compiled = compile_filter(&expr, "c").unwrap();
        assert!(compiled.sql.contains("NOT IN"));
    }
}
