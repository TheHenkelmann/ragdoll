// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::State;
use axum::Extension;
use axum::Json;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{
    encode_session_token, hash_password, validate_email, validate_password, AuthContext,
    AuthPrincipal,
};

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
    pub permissions: Vec<String>,
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
        &state.config.secret,
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
    let mut permissions: Vec<String> = auth
        .permissions
        .iter()
        .map(|p| p.as_str().to_string())
        .collect();
    permissions.sort();
    Ok(Json(AuthStatusResponse {
        email,
        is_superadmin,
        password_is_default,
        permissions,
    }))
}

#[derive(Debug, serde::Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ChangePasswordResponse {
    pub changed: bool,
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, ApiError> {
    let email = match &auth.principal {
        AuthPrincipal::Session {
            email,
            is_superadmin,
            ..
        } => {
            if *is_superadmin {
                return Err(ApiError::forbidden(
                    "superadmin password cannot be changed via the UI; set RAGDOLL_SUPERADMIN_PW",
                ));
            }
            email.clone()
        }
        _ => return Err(ApiError::forbidden("session token required")),
    };
    validate_password(&body.new_password).map_err(ApiError::bad_request)?;
    crate::auth::authenticate_user(&state.pool, &email, &body.current_password)
        .await
        .map_err(|_| ApiError::unauthorized("invalid current password"))?;
    let hash = hash_password(&body.new_password).map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    conn.execute(
        "UPDATE users SET password_hash = ?1, password_is_default = 0 WHERE email = ?2",
        (hash.as_str(), email.as_str()),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ChangePasswordResponse { changed: true }))
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
