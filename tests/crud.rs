mod support;

use axum::http::StatusCode;

use support::{json_request, setup_test_app};

#[tokio::test]
async fn create_and_list_release() {
    let app = setup_test_app().await;
    let body = r#"{"tag":"v2","message":"second","init":{"type":"new"}}"#;
    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/releases",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tag"], "v2");

    let (status, list) = json_request(
        &app,
        "GET",
        "/api/v1/releases",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().iter().any(|r| r["tag"] == "v2"));
}

#[tokio::test]
async fn fork_release_copies_settings() {
    let app = setup_test_app().await;
    let body = r#"{"tag":"forked","message":"","init":{"type":"fork","source":"first-release"}}"#;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/releases",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, settings) = json_request(
        &app,
        "GET",
        "/api/v1/releases/forked/settings",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settings["embedding_model"], "BAAI/bge-m3");
}

#[tokio::test]
async fn rename_release() {
    let app = setup_test_app().await;
    let create = r#"{"tag":"rename-me","message":"","init":{"type":"new"}}"#;
    json_request(
        &app,
        "POST",
        "/api/v1/releases",
        Some(create.into()),
        Some(&app.token),
    )
    .await;

    let (status, json) = json_request(
        &app,
        "PATCH",
        "/api/v1/releases/rename-me",
        Some(r#"{"tag":"renamed"}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tag"], "renamed");
}

#[tokio::test]
async fn create_stage_points_at_release() {
    let app = setup_test_app().await;
    let create = r#"{"tag":"stage-release","message":"","init":{"type":"new"}}"#;
    json_request(
        &app,
        "POST",
        "/api/v1/releases",
        Some(create.into()),
        Some(&app.token),
    )
    .await;

    let (status, json) = json_request(
        &app,
        "POST",
        "/api/v1/stages",
        Some(r#"{"tag":"staging","release_tag":"stage-release"}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tag"], "staging");
    assert_eq!(json["release_tag"], "stage-release");
}

#[tokio::test]
async fn create_and_delete_user() {
    let app = setup_test_app().await;
    let (status, created) = json_request(
        &app,
        "POST",
        "/api/v1/users",
        Some(r#"{"email":"newuser@example.com","password":"secret123"}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let user_id = created["id"].as_str().unwrap();

    let (status, users) = json_request(
        &app,
        "GET",
        "/api/v1/users",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(users.as_array().unwrap().iter().any(|u| u["id"] == user_id));

    let (status, _) = json_request(
        &app,
        "DELETE",
        &format!("/api/v1/users/{user_id}"),
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn api_key_lifecycle() {
    let app = setup_test_app().await;
    let (status, created) = json_request(
        &app,
        "POST",
        "/api/v1/api_keys",
        Some(r#"{"name":"ci-key"}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let key_id = created["id"].as_str().unwrap();
    assert!(created["token"].is_string());

    let (status, keys) = json_request(
        &app,
        "GET",
        "/api/v1/api_keys",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(keys.as_array().unwrap().iter().any(|k| k["id"] == key_id));

    let (status, _) = json_request(
        &app,
        "DELETE",
        &format!("/api/v1/api_keys/{key_id}"),
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn db_viewer_rejects_api_keys_table() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/db/api_keys",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn db_viewer_lists_sources_table() {
    let app = setup_test_app().await;
    let (status, json) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/db/sources?limit=5",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.is_array());
}

#[tokio::test]
async fn db_viewer_rejects_unknown_table() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "GET",
        "/api/v1/releases/first-release/db/not_a_table",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn download_model_rejects_non_whitelisted_name() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/models/unknown-model/download",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unknown_release_returns_not_found() {
    let app = setup_test_app().await;
    let (status, _) = json_request(
        &app,
        "GET",
        "/api/v1/releases/does-not-exist/settings",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
