//! Integration tests for envelope encryption bootstrap and secret rotation.

use std::path::PathBuf;
use std::sync::Arc;

use ragdoll::api::router::build_state_with_provider;
use ragdoll::config::Config;
use ragdoll::crypto::{legacy_encrypt, Crypto};
use ragdoll::db::{migrations, DbPool};
use ragdoll::generation::MockGenerator;
use ragdoll::models::{MockModelProvider, ModelProvider};

async fn migrate_pool(config: &Config) -> DbPool {
    config.ensure_directories().unwrap();
    let pool = DbPool::connect_path(&config.db_path).await.unwrap();
    migrations::run_migrations(&pool, &config.migrations_dir)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn legacy_credentials_migrate_on_first_bootstrap() {
    let dir = tempfile::tempdir().unwrap();
    let secret = "legacy-integration-secret";
    let mut config = Config::for_test(dir.path().to_path_buf(), secret);
    config.migrations_dir = PathBuf::from("migrations");
    let pool = migrate_pool(&config).await;

    let release_id = "00000000-0000-0000-0000-000000000001";
    let (nonce, ct) = legacy_encrypt(secret, "sk-from-legacy").unwrap();
    let conn = pool.connect_one().await.unwrap();
    conn.execute(
        "INSERT INTO llm_credentials (id, release_id, name, provider, nonce, ciphertext)
         VALUES ('cred-legacy', ?1, 'openai', 'openai', ?2, ?3)",
        (release_id, nonce.as_str(), ct.as_str()),
    )
    .await
    .unwrap();
    drop(conn);

    let models: Arc<dyn ModelProvider> = Arc::new(MockModelProvider);
    let generator: Arc<dyn ragdoll::generation::Generator> = Arc::new(MockGenerator::default());
    let state = build_state_with_provider(config.clone(), pool.clone(), models, generator, vec![])
        .await
        .expect("bootstrap should migrate legacy credentials");

    let conn = state.pool.connect_one().await.unwrap();
    let mut rows = conn
        .query(
            "SELECT nonce, ciphertext FROM llm_credentials WHERE id = 'cred-legacy'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let n: String = row.get(0).unwrap();
    let c: String = row.get(1).unwrap();
    assert_eq!(state.crypto.decrypt(&n, &c).unwrap(), "sk-from-legacy");

    let mut meta = conn
        .query("SELECT value FROM meta WHERE key = 'crypto.scheme'", ())
        .await
        .unwrap();
    let scheme: String = meta.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(scheme, "envelope-v1");
}

#[tokio::test]
async fn secret_rotation_with_old_preserves_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::for_test(dir.path().to_path_buf(), "rotate-old");
    config.migrations_dir = PathBuf::from("migrations");
    let pool = migrate_pool(&config).await;

    let crypto = Crypto::bootstrap(&pool, "rotate-old", None).await.unwrap();
    let (nonce, ct) = crypto.encrypt("sk-survive-rotation").unwrap();
    let release_id = "00000000-0000-0000-0000-000000000001";
    let conn = pool.connect_one().await.unwrap();
    conn.execute(
        "INSERT INTO llm_credentials (id, release_id, name, provider, nonce, ciphertext)
         VALUES ('cred-rot', ?1, 'openai', 'openai', ?2, ?3)",
        (release_id, nonce.as_str(), ct.as_str()),
    )
    .await
    .unwrap();
    drop(conn);

    config.secret = "rotate-new".to_string();
    config.secret_old = Some("rotate-old".to_string());
    let models: Arc<dyn ModelProvider> = Arc::new(MockModelProvider);
    let generator: Arc<dyn ragdoll::generation::Generator> = Arc::new(MockGenerator::default());
    let state = build_state_with_provider(config.clone(), pool.clone(), models, generator, vec![])
        .await
        .expect("rewrap with SECRET_OLD");

    let conn = state.pool.connect_one().await.unwrap();
    let mut rows = conn
        .query(
            "SELECT nonce, ciphertext FROM llm_credentials WHERE id = 'cred-rot'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let n: String = row.get(0).unwrap();
    let c: String = row.get(1).unwrap();
    assert_eq!(state.crypto.decrypt(&n, &c).unwrap(), "sk-survive-rotation");

    // After rewrap, new secret alone is enough.
    config.secret_old = None;
    let models2: Arc<dyn ModelProvider> = Arc::new(MockModelProvider);
    let generator2: Arc<dyn ragdoll::generation::Generator> = Arc::new(MockGenerator::default());
    let state2 = build_state_with_provider(config, pool, models2, generator2, vec![])
        .await
        .expect("boot with new secret only");
    assert_eq!(
        state2.crypto.decrypt(&n, &c).unwrap(),
        "sk-survive-rotation"
    );
}

#[tokio::test]
async fn wrong_secret_without_old_fails_bootstrap() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::for_test(dir.path().to_path_buf(), "good-secret");
    config.migrations_dir = PathBuf::from("migrations");
    let pool = migrate_pool(&config).await;
    Crypto::bootstrap(&pool, "good-secret", None).await.unwrap();

    config.secret = "bad-secret".to_string();
    let models: Arc<dyn ModelProvider> = Arc::new(MockModelProvider);
    let generator: Arc<dyn ragdoll::generation::Generator> = Arc::new(MockGenerator::default());
    let err = build_state_with_provider(config, pool, models, generator, vec![])
        .await
        .err()
        .expect("bootstrap must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("RAGDOLL_SECRET_OLD") || msg.contains("unwrap"),
        "unexpected: {msg}"
    );
}
