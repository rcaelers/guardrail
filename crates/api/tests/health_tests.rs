#![cfg(test)]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
};
use sqlx::PgPool;
use testware::{create_settings, create_webauthn};
use tower::ServiceExt;

use api::state::AppState;
use api::{routes::routes, worker::TestMinidumpProcessor};
use repos::Repo;
use tower_http::trace::TraceLayer;

async fn setup(pool: &PgPool) -> Router {
    let settings = create_settings();
    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let worker = Arc::new(TestMinidumpProcessor::new());

    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store.clone(),
        worker: worker.clone(),
    };

    let app: Router = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    app
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_health_live_ok(pool: PgPool) {
    let app = setup(&pool).await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/live")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert!(body.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_health_ready_ok(pool: PgPool) {
    let app = setup(&pool).await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/ready")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert!(body.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_health_ready_not_ok(pool: PgPool) {
    let app = setup(&pool).await;

    pool.close().await;
    let request = Request::builder()
        .method("GET")
        .uri("/api/ready")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert!(body.is_empty());
}
