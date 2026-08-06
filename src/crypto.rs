// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::{anyhow, Context, Result};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use base64::{
    engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD as BASE64URL},
    Engine as _,
};
use chacha20poly1305::aead::{Aead, Generate, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::db::DbPool;

/// Legacy HKDF info used when credentials were encrypted directly from the master secret.
const LEGACY_CREDENTIAL_KEY_INFO: &[u8] = b"ragdoll-credential-encryption-v1";
/// HKDF info for deriving the key-encryption key (KEK) that wraps the DEK.
const KEK_INFO: &[u8] = b"ragdoll-kek-v1";

const META_SCHEME: &str = "crypto.scheme";
const META_DEK_NONCE: &str = "crypto.dek_nonce";
const META_DEK_CIPHERTEXT: &str = "crypto.dek_ciphertext";
const SCHEME_ENVELOPE_V1: &str = "envelope-v1";

const WEBHOOK_SECRET_BYTES: usize = 32;

/// Namespace prefix for Ragdoll-issued bearer credentials shown to users.
pub const RAGDOLL_TOKEN_PREFIX: &str = "rd_";
/// Webhook signing secret prefix (`rd_` namespace + `whsec_` type tag).
pub const WEBHOOK_SECRET_PREFIX: &str = "rd_whsec_";

#[derive(Clone)]
pub struct Crypto {
    dek: [u8; 32],
}

impl Crypto {
    /// Construct from a raw DEK (tests / internal use after bootstrap).
    pub fn from_dek(dek: [u8; 32]) -> Self {
        Self { dek }
    }

    /// Load or create the instance DEK, migrate legacy credentials, and optionally
    /// rewrap after `RAGDOLL_SECRET` rotation via `secret_old`.
    pub async fn bootstrap(pool: &DbPool, secret: &str, secret_old: Option<&str>) -> Result<Self> {
        let conn = pool
            .connect_one()
            .await
            .context("connect for crypto bootstrap")?;
        let scheme = meta_get(&conn, META_SCHEME).await?;

        if scheme.as_deref() == Some(SCHEME_ENVELOPE_V1) {
            let nonce = meta_get(&conn, META_DEK_NONCE)
                .await?
                .ok_or_else(|| anyhow!("crypto.dek_nonce missing for envelope-v1"))?;
            let ciphertext = meta_get(&conn, META_DEK_CIPHERTEXT)
                .await?
                .ok_or_else(|| anyhow!("crypto.dek_ciphertext missing for envelope-v1"))?;

            let kek = derive_kek(secret)?;
            match unwrap_dek(&kek, &nonce, &ciphertext) {
                Ok(dek) => {
                    if secret_old.is_some() {
                        tracing::warn!(
                            "RAGDOLL_SECRET_OLD is set but the current RAGDOLL_SECRET already \
                             unwraps the DEK; remove RAGDOLL_SECRET_OLD after a successful rotation"
                        );
                    }
                    return Ok(Self { dek });
                }
                Err(primary_err) => {
                    let Some(old) = secret_old.filter(|s| !s.is_empty()) else {
                        return Err(anyhow!(
                            "cannot unwrap credential DEK with RAGDOLL_SECRET ({primary_err}). \
                             If you rotated the secret, set RAGDOLL_SECRET_OLD to the previous \
                             value for one restart so the DEK can be rewrapped"
                        ));
                    };
                    let old_kek = derive_kek(old)?;
                    let dek = unwrap_dek(&old_kek, &nonce, &ciphertext).map_err(|e| {
                        anyhow!(
                            "cannot unwrap credential DEK with RAGDOLL_SECRET or \
                             RAGDOLL_SECRET_OLD ({e})"
                        )
                    })?;
                    let (new_nonce, new_ct) = wrap_dek(&kek, &dek)?;
                    meta_set(&conn, META_DEK_NONCE, &new_nonce).await?;
                    meta_set(&conn, META_DEK_CIPHERTEXT, &new_ct).await?;
                    tracing::info!("rewrapped DEK after secret rotation");
                    return Ok(Self { dek });
                }
            }
        }

        // Fresh install or upgrade from pre-envelope: create DEK and migrate.
        let mut dek = [0u8; 32];
        OsRng.fill_bytes(&mut dek);
        let kek = derive_kek(secret)?;
        let (nonce, ciphertext) = wrap_dek(&kek, &dek)?;
        meta_set(&conn, META_SCHEME, SCHEME_ENVELOPE_V1).await?;
        meta_set(&conn, META_DEK_NONCE, &nonce).await?;
        meta_set(&conn, META_DEK_CIPHERTEXT, &ciphertext).await?;

        let crypto = Self { dek };
        let migrated = migrate_legacy_credentials(&conn, secret, &crypto).await?;
        if migrated > 0 {
            tracing::info!(count = migrated, "migrated credentials to envelope-v1");
        } else {
            tracing::info!("initialized envelope-v1 credential encryption");
        }
        Ok(crypto)
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<(String, String)> {
        encrypt_with_key(&self.dek, plaintext)
    }

    pub fn decrypt(&self, nonce_b64: &str, ciphertext_b64: &str) -> Result<String> {
        decrypt_with_key(&self.dek, nonce_b64, ciphertext_b64)
    }
}

/// Decrypt an LLM credential key from the database.
pub async fn load_credential_key(
    conn: &libsql::Connection,
    crypto: &Crypto,
    credential_id: &str,
    release_id: &str,
) -> Result<String> {
    let mut rows = conn
        .query(
            "SELECT nonce, ciphertext FROM llm_credentials WHERE id = ?1 AND release_id = ?2",
            (credential_id, release_id),
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow!("llm credential not found"))?;
    let nonce: String = row.get(0)?;
    let ciphertext: String = row.get(1)?;
    crypto
        .decrypt(&nonce, &ciphertext)
        .context("decrypt llm credential")
}

fn derive_kek(secret: &str) -> Result<[u8; 32]> {
    derive_key(secret, KEK_INFO)
}

fn derive_legacy_credential_key(secret: &str) -> Result<[u8; 32]> {
    derive_key(secret, LEGACY_CREDENTIAL_KEY_INFO)
}

fn derive_key(secret: &str, info: &[u8]) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(None, secret.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(info, &mut key)
        .map_err(|e| anyhow!("derive key: {e}"))?;
    Ok(key)
}

fn wrap_dek(kek: &[u8; 32], dek: &[u8; 32]) -> Result<(String, String)> {
    encrypt_with_key(kek, &BASE64.encode(dek))
}

fn unwrap_dek(kek: &[u8; 32], nonce_b64: &str, ciphertext_b64: &str) -> Result<[u8; 32]> {
    let plain = decrypt_with_key(kek, nonce_b64, ciphertext_b64)?;
    let bytes = BASE64
        .decode(plain.as_bytes())
        .context("decode wrapped DEK")?;
    if bytes.len() != 32 {
        return Err(anyhow!("wrapped DEK has unexpected length {}", bytes.len()));
    }
    let mut dek = [0u8; 32];
    dek.copy_from_slice(&bytes);
    Ok(dek)
}

fn encrypt_with_key(key: &[u8; 32], plaintext: &str) -> Result<(String, String)> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).context("init cipher")?;
    let nonce = XNonce::generate();
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("encrypt: {e}"))?;
    Ok((BASE64.encode(nonce), BASE64.encode(ciphertext)))
}

