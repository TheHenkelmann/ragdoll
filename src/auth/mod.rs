// SPDX-License-Identifier: AGPL-3.0-only

pub mod bootstrap;
pub mod jwt;
pub mod middleware;
pub mod password;

pub use bootstrap::{authenticate_user, ensure_superadmin, validate_email};
pub use jwt::{encode_api_key_token, encode_session_token, verify_token, AuthClaims, TokenKind};
pub use middleware::{require_auth, require_superadmin, AuthContext, AuthPrincipal};
pub use password::{hash_password, verify_password};
