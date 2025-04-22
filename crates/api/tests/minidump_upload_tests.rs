#![cfg(test)]

use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use bytes::Bytes;
use chrono::Utc;
use object_store::ObjectStore;
use object_store::path::Path;
use repos::attachment::AttachmentsRepo;
use repos::crash::CrashRepo;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;

use api::routes::routes;
use api::state::AppState;
use api::worker::TestMinidumpProcessor;
use common::QueryParams;
use common::token::generate_api_token;
use data::api_token::NewApiToken;
use repos::Repo;
use repos::api_token::ApiTokenRepo;
use repos::product::ProductRepo;
use repos::symbols::SymbolsRepo;

use testware::{
    create_settings, create_test_product_with_details, create_test_token, create_test_version,
    create_webauthn,
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
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"minidump_file\"; filename=\"test.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--{}--\r\n",
        boundary, content, boundary
    );

    let (token, _) = create_test_token(pool, "Test Token", None, None, &["minidump-upload"]).await;

    let product =
        create_test_product_with_details(pool, "TestProduct", "Test product description").await;
    let _version =
        create_test_version(pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

    (app, store, boundary.to_owned(), content.to_owned(), body, token)
}

async fn get_object(store: Arc<dyn ObjectStore>, path: &str) -> Bytes {
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
    let (app, store, boundary, content, body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_ok(response).await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 1);
    let crash = &crashes[0];
    assert!(crash.minidump.is_some());
    assert_eq!(crash.state, data::crash::State::Pending);

    let path = format!("minidumps/{}", crashes[0].minidump.unwrap());
    let object = get_object(store, &path).await;
    assert_eq!(object, Bytes::from(content));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_with_attachments_ok(pool: PgPool) {
    let (app, store, boundary, content, _body, token) = setup(&pool).await;

    let attachment1_content = "LOG DATA 1";
    let attachment2_content = "LOG DATA 2";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"minidump_file\"; filename=\"test.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n\
         --{}\r\nContent-Disposition: form-data; name=\"attachment1\"; filename=\"log.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n\
         --{}\r\nContent-Disposition: form-data; name=\"attachment2\"; filename=\"trace.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n\
         --{}--\r\n",
        boundary, content, boundary, attachment1_content, boundary, attachment2_content, boundary
    );

    log::info!("Body: {}", body);
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_ok(response).await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 1);
    let crash = &crashes[0];
    assert!(crash.minidump.is_some());
    assert_eq!(crash.state, data::crash::State::Pending);

    let attachments = AttachmentsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch attachments from database");

    assert_eq!(attachments.len(), 2);
    let attachment1 = &attachments[0];
    let attachment2 = &attachments[1];
    assert_eq!(attachment1.name, "log.txt");
    assert_eq!(attachment2.name, "trace.txt");

    let path = format!("attachments/{}", attachments[0].filename);
    let object = get_object(store.clone(), &path).await;
    assert_eq!(object, Bytes::from(attachment1_content));

    let path = format!("attachments/{}", attachments[1].filename);
    let object = get_object(store.clone(), &path).await;
    assert_eq!(object, Bytes::from(attachment2_content));

    let path = format!("minidumps/{}", crashes[0].minidump.unwrap());
    let object = get_object(store, &path).await;
    assert_eq!(object, Bytes::from(content));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_such_product(pool: PgPool) {
    let (app, _store, boundary, _content, body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProductxx&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("product TestProductxx not found"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_such_version(pool: PgPool) {
    let (app, _store, boundary, _content, body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=2.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("version 2.0.0 of product TestProduct not found"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty_version(pool: PgPool) {
    let (app, _store, boundary, _content, body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: version cannot be empty"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty_product(pool: PgPool) {
    let (app, _store, boundary, _content, body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: product name cannot be empty"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_content_type(pool: PgPool) {
    let (app, _store, boundary, content, _body, token) = setup(&pool).await;

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"minidump_file\"; filename=\"test.sym\"\r\nContent-Type: text/octet-stream\r\n\r\n{}\r\n--{}--\r\n",
        boundary, content, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid minidump content type: text/octet-stream"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_multipart(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";
    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"minidump_file\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--{}--\r\n",
        boundary2, content, boundary2
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: failed to read multipart field from upload"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_boundary(pool: PgPool) {
    let (app, _store, boundary, content, _body, token) = setup(&pool).await;

    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"minidump_file\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--{}--\r\n",
        boundary, content, boundary2
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::INTERNAL_SERVER_ERROR, Some("internal failure"))
        .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_wrong_entitlement(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

    let (token, _) = create_test_token(&pool, "Wrong", None, None, &["token"]).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::FORBIDDEN, Some("insufficient permissions")).await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_expired_entitlement(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

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
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::UNAUTHORIZED,
        Some("API token is expired or inactive"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_inactive_entitlement(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

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
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::UNAUTHORIZED,
        Some("API token is expired or inactive"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_other_product(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

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
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::FORBIDDEN,
        Some("access denied for product TestProduct"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_unknown_token(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_tokenx"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::UNAUTHORIZED, Some("invalid API token")).await;

    let crashes = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("failed to fetch symbol entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_token(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::UNAUTHORIZED, Some("missing API token")).await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_version(pool: PgPool) {
    let (app, _store, boundary, _content, body, token) = setup(&pool).await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("Failed to deserialize query string: missing field `version`"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");
    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_product(pool: PgPool) {
    let (app, _store, boundary, _content, body, token) = setup(&pool).await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("Failed to deserialize query string: missing field `product`"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(""))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: failed to read multipart field from upload"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_wrong_name(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MINIDUMP DATA";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"xminidump_file\"; filename=\"test.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--{}--\r\n",
        boundary, content, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: expect crash as first document"),
    )
    .await;

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 0);
}
