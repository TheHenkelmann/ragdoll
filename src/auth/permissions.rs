// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashSet;
use std::str::FromStr;

use crate::api::error::ApiError;
use crate::auth::AuthContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    SourcesRead,
    SourcesWrite,
    SourcesDelete,
    ChunksRead,
    ChunksWrite,
    ChunksDelete,
    QueriesRun,
    QueriesRead,
    QueriesDelete,
    PlaygroundRun,
    PlaygroundRead,
    DbRead,
    SettingsRead,
    SettingsWrite,
    LlmModelsRead,
    LlmModelsWrite,
    LlmModelsDelete,
    LlmCredentialsRead,
    LlmCredentialsWrite,
    LlmCredentialsDelete,
    AnalyticsRead,
    ReleasesRead,
    ReleasesWrite,
    ReleasesDelete,
    StagesRead,
    StagesWrite,
    StagesDelete,
    ModelsRead,
    ModelsDownload,
    ModelsDelete,
    BackupsRead,
    BackupsCreate,
    BackupsUpload,
    BackupsDownload,
    BackupsRestore,
    BackupsDelete,
    UsersRead,
    UsersWrite,
    UsersDelete,
    ApiKeysRead,
    ApiKeysWrite,
    ApiKeysDelete,
    WebhooksRead,
    WebhooksWrite,
    WebhooksDelete,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourcesRead => "sources:read",
            Self::SourcesWrite => "sources:write",
            Self::SourcesDelete => "sources:delete",
            Self::ChunksRead => "chunks:read",
            Self::ChunksWrite => "chunks:write",
            Self::ChunksDelete => "chunks:delete",
            Self::QueriesRun => "queries:run",
            Self::QueriesRead => "queries:read",
            Self::QueriesDelete => "queries:delete",
            Self::PlaygroundRun => "playground:run",
            Self::PlaygroundRead => "playground:read",
            Self::DbRead => "db:read",
            Self::SettingsRead => "settings:read",
            Self::SettingsWrite => "settings:write",
            Self::LlmModelsRead => "llm_models:read",
            Self::LlmModelsWrite => "llm_models:write",
            Self::LlmModelsDelete => "llm_models:delete",
            Self::LlmCredentialsRead => "llm_credentials:read",
            Self::LlmCredentialsWrite => "llm_credentials:write",
            Self::LlmCredentialsDelete => "llm_credentials:delete",
            Self::AnalyticsRead => "analytics:read",
            Self::ReleasesRead => "releases:read",
            Self::ReleasesWrite => "releases:write",
            Self::ReleasesDelete => "releases:delete",
            Self::StagesRead => "stages:read",
            Self::StagesWrite => "stages:write",
            Self::StagesDelete => "stages:delete",
            Self::ModelsRead => "models:read",
            Self::ModelsDownload => "models:download",
            Self::ModelsDelete => "models:delete",
            Self::BackupsRead => "backups:read",
            Self::BackupsCreate => "backups:create",
            Self::BackupsUpload => "backups:upload",
            Self::BackupsDownload => "backups:download",
            Self::BackupsRestore => "backups:restore",
            Self::BackupsDelete => "backups:delete",
            Self::UsersRead => "users:read",
            Self::UsersWrite => "users:write",
            Self::UsersDelete => "users:delete",
            Self::ApiKeysRead => "api_keys:read",
            Self::ApiKeysWrite => "api_keys:write",
            Self::ApiKeysDelete => "api_keys:delete",
            Self::WebhooksRead => "webhooks:read",
            Self::WebhooksWrite => "webhooks:write",
            Self::WebhooksDelete => "webhooks:delete",
        }
    }

    pub fn all() -> &'static [Permission] {
        &[
            Self::SourcesRead,
            Self::SourcesWrite,
            Self::SourcesDelete,
            Self::ChunksRead,
            Self::ChunksWrite,
            Self::ChunksDelete,
            Self::QueriesRun,
            Self::QueriesRead,
            Self::QueriesDelete,
            Self::PlaygroundRun,
            Self::PlaygroundRead,
            Self::DbRead,
            Self::SettingsRead,
            Self::SettingsWrite,
            Self::LlmModelsRead,
            Self::LlmModelsWrite,
            Self::LlmModelsDelete,
            Self::LlmCredentialsRead,
            Self::LlmCredentialsWrite,
            Self::LlmCredentialsDelete,
            Self::AnalyticsRead,
            Self::ReleasesRead,
            Self::ReleasesWrite,
            Self::ReleasesDelete,
            Self::StagesRead,
            Self::StagesWrite,
            Self::StagesDelete,
            Self::ModelsRead,
            Self::ModelsDownload,
            Self::ModelsDelete,
            Self::BackupsRead,
            Self::BackupsCreate,
            Self::BackupsUpload,
            Self::BackupsDownload,
            Self::BackupsRestore,
            Self::BackupsDelete,
            Self::UsersRead,
            Self::UsersWrite,
            Self::UsersDelete,
            Self::ApiKeysRead,
            Self::ApiKeysWrite,
            Self::ApiKeysDelete,
            Self::WebhooksRead,
            Self::WebhooksWrite,
            Self::WebhooksDelete,
        ]
    }
}

impl FromStr for Permission {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Permission::all()
            .iter()
            .copied()
            .find(|p| p.as_str() == s)
            .ok_or(())
    }
}

/// Parse a stored JSON permission array. No permission is forced here.
pub fn parse_permissions(raw: &str) -> HashSet<Permission> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return HashSet::new();
    };
    let Some(arr) = value.as_array() else {
        return HashSet::new();
    };
    arr.iter()
        .filter_map(|v| v.as_str())
        .filter_map(|s| Permission::from_str(s).ok())
        .collect()
}