fn decrypt_with_key(key: &[u8; 32], nonce_b64: &str, ciphertext_b64: &str) -> Result<String> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).context("init cipher")?;
    let nonce_bytes = BASE64.decode(nonce_b64).context("decode nonce")?;
    let nonce = XNonce::try_from(nonce_bytes.as_slice()).map_err(|_| anyhow!("invalid nonce"))?;
    let ciphertext = BASE64.decode(ciphertext_b64).context("decode ciphertext")?;
    let plaintext = cipher
        .decrypt(&nonce, ciphertext.as_ref())
        .map_err(|e| anyhow!("decrypt: {e}"))?;
    String::from_utf8(plaintext).context("plaintext is not utf-8")
}

/// Decrypt with the pre-envelope scheme (secret → HKDF → direct credential key).
pub fn legacy_decrypt(secret: &str, nonce_b64: &str, ciphertext_b64: &str) -> Result<String> {
    let key = derive_legacy_credential_key(secret)?;
    decrypt_with_key(&key, nonce_b64, ciphertext_b64)
}

/// Encrypt with the pre-envelope scheme (tests / seeding legacy rows).
pub fn legacy_encrypt(secret: &str, plaintext: &str) -> Result<(String, String)> {
    let key = derive_legacy_credential_key(secret)?;
    encrypt_with_key(&key, plaintext)
}

