mod support;

use axum::http::StatusCode;

use support::{json_request, setup_test_app};

#[tokio::test]
async fn invalid_bearer_token_returns_unauthorized() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "GET",
        "/api/v1/releases",
        None,
        Some("not-a-valid-jwt"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoked_api_key_returns_unauthorized() {
    let app = setup_test_app().await;
    let token = app.create_api_key_token("temp-key").await;
    let conn = app.state.pool.connect_one().await.unwrap();
    conn.execute("DELETE FROM api_keys", ())
        .await
        .unwrap();

    let (status, _) = json_request(
        &app,
        "GET",
        "/api/v1/releases",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_key_can_query_stage_plane() {
    let app = setup_test_app().await;
    support::seed_demo_chunk(&app.state).await;
    let token = app.create_api_key_token("stage-reader").await;
    let body = r#"[{"text":"RAG pipeline","top_k":3,"rerank":false}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/stages/prod/queries",
        Some(body.into()),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["items"][0]["status"], 200);
}

#[tokio::test]
async fn non_superadmin_cannot_post_sources() {
    let app = setup_test_app().await;
    let token = app
        .create_user_token("user@example.com", "secret", false)
        .await;
    let body = r#"[{"type":"text","name":"demo","content":"hello"}]"#;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/sources",
        Some(body.into()),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_text_source_creates_pending_job() {
    let app = setup_test_app().await;
    let body = r#"[{"type":"text","name":"readme","content":"ingest me"}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/sources",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let item = &json["items"][0];
    assert_eq!(item["status"], 200);
    let source_id = item["result"]["source_id"].as_str().unwrap();
    let job_id = item["result"]["job_id"].as_str().unwrap();

    let conn = app.state.pool.connect_one().await.unwrap();
    let mut rows = conn
        .query(
            "SELECT status FROM sources WHERE id = ?1",
            [source_id],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let status: String = row.get(0).unwrap();
    assert_eq!(status, "pending");

    let mut job_rows = conn
        .query("SELECT status FROM ingest_jobs WHERE id = ?1", [job_id])
        .await
        .unwrap();
    let job = job_rows.next().await.unwrap().unwrap();
    let job_status: String = job.get(0).unwrap();
    assert_eq!(job_status, "pending");
}

#[tokio::test]
async fn post_source_rejects_invalid_type() {
    let app = setup_test_app().await;
    let body = r#"[{"type":"ftp","name":"x","content":"y"}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/sources",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["items"][0]["status"], 400);
}

#[tokio::test]
async fn post_source_rejects_missing_text_content() {
    let app = setup_test_app().await;
    let body = r#"[{"type":"text","name":"empty"}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/sources",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["items"][0]["status"], 400);
}

#[tokio::test]
async fn post_file_source_writes_staging_file() {
    let app = setup_test_app().await;
    let encoded = "ZmlsZSBib2R5";
    let body = format!(
        r#"[{{"type":"file","name":"notes.txt","content":"{encoded}"}}]"#
    );
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/sources",
        Some(body),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["items"][0]["status"], 200);
    let source_id = json["items"][0]["result"]["source_id"].as_str().unwrap();
    let staging = app
        .state
        .config
        .staging_dir
        .join(format!("{source_id}.txt"));
    assert!(staging.exists());
}

#[tokio::test]
async fn put_source_requires_id() {
    let app = setup_test_app().await;
    let body = r#"[{"type":"text","name":"demo","content":"hello"}]"#;
    let (status, json) = json_request(
        &app,
        "PUT",
        "/api/v1/releases/first-release/sources",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["items"][0]["status"], 400);
}

#[tokio::test]
async fn get_sources_lists_created_source() {
    let app = setup_test_app().await;
    let body = r#"[{"type":"text","name":"listed","content":"visible"}]"#;
    json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/sources",
        Some(body.into()),
        Some(&app.token),
    )
    .await;

    let (status, json) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/sources?limit=10",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.as_array().unwrap().iter().any(|s| s["name"] == "listed"));
}
