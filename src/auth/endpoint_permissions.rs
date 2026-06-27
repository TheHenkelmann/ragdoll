// SPDX-License-Identifier: AGPL-3.0-only

use crate::auth::Permission;

#[derive(Debug, Clone, Copy)]
pub struct RoutePermission {
    pub method: &'static str,
    pub path_suffix: &'static str,
    pub permission: Option<Permission>,
}

/// Catalog of guarded API routes. Public routes (health, login, static) are omitted.
pub fn endpoint_permission_map() -> &'static [RoutePermission] {
    &[
        RoutePermission {
            method: "GET",
            path_suffix: "/users",
            permission: Some(Permission::UsersRead),
        },
        RoutePermission {
            method: "POST",
            path_suffix: "/users",
            permission: Some(Permission::UsersWrite),
        },
        RoutePermission {
            method: "PATCH",
            path_suffix: "/users/{id}",
            permission: Some(Permission::UsersWrite),
        },
        RoutePermission {
            method: "DELETE",
            path_suffix: "/users/{id}",
            permission: Some(Permission::UsersDelete),
        },
        RoutePermission {
            method: "GET",
            path_suffix: "/api_keys",
            permission: Some(Permission::ApiKeysRead),
        },
        RoutePermission {
            method: "POST",
            path_suffix: "/api_keys",
            permission: Some(Permission::ApiKeysWrite),
        },
        RoutePermission {
            method: "PATCH",
            path_suffix: "/api_keys/{id}",
            permission: Some(Permission::ApiKeysWrite),
        },
        RoutePermission {
            method: "DELETE",
            path_suffix: "/api_keys/{id}",
            permission: Some(Permission::ApiKeysDelete),
        },
        RoutePermission {
            method: "GET",
            path_suffix: "/releases",
            permission: Some(Permission::ReleasesRead),
        },
        RoutePermission {
            method: "POST",
            path_suffix: "/releases",
            permission: Some(Permission::ReleasesWrite),
        },
        RoutePermission {
            method: "GET",
            path_suffix: "/releases/{tag}/sources",
            permission: Some(Permission::SourcesRead),
        },
        RoutePermission {
            method: "POST",
            path_suffix: "/releases/{tag}/queries",
            permission: Some(Permission::QueriesRun),
        },
        RoutePermission {
            method: "POST",
            path_suffix: "/playground/{tag}/queries",
            permission: Some(Permission::PlaygroundRun),
        },
        RoutePermission {
            method: "GET",
            path_suffix: "/releases/{tag}/webhooks",
            permission: Some(Permission::WebhooksRead),
        },
        RoutePermission {
            method: "POST",
            path_suffix: "/releases/{tag}/webhooks",
            permission: Some(Permission::WebhooksWrite),
        },
        RoutePermission {
            method: "GET",
            path_suffix: "/releases/{tag}/webhooks/{id}/secret",
            permission: Some(Permission::WebhooksRead),
        },
        RoutePermission {
            method: "POST",
            path_suffix: "/releases/{tag}/webhooks/{id}/test",
            permission: Some(Permission::WebhooksWrite),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_permission_has_route_coverage_sample() {
        let covered: HashSet<_> = endpoint_permission_map()
            .iter()
            .filter_map(|r| r.permission)
            .collect();
        assert!(covered.contains(&Permission::QueriesRun));
        assert!(covered.contains(&Permission::WebhooksRead));
    }

    #[test]
    fn permission_catalog_is_complete_count() {
        assert_eq!(Permission::all().len(), 45);
    }
}
