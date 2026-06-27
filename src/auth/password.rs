// SPDX-License-Identifier: AGPL-3.0-only

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| anyhow::anyhow!("hash password: {e}"))
}

pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("parse hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Strong password policy: min 12 chars, upper, lower, digit, symbol.
pub fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < 12 {
        return Err("password must be at least 12 characters".into());
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err("password must include an uppercase letter".into());
    }
    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        return Err("password must include a lowercase letter".into());
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("password must include a number".into());
    }
    if !password.chars().any(|c| !c.is_ascii_alphanumeric()) {
        return Err("password must include a symbol".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_password() {
        let hash = hash_password("ragdoll-test-password").unwrap();
        assert!(verify_password("ragdoll-test-password", &hash).unwrap());
        assert!(!verify_password("wrong-password", &hash).unwrap());
    }

    #[test]
    fn validate_password_enforces_policy() {
        assert!(validate_password("Short1!").is_err());
        assert!(validate_password("longpassword1!").is_err());
        assert!(validate_password("LongPassword!!").is_err());
        assert!(validate_password("Secret123!@#Pass").is_ok());
    }
}