/// Like [`parse_permissions`] but always includes the user-forced permission.
pub fn parse_permissions_with_forced(raw: &str) -> HashSet<Permission> {
    with_forced_permissions(parse_permissions(raw))
}

/// Always granted to every user (not to API keys).
pub const FORCED_PERMISSION: Permission = Permission::ReleasesRead;

pub fn with_forced_permissions(mut perms: HashSet<Permission>) -> HashSet<Permission> {
    perms.insert(FORCED_PERMISSION);
    perms
}

fn parse_permission_list(raw: &[String]) -> Result<HashSet<Permission>, ApiError> {
    let mut set = HashSet::new();
    for item in raw {
        let perm = item
            .parse::<Permission>()
            .map_err(|_| ApiError::bad_request(format!("unknown permission: {item}")))?;
        set.insert(perm);
    }
    Ok(set)
}

/// User permissions: `releases:read` is always granted, plus at least one more.
pub fn parse_and_validate_granted_permissions(
    raw: &[String],
) -> Result<HashSet<Permission>, ApiError> {
    let mut set = parse_permission_list(raw)?;
    set.insert(FORCED_PERMISSION);
    if set.len() <= 1 {
        return Err(ApiError::bad_request(
            "at least one permission besides releases:read is required",
        ));
    }
    Ok(set)
}

/// API key permissions: nothing forced, but at least one permission is required.
pub fn parse_and_validate_api_key_permissions(
    raw: &[String],
) -> Result<HashSet<Permission>, ApiError> {
    let set = parse_permission_list(raw)?;
    if set.is_empty() {
        return Err(ApiError::bad_request("at least one permission is required"));
    }
    Ok(set)
}

pub fn permission_set_to_vec(perms: &HashSet<Permission>) -> Vec<String> {
    let mut items: Vec<String> = perms.iter().map(|p| p.as_str().to_string()).collect();
    items.sort();
    items
}

pub fn permissions_to_json(perms: &HashSet<Permission>) -> String {
    let mut items: Vec<&str> = perms.iter().map(|p| p.as_str()).collect();
    items.sort_unstable();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
}

pub fn authorize(auth: &AuthContext, permission: Permission) -> Result<(), ApiError> {
    if auth.is_superadmin() {
        return Ok(());
    }
    if auth.permissions.contains(&permission) {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!(
            "permission required: {}",
            permission.as_str()
        )))
    }
}

pub fn authorize_any(auth: &AuthContext, permissions: &[Permission]) -> Result<(), ApiError> {
    if auth.is_superadmin() {
        return Ok(());
    }
    if permissions.iter().any(|p| auth.permissions.contains(p)) {
        Ok(())
    } else {
        Err(ApiError::forbidden("insufficient permissions"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthPrincipal;

    fn ctx_with(perms: &[Permission]) -> AuthContext {
        AuthContext {
            principal: AuthPrincipal::Session {
                user_id: "u1".into(),
                email: "a@b.com".into(),
                is_superadmin: false,
            },
            permissions: perms.iter().copied().collect(),
            rpm: None,
            rph: None,
        }
    }

    fn superadmin_ctx() -> AuthContext {
        AuthContext {
            principal: AuthPrincipal::Session {
                user_id: "u1".into(),
                email: "a@b.com".into(),
                is_superadmin: true,
            },
            permissions: HashSet::new(),
            rpm: None,
            rph: None,
        }
    }

    #[test]
    fn superadmin_allowed_everywhere() {
        let auth = superadmin_ctx();
        for perm in Permission::all() {
            assert!(authorize(&auth, *perm).is_ok());
        }
    }

    #[test]
    fn empty_permissions_denied() {
        let auth = ctx_with(&[]);
        assert!(authorize(&auth, Permission::SourcesRead).is_err());
    }

    #[test]
    fn specific_permission_grants_access() {
        let auth = ctx_with(&[Permission::SourcesRead]);
        assert!(authorize(&auth, Permission::SourcesRead).is_ok());
        assert!(authorize(&auth, Permission::SourcesWrite).is_err());
    }

    #[test]
    fn parse_permissions_does_not_force_releases_read() {
        let perms = parse_permissions(r#"["sources:read"]"#);
        assert!(!perms.contains(&Permission::ReleasesRead));
        assert!(perms.contains(&Permission::SourcesRead));
    }

    #[test]
    fn user_parse_forces_releases_read() {
        let perms = parse_permissions_with_forced(r#"["sources:read"]"#);
        assert!(perms.contains(&Permission::ReleasesRead));
        assert!(perms.contains(&Permission::SourcesRead));
    }

    #[test]
    fn user_grant_requires_additional_permission() {
        assert!(parse_and_validate_granted_permissions(&[]).is_err());
        assert!(parse_and_validate_granted_permissions(&["releases:read".into()]).is_err());
        let perms = parse_and_validate_granted_permissions(&["sources:read".into()]);
        assert!(perms.is_ok());
        let perms = perms.ok().unwrap();
        assert!(perms.contains(&Permission::ReleasesRead));
        assert!(perms.contains(&Permission::SourcesRead));
    }

    #[test]
    fn api_key_grant_requires_one_permission_without_forcing() {
        assert!(parse_and_validate_api_key_permissions(&[]).is_err());
        let perms = parse_and_validate_api_key_permissions(&["sources:read".into()]);
        assert!(perms.is_ok());
        let perms = perms.ok().unwrap();
        assert!(!perms.contains(&Permission::ReleasesRead));
        assert!(perms.contains(&Permission::SourcesRead));
        assert_eq!(perms.len(), 1);
    }
}
