// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::State;
use axum::Extension;
use axum::Json;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{encode_session_token, validate_email, AuthContext};

#[derive(Debug, serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub email: String,
    pub is_superadmin: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct AuthStatusResponse {
    pub email: String,
    pub is_superadmin: bool,
    pub password_is_default: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct AuthInfoResponse {
    pub default_admin_email: String,
    pub password_is_default: bool,
}

pub async fn get_auth_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AuthInfoResponse>, ApiError> {
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT password_is_default FROM users WHERE is_superadmin = 1 LIMIT 1",
            (),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let password_is_default = if state.config.superadmin_password.is_some() {
        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
        {
            let v: i64 = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
            v != 0
        } else {
            false
        }
    } else {
        true
    };
    Ok(Json(AuthInfoResponse {
        default_admin_email: state.config.superadmin_email.clone(),
        password_is_default,
    }))
}

pub async fn post_login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    if !validate_email(&body.email) {
        return Err(ApiError::bad_request("invalid email"));
    }
    let (user_id, is_superadmin, _) =
        crate::auth::authenticate_user(&state.pool, &body.email, &body.password)
            .await
            .map_err(|_| ApiError::unauthorized("invalid credentials"))?;

    let token = encode_session_token(
        &state.config.jwt_secret,
        &user_id,
        &body.email,
        is_superadmin,
        86_400,
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(LoginResponse {
        token,
        email: body.email,
        is_superadmin,
    }))
}

pub async fn get_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<AuthStatusResponse>, ApiError> {
    let (email, is_superadmin) = match &auth.principal {
        crate::auth::AuthPrincipal::Session {
            email,
            is_superadmin,
            ..
        } => (email.clone(), *is_superadmin),
        _ => return Err(ApiError::forbidden("session token required")),
    };
    let password_is_default = query_password_is_default(&state, &email).await?;
    Ok(Json(AuthStatusResponse {
        email,
        is_superadmin,
        password_is_default,
    }))
}

async fn query_password_is_default(state: &AppState, email: &str) -> Result<bool, ApiError> {
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT password_is_default FROM users WHERE email = ?1",
            [email],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let password_is_default: i64 = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(password_is_default != 0)
}
