// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::api::router::AppState;
use crate::api::router::API_V1_PREFIX;
use crate::auth::jwt::{AuthClaims, TokenKind};
use crate::auth::verify_token;

#[derive(Debug, Clone)]
pub enum AuthPrincipal {
    Session {
        user_id: String,
        email: String,
        is_superadmin: bool,
    },
    ApiKey {
        key_id: String,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub principal: AuthPrincipal,
}

impl AuthContext {
    pub fn is_superadmin(&self) -> bool {
        matches!(
            self.principal,
            AuthPrincipal::Session {
                is_superadmin: true,
                ..
            }
        )
    }

    pub fn is_api_key(&self) -> bool {
        matches!(self.principal, AuthPrincipal::ApiKey { .. })
    }
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path().to_string();
    if is_public_path(&path) {
        return Ok(next.run(req).await);
    }

    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims =
        verify_token(&state.config.jwt_secret, token).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let auth = resolve_principal(&state, &claims)
        .await
        .map_err(|err| principal_error_status(&err.to_string()))?;

    if is_stage_plane_write(&path, req.method()) && !auth.is_api_key() {
        return Err(StatusCode::FORBIDDEN);
    }

    req.extensions_mut().insert(auth);
    Ok(next.run(req).await)
}

fn is_public_path(path: &str) -> bool {
    if path == "/favicon.ico" || path.starts_with("/assets/") {
        return true;
    }

    if !path.starts_with(API_V1_PREFIX) {
        return true;
    }

    path == "/api/v1/health"
        || path == "/api/v1/auth/login"
        || path == "/api/v1/auth/info"
        || path.starts_with("/api/v1/swagger-ui")
        || path == "/api/v1/openapi.json"
}

fn is_write_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// Production-plane writes: `/api/v1/stages/{tag}/sources`, etc.
fn is_stage_plane_write(path: &str, method: &Method) -> bool {
    if !is_write_method(method) {
        return false;
    }
    path.strip_prefix("/api/v1/stages/")
        .is_some_and(|rest| rest.contains('/'))
}

fn principal_error_status(message: &str) -> StatusCode {
    if message.contains("user not found") || message.contains("api key revoked") {
        StatusCode::UNAUTHORIZED
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn resolve_principal(state: &AppState, claims: &AuthClaims) -> anyhow::Result<AuthContext> {
    match claims.typ {
        TokenKind::Session => {
            let email = claims
                .email
                .clone()
                .ok_or_else(|| anyhow::anyhow!("session token missing email"))?;
            Ok(AuthContext {
                principal: AuthPrincipal::Session {
                    user_id: claims.sub.clone(),
                    email,
                    is_superadmin: claims.is_superadmin,
                },
            })
        }
        TokenKind::Apikey => {
            let conn = state.pool.connect_one().await?;
            let mut rows = conn
                .query(
                    "SELECT id, name FROM api_keys WHERE id = ?1",
                    [claims.sub.as_str()],
                )
                .await?;
            let row = rows
                .next()
                .await?
                .ok_or_else(|| anyhow::anyhow!("api key revoked"))?;
            Ok(AuthContext {
                principal: AuthPrincipal::ApiKey {
                    key_id: row.get(0)?,
                    name: row.get(1)?,
                },
            })
        }
    }
}

pub fn require_auth(req: &Request<Body>) -> Result<&AuthContext, crate::api::error::ApiError> {
    req.extensions()
        .get::<AuthContext>()
        .ok_or_else(|| crate::api::error::ApiError::unauthorized("authentication required"))
}

pub fn require_superadmin(auth: &AuthContext) -> Result<(), crate::api::error::ApiError> {
    if auth.is_superadmin() {
        Ok(())
    } else {
        Err(crate::api::error::ApiError::forbidden(
            "superadmin required",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;

    #[test]
    fn is_public_path_allows_health_and_static() {
        assert!(is_public_path("/api/v1/health"));
        assert!(is_public_path("/api/v1/auth/login"));
        assert!(is_public_path("/api/v1/auth/info"));
        assert!(is_public_path("/assets/app.js"));
        assert!(is_public_path("/favicon.ico"));
        assert!(!is_public_path("/api/v1/releases"));
    }

    #[test]
    fn is_write_method_detects_mutating_verbs() {
        assert!(is_write_method(&Method::POST));
        assert!(is_write_method(&Method::DELETE));
        assert!(!is_write_method(&Method::GET));
    }

    #[test]
    fn is_stage_plane_write_only_for_stage_subresources() {
        assert!(is_stage_plane_write(
            "/api/v1/stages/prod/sources",
            &Method::POST
        ));
        assert!(!is_stage_plane_write(
            "/api/v1/releases/first-release/sources",
            &Method::POST
        ));
        assert!(!is_stage_plane_write(
            "/api/v1/stages/prod/sources",
            &Method::GET
        ));
    }

    #[test]
    fn principal_error_status_maps_known_messages() {
        assert_eq!(
            principal_error_status("api key revoked"),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            principal_error_status("database is locked"),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn auth_context_flags() {
        let session = AuthContext {
            principal: AuthPrincipal::Session {
                user_id: "u1".into(),
                email: "a@b.com".into(),
                is_superadmin: true,
            },
        };
        assert!(session.is_superadmin());
        assert!(!session.is_api_key());

        let api = AuthContext {
            principal: AuthPrincipal::ApiKey {
                key_id: "k1".into(),
                name: "prod".into(),
            },
        };
        assert!(!api.is_superadmin());
        assert!(api.is_api_key());
    }
}
