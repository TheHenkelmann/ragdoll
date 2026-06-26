mod support;

use axum::http::StatusCode;

use support::{json_request, seed_demo_chunk, setup_test_app};

#[tokio::test]
async fn query_with_rerank_returns_rerank_scores() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;

    let body = r#"[{"text":"RAG pipeline retrieval","top_k":3,"rerank":true}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let result = &json["items"][0]["result"];
    assert_eq!(json["items"][0]["status"], 200);
    let matches = result["matches"].as_array().unwrap();
    assert!(!matches.is_empty());
    assert!(matches[0]["rerank_score"].is_number());
}

#[tokio::test]
async fn query_respects_top_k() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;

    let body = r#"[{"text":"RAG","top_k":1,"rerank":false}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let matches = json["items"][0]["result"]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
}

#[tokio::test]
async fn query_playground_mode_returns_all_candidates() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;

    let body = r#"[{"text":"RAG pipeline","top_k":1,"rerank":true}]"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries?playground=true",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let matches = json["items"][0]["result"]["matches"].as_array().unwrap();
    assert!(!matches.is_empty());
}

#[tokio::test]
async fn get_queries_after_search() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    let body = r#"[{"text":"RAG","top_k":3,"rerank":false}]"#;
    json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&app.token),
    )
    .await;

    let (status, json) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/queries?limit=5",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_query_detail_includes_chunks() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    let body = r#"[{"text":"local RAG pipeline","top_k":3,"rerank":false}]"#;
    let (_, posted) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/queries",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    let query_id = posted["items"][0]["result"]["query_id"].as_str().unwrap();

    let (status, json) = json_request(
        &app,
        "GET",
        &format!("/api/v1/releases/first-release/queries/{query_id}"),
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], query_id);

    let conn = app.state.pool.connect_one().await.unwrap();
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM query_chunks WHERE query_id = ?1",
            [query_id],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let count: i64 = row.get(0).unwrap();
    assert!(count > 0, "expected query_chunks rows for {query_id}");
}

#[tokio::test]
async fn get_query_detail_not_found() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/queries/missing-id",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_queries_requires_filter() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "DELETE",
        "/api/v1/releases/first-release/queries",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_chunk_updates_content() {
    let app = setup_test_app().await;
    seed_demo_chunk(&app.state).await;
    let chunk_id = "00000000-0000-0000-0000-000000000098";
    let body = r#"{"content":"updated chunk text"}"#;
    let (status, json) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/releases/first-release/chunks/{chunk_id}"),
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["content"], "updated chunk text");
}
