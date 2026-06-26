use ragdoll::db::{migrations, DbPool};
use ragdoll::settings::SettingsCache;

#[tokio::test]
async fn migrations_seed_release_and_settings() {
    let dir = tempfile::tempdir().unwrap();
    let config = ragdoll::Config::for_test(dir.path().to_path_buf(), "secret");
    config.ensure_directories().unwrap();
    let pool = DbPool::connect_path(&config.db_path).await.unwrap();
    migrations::run_migrations(&pool, &config.migrations_dir)
        .await
        .unwrap();

    let conn = pool.connect_one().await.unwrap();
    let mut rows = conn
        .query("SELECT tag FROM releases WHERE id = ?1", [
            "00000000-0000-0000-0000-000000000001",
        ])
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let tag: String = row.get(0).unwrap();
    assert_eq!(tag, "first-release");

    let cache = SettingsCache::new();
    let settings = cache
        .get_or_load(&pool, "00000000-0000-0000-0000-000000000001")
        .await
        .unwrap();
    assert_eq!(settings.embedding_model, "BAAI/bge-m3");
    assert_eq!(settings.max_batch_size, 100);
}