async fn migrate_legacy_credentials(
    conn: &libsql::Connection,
    secret: &str,
    crypto: &Crypto,
) -> Result<u32> {
    let mut rows = conn
        .query("SELECT id, nonce, ciphertext FROM llm_credentials", ())
        .await?;
    let mut migrated = 0u32;
    while let Some(row) = rows.next().await? {
        let id: String = row.get(0)?;
        let nonce: String = row.get(1)?;
        let ciphertext: String = row.get(2)?;
        match legacy_decrypt(secret, &nonce, &ciphertext) {
            Ok(plain) => {
                let (new_nonce, new_ct) = crypto.encrypt(&plain)?;
                conn.execute(
                    "UPDATE llm_credentials SET nonce = ?1, ciphertext = ?2, updated_at = datetime('now')
                     WHERE id = ?3",
                    (new_nonce.as_str(), new_ct.as_str(), id.as_str()),
                )
                .await?;
                migrated += 1;
            }
            Err(err) => {
                tracing::warn!(
                    credential_id = %id,
                    error = %err,
                    "skipping llm_credential during envelope migration (legacy decrypt failed)"
                );
            }
        }
    }
    Ok(migrated)
}

async fn meta_get(conn: &libsql::Connection, key: &str) -> Result<Option<String>> {
    let mut rows = conn
        .query("SELECT value FROM meta WHERE key = ?1", [key])
        .await?;
    Ok(match rows.next().await? {
        Some(row) => Some(row.get(0)?),
        None => None,
    })
}

