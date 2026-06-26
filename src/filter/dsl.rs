// SPDX-License-Identifier: AGPL-3.0-only

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const MAX_DEPTH: usize = 8;
pub const MAX_CONDITIONS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FilterExpr {
    And { and: Vec<FilterExpr> },
    Or { or: Vec<FilterExpr> },
    Not { not: Box<FilterExpr> },
    Condition(Condition),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    pub field: String,
    pub op: FilterOp,
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    Nin,
    Contains,
    Exists,
}

#[derive(Debug, Error)]
pub enum FilterError {
    #[error("invalid filter JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid base64 filter: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("filter validation error: {0}")]
    Validation(String),
    #[error("unsupported field: {0}")]
    UnsupportedField(String),
    #[error("unsupported operator for field: {field} op {op:?}")]
    UnsupportedOperator { field: String, op: FilterOp },
}

pub fn decode_filter_param(raw: &str) -> Result<FilterExpr, FilterError> {
    if raw.trim_start().starts_with('{') {
        let expr: FilterExpr = serde_json::from_str(raw)?;
        validate_filter(&expr, 1, &mut 0)?;
        return Ok(expr);
    }

    use base64::Engine;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw)?;
    let expr: FilterExpr = serde_json::from_slice(&decoded)?;
    validate_filter(&expr, 1, &mut 0)?;
    Ok(expr)
}

pub fn validate_filter(expr: &FilterExpr, depth: usize, count: &mut usize) -> Result<(), FilterError> {
    if depth > MAX_DEPTH {
        return Err(FilterError::Validation(format!(
            "filter depth exceeds {MAX_DEPTH}"
        )));
    }

    match expr {
        FilterExpr::And { and: items } | FilterExpr::Or { or: items } => {
            for item in items {
                validate_filter(item, depth + 1, count)?;
            }
        }
        FilterExpr::Not { not: inner } => validate_filter(inner, depth + 1, count)?,
        FilterExpr::Condition(cond) => {
            *count += 1;
            if *count > MAX_CONDITIONS {
                return Err(FilterError::Validation(format!(
                    "filter exceeds {MAX_CONDITIONS} conditions"
                )));
            }
            validate_condition(cond)?;
        }
    }

    Ok(())
}

fn validate_condition(cond: &Condition) -> Result<(), FilterError> {
    resolve_field(&cond.field)?;
    match cond.op {
        FilterOp::Exists => Ok(()),
        FilterOp::In | FilterOp::Nin => {
            if !cond.value.is_array() {
                return Err(FilterError::Validation(format!(
                    "operator {:?} requires array value",
                    cond.op
                )));
            }
            Ok(())
        }
        _ if cond.value.is_null() => Err(FilterError::Validation(format!(
            "operator {:?} requires value",
            cond.op
        ))),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedField {
    Column(&'static str),
    Metadata(String),
}

pub fn resolve_field(field: &str) -> Result<ResolvedField, FilterError> {
    match field {
        "id" => Ok(ResolvedField::Column("id")),
        "source_id" => Ok(ResolvedField::Column("source_id")),
        "created_at" => Ok(ResolvedField::Column("created_at")),
        "updated_at" => Ok(ResolvedField::Column("updated_at")),
        "name" => Ok(ResolvedField::Column("name")),
        "status" => Ok(ResolvedField::Column("status")),
        "type" => Ok(ResolvedField::Column("type")),
        meta if meta.starts_with("meta.") => {
            let path = &meta[5..];
            if path.is_empty() || !path.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                return Err(FilterError::Validation(format!(
                    "invalid metadata path: {meta}"
                )));
            }
            Ok(ResolvedField::Metadata(format!("$.{path}")))
        }
        other => Err(FilterError::UnsupportedField(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn parses_and_condition() {
        let raw = r#"{"and":[{"field":"source_id","op":"in","value":["a"]}]}"#;
        let expr = decode_filter_param(raw).unwrap();
        assert!(matches!(expr, FilterExpr::And { .. }));
    }

    #[test]
    fn parses_base64_filter() {
        let json = r#"{"field":"source_id","op":"eq","value":"abc"}"#;
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
        let expr = decode_filter_param(&encoded).unwrap();
        assert!(matches!(expr, FilterExpr::Condition(_)));
    }

    #[test]
    fn rejects_unsupported_field() {
        let raw = r#"{"field":"unknown","op":"eq","value":"x"}"#;
        let err = decode_filter_param(raw).unwrap_err();
        assert!(matches!(err, FilterError::UnsupportedField(_)));
    }

    #[test]
    fn rejects_in_without_array() {
        let raw = r#"{"field":"source_id","op":"in","value":"x"}"#;
        let err = decode_filter_param(raw).unwrap_err();
        assert!(matches!(err, FilterError::Validation(_)));
    }

    #[test]
    fn resolves_metadata_field() {
        let resolved = resolve_field("meta.department").unwrap();
        assert!(matches!(resolved, ResolvedField::Metadata(path) if path == "$.department"));
    }

    #[test]
    fn compiles_or_expression() {
        let expr = FilterExpr::Or {
            or: vec![
                FilterExpr::Condition(Condition {
                    field: "status".to_string(),
                    op: FilterOp::Eq,
                    value: serde_json::json!("completed"),
                }),
                FilterExpr::Condition(Condition {
                    field: "status".to_string(),
                    op: FilterOp::Eq,
                    value: serde_json::json!("failed"),
                }),
            ],
        };
        let compiled = crate::filter::sql::compile_filter(&expr, "s").unwrap();
        assert!(compiled.sql.contains(" OR "));
    }

    #[test]
    fn rejects_filter_exceeding_max_conditions() {
        let mut conditions = Vec::new();
        for idx in 0..=MAX_CONDITIONS {
            conditions.push(FilterExpr::Condition(Condition {
                field: "source_id".to_string(),
                op: FilterOp::Eq,
                value: serde_json::json!(format!("s{idx}")),
            }));
        }
        let expr = FilterExpr::And { and: conditions };
        assert!(validate_filter(&expr, 1, &mut 0).is_err());
    }
}
