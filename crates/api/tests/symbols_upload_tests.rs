#![cfg(test)]

use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use chrono::Utc;
use object_store::ObjectStore;
use object_store::path::Path;
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
    create_settings, create_test_product_with_details, create_test_token, create_webauthn,
};

async fn setup(pool: &PgPool) -> (Router, Arc<dyn ObjectStore>, String, String, String, String) {
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let worker = Arc::new(TestMinidumpProcessor::new());

    let settings = Arc::new(settings);
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

    let config = &SymbolsBodyConfig {
        boundary,
        ..Default::default()
    };
    let body = create_body_from_config(config);

    let product =
        create_test_product_with_details(pool, "TestProduct", "Test product description").await;

    let (token, _) =
        create_test_token(pool, "Test Token", Some(product.id), None, &["symbol-upload"]).await;

    (app, store, boundary.to_owned(), config.content.to_owned(), body, token)
}

#[derive(Debug, Clone)]
pub struct SymbolsBodyConfig<'a> {
    pub boundary: &'a str,
    pub product: Option<&'a str>,
    pub version: Option<&'a str>,
    pub build_id: Option<&'a str>,
    pub extra: Option<String>,
    pub content: &'a str,
    pub channel: Option<&'a str>,
    pub commit: Option<&'a str>,
    pub symbols_field_name: &'a str,
    pub symbols_filename: Option<&'a str>,
    pub symbols_content_type: &'a str,
    pub annotation_content_type: &'a str,
}

impl<'a> Default for SymbolsBodyConfig<'a> {
    fn default() -> Self {
        Self {
            boundary: "----WebKitFormBoundary7MA4YWxkTrZu0gW",
            product: Some("TestProduct"),
            version: Some("1.0.0"),
            build_id: Some("EE9E2672A6863B084C4C44205044422E1"),
            extra: None,
            content: "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                      Hello world\r\n\
                      Hello world\r\n\
                      Hello world\r\n\
                      Hello world\r\n\
                      Hello world\r\n\
                      Hello world\r\n",
            channel: Some("test-channel"),
            commit: Some("test-commit"),
            symbols_field_name: "upload_file_symbols",
            symbols_filename: Some("crash.pdb"),
            symbols_content_type: "application/octet-stream",
            annotation_content_type: "text/plain",
        }
    }
}

pub fn create_body_from_config(config: &SymbolsBodyConfig) -> String {
    let mut body = if let Some(filename) = config.symbols_filename {
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{symbols_field_name}\"; filename=\"{symbols_filename}\"\r\nContent-Type: {symbols_content_type}\r\n\r\n{content}\r\n",
            boundary = config.boundary,
            symbols_field_name = config.symbols_field_name,
            symbols_filename = filename,
            symbols_content_type = config.symbols_content_type,
            content = config.content
        )
    } else {
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{symbols_field_name}\"\r\nContent-Type: {symbols_content_type}\r\n\r\n{content}\r\n",
            boundary = config.boundary,
            symbols_field_name = config.symbols_field_name,
            symbols_content_type = config.symbols_content_type,
            content = config.content
        )
    };

    if let Some(product) = config.product {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"product\"\r\nContent-Type: {annotation_content_type}\r\n\r\n{product}\r\n",
            boundary = config.boundary,
            annotation_content_type = config.annotation_content_type
        );
    }

    if let Some(version) = config.version {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"version\"\r\nContent-Type: {annotation_content_type}\r\n\r\n{version}\r\n",
            boundary = config.boundary,
            annotation_content_type = config.annotation_content_type
        );
    }

    if let Some(channel) = config.channel {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"channel\"\r\nContent-Type: {annotation_content_type}\r\n\r\n{channel}\r\n",
            boundary = config.boundary,
            channel = channel,
            annotation_content_type = config.annotation_content_type
        );
    }

    if let Some(commit) = config.commit {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"commit\"\r\nContent-Type: {annotation_content_type}\r\n\r\n{commit}\r\n",
            boundary = config.boundary,
            commit = commit,
            annotation_content_type = config.annotation_content_type
        );
    }

    if let Some(build_id) = config.build_id {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"build_id\"\r\nContent-Type: {annotation_content_type}\r\n\r\n{build_id}\r\n",
            boundary = config.boundary,
            build_id = build_id,
            annotation_content_type = config.annotation_content_type
        );
    }

    if let Some(extra) = &config.extra {
        body = format!("{body}{extra}");
    }

    body = format!("{body}--{boundary}--\r\n", boundary = config.boundary);

    body
}

