// SPDX-License-Identifier: AGPL-3.0-only

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Session,
    Apikey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub typ: TokenKind,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub is_superadmin: bool,
    #[serde(default)]
    pub name: Option<String>,
    pub iat: i64,
    #[serde(default)]
    pub exp: Option<i64>,
}

pub fn encode_session_token(
    secret: &str,
    user_id: &str,
    email: &str,
    is_superadmin: bool,
    ttl_secs: i64,
) -> anyhow::Result<String> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let claims = AuthClaims {
        sub: user_id.to_string(),
        typ: TokenKind::Session,
        email: Some(email.to_string()),
        is_superadmin,
        name: None,
        iat: now,
        exp: Some(now + ttl_secs),
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

pub fn encode_api_key_token(
    secret: &str,
    key_id: &str,
    name: &str,
    created_at: &str,
) -> anyhow::Result<String> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let claims = AuthClaims {
        sub: key_id.to_string(),
        typ: TokenKind::Apikey,
        email: None,
        is_superadmin: false,
        name: Some(name.to_string()),
        iat: now,
        exp: None,
    };
    let _ = created_at;
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

pub fn verify_token(secret: &str, token: &str) -> anyhow::Result<AuthClaims> {
    let mut validation = Validation::default();
    validation.validate_exp = false;
    validation.required_spec_claims.remove("exp");
    let data = decode::<AuthClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    let claims = data.claims;
    if claims.typ == TokenKind::Session {
        if let Some(exp) = claims.exp {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            if now > exp {
                anyhow::bail!("session token expired");
            }
        }
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_token_roundtrip() {
        let token =
            encode_session_token("secret", "user-1", "admin@ragdoll.ai", true, 3600).unwrap();
        let claims = verify_token("secret", &token).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.typ, TokenKind::Session);
        assert!(claims.is_superadmin);
    }

    #[test]
    fn api_key_token_has_no_expiry() {
        let token = encode_api_key_token("secret", "key-1", "prod", "2026-01-01").unwrap();
        let claims = verify_token("secret", &token).unwrap();
        assert_eq!(claims.typ, TokenKind::Apikey);
        assert!(claims.exp.is_none());
    }

    #[test]
    fn wrong_secret_fails_verification() {
        let token =
            encode_session_token("secret", "user-1", "admin@ragdoll.ai", true, 3600).unwrap();
        assert!(verify_token("other-secret", &token).is_err());
    }
}
