#![cfg(test)]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
};
use common::token::decode_api_token;
use jsonwebtoken::{Algorithm, Validation};
use sqlx::PgPool;
use testware::{
    create_settings, create_test_product_with_details, create_test_token, create_test_user,
    create_webauthn,
};
use tower::ServiceExt;

use api::state::AppState;
use api::{routes::routes, worker::TestMinidumpProcessor};
use repos::Repo;
use tower_http::trace::TraceLayer;

async fn setup(pool: &PgPool) -> (Router, AppState) {
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

    (app, state)
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_token_jwt_admin_ok(pool: PgPool) {
    let (app, state) = setup(&pool).await;

    let (token, _) = create_test_token(&pool, "Test Token", None, None, &["token"]).await;

    let request = Request::builder()
        .method("POST")
        .header("Authorization", format!("Bearer {token}"))
        .uri("/api/auth/jwt")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!response_json["token"].as_str().unwrap().is_empty());

    let jwt = response_json["token"].as_str().unwrap();
    let public_key = state.settings.auth.jwk.public_key.clone();

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_exp = true;
    validation.validate_nbf = false;
    validation.validate_aud = false;

    let decoded_jwt = jsonwebtoken::decode::<serde_json::Value>(
        jwt,
        &jsonwebtoken::DecodingKey::from_ed_pem(public_key.as_bytes()).unwrap(),
        &validation,
    )
    .unwrap();

    assert_eq!(decoded_jwt.claims["aud"].as_str().unwrap(), "guardrail");
    assert_eq!(decoded_jwt.claims["sub"].as_str().unwrap(), "admin");
    assert_eq!(decoded_jwt.claims["username"].as_str().unwrap(), "admin");
    assert_eq!(decoded_jwt.claims["iss"].as_str().unwrap(), state.settings.auth.id);
    assert_eq!(decoded_jwt.claims["role"].as_str().unwrap(), "guardrail_apiuser");
    assert!(decoded_jwt.claims["exp"].is_number());
    assert!(decoded_jwt.claims["iat"].is_number());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_token_jwt_user_ok(pool: PgPool) {
    let (app, state) = setup(&pool).await;

    let user = create_test_user(&pool, "testuser", false).await;

    let (token, _) = create_test_token(&pool, "Test Token", None, Some(user.id), &["token"]).await;

    let request = Request::builder()
        .method("POST")
        .header("Authorization", format!("Bearer {token}"))
        .uri("/api/auth/jwt")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!response_json["token"].as_str().unwrap().is_empty());

    let jwt = response_json["token"].as_str().unwrap();
    let public_key = state.settings.auth.jwk.public_key.clone();

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_exp = true;
    validation.validate_nbf = false;
    validation.validate_aud = false;

    let decoded_jwt = jsonwebtoken::decode::<serde_json::Value>(
        jwt,
        &jsonwebtoken::DecodingKey::from_ed_pem(public_key.as_bytes()).unwrap(),
        &validation,
    )
    .unwrap();

    assert_eq!(decoded_jwt.claims["aud"].as_str().unwrap(), "guardrail");
    assert_eq!(decoded_jwt.claims["sub"].as_str().unwrap(), user.username);
    assert_eq!(decoded_jwt.claims["username"].as_str().unwrap(), user.username);
    assert_eq!(decoded_jwt.claims["iss"].as_str().unwrap(), state.settings.auth.id);
    assert_eq!(decoded_jwt.claims["role"].as_str().unwrap(), "guardrail_apiuser");
    assert!(decoded_jwt.claims["exp"].is_number());
    assert!(decoded_jwt.claims["iat"].is_number());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_token_jwt_invalid_token(pool: PgPool) {
    let (app, _state) = setup(&pool).await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;

    let (token_nok, _) = create_test_token(
        &pool,
        "Test Minidump Token",
        Some(product.id),
        None,
        &["minidump-upload"],
    )
    .await;

    let request = Request::builder()
        .method("POST")
        .header("Authorization", format!("Bearer {token_nok}"))
        .uri("/api/auth/jwt")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["error"], "insufficient permissions");
    assert_eq!(response_json["result"], "failed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_token_ok(pool: PgPool) {
    let (app, _state) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/token")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let token = response_json
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let token_hash = response_json
        .get("token_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let token_id = response_json
        .get("token_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let (id, hash) = decode_api_token(token).unwrap();
    assert_eq!(id.to_string(), token_id);
    assert!(common::token::verify_api_secret(&hash, token_hash).unwrap_or(false));
}
