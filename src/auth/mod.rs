// SPDX-License-Identifier: AGPL-3.0-only

pub mod bootstrap;
pub mod jwt;
pub mod middleware;
pub mod password;

pub use bootstrap::{authenticate_user, ensure_superadmin, validate_email};
pub use jwt::{AuthClaims, TokenKind, encode_api_key_token, encode_session_token, verify_token};
pub use middleware::{AuthContext, AuthPrincipal, require_auth, require_superadmin};
pub use password::{hash_password, verify_password};
