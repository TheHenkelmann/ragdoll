mod support;

use axum::http::StatusCode;

use support::{json_request, seed_llm_setup, setup_test_app};

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

    let (status, list) =
        json_request(&app, "GET", "/api/v1/releases", None, Some(&app.token)).await;
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
async fn fork_release_copies_llm_credentials_and_models() {
    let app = setup_test_app().await;
    let release_id = "00000000-0000-0000-0000-000000000001";
    let (source_cred_id, _source_model_id) = seed_llm_setup(&app, release_id).await;

    let body =
        r#"{"tag":"forked-llm","message":"","init":{"type":"fork","source":"first-release"}}"#;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/releases",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, credentials) = json_request(
        &app,
        "GET",
        "/api/v1/releases/forked-llm/llm_credentials",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let creds = credentials.as_array().unwrap();
    assert_eq!(creds.len(), 1);
    assert_eq!(creds[0]["name"], "test-openai");
    assert_ne!(creds[0]["id"].as_str().unwrap(), source_cred_id);

    let (status, models) = json_request(
        &app,
        "GET",
        "/api/v1/releases/forked-llm/llm_models",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let model_list = models.as_array().unwrap();
    assert_eq!(model_list.len(), 1);
    assert_eq!(model_list[0]["tag"], "mock-model");
    assert_eq!(
        model_list[0]["credential_id"].as_str().unwrap(),
        creds[0]["id"].as_str().unwrap()
    );

    let (status, test_result) = json_request(
        &app,
        "POST",
        "/api/v1/releases/forked-llm/llm_models/mock-model/test",
        None,
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(test_result["ok"], true);
    assert!(test_result["message"].is_string());
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
        Some(r#"{"email":"newuser@example.com","password":"Secret123!@#Pass","permissions":["sources:read"]}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let user_id = created["id"].as_str().unwrap();

    let (status, users) = json_request(&app, "GET", "/api/v1/users", None, Some(&app.token)).await;
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
        Some(r#"{"name":"ci-key","permissions":["sources:read"]}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let key_id = created["id"].as_str().unwrap();
    assert!(created["token"].is_string());

    let (status, keys) =
        json_request(&app, "GET", "/api/v1/api_keys", None, Some(&app.token)).await;
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
async fn api_key_rejects_duplicate_name() {
    let app = setup_test_app().await;
    let body = r#"{"name":"ci-key","permissions":["sources:read"]}"#;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/api_keys",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, err) = json_request(
        &app,
        "POST",
        "/api/v1/api_keys",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["detail"].as_str().unwrap().contains("already exists"));
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
async fn webhook_urls_are_unique_per_release() {
    let app = setup_test_app().await;
    let body = r#"{"type":"ingest_status","url":"https://example.com/hook","events":["completed","failed"],"active":true}"#;
    let (status, _) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/webhooks",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, err) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/webhooks",
        Some(body.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["detail"]
        .as_str()
        .unwrap()
        .contains("webhook url already exists"));
}

#[tokio::test]
async fn webhook_patch_rejects_duplicate_url_in_same_release() {
    let app = setup_test_app().await;
    let (status, first) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/webhooks",
        Some(r#"{"type":"ingest_status","url":"https://example.com/first"}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["id"].is_string());

    let (status, second) = json_request(
        &app,
        "POST",
        "/api/v1/releases/first-release/webhooks",
        Some(r#"{"type":"ingest_status","url":"https://example.com/second"}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let second_id = second["id"].as_str().unwrap();

    let (status, err) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/releases/first-release/webhooks/{second_id}"),
        Some(r#"{"url":"https://example.com/first"}"#.into()),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["detail"]
        .as_str()
        .unwrap()
        .contains("webhook url already exists"));
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
    assert!(json.get("columns").and_then(|v| v.as_array()).is_some());
    assert!(json.get("rows").and_then(|v| v.as_array()).is_some());
    assert!(json.get("facets").and_then(|v| v.as_object()).is_some());
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

#[tokio::test]
async fn backup_create_list_and_delete() {
    let app = setup_test_app().await;

    let (status, list) =
        json_request(&app, "GET", "/api/v1/backups", None, Some(&app.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list["backups"].is_array());

    let (status, created) =
        json_request(&app, "POST", "/api/v1/backups", None, Some(&app.token)).await;
    assert_eq!(status, StatusCode::OK);
    let file_name = created["file_name"].as_str().unwrap();
    assert!(file_name.starts_with("ragdoll-"));
    assert!(file_name.ends_with("-manual.db"));

    let (status, list_after) =
        json_request(&app, "GET", "/api/v1/backups", None, Some(&app.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        list_after["backups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["file_name"] == file_name)
    );

    let (status, deleted) = json_request(
        &app,
        "DELETE",
        "/api/v1/backups/delete",
        Some(format!(r#"{{"file_name":"{file_name}"}}"#)),
        Some(&app.token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted["file_name"], file_name);
}
