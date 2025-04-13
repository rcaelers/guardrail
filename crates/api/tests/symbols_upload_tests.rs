#![cfg(test)]

use std::sync::Arc;

use api::routes::routes;
use api::state::AppState;
use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use common::{QueryParams, hash_token};
use data::api_token::NewApiToken;
use data::product::{NewProduct, Product};
use data::version::{NewVersion, Version};
use repos::Repo;
use repos::api_token::ApiTokenRepo;
use repos::product::ProductRepo;
use repos::version::VersionRepo;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;
use webauthn_rs::prelude::*;

use common::settings::Settings;
use repos::symbols::SymbolsRepo;
use tracing_subscriber::EnvFilter;

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .with_level(true)
        .init();

    //    tracing_log::LogTracer::init().expect("Failed to set logger");
}

pub async fn create_test_product_with_details(
    pool: &PgPool,
    name: &str,
    description: &str,
) -> Product {
    let new_product = NewProduct {
        name: name.to_string(),
        description: description.to_string(),
    };

    let product_id = ProductRepo::create(pool, new_product)
        .await
        .expect("Failed to insert test product");

    ProductRepo::get_by_id(pool, product_id)
        .await
        .expect("Failed to retrieve created product")
        .expect("Created product not found")
}

pub async fn create_test_version(
    pool: &PgPool,
    name: &str,
    hash: &str,
    tag: &str,
    product_id: Uuid,
) -> Version {
    let new_version = NewVersion {
        name: name.to_string(),
        hash: hash.to_string(),
        tag: tag.to_string(),
        product_id,
    };

    let version_id = VersionRepo::create(pool, new_version)
        .await
        .expect("Failed to insert test version");

    VersionRepo::get_by_id(pool, version_id)
        .await
        .expect("Failed to retrieve created version")
        .expect("Created version not found")
}

fn create_webauthn(settings: Arc<Settings>) -> Arc<Webauthn> {
    let rp_id = settings.auth.id.as_str();
    let rp_origin = Url::parse(settings.auth.origin.as_str()).expect("Invalid URL");
    let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
    let builder = builder.rp_name(settings.auth.name.as_str());

    Arc::new(builder.build().expect("Invalid configuration"))
}

async fn create_token(pool: &PgPool, token: &str, product: Option<Uuid>, entitement: &str) -> Uuid {
    let token_hash = hash_token(token).expect("Failed to hash token");
    let new_token = NewApiToken {
        description: "Test API token".to_string(),
        token_hash,
        product_id: product,
        user_id: None,
        entitlements: vec![entitement.to_string()],
        expires_at: None,
    };

    ApiTokenRepo::create(pool, new_token)
        .await
        .expect("Failed to insert test API token")
}

fn create_setting() -> Arc<Settings> {
    let mut settings = Settings::default();
    tracing::info!("Logging initialized");

    settings.auth.id = "localhost".to_string();
    settings.auth.origin = "http://localhost:3000".to_string();
    settings.auth.name = "TestApp".to_string();

    Arc::new(settings)
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json, json!({ "result": "ok" }));

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 1);
    let symbols = &allsymbols[0];
    assert_eq!(symbols.module_id, "crash.pdb");
    assert_eq!(symbols.build_id, "EE9E2672A6863B084C4C44205044422E1");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_no_such_product(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProductxx&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        response_json,
        json!({ "result": "failed", "error": "Product TestProductxx not found" })
    );

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_no_such_version(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct&version=2.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        response_json,
        json!({ "result": "failed", "error": "Version 2.0.0 of product TestProduct not found" })
    );

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_empty_version(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct&version=")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        response_json,
        json!({ "result": "failed", "error": "general failure : version cannot be empty" })
    );

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_empty_product(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        response_json,
        json!({ "result": "failed", "error": "general failure : product name cannot be empty" })
    );

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_wrong_entitlement(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "minidump-upload").await;
    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct&version=2.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    //let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    //TODO: also return json
    assert_eq!(body, "Forbidden: Insufficient permissions");

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_unknown_token(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "minidump-upload").await;
    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct&version=2.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_tokenx"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    //let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    //TODO: also return json
    assert_eq!(body, "Unauthorized: Invalid API token");

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_no_token(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct&version=2.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    //let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    //TODO: also return json
    assert_eq!(body, "Unauthorized: Missing API token");

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_version(pool: PgPool) {
    //init_logging();
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    // let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
    //     .await
    //     .unwrap();
    // let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // assert_eq!(response_json, json!({ "result": "ok" }));

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_product(pool: PgPool) {
    let settings = create_setting();

    let repo = Repo::new(pool.clone());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\nMODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n--{}--\r\n",
        boundary, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version = create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", product.id).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // tracing::info!("Response: {:?}", response);

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    // let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
    //     .await
    //     .unwrap();
    //let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    //assert_eq!(response_json, json!({ "result": "ok" }));

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}
