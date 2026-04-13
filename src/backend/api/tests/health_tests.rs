#![cfg(test)]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
};
use common::token::generate_api_token;
use data::api_token::NewApiToken;
use repos::api_token::ApiTokenRepo;
use testware::setup::TestSetup;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;

use api::state::AppState;
use api::{routes::routes, worker::TestWorker};
use repos::Repo;
use testware::create_settings;

async fn setup(db: &surrealdb::Surreal<surrealdb::engine::any::Any>) -> Router {
    let settings = create_settings();
    let repo = Repo::new(db.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let worker = Arc::new(TestWorker::new());

    let settings = Arc::new(settings);
    let state = AppState {
        repo,
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

#[tokio::test]
async fn test_health_live_ok() {
    let db = TestSetup::create_db().await;
    let app = setup(&db).await;

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

#[tokio::test]
async fn test_health_ready_ok() {
    let db = TestSetup::create_db().await;
    let (token_id, _token, token_hash) =
        generate_api_token().expect("Failed to generate API token");
    let new_token = NewApiToken {
        description: "Default API token".to_string(),
        token_id,
        token_hash,
        product_id: None,
        user_id: None,
        entitlements: vec!["token".to_string()],
        expires_at: None,
        is_active: true,
    };
    ApiTokenRepo::create(&db, new_token)
        .await
        .expect("Failed to create API token");

    let app = setup(&db).await;

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
