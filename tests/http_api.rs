mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use support::{json_request, setup_test_app};

#[tokio::test]
async fn health_is_public() {
    let app = setup_test_app().await;
    let response = app
        .router
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn protected_route_requires_auth() {
    let app = setup_test_app().await;
    let response = app
        .router
        .oneshot(
            Request::builder()
                .uri("/api/v1/releases")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_returns_token() {
    let app = setup_test_app().await;
    let body = r#"{"email":"admin@ragdoll.ai","password":"admin"}"#;
    let response = app
        .router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.get("token").and_then(|v| v.as_str()).is_some());
}

#[tokio::test]
async fn list_releases_with_session_token() {
    let app = setup_test_app().await;
    let response = app
        .router
        .oneshot(
            Request::builder()
                .uri("/api/v1/releases")
                .header("authorization", format!("Bearer {}", app.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["tag"] == "first-release"));
}

#[tokio::test]
async fn stage_plane_write_forbidden_for_session_token() {
    let app = setup_test_app().await;
    let body = r#"[{"type":"text","name":"demo","content":"hello"}]"#;
    let response = app
        .router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/stages/prod/sources")
                .header("authorization", format!("Bearer {}", app.token))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn query_returns_matches_with_mock_embedder() {
    let app = setup_test_app().await;
    support::seed_demo_chunk(&app.state).await;
    let token = app.create_api_key_token("query-test").await;

    let body = r#"[{"text":"local RAG pipeline","top_k":5,"rerank":false}]"#;
    let response = app
        .router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/releases/first-release/queries")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["status"], 200);
    let matches = items[0]["result"]["matches"].as_array().unwrap();
    assert!(!matches.is_empty());
}

#[tokio::test]
async fn health_returns_ready_payload() {
    let app = setup_test_app().await;
    let (status, json) = json_request(&app, "GET", "/api/v1/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["ready"], true);
    assert!(json["embedding_mismatch_count"].is_number());
}

#[tokio::test]
async fn auth_info_is_public() {
    let app = setup_test_app().await;
    let (status, json) = json_request(&app, "GET", "/api/v1/auth/info", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["default_admin_email"], "admin@ragdoll.ai");
}

#[tokio::test]
async fn auth_status_requires_session() {
    let app = setup_test_app().await;
    let (status, json) =
        json_request(&app, "GET", "/api/v1/auth/status", None, Some(&app.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["email"], "admin@ragdoll.ai");
    assert_eq!(json["is_superadmin"], true);
}

#[tokio::test]
async fn login_rejects_invalid_email() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/auth/login",
        Some(r#"{"email":"bad","password":"x"}"#.into()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_stages_includes_prod() {
    let app = setup_test_app().await;
    let (status, json) = json_request(&app, "GET", "/api/v1/stages", None, Some(&app.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.as_array().unwrap().iter().any(|s| s["tag"] == "prod"));
}

#[tokio::test]
async fn get_models_lists_whitelisted_models() {
    let app = setup_test_app().await;
    let (status, json) = json_request(&app, "GET", "/api/v1/models", None, Some(&app.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["embedding_dim"], 1024);
    let names: Vec<_> = json["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["name"].as_str())
        .collect();
    assert!(names.contains(&"BAAI/bge-m3"));
}

#[tokio::test]
async fn get_release_settings_returns_defaults() {
    let app = setup_test_app().await;
    let (status, json) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/settings",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["chunking_strategy"], "semantic_split");
    assert_eq!(json["embedding_model"], "BAAI/bge-m3");
    assert_eq!(json["rerank_max_length"], 256);
}

#[tokio::test]
async fn patch_release_settings_updates_cache() {
    let app = setup_test_app().await;
    let body = r#"{"sentence_buffer":4}"#;
    let (status, json) = json_request(
        &app,
        "PATCH",
        "/api/v1/releases/first-release/settings",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["sentence_buffer"], 4);
}

#[tokio::test]
async fn analytics_for_release_is_available() {
    let app = setup_test_app().await;
    let (status, json) = json_request(
        &app,
        "GET",
        "/api/v1/analytics?lens=release&tag=first-release&days=14",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["request_count"].is_number());
    assert!(json["daily_requests"].is_array());
}

#[tokio::test]
async fn list_chunks_after_seed() {
    let app = setup_test_app().await;
    support::seed_demo_chunk(&app.state).await;
    let (status, json) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/chunks?limit=10",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!json.as_array().unwrap().is_empty());
}
