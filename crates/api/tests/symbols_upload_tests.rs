#![cfg(test)]

use std::sync::Arc;

use api::routes::routes;
use api::state::AppState;
use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use common::QueryParams;
use object_store::ObjectStore;
use object_store::path::Path;
use repos::Repo;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;

use repos::symbols::SymbolsRepo;

use testware::{
    create_settings, create_test_product_with_details, create_test_version, create_token,
    create_webauthn, init_logging,
};

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload(pool: PgPool) {
    init_logging();
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());

    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store.clone(),
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n\
                   Hello world\r\n\
                   Hello world\r\n\
                   Hello world\r\n\
                   Hello world\r\n";

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--{}--\r\n",
        boundary, content, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?product=TestProduct&version=1.0.0")
        .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
        .header("Authorization", format!("Bearer {}", "test_token"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    tracing::info!("Response: {:?}", response);

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

    let file = store
        .get(&Path::from(symbols.file_location.clone()))
        .await
        .unwrap();
    let content_bytes = file.bytes().await.unwrap();
    tracing::info!("size = {} ", content_bytes.len());
    let content_str = String::from_utf8_lossy(&content_bytes);
    assert_eq!(content_str, content);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_no_such_product(pool: PgPool) {
    //init_logging();
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
    };
    let app = Router::new()
        .nest("/api", routes(state.clone()).await)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"upload_file_symbols\"; filename=\"test.sym\"\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--{}--\r\n",
        boundary, content, boundary
    );

    create_token(&pool, "test_token", None, "symbol-upload").await;

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let state = AppState {
        repo,
        webauthn: create_webauthn(settings.clone()),
        settings: settings.clone(),
        storage: store,
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
    let _version =
        create_test_version(&pool, "1.0.0", "test_hash", "v1_0_0", Some(product.id)).await;

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
