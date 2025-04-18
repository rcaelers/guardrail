#![cfg(test)]

use std::sync::Arc;

use api::routes::routes;
use api::state::AppState;
use api::worker::TestMinidumpProcessor;
use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use common::QueryParams;
use repos::Repo;
use repos::crash::CrashRepo;
use sqlx::PgPool;
use tower::ServiceExt;

use testware::{
    create_settings, create_test_product_with_details, create_test_token, create_test_version,
    create_webauthn,
};
use tower_http::trace::TraceLayer;

async fn setup(pool: PgPool) -> (Router, String) {
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());

    let worker = Arc::new(TestMinidumpProcessor::new());

    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
        worker,
    };
    let app: Router = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

    let (token, _) =
        create_test_token(&pool, "Test Token", Some(product.id), None, &["minidump-upload"]).await;

    (app, token)
}

fn create_body() -> (String, String) {
    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"test.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\nMINIDUMP DATA\r\n--{}--\r\n",
        boundary, boundary
    );

    (boundary.to_owned(), body)
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_ok(pool: PgPool) {
    let (app, token) = setup(pool.clone()).await;
    let (boundary, body) = create_body();
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap(); // Set limit to 1 MB
    tracing::info!("Response body: {:?}", body);
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["result"], "ok");
    assert!(response_json["crash_id"].is_string());

    let crashes = CrashRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch crash entry from database");

    assert_eq!(crashes.len(), 1);
    let crash = &crashes[0];
    assert!(crash.minidump.is_some());
    assert_eq!(crash.state, data::crash::State::Pending);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_product(pool: PgPool) {
    let (app, token) = setup(pool.clone()).await;
    let (boundary, body) = create_body();

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    tracing::info!("Response body: {:?}", body);
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["result"], "failed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_version(pool: PgPool) {
    let (app, token) = setup(pool.clone()).await;
    let (boundary, body) = create_body();

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["result"], "failed");
}
