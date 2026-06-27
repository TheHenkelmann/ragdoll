// SPDX-License-Identifier: AGPL-3.0-only

pub mod bootstrap;
pub mod endpoint_permissions;
pub mod jwt;
pub mod middleware;
pub mod password;
pub mod permissions;
pub mod rate_limit;

pub use bootstrap::{authenticate_user, ensure_superadmin, validate_email};
pub use jwt::{encode_api_key_token, encode_session_token, verify_token, AuthClaims, TokenKind};
pub use middleware::{require_auth, require_superadmin, AuthContext, AuthPrincipal};
pub use password::{hash_password, validate_password, verify_password};
pub use permissions::{
    authorize, authorize_any, parse_and_validate_api_key_permissions,
    parse_and_validate_granted_permissions, parse_permissions, parse_permissions_with_forced,
    permission_set_to_vec, permissions_to_json, Permission,
};
