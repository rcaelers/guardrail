#![cfg(test)]

use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use bytes::Bytes;
use chrono::Utc;
use futures::TryStreamExt;
use object_store::ObjectStore;
use object_store::path::Path;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;
use tracing::info;

use api::routes::routes;
use api::state::AppState;
use api::worker::TestMinidumpProcessor;
use common::token::generate_api_token;
use data::api_token::NewApiToken;
use repos::Repo;
use repos::api_token::ApiTokenRepo;
use repos::product::ProductRepo;

use testware::{
    create_settings, create_test_product_with_details, create_test_token, create_webauthn,
};

async fn setup(pool: &PgPool) -> (Router, Arc<dyn ObjectStore>, String, String, String, String) {
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let worker = Arc::new(TestMinidumpProcessor::new());

    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store.clone(),
        worker,
    };
    let app: Router = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let content = "MINIDUMP DATA";

    // Create a body that includes all required fields
    let body = create_body(boundary, Some("TestProduct"), Some("1.0.0"), None);

    let product =
        create_test_product_with_details(pool, "TestProduct", "Test product description").await;

    let (token, _) =
        create_test_token(pool, "Test Token", Some(product.id), None, &["minidump-upload"]).await;

    (app, store, boundary.to_owned(), content.to_owned(), body, token)
}

pub fn create_body(
    boundary: &str,
    product: Option<&str>,
    version: Option<&str>,
    extra: Option<String>,
) -> String {
    let content = "MINIDUMP DATA";

    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"test.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\n{content}\r\n"
    );

    if let Some(product) = product {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"product\"\r\nContent-Type: text/plain\r\n\r\n{product}\r\n"
        );
    }

    if let Some(version) = version {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"version\"\r\nContent-Type: text/plain\r\n\r\n{version}\r\n"
        );
    }

    body = format!(
        "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"channel\"\r\nContent-Type: text/plain\r\n\r\ntest-channel\r\n"
    );
    body = format!(
        "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"commit\"\r\nContent-Type: text/plain\r\n\r\ntest-commit\r\n"
    );
    body = format!(
        "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"build_id\"\r\nContent-Type: text/plain\r\n\r\n2025-05-15T20:26:15+02:00\r\n"
    );

    if let Some(extra) = extra {
        body = format!("{body}{extra}");
    }

    body = format!("{body}--{boundary}--\r\n");

    body
}

async fn _get_object(store: Arc<dyn ObjectStore>, path: &str) -> Bytes {
    let object = store
        .get(&Path::from(path))
        .await
        .expect("Failed to get minidump object");
    object
        .bytes()
        .await
        .expect("Failed to read minidump object")
}

async fn assert_response_ok(response: axum::http::Response<Body>) -> serde_json::Value {
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["result"], "ok");
    response_json
}

