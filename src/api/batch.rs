// SPDX-License-Identifier: AGPL-3.0-only

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

impl ProblemDetails {
    pub fn new(status: StatusCode, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            problem_type: "about:blank".to_string(),
            title: title.into(),
            status: status.as_u16(),
            detail: detail.into(),
            instance: None,
        }
    }
}

pub struct ApiError {
    pub status: StatusCode,
    pub problem: ProblemDetails,
}

impl ApiError {
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            problem: ProblemDetails::new(StatusCode::BAD_REQUEST, "Bad Request", detail),
        }
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            problem: ProblemDetails::new(StatusCode::NOT_FOUND, "Not Found", detail),
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            problem: ProblemDetails::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                detail,
            ),
        }
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            problem: ProblemDetails::new(StatusCode::UNAUTHORIZED, "Unauthorized", detail),
        }
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            problem: ProblemDetails::new(StatusCode::FORBIDDEN, "Forbidden", detail),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.problem)).into_response()
    }
}

#[derive(Debug, Serialize)]
pub struct BatchItemResult<T> {
    pub index: usize,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProblemDetails>,
}

impl<T: Serialize> BatchItemResult<T> {
    pub fn ok(index: usize, result: T) -> Self {
        Self {
            index,
            status: StatusCode::OK.as_u16(),
            result: Some(result),
            error: None,
        }
    }

    pub fn err(index: usize, status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            index,
            status: status.as_u16(),
            result: None,
            error: Some(ProblemDetails::new(
                status,
                status.canonical_reason().unwrap_or("Error"),
                detail,
            )),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BatchResponse<T> {
    pub items: Vec<BatchItemResult<T>>,
}

impl<T: Serialize> BatchResponse<T> {
    pub fn http_status(&self) -> StatusCode {
        let all_ok = self
            .items
            .iter()
            .all(|item| (200..300).contains(&item.status));
        let all_failed = self.items.iter().all(|item| item.status >= 400);
        if all_ok {
            StatusCode::OK
        } else if all_failed {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::MULTI_STATUS
        }
    }
}

impl<T: Serialize> IntoResponse for BatchResponse<T> {
    fn into_response(self) -> Response {
        (self.http_status(), Json(self)).into_response()
    }
}