async fn assert_response_ok(response: axum::http::Response<Body>) -> serde_json::Value {
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json, json!({ "result": "ok" }));
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
async fn test_symbol_upload_ok(pool: PgPool) {
    let (app, store, boundary, content, body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_ok(response).await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 1);
    let symbols = &allsymbols[0];
    assert_eq!(symbols.module_id, "crash.pdb");
    assert_eq!(symbols.build_id, "EE9E2672A6863B084C4C44205044422E1");

    let file = store
        .get(&Path::from(symbols.storage_path.clone()))
        .await
        .unwrap();
    let content_bytes = file.bytes().await.unwrap();
    tracing::info!("size = {} ", content_bytes.len());
    let content_str = String::from_utf8_lossy(&content_bytes);
    assert_eq!(content_str, content);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_no_such_product(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        product: Some("TestProductxx"),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_empty_version(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        version: Some(""),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_empty_product(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        product: Some(""),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_content_type(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        symbols_content_type: "text/octet-stream",
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid symbols content type: text/octet-stream"),
    )
    .await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_header(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid symbols file header"),
    )
    .await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_build_id_1(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1EE9E2672A6863B084C4C44205044422E1EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid build_id length"),
    )
    .await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_build_id_2(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084@4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid build_id format"),
    )
    .await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_module_id1(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crashxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid module_id length"),
    )
    .await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_module_id2(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 cr&ash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid module_id format"),
    )
    .await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_module_id3(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 ../crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: invalid module_id format"),
    )
    .await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_multipart(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";
    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: boundary2,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_invalid_boundary(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let content = "MODULE windows x86_64 EE9E2672A6863B084C4C44205044422E1 crash.pdb\r\n\
                   Hello world\r\n\
                   Hello world\r\n";
    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: boundary2,
        content,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_wrong_entitlement(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

    let (token, _) = create_test_token(&pool, "Wrong", None, None, &["token"]).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::FORBIDDEN, Some("insufficient permissions")).await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_expired_entitlement(pool: PgPool) {
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
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_inactive_entitlement(pool: PgPool) {
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
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_other_product(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

    let product = create_test_product_with_details(&pool, "AnotherProduct", "description").await;

    let (token_id, token, token_hash) = generate_api_token().expect("Failed to generate API token");
    let new_token = NewApiToken {
        description: "Test API token".to_string(),
        token_id,
        token_hash,
        product_id: Some(product.id),
        user_id: None,
        entitlements: vec!["symbol-upload".to_string()],
        expires_at: Some((Utc::now() + chrono::Duration::days(1)).naive_utc()),
        is_active: true,
    };
    ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to insert test API token");

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_unknown_token(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {}", "test_tokenx"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::UNAUTHORIZED, Some("invalid API token")).await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_no_token(pool: PgPool) {
    let (app, _store, boundary, _content, body, _token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::UNAUTHORIZED, Some("missing API token")).await;

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_version(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        version: None,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");
    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_product(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&SymbolsBodyConfig {
        boundary: &boundary,
        product: None,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload?version=1.0.0")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_upload_empty(pool: PgPool) {
    let (app, _store, boundary, _content, _body, token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
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

    let allsymbols = SymbolsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to fetch symbol entry from database");

    assert_eq!(allsymbols.len(), 0);
}