async fn assert_response_error(
    response: axum::http::Response<Body>,
    status_code: StatusCode,
    error_message: Option<&str>,
) -> serde_json::Value {
    assert_eq!(response.status(), status_code);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        response_json,
        json!({ "result": "failed", "error": error_message.unwrap_or("general failure") })
    );
    response_json
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_ok(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body(&boundary, Some("TestProduct"), Some("1.0.0"), None);
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let result = assert_response_ok(response).await;

    let crash_id = result["crash_id"].as_str().unwrap();
    info!("Crash ID: {}", crash_id);

    let crash_info = store
        .get(&Path::from(format!("crashes/{crash_id}.json")))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    let crash_info: serde_json::Value =
        serde_json::from_slice(&crash_info).expect("Failed to parse crash info JSON");

    assert_eq!(crash_info["product"].as_str().unwrap(), "TestProduct");
    assert_eq!(crash_info["version"].as_str().unwrap(), "1.0.0");
    assert_eq!(crash_info["channel"].as_str().unwrap(), "test-channel");
    assert_eq!(crash_info["commit"].as_str().unwrap(), "test-commit");
    assert_eq!(crash_info["build_id"].as_str().unwrap(), "2025-05-15T20:26:15+02:00");
    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 0);
    assert_eq!(crash_info["attachments"].as_array().unwrap().len(), 0);

    let minidump = crash_info["minidump"]["storage_path"]
        .as_str()
        .expect("minidump_id is missing");
    let minidump = store
        .get(&Path::from(minidump))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    assert_eq!(minidump, Bytes::from("MINIDUMP DATA"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_with_attachments_ok(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let attachment1_content = "LOG DATA 1";
    let attachment2_content = "LOG DATA 2";
    let body = create_body(
        &boundary,
        Some("TestProduct"),
        Some("1.0.0"),
        Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"attachment1\"; filename=\"log1.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment1_content}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"attachment2\"; filename=\"log2.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment2_content}\r\n"
        )),
    );

    log::info!("Body: {body}");
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let result = assert_response_ok(response).await;

    let crash_id = result["crash_id"].as_str().unwrap();
    info!("Crash ID: {}", crash_id);

    let crash_info = store
        .get(&Path::from(format!("crashes/{crash_id}.json")))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    let crash_info: serde_json::Value =
        serde_json::from_slice(&crash_info).expect("Failed to parse crash info JSON");

    assert_eq!(crash_info["product"].as_str().unwrap(), "TestProduct");
    assert_eq!(crash_info["version"].as_str().unwrap(), "1.0.0");
    assert_eq!(crash_info["channel"].as_str().unwrap(), "test-channel");
    assert_eq!(crash_info["commit"].as_str().unwrap(), "test-commit");
    assert_eq!(crash_info["build_id"].as_str().unwrap(), "2025-05-15T20:26:15+02:00");
    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 0);
    assert_eq!(crash_info["attachments"].as_array().unwrap().len(), 2);

    let minidump = crash_info["minidump"]["storage_path"]
        .as_str()
        .expect("minidump_id is missing");
    let minidump = store
        .get(&Path::from(minidump))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    assert_eq!(minidump, Bytes::from("MINIDUMP DATA"));

    let attachment = crash_info["attachments"].as_array().unwrap();
    for (i, att) in attachment.iter().enumerate() {
        let storage_path = att["storage_path"]
            .as_str()
            .expect("storage_path is missing");
        let filename = att["filename"].as_str().expect("filename is missing");
        assert_eq!(filename, format!("log{}.txt", i + 1));
        let name = att["name"].as_str().expect("name is missing");
        assert_eq!(name, format!("attachment{}", i + 1));
        let object = store
            .get(&Path::from(storage_path))
            .await
            .expect("Failed to get attachment object")
            .bytes()
            .await
            .expect("Failed to read attachment object");
        assert_eq!(object, Bytes::from(format!("LOG DATA {}", i + 1)));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_with_annotations_ok(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body(
        &boundary,
        Some("TestProduct"),
        Some("1.0.0"),
        Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"features\"; \r\nContent-Type: text/plain\r\n\r\ntracing\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"ui\"; \r\nContent-Type: text/plain\r\n\r\nQt\r\n"
        )),
    );

    log::info!("Body: {body}");
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let result = assert_response_ok(response).await;

    let crash_id = result["crash_id"].as_str().unwrap();
    info!("Crash ID: {}", crash_id);

    let crash_info = store
        .get(&Path::from(format!("crashes/{crash_id}.json")))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    let crash_info: serde_json::Value =
        serde_json::from_slice(&crash_info).expect("Failed to parse crash info JSON");

    assert_eq!(crash_info["product"].as_str().unwrap(), "TestProduct");
    assert_eq!(crash_info["version"].as_str().unwrap(), "1.0.0");
    assert_eq!(crash_info["channel"].as_str().unwrap(), "test-channel");
    assert_eq!(crash_info["commit"].as_str().unwrap(), "test-commit");
    assert_eq!(crash_info["build_id"].as_str().unwrap(), "2025-05-15T20:26:15+02:00");
    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 2);
    assert_eq!(crash_info["attachments"].as_array().unwrap().len(), 0);

    let minidump = crash_info["minidump"]["storage_path"]
        .as_str()
        .expect("minidump_id is missing");
    let minidump = store
        .get(&Path::from(minidump))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    assert_eq!(minidump, Bytes::from("MINIDUMP DATA"));

    let annotations = crash_info["annotations"].as_object().unwrap();
    assert_eq!(annotations["features"].as_str().unwrap(), "tracing");
    assert_eq!(annotations["ui"].as_str().unwrap(), "Qt");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_such_product(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body(&boundary, Some("TestProductxx"), Some("1.0.0"), None);

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("product TestProductxx not found"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty_version(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body(&boundary, Some("TestProduct"), Some(""), None);

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: required annotation 'version' cannot be empty"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty_product(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body(&boundary, Some(""), Some("1.0.0"), None);
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: required annotation 'product' cannot be empty"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_content_type(pool: PgPool) {
    let (app, store, boundary, _content, body, token) = setup(&pool).await;

    let body = body.replace("application/octet-stream", "text/octet-stream");

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid annotation content type: text/octet-stream"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_multipart(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";
    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";

    let body = format!(
        "--{boundary2}\r\nContent-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\n{content}\r\n--{boundary2}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: failed to read multipart field from upload"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_boundary(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";
    let body = create_body(boundary2, Some("TestProduct"), Some("1.0.0"), None);

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: failed to read multipart field from upload"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_wrong_entitlement(pool: PgPool) {
    let (app, store, boundary, _content, body, _token) = setup(&pool).await;

    let (token, _) = create_test_token(&pool, "Wrong", None, None, &["token"]).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::FORBIDDEN, Some("insufficient permissions")).await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_expired_entitlement(pool: PgPool) {
    let (app, store, boundary, _content, body, _token) = setup(&pool).await;

    let product = ProductRepo::get_by_name(&pool, "TestProduct")
        .await
        .expect("Failed to retrieve product")
        .expect("Product not found");

    let (token_id, token, token_hash) = generate_api_token().expect("Failed to generate API token");

    let new_token = NewApiToken {
        description: "Test API token".to_string(),
        token_id,
        token_hash,
        product_id: Some(product.id),
        user_id: None,
        entitlements: vec!["symbol-upload".to_string()],
        expires_at: Some((Utc::now() - chrono::Duration::days(1)).naive_utc()),
        is_active: true,
    };
    ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to insert test API token");

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::UNAUTHORIZED,
        Some("API token is expired or inactive"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_inactive_entitlement(pool: PgPool) {
    let (app, store, boundary, _content, body, _token) = setup(&pool).await;

    let product = ProductRepo::get_by_name(&pool, "TestProduct")
        .await
        .expect("Failed to retrieve product")
        .expect("Product not found");

    let (token_id, token, token_hash) = generate_api_token().expect("Failed to generate API token");
    let new_token = NewApiToken {
        description: "Test API token".to_string(),
        token_id,
        token_hash,
        product_id: Some(product.id),
        user_id: None,
        entitlements: vec!["symbol-upload".to_string()],
        expires_at: Some((Utc::now() + chrono::Duration::days(1)).naive_utc()),
        is_active: false,
    };
    ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to insert test API token");

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::UNAUTHORIZED,
        Some("API token is expired or inactive"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_other_product(pool: PgPool) {
    let (app, store, boundary, _content, body, _token) = setup(&pool).await;

    let product = create_test_product_with_details(&pool, "AnotherProduct", "description").await;

    let (token_id, token, token_hash) = generate_api_token().expect("Failed to generate API token");
    let new_token = NewApiToken {
        description: "Test API token".to_string(),
        token_id,
        token_hash,
        product_id: Some(product.id),
        user_id: None,
        entitlements: vec!["minidump-upload".to_string()],
        expires_at: Some((Utc::now() + chrono::Duration::days(1)).naive_utc()),
        is_active: true,
    };
    ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to insert test API token");

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::FORBIDDEN,
        Some("access denied for product TestProduct"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_unknown_token(pool: PgPool) {
    let (app, store, boundary, _content, body, _token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {}", "test_tokenx"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::UNAUTHORIZED, Some("invalid API token")).await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_token(pool: PgPool) {
    let (app, store, boundary, _content, body, _token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::UNAUTHORIZED, Some("missing API token")).await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_version(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body(&boundary, Some("TestProduct"), None, None);
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: required annotation 'version' is missing"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_product(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body(&boundary, None, Some("1.0.0"), None);

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: required annotation 'product' is missing"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty(pool: PgPool) {
    let (app, store, boundary, _content, _body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(""))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: failed to read multipart field from upload"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_wrong_name(pool: PgPool) {
    let (app, store, boundary, _content, body, token) = setup(&pool).await;

    let body = body.replace("upload_file_minidump", "xupload_file_minidump");

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: no minidump found in submission"),
    )
    .await;

    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), 0);
}
