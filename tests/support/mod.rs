#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use ragdoll::api::router::{build_router, build_state_with_provider, AppState};
use ragdoll::auth::{encode_api_key_token, encode_session_token, ensure_superadmin, hash_password};
use ragdoll::config::Config;
use ragdoll::crypto::format_api_key_token;
use ragdoll::db::{migrations, DbPool};
use ragdoll::generation::MockGenerator;
use ragdoll::models::{Embedder, MockEmbedder, MockModelProvider, ModelProvider};
use tempfile::TempDir;

pub struct TestApp {
    pub router: axum::Router,
    pub state: Arc<AppState>,
    pub token: String,
    pub _dir: TempDir,
}

pub async fn setup_test_app() -> TestApp {
    let dir = tempfile::tempdir().expect("tempdir");
    let static_dir = dir.path().join("static");
    std::fs::create_dir_all(static_dir.join("assets")).expect("static assets dir");
    std::fs::write(static_dir.join("index.html"), "<html></html>").expect("index.html");
    std::fs::write(static_dir.join("assets/favicon.ico"), "").expect("favicon");

    let mut config = Config::for_test(dir.path().to_path_buf(), "test-jwt-secret");
    config.static_dir = static_dir;
    config.migrations_dir = PathBuf::from("migrations");
    config.ensure_directories().expect("ensure dirs");

    let pool = DbPool::connect_path(&config.db_path)
        .await
        .expect("connect db");
    migrations::run_migrations(&pool, &config.migrations_dir)
        .await
        .expect("migrate");
    ensure_superadmin(&pool, &config)
        .await
        .expect("bootstrap admin");

    let models: Arc<dyn ModelProvider> = Arc::new(MockModelProvider);
    let generator: Arc<dyn ragdoll::generation::Generator> = Arc::new(MockGenerator::default());
    let state = build_state_with_provider(config.clone(), pool, models, generator, vec![])
        .await
        .expect("build state");
    let router = build_router(state.clone());

    let conn = state.pool.connect_one().await.expect("conn");
    let mut rows = conn
        .query("SELECT id FROM users WHERE is_superadmin = 1 LIMIT 1", ())
        .await
        .expect("query admin");
    let row = rows.next().await.expect("next").expect("admin row");
    let user_id: String = row.get(0).expect("user id");
    let token = encode_session_token(&config.secret, &user_id, "admin@ragdoll.ai", true, 3600)
        .expect("session token");

    TestApp {
        router,
        state,
        token,
        _dir: dir,
    }
}

impl TestApp {
    pub fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub fn bearer(&self, token: &str) -> String {
        format!("Bearer {token}")
    }

    pub async fn create_api_key_token(&self, name: &str) -> String {
        let key_id = uuid::Uuid::new_v4().to_string();
        let conn = self.state.pool.connect_one().await.expect("conn");
        let all_perms = r#"["sources:read","sources:write","sources:delete","chunks:read","chunks:write","chunks:delete","queries:run","queries:read","queries:delete","playground:run","playground:read","db:read","settings:read","settings:write","llm_models:read","llm_models:write","llm_models:delete","llm_credentials:read","llm_credentials:write","llm_credentials:delete","analytics:read","releases:read","releases:write","releases:delete","stages:read","stages:write","stages:delete","models:read","models:download","models:delete","backups:read","backups:create","backups:upload","backups:download","backups:restore","backups:delete","users:read","users:write","users:delete","api_keys:read","api_keys:write","api_keys:delete","webhooks:read","webhooks:write","webhooks:delete"]"#;
        conn.execute(
            "INSERT INTO api_keys (id, name, permissions) VALUES (?1, ?2, ?3)",
            (key_id.as_str(), name, all_perms),
        )
        .await
        .expect("insert api key");
        format_api_key_token(
            &encode_api_key_token(
                &self.state.config.secret,
                &key_id,
                name,
                "2024-01-01T00:00:00Z",
            )
            .expect("api key token"),
        )
    }

