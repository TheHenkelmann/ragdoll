mod support;

use axum::http::StatusCode;

use support::{json_request, seed_demo_chunk, seed_llm_setup, setup_test_app};

async fn api_key_token(app: &support::TestApp) -> String {
    app.create_api_key_token("generation-tests").await
}

#[tokio::test]
async fn sync_generation_returns_answer() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    seed_llm_setup(&app, "00000000-0000-0000-0000-000000000001").await;
    let token = api_key_token(&app).await;

    let body = r#"[{"text":"RAG pipeline","top_k":3,"rerank":false,"generation":{"stream":false,"tag":"mock-model","system_prompt":"Answer briefly."}}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["items"][0]["status"], 200);
    assert!(json["items"][0]["result"]["answer"]["text"]
        .as_str()
        .unwrap()
        .starts_with("Mock answer:"));
    assert!(json["items"][0]["result"]["latency"]["generation_ms"].is_number());
    assert!(json["items"][0]["result"]["usage"]["prompt_tokens"].is_number());
    assert!(!json["items"][0]["result"]["answer"]
        .as_object()
        .unwrap()
        .contains_key("generation_ms"));
}

#[tokio::test]
async fn streaming_generation_requires_single_item() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    seed_llm_setup(&app, "00000000-0000-0000-0000-000000000001").await;
    let token = api_key_token(&app).await;

    let body = r#"[{"text":"RAG","generation":{"stream":true,"tag":"mock-model"}},{"text":"other","generation":{"stream":true,"tag":"mock-model"}}]"#;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn generation_without_tag_fails_per_item() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    seed_llm_setup(&app, "00000000-0000-0000-0000-000000000001").await;
    let token = api_key_token(&app).await;

    let body = r#"[{"text":"RAG","generation":{"stream":false}}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["items"][0]["error"]["detail"]
        .as_str()
        .unwrap()
        .contains("generation.tag is required"));
}

#[tokio::test]
async fn generation_with_unknown_tag_fails_per_item() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    let token = api_key_token(&app).await;

    let body = r#"[{"text":"RAG","generation":{"stream":false,"tag":"missing-model","system_prompt":"Answer briefly."}}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["items"][0]["error"]["detail"]
        .as_str()
        .unwrap()
        .contains("llm model"));
}

#[tokio::test]
async fn generation_without_system_prompt_fails_per_item() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    seed_llm_setup(&app, "00000000-0000-0000-0000-000000000001").await;
    let token = api_key_token(&app).await;

    let body = r#"[{"text":"RAG","generation":{"stream":false,"tag":"mock-model"}}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["items"][0]["error"]["detail"]
        .as_str()
        .unwrap()
        .contains("generation.system_prompt is required"));
}
