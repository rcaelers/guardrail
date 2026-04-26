#![cfg(test)]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
};
use jsonwebtoken::{Algorithm, Validation};
use testware::setup::TestSetup;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;

use api::state::AppState;
use api::{routes::routes, worker::TestWorker};
use common::token::decode_api_token;
use repos::Repo;
use testware::{
    create_settings, create_test_product_with_details, create_test_token, create_test_user,
};

async fn setup(db: &surrealdb::Surreal<surrealdb::engine::any::Any>) -> (Router, AppState) {
    let mut settings = create_settings();
    settings.database.namespace = "test".to_string();
    settings.database.database = "test".to_string();
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

    (app, state)
}

async fn assert_jwt_authenticates(state: &AppState, jwt: &str, username: &str, is_admin: bool) {
    let db = state
        .repo
        .authenticated(jwt)
        .await
        .expect("JWT should authenticate with SurrealDB");
    let mut result = db
        .query("RETURN fn::auth::username(); RETURN fn::auth::is_admin();")
        .await
        .expect("auth helper query should run");
    let actual_username: Option<String> = result.take(0).expect("username result should decode");
    let actual_is_admin: Option<bool> = result.take(1).expect("is_admin result should decode");

    assert_eq!(actual_username.as_deref(), Some(username));
    assert_eq!(actual_is_admin, Some(is_admin));
}

#[tokio::test]
async fn test_token_jwt_admin_ok() {
    let db = TestSetup::create_db().await;
    let (app, state) = setup(&db).await;

    let (token, _) = create_test_token(&db, "Test Token", None, None, &["token"]).await;

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
    assert!(decoded_jwt.claims["user_id"].is_null());
    assert!(decoded_jwt.claims["is_admin"].as_bool().unwrap());
    assert_eq!(decoded_jwt.claims["iss"].as_str().unwrap(), state.settings.auth.id);
    assert!(decoded_jwt.claims.get("role").is_none());
    assert!(decoded_jwt.claims["exp"].is_number());
    assert!(decoded_jwt.claims["iat"].is_number());
    // SurrealDB record-access claims
    assert_eq!(decoded_jwt.claims["ac"].as_str().unwrap(), "guardrail_api");
    assert_eq!(decoded_jwt.claims["ns"].as_str().unwrap(), state.settings.database.namespace);
    assert_eq!(decoded_jwt.claims["db"].as_str().unwrap(), state.settings.database.database);
    assert_eq!(decoded_jwt.claims["id"].as_str().unwrap(), "users:admin");

    assert_jwt_authenticates(&state, jwt, "admin", true).await;
}

#[tokio::test]
async fn test_token_jwt_user_ok() {
    let db = TestSetup::create_db().await;
    let (app, state) = setup(&db).await;

    let user = create_test_user(&db, "testuser", false).await;

    let (token, _) =
        create_test_token(&db, "Test Token", None, Some(user.id.clone()), &["token"]).await;

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
    assert_eq!(decoded_jwt.claims["user_id"].as_str().unwrap(), user.id.to_string());
    assert!(!decoded_jwt.claims["is_admin"].as_bool().unwrap());
    assert_eq!(decoded_jwt.claims["iss"].as_str().unwrap(), state.settings.auth.id);
    assert!(decoded_jwt.claims.get("role").is_none());
    assert!(decoded_jwt.claims["exp"].is_number());
    assert!(decoded_jwt.claims["iat"].is_number());
    // SurrealDB record-access claims
    assert_eq!(decoded_jwt.claims["ac"].as_str().unwrap(), "guardrail_api");
    assert_eq!(decoded_jwt.claims["ns"].as_str().unwrap(), state.settings.database.namespace);
    assert_eq!(decoded_jwt.claims["db"].as_str().unwrap(), state.settings.database.database);
    assert_eq!(decoded_jwt.claims["id"].as_str().unwrap(), format!("users:{}", user.id));

    assert_jwt_authenticates(&state, jwt, &user.username, false).await;
}

#[tokio::test]
async fn test_token_jwt_invalid_token() {
    let db = TestSetup::create_db().await;
    let (app, _state) = setup(&db).await;

    let product =
        create_test_product_with_details(&db, "TestProduct", "Test product description").await;

    let (token_nok, _) =
        create_test_token(&db, "Test Minidump Token", Some(product.id), None, &["minidump-upload"])
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

#[tokio::test]
async fn test_token_ok() {
    let db = TestSetup::create_db().await;
    let (app, _state) = setup(&db).await;

    let (admin_token, _) = create_test_token(&db, "Admin Token", None::<String>, None, &[]).await;

    let request = Request::builder()
        .method("POST")
        .header("Authorization", format!("Bearer {admin_token}"))
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