    pub async fn create_user_token(&self, email: &str, password: &str, superadmin: bool) -> String {
        let user_id = uuid::Uuid::new_v4().to_string();
        let hash = hash_password(password).expect("hash password");
        let conn = self.state.pool.connect_one().await.expect("conn");
        conn.execute(
            "INSERT INTO users (id, email, password_hash, is_superadmin, password_is_default, permissions)
             VALUES (?1, ?2, ?3, ?4, 0, '[]')",
            (
                user_id.as_str(),
                email,
                hash.as_str(),
                if superadmin { 1i64 } else { 0i64 },
            ),
        )
        .await
        .expect("insert user");
        encode_session_token(&self.state.config.secret, &user_id, email, superadmin, 3600)
            .expect("session token")
    }
}

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

pub async fn json_request(
    app: &TestApp,
    method: &str,
    uri: &str,
    body: Option<String>,
    auth_token: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = auth_token {
        builder = builder.header("authorization", app.bearer(token));
    }
    let payload = body.unwrap_or_default();
    if !payload.is_empty() || matches!(method, "POST" | "PUT" | "PATCH") {
        builder = builder.header("content-type", "application/json");
    }
    let response = app
        .router
        .clone()
        .oneshot(builder.body(Body::from(payload)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    if status == StatusCode::NO_CONTENT {
        return (status, serde_json::Value::Null);
    }
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::String(
            String::from_utf8_lossy(&bytes).into_owned(),
        ))
    };
    (status, json)
}

pub async fn seed_demo_chunk(state: &AppState) {
    let release_id = "00000000-0000-0000-0000-000000000001";
    let source_id = "00000000-0000-0000-0000-000000000099";
    let chunk_id = "00000000-0000-0000-0000-000000000098";
    let content = "Ragdoll is a local RAG pipeline for retrieval.";

    let embedder = MockEmbedder;
    let vector = embedder.embed_one(content).await.expect("embed");
    let vector_json = serde_json::to_string(&vector).expect("vector json");

    let conn = state.pool.connect_one().await.expect("conn");
    conn.execute(
        "INSERT OR REPLACE INTO sources (id, release_id, name, type, status, metadata, config)
         VALUES (?1, ?2, 'demo', 'text', 'completed', '{}', '{}')",
        (source_id, release_id),
    )
    .await
    .expect("insert source");
    conn.execute(
        "INSERT OR REPLACE INTO chunks (
            id, release_id, source_id, ordinal, content, metadata, provenance, embedding,
            embedding_model, embedding_dim, embedding_version
         ) VALUES (?1, ?2, ?3, 0, ?4, '{}', '[]', vector32(?5), 'BAAI/bge-m3', 1024, '1')",
        (
            chunk_id,
            release_id,
            source_id,
            content,
            vector_json.as_str(),
        ),
    )
    .await
    .expect("insert chunk");
}

pub async fn seed_llm_setup(app: &TestApp, release_id: &str) -> (String, String) {
    use ragdoll::crypto::Crypto;

    let crypto = Crypto::from_secret(&app.state.config.secret).unwrap();
    let (nonce, ciphertext) = crypto.encrypt("sk-test").unwrap();
    let cred_id = uuid::Uuid::new_v4().to_string();
    let model_id = uuid::Uuid::new_v4().to_string();

    let conn = app.state.pool.connect_one().await.expect("conn");
    conn.execute(
        "INSERT INTO llm_credentials (id, release_id, name, provider, nonce, ciphertext) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (
            cred_id.as_str(),
            release_id,
            "test-openai",
            "openai",
            nonce.as_str(),
            ciphertext.as_str(),
        ),
    )
    .await
    .expect("insert credential");
    conn.execute(
        "INSERT INTO llm_models (
            id, release_id, tag, model_name, provider, credential_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (
            model_id.as_str(),
            release_id,
            "mock-model",
            "gpt-test",
            "openai",
            cred_id.as_str(),
        ),
    )
    .await
    .expect("insert model");
    (cred_id, model_id)
}
