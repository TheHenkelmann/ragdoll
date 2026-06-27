// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::{Context, Result};
use argon2::password_hash::rand_core::RngCore;
use base64::{
    engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD as BASE64URL},
    Engine as _,
};
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;

const CREDENTIAL_KEY_INFO: &[u8] = b"ragdoll-credential-encryption-v1";
const WEBHOOK_SECRET_BYTES: usize = 32;

/// Namespace prefix for Ragdoll-issued bearer credentials shown to users.
pub const RAGDOLL_TOKEN_PREFIX: &str = "rd_";
/// Webhook signing secret prefix (`rd_` namespace + `whsec_` type tag).
pub const WEBHOOK_SECRET_PREFIX: &str = "rd_whsec_";

#[derive(Clone)]
pub struct Crypto {
    credential_key: [u8; 32],
}

impl Crypto {
    pub fn from_secret(secret: &str) -> Result<Self> {
        let hk = Hkdf::<Sha256>::new(None, secret.as_bytes());
        let mut credential_key = [0u8; 32];
        hk.expand(CREDENTIAL_KEY_INFO, &mut credential_key)
            .map_err(|e| anyhow::anyhow!("derive credential encryption key: {e}"))?;
        Ok(Self { credential_key })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<(String, String)> {
        let cipher =
            XChaCha20Poly1305::new_from_slice(&self.credential_key).context("init cipher")?;
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encrypt credential: {e}"))?;
        Ok((BASE64.encode(nonce), BASE64.encode(ciphertext)))
    }

    pub fn decrypt(&self, nonce_b64: &str, ciphertext_b64: &str) -> Result<String> {
        let cipher =
            XChaCha20Poly1305::new_from_slice(&self.credential_key).context("init cipher")?;
        let nonce_bytes = BASE64.decode(nonce_b64).context("decode nonce")?;
        let nonce = XNonce::from_slice(&nonce_bytes);
        let ciphertext = BASE64.decode(ciphertext_b64).context("decode ciphertext")?;
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| anyhow::anyhow!("decrypt credential: {e}"))?;
        String::from_utf8(plaintext).context("credential plaintext is not utf-8")
    }
}

/// Prefix a JWT API key for display/storage by the client (`rd_` + JWT).
pub fn format_api_key_token(jwt: &str) -> String {
    if jwt.starts_with(RAGDOLL_TOKEN_PREFIX) {
        jwt.to_string()
    } else {
        format!("{RAGDOLL_TOKEN_PREFIX}{jwt}")
    }
}

/// Accept both prefixed (`rd_…`) and legacy unprefixed bearer tokens.
pub fn normalize_bearer_token(token: &str) -> &str {
    token.strip_prefix(RAGDOLL_TOKEN_PREFIX).unwrap_or(token)
}

/// Generate a high-entropy webhook signing secret (`rd_whsec_` + 32 random bytes, base64url).
pub fn generate_webhook_secret() -> String {
    let mut bytes = [0u8; WEBHOOK_SECRET_BYTES];
    OsRng.fill_bytes(&mut bytes);
    format!("{WEBHOOK_SECRET_PREFIX}{}", BASE64URL.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let crypto = Crypto::from_secret("test-secret").unwrap();
        let (nonce, ciphertext) = crypto.encrypt("sk-test-api-key").unwrap();
        let plain = crypto.decrypt(&nonce, &ciphertext).unwrap();
        assert_eq!(plain, "sk-test-api-key");
    }

    #[test]
    fn wrong_secret_fails_decrypt() {
        let crypto = Crypto::from_secret("secret-a").unwrap();
        let other = Crypto::from_secret("secret-b").unwrap();
        let (nonce, ciphertext) = crypto.encrypt("key").unwrap();
        assert!(other.decrypt(&nonce, &ciphertext).is_err());
    }

    #[test]
    fn generate_webhook_secret_has_prefix_and_entropy() {
        let secret = generate_webhook_secret();
        assert!(secret.starts_with(WEBHOOK_SECRET_PREFIX));
        let encoded = secret.strip_prefix(WEBHOOK_SECRET_PREFIX).unwrap();
        let bytes = BASE64URL.decode(encoded).unwrap();
        assert_eq!(bytes.len(), WEBHOOK_SECRET_BYTES);
        assert_ne!(generate_webhook_secret(), secret);
    }

    #[test]
    fn api_key_token_prefix_roundtrip() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.payload.sig";
        let prefixed = format_api_key_token(jwt);
        assert!(prefixed.starts_with(RAGDOLL_TOKEN_PREFIX));
        assert_eq!(normalize_bearer_token(&prefixed), jwt);
        assert_eq!(normalize_bearer_token(jwt), jwt);
    }
}