async fn meta_set(conn: &libsql::Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )
    .await?;
    Ok(())
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
    use crate::config::Config;
    use crate::db::{migrations, DbPool};
    use tempfile::TempDir;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let mut dek = [0u8; 32];
        OsRng.fill_bytes(&mut dek);
        let crypto = Crypto::from_dek(dek);
        let (nonce, ciphertext) = crypto.encrypt("sk-test-api-key").unwrap();
        let plain = crypto.decrypt(&nonce, &ciphertext).unwrap();
        assert_eq!(plain, "sk-test-api-key");
    }

    #[test]
    fn wrong_dek_fails_decrypt() {
        let mut dek_a = [0u8; 32];
        let mut dek_b = [0u8; 32];
        OsRng.fill_bytes(&mut dek_a);
        OsRng.fill_bytes(&mut dek_b);
        let crypto = Crypto::from_dek(dek_a);
        let other = Crypto::from_dek(dek_b);
        let (nonce, ciphertext) = crypto.encrypt("key").unwrap();
        assert!(other.decrypt(&nonce, &ciphertext).is_err());
    }

    #[test]
    fn wrap_unwrap_dek_roundtrip() {
        let kek = derive_kek("test-secret").unwrap();
        let mut dek = [0u8; 32];
        OsRng.fill_bytes(&mut dek);
        let (nonce, ct) = wrap_dek(&kek, &dek).unwrap();
        let unwrapped = unwrap_dek(&kek, &nonce, &ct).unwrap();
        assert_eq!(unwrapped, dek);
    }

    #[test]
    fn wrong_kek_fails_unwrap() {
        let kek_a = derive_kek("secret-a").unwrap();
        let kek_b = derive_kek("secret-b").unwrap();
        let mut dek = [0u8; 32];
        OsRng.fill_bytes(&mut dek);
        let (nonce, ct) = wrap_dek(&kek_a, &dek).unwrap();
        assert!(unwrap_dek(&kek_b, &nonce, &ct).is_err());
    }

    #[test]
    fn legacy_encrypt_decrypt_roundtrip() {
        let (nonce, ct) = legacy_encrypt("legacy-secret", "sk-legacy").unwrap();
        let plain = legacy_decrypt("legacy-secret", &nonce, &ct).unwrap();
        assert_eq!(plain, "sk-legacy");
        assert!(legacy_decrypt("other", &nonce, &ct).is_err());
    }

    #[test]
    fn rewrap_with_new_kek() {
        let old_kek = derive_kek("old-secret").unwrap();
        let new_kek = derive_kek("new-secret").unwrap();
        let mut dek = [0u8; 32];
        OsRng.fill_bytes(&mut dek);
        let (nonce, ct) = wrap_dek(&old_kek, &dek).unwrap();
        let unwrapped = unwrap_dek(&old_kek, &nonce, &ct).unwrap();
        let (new_nonce, new_ct) = wrap_dek(&new_kek, &unwrapped).unwrap();
        assert_eq!(unwrap_dek(&new_kek, &new_nonce, &new_ct).unwrap(), dek);
        assert!(unwrap_dek(&old_kek, &new_nonce, &new_ct).is_err());
    }

    #[tokio::test]
    async fn bootstrap_creates_envelope_and_encrypts() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "boot-secret");
        config.ensure_directories().unwrap();
        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let crypto = Crypto::bootstrap(&pool, "boot-secret", None).await.unwrap();
        let (nonce, ct) = crypto.encrypt("sk-x").unwrap();
        assert_eq!(crypto.decrypt(&nonce, &ct).unwrap(), "sk-x");

        let conn = pool.connect_one().await.unwrap();
        assert_eq!(
            meta_get(&conn, META_SCHEME).await.unwrap().as_deref(),
            Some(SCHEME_ENVELOPE_V1)
        );

        // Second boot is idempotent.
        let crypto2 = Crypto::bootstrap(&pool, "boot-secret", None).await.unwrap();
        assert_eq!(crypto2.decrypt(&nonce, &ct).unwrap(), "sk-x");
    }

    #[tokio::test]
    async fn bootstrap_migrates_legacy_credentials() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "migrate-secret");
        config.ensure_directories().unwrap();
        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let conn = pool.connect_one().await.unwrap();
        conn.execute(
            "INSERT INTO releases (id, tag, message) VALUES ('r1', 'rel', '')",
            (),
        )
        .await
        .unwrap();
        let (nonce, ct) = legacy_encrypt("migrate-secret", "sk-legacy-key").unwrap();
        conn.execute(
            "INSERT INTO llm_credentials (id, release_id, name, provider, nonce, ciphertext)
             VALUES ('c1', 'r1', 'openai', 'openai', ?1, ?2)",
            (nonce.as_str(), ct.as_str()),
        )
        .await
        .unwrap();
        drop(conn);

        let crypto = Crypto::bootstrap(&pool, "migrate-secret", None)
            .await
            .unwrap();
        let conn = pool.connect_one().await.unwrap();
        let mut rows = conn
            .query(
                "SELECT nonce, ciphertext FROM llm_credentials WHERE id = 'c1'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let n: String = row.get(0).unwrap();
        let c: String = row.get(1).unwrap();
        assert_eq!(crypto.decrypt(&n, &c).unwrap(), "sk-legacy-key");
        // Legacy key must no longer decrypt the row.
        assert!(legacy_decrypt("migrate-secret", &n, &c).is_err());
    }

    #[tokio::test]
    async fn bootstrap_rewraps_on_secret_rotation() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "old-secret");
        config.ensure_directories().unwrap();
        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let crypto = Crypto::bootstrap(&pool, "old-secret", None).await.unwrap();
        let (nonce, ct) = crypto.encrypt("sk-keep").unwrap();

        let rotated = Crypto::bootstrap(&pool, "new-secret", Some("old-secret"))
            .await
            .unwrap();
        assert_eq!(rotated.decrypt(&nonce, &ct).unwrap(), "sk-keep");

        let again = Crypto::bootstrap(&pool, "new-secret", None).await.unwrap();
        assert_eq!(again.decrypt(&nonce, &ct).unwrap(), "sk-keep");
    }

    #[tokio::test]
    async fn bootstrap_fails_wrong_secret_without_old() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "correct");
        config.ensure_directories().unwrap();
        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();
        Crypto::bootstrap(&pool, "correct", None).await.unwrap();
        let err = match Crypto::bootstrap(&pool, "wrong", None).await {
            Ok(_) => panic!("expected bootstrap to fail with wrong secret"),
            Err(e) => e,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("RAGDOLL_SECRET_OLD") || msg.contains("unwrap"),
            "unexpected error: {msg}"
        );
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
