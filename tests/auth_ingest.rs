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
    conn.execute("DELETE FROM api_keys", ()).await.unwrap();

    let (status, _) = json_request(&app, "GET", "/api/v1/releases", None, Some(&token)).await;
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
async fn post_text_source_creates_pending_job_without_source_row() {
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
    let mut source_rows = conn
        .query("SELECT 1 FROM sources WHERE id = ?1", [source_id])
        .await
        .unwrap();
    assert!(source_rows.next().await.unwrap().is_none());

    let mut job_rows = conn
        .query(
            "SELECT status, source_name, source_type FROM ingest_jobs WHERE id = ?1",
            [job_id],
        )
        .await
        .unwrap();
    let job = job_rows.next().await.unwrap().unwrap();
    let job_status: String = job.get(0).unwrap();
    let source_name: String = job.get(1).unwrap();
    let source_type: String = job.get(2).unwrap();
    assert_eq!(job_status, "pending");
    assert_eq!(source_name, "readme");
    assert_eq!(source_type, "text");
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
    let body = format!(r#"[{{"type":"file","name":"notes.txt","content":"{encoded}"}}]"#);
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
async fn get_sources_lists_committed_source() {
    let app = setup_test_app().await;
    support::seed_demo_chunk(&app.state).await;

    let (status, json) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/sources?limit=10",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["name"] == "demo"));
}

#[tokio::test]
async fn post_replace_targets_existing_source_without_deleting() {
    use sha2::{Digest, Sha256};

    let app = setup_test_app().await;
    support::seed_demo_chunk(&app.state).await;
    let source_id = "00000000-0000-0000-0000-000000000099";
    let content = "Ragdoll is a local RAG pipeline for retrieval.";
    let content_hash = format!("{:x}", Sha256::digest(content.as_bytes()));

    let conn = app.state.pool.connect_one().await.unwrap();
    conn.execute(
        "UPDATE sources SET content_hash = ?1 WHERE id = ?2",
        (content_hash.as_str(), source_id),
    )
    .await
    .unwrap();

    let body = format!(r#"[{{"type":"text","name":"duplicate","content":"{content}"}}]"#);
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/sources",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["items"][0]["status"], 200);
    assert_eq!(
        json["items"][0]["result"]["source_id"].as_str().unwrap(),
        source_id
    );
    assert!(!json["items"][0]["result"]["job_id"]
        .as_str()
        .unwrap()
        .is_empty());

    let mut chunk_rows = conn
        .query("SELECT COUNT(*) FROM chunks WHERE source_id = ?1", [source_id])
        .await
        .unwrap();
    let chunk_count: i64 = chunk_rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(chunk_count, 1);
}

#[tokio::test]
async fn put_enqueues_job_without_deleting_existing_source() {
    let app = setup_test_app().await;
    support::seed_demo_chunk(&app.state).await;
    let source_id = "00000000-0000-0000-0000-000000000099";

    let body = format!(
        r#"[{{"id":"{source_id}","type":"text","name":"updated","content":"new content"}}]"#
    );
    let (status, json) = json_request(
        &app,
        "PUT",
        "/api/v1/releases/first-release/sources",
        Some(body),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["items"][0]["status"], 200);
    assert!(!json["items"][0]["result"]["job_id"]
        .as_str()
        .unwrap()
        .is_empty());

    let conn = app.state.pool.connect_one().await.unwrap();
    let mut source_rows = conn
        .query("SELECT name FROM sources WHERE id = ?1", [source_id])
        .await
        .unwrap();
    let row = source_rows.next().await.unwrap().unwrap();
    let name: String = row.get(0).unwrap();
    assert_eq!(name, "demo");

    let mut chunk_rows = conn
        .query("SELECT COUNT(*) FROM chunks WHERE source_id = ?1", [source_id])
        .await
        .unwrap();
    let chunk_count: i64 = chunk_rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(chunk_count, 1);
}

fn pct_encode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![b as char]
            }
            _ => format!("%{b:02X}").chars().collect::<Vec<_>>(),
        })
        .collect()
}

#[tokio::test]
async fn delete_source_with_id_filter() {
    let app = setup_test_app().await;
    support::seed_demo_chunk(&app.state).await;
    let source_id = "00000000-0000-0000-0000-000000000099".to_string();

    let filter = pct_encode(&format!(
        r#"{{"field":"id","op":"eq","value":"{source_id}"}}"#
    ));
    let (status, deleted) = json_request(
        &app,
        "DELETE",
        &format!("/api/v1/releases/first-release/sources?filter={filter}"),
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["deleted"], true);

    let conn = app.state.pool.connect_one().await.unwrap();
    let mut rows = conn
        .query("SELECT 1 FROM sources WHERE id = ?1", [source_id.as_str()])
        .await
        .unwrap();
    assert!(rows.next().await.unwrap().is_none());
}
