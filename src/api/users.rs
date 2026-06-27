// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{
    authorize, hash_password, parse_and_validate_granted_permissions,
    parse_permissions_with_forced, permission_set_to_vec, permissions_to_json, validate_email,
    validate_password, AuthContext, Permission,
};

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub is_superadmin: bool,
    pub permissions: Vec<String>,
    pub created_at: String,
}

pub async fn get_users(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<UserRecord>>, ApiError> {
    authorize(&auth, Permission::UsersRead)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, email, is_superadmin, permissions, created_at FROM users ORDER BY created_at DESC",
            (),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let is_superadmin: i64 = row.get(2).map_err(|e| ApiError::internal(e.to_string()))?;
        let perms_raw: String = row.get(3).unwrap_or_else(|_| "[]".to_string());
        let perms = parse_permissions_with_forced(&perms_raw);
        items.push(UserRecord {
            id: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            email: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            is_superadmin: is_superadmin != 0,
            permissions: permission_set_to_vec(&perms),
            created_at: row.get(4).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }
    Ok(Json(items))
}

pub async fn post_users(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<UserRecord>, ApiError> {
    authorize(&auth, Permission::UsersWrite)?;
    if !validate_email(&body.email) {
        return Err(ApiError::bad_request("invalid email"));
    }
    validate_password(&body.password).map_err(ApiError::bad_request)?;
    let id = Uuid::new_v4().to_string();
    let hash = hash_password(&body.password).map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let permissions = parse_and_validate_granted_permissions(&body.permissions)?;
    let permissions_json = permissions_to_json(&permissions);
    conn.execute(
        "INSERT INTO users (id, email, password_hash, is_superadmin, password_is_default, permissions)
         VALUES (?1, ?2, ?3, 0, 0, ?4)",
        (id.as_str(), body.email.as_str(), hash.as_str(), permissions_json.as_str()),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(UserRecord {
        id,
        email: body.email,
        is_superadmin: false,
        permissions: permission_set_to_vec(&permissions),
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    }))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&auth, Permission::UsersDelete)?;
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT is_superadmin FROM users WHERE id = ?1",
            [id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let is_superadmin: i64 = row.get(0).map_err(|e| ApiError::internal(e.to_string()))?;
    if is_superadmin != 0 {
        return Err(ApiError::bad_request("cannot delete superadmin"));
    }
    conn.execute("DELETE FROM users WHERE id = ?1", [id.as_str()])
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub password: Option<String>,
    pub permissions: Option<Vec<String>>,
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<UserRecord>, ApiError> {
    authorize(&auth, Permission::UsersWrite)?;
    if body.email.is_none() && body.password.is_none() && body.permissions.is_none() {
        return Err(ApiError::bad_request(
            "email, password, or permissions required",
        ));
    }
    let conn = state
        .pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT id, email, is_superadmin, permissions, created_at FROM users WHERE id = ?1",
            [id.as_str()],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let is_superadmin: i64 = row.get(2).map_err(|e| ApiError::internal(e.to_string()))?;
    if is_superadmin != 0 {
        return Err(ApiError::bad_request("cannot edit superadmin via UI"));
    }
    let current_email: String = row.get(1).map_err(|e| ApiError::internal(e.to_string()))?;
    let created_at: String = row.get(4).map_err(|e| ApiError::internal(e.to_string()))?;
    let perms_raw: String = row.get(3).unwrap_or_else(|_| "[]".to_string());
    let mut permissions: Vec<String> =
        permission_set_to_vec(&parse_permissions_with_forced(&perms_raw));

    let new_email = if let Some(email) = body.email {
        if !validate_email(&email) {
            return Err(ApiError::bad_request("invalid email"));
        }
        conn.execute(
            "UPDATE users SET email = ?1 WHERE id = ?2",
            (email.as_str(), id.as_str()),
        )
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
        email
    } else {
        current_email
    };

    if let Some(password) = body.password {
        validate_password(&password).map_err(ApiError::bad_request)?;
        let hash = hash_password(&password).map_err(|e| ApiError::internal(e.to_string()))?;
        conn.execute(
            "UPDATE users SET password_hash = ?1, password_is_default = 0 WHERE id = ?2",
            (hash.as_str(), id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    if let Some(perms) = body.permissions {
        let parsed = parse_and_validate_granted_permissions(&perms)?;
        let permissions_json = permissions_to_json(&parsed);
        conn.execute(
            "UPDATE users SET permissions = ?1 WHERE id = ?2",
            (permissions_json.as_str(), id.as_str()),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        permissions = permission_set_to_vec(&parsed);
    }

    Ok(Json(UserRecord {
        id,
        email: new_email,
        is_superadmin: false,
        permissions,
        created_at,
    }))
}
