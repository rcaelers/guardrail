#![cfg(test)]

use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use bytes::Bytes;
use chrono::Utc;
use data::product::Product;
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

async fn setup(
    pool: &PgPool,
) -> (Router, Arc<dyn ObjectStore>, String, Arc<TestMinidumpProcessor>, String, String) {
    setup_with_storage(pool, Arc::new(object_store::memory::InMemory::new())).await
}

async fn setup_with_storage(
    pool: &PgPool,
    store: Arc<dyn ObjectStore>,
) -> (Router, Arc<dyn ObjectStore>, String, Arc<TestMinidumpProcessor>, String, String) {
    let settings = create_settings();

    let repo = Repo::new(pool.clone());
    let worker = Arc::new(TestMinidumpProcessor::new());

    let settings = Arc::new(settings);
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

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary,
        ..Default::default()
    });

    let product =
        create_test_product_with_details(pool, "TestProduct", "Test product description").await;

    let (token, _) =
        create_test_token(pool, "Test Token", Some(product.id), None, &["minidump-upload"]).await;

    (app, store, boundary.to_owned(), worker, body, token)
}

#[derive(Debug, Clone)]
pub struct MinidumpBodyConfig<'a> {
    pub boundary: &'a str,
    pub product: Option<&'a str>,
    pub version: Option<&'a str>,
    pub build_date: Option<&'a str>,
    pub extra: Option<String>,
    pub content: &'a str,
    pub channel: Option<&'a str>,
    pub commit: Option<&'a str>,
    pub minidump_field_name: &'a str,
    pub minidump_filename: Option<&'a str>,
    pub minidump_content_type: &'a str,
    pub annotation_content_type: &'a str,
}

impl<'a> Default for MinidumpBodyConfig<'a> {
    fn default() -> Self {
        Self {
            boundary: "----WebKitFormBoundary7MA4YWxkTrZu0gW",
            product: Some("TestProduct"),
            version: Some("1.0.0"),
            build_date: Some("2025-05-15T20:26:15+02:00"),
            extra: None,
            content: "MINIDUMP DATA",
            channel: Some("test-channel"),
            commit: Some("test-commit"),
            minidump_field_name: "upload_file_minidump",
            minidump_filename: Some("test.dmp"),
            minidump_content_type: "application/octet-stream",
            annotation_content_type: "text/plain",
        }
    }
}

pub fn create_body_from_config(config: &MinidumpBodyConfig) -> String {
    let mut body = if let Some(filename) = config.minidump_filename {
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{minidump_field_name}\"; filename=\"{minidump_filename}\"\r\nContent-Type: {minidump_content_type}\r\n\r\n{content}\r\n",
            boundary = config.boundary,
            minidump_field_name = config.minidump_field_name,
            minidump_filename = filename,
            minidump_content_type = config.minidump_content_type,
            content = config.content
        )
    } else {
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{minidump_field_name}\"\r\nContent-Type: {minidump_content_type}\r\n\r\n{content}\r\n",
            boundary = config.boundary,
            minidump_field_name = config.minidump_field_name,
            minidump_content_type = config.minidump_content_type,
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

    if let Some(build_date) = config.build_date {
        body = format!(
            "{body}--{boundary}\r\nContent-Disposition: form-data; name=\"build_date\"\r\nContent-Type: {annotation_content_type}\r\n\r\n{build_date}\r\n",
            boundary = config.boundary,
            build_date = build_date,
            annotation_content_type = config.annotation_content_type
        );
    }

    if let Some(extra) = &config.extra {
        body = format!("{body}{extra}");
    }

    body = format!("{body}--{boundary}--\r\n", boundary = config.boundary);

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

async fn assert_count_crashes(store: Arc<dyn ObjectStore>, expected_count: usize) {
    let prefix = &Path::from("crashes/");
    let crashes = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(crashes.await.unwrap().len(), expected_count);
}

async fn assert_count_minidumps(store: Arc<dyn ObjectStore>, expected_count: usize) {
    let prefix = &Path::from("minidumps/");
    let minidumps = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(minidumps.await.unwrap().len(), expected_count);
}

async fn assert_count_attachments(store: Arc<dyn ObjectStore>, expected_count: usize) {
    let prefix = &Path::from("attachments/");
    let attachments = store
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect::<Vec<Path>>();
    assert_eq!(attachments.await.unwrap().len(), expected_count);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_ok(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        ..Default::default()
    });
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

    let crash_info = store
        .get(&Path::from(format!("crashes/{crash_id}.json")))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    let crash_info: serde_json::Value =
        serde_json::from_slice(&crash_info).expect("Failed to parse crash info JSON");

    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 5);
    assert_eq!(
        crash_info["annotations"]["product"]["value"]
            .as_str()
            .unwrap(),
        "TestProduct"
    );
    assert_eq!(
        crash_info["annotations"]["product"]["source"]
            .as_str()
            .unwrap(),
        "submission"
    );
    assert_eq!(
        crash_info["annotations"]["version"]["value"]
            .as_str()
            .unwrap(),
        "1.0.0"
    );
    assert_eq!(
        crash_info["annotations"]["channel"]["value"]
            .as_str()
            .unwrap(),
        "test-channel"
    );
    assert_eq!(
        crash_info["annotations"]["commit"]["value"]
            .as_str()
            .unwrap(),
        "test-commit"
    );
    assert_eq!(
        crash_info["annotations"]["build_date"]["value"]
            .as_str()
            .unwrap(),
        "2025-05-15T20:26:15+02:00"
    );
    assert_eq!(crash_info["attachments"].as_array().unwrap().len(), 0);
    assert_eq!(crash_info["minidump"]["filename"].as_str().unwrap(), "test.dmp");

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
async fn test_minidump_upload_ok_without_filename(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        minidump_filename: None,
        ..Default::default()
    });
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

    let crash_info = store
        .get(&Path::from(format!("crashes/{crash_id}.json")))
        .await
        .expect("Failed to get minidump object")
        .bytes()
        .await
        .expect("Failed to read minidump object");
    let crash_info: serde_json::Value =
        serde_json::from_slice(&crash_info).expect("Failed to parse crash info JSON");

    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 5);
    assert_eq!(
        crash_info["annotations"]["product"]["value"]
            .as_str()
            .unwrap(),
        "TestProduct"
    );
    assert_eq!(
        crash_info["annotations"]["product"]["source"]
            .as_str()
            .unwrap(),
        "submission"
    );
    assert_eq!(
        crash_info["annotations"]["version"]["value"]
            .as_str()
            .unwrap(),
        "1.0.0"
    );
    assert_eq!(
        crash_info["annotations"]["channel"]["value"]
            .as_str()
            .unwrap(),
        "test-channel"
    );
    assert_eq!(
        crash_info["annotations"]["commit"]["value"]
            .as_str()
            .unwrap(),
        "test-commit"
    );
    assert_eq!(
        crash_info["annotations"]["build_date"]["value"]
            .as_str()
            .unwrap(),
        "2025-05-15T20:26:15+02:00"
    );
    assert_eq!(crash_info["attachments"].as_array().unwrap().len(), 0);
    assert_eq!(crash_info["minidump"]["filename"].as_str().unwrap(), "unnamed_minidump");

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

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_with_attachments_ok(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let attachment1_content = "LOG DATA 1";
    let attachment2_content = "LOG DATA 2";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        extra: Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"attachment1\"; filename=\"log.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment1_content}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"attachment2\"; filename=\"log2.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment2_content}\r\n"
        )),
        ..Default::default()
    });

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

    assert_eq!(
        crash_info["annotations"]["product"]["value"]
            .as_str()
            .unwrap(),
        "TestProduct"
    );
    assert_eq!(
        crash_info["annotations"]["product"]["source"]
            .as_str()
            .unwrap(),
        "submission"
    );
    assert_eq!(
        crash_info["annotations"]["version"]["value"]
            .as_str()
            .unwrap(),
        "1.0.0"
    );
    assert_eq!(
        crash_info["annotations"]["channel"]["value"]
            .as_str()
            .unwrap(),
        "test-channel"
    );
    assert_eq!(
        crash_info["annotations"]["commit"]["value"]
            .as_str()
            .unwrap(),
        "test-commit"
    );
    assert_eq!(
        crash_info["annotations"]["build_date"]["value"]
            .as_str()
            .unwrap(),
        "2025-05-15T20:26:15+02:00"
    );
    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 5);
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
        assert_eq!(filename, vec! { "log.txt", "log2.txt" }[i]);
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

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 2).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_with_attachments_no_name(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let attachment1_content = "LOG DATA 1";
    let attachment2_content = "LOG DATA 2";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        extra: Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; filename=\"log.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment1_content}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"attachment2\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment2_content}\r\n"
        )),
        ..Default::default()
    });

    log::info!("Body: {body}");
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
        Some("general failure: name field for attachment is missing"),
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
async fn test_minidump_upload_with_annotations_ok(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        extra: Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"features\"; \r\nContent-Type: text/plain\r\n\r\ntracing\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"ui\"; \r\nContent-Type: text/plain\r\n\r\nQt\r\n"
        )),
        ..Default::default()
    });

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

    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 7);
    assert_eq!(
        crash_info["annotations"]["product"]["value"]
            .as_str()
            .unwrap(),
        "TestProduct"
    );
    assert_eq!(
        crash_info["annotations"]["product"]["source"]
            .as_str()
            .unwrap(),
        "submission"
    );
    assert_eq!(
        crash_info["annotations"]["version"]["value"]
            .as_str()
            .unwrap(),
        "1.0.0"
    );
    assert_eq!(
        crash_info["annotations"]["channel"]["value"]
            .as_str()
            .unwrap(),
        "test-channel"
    );
    assert_eq!(
        crash_info["annotations"]["commit"]["value"]
            .as_str()
            .unwrap(),
        "test-commit"
    );
    assert_eq!(
        crash_info["annotations"]["features"]["value"]
            .as_str()
            .unwrap(),
        "tracing"
    );
    assert_eq!(
        crash_info["annotations"]["features"]["source"]
            .as_str()
            .unwrap(),
        "submission"
    );
    assert_eq!(crash_info["annotations"]["ui"]["value"].as_str().unwrap(), "Qt");
    assert_eq!(crash_info["annotations"]["ui"]["source"].as_str().unwrap(), "submission");
    assert_eq!(
        crash_info["annotations"]["build_date"]["value"]
            .as_str()
            .unwrap(),
        "2025-05-15T20:26:15+02:00"
    );
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

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty_version(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        version: Some(""),
        ..Default::default()
    });

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty_product(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        product: Some(""),
        ..Default::default()
    });
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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_annotation_no_name(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        extra: Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; \r\nContent-Type: text/plain\r\n\r\nannotation\r\n"
        )),
        ..Default::default()
    });
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
        Some("general failure: name field is missing for annotation"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_annotation_no_value(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        extra: Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"ui\";\r\nContent-Type: text/plain\r\n\r\n"
        )),
        ..Default::default()
    });
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
        Some("general failure: failed to read field text for annotation 'ui'"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_content_type(pool: PgPool) {
    let (app, store, boundary, _worker, body, token) = setup(&pool).await;

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
        Some("general failure: invalid minidump content type: text/octet-stream"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_multipart(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_invalid_boundary(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: boundary2,
        ..Default::default()
    });

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_wrong_entitlement(pool: PgPool) {
    let (app, store, boundary, _worker, body, _token) = setup(&pool).await;

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_expired_entitlement(pool: PgPool) {
    let (app, store, boundary, _worker, body, _token) = setup(&pool).await;

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_inactive_entitlement(pool: PgPool) {
    let (app, store, boundary, _worker, body, _token) = setup(&pool).await;

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_token_for_other_product(pool: PgPool) {
    let (app, store, boundary, _worker, body, _token) = setup(&pool).await;

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
        StatusCode::BAD_REQUEST,
        Some(
            "validation of product AnotherProduct failed: access denied for product AnotherProduct",
        ),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_unknown_token(pool: PgPool) {
    let (app, store, boundary, _worker, body, _token) = setup(&pool).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {}", "test_tokenx"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(response, StatusCode::UNAUTHORIZED, Some("invalid API token")).await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_no_token(pool: PgPool) {
    let (app, store, boundary, _worker, body, _token) = setup(&pool).await;

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
async fn test_minidump_upload_token_without_product(pool: PgPool) {
    let (app, store, boundary, _worker, body, _token) = setup(&pool).await;

    let (token_id, token, token_hash) = generate_api_token().expect("Failed to generate API token");
    let new_token = NewApiToken {
        description: "Test API token".to_string(),
        token_id,
        token_hash,
        product_id: None,
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
        Some("access denied for product API token is not associated with any product"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_version(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        product: Some("TestProduct"),
        version: None,
        ..Default::default()
    });
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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_symbol_no_product(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        product: None,
        ..Default::default()
    });

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_empty(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_wrong_name(pool: PgPool) {
    let (app, store, boundary, _worker, body, token) = setup(&pool).await;

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

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_product_too_old(pool: PgPool) {
    let (app, store, boundary, _worker, _body, token) = setup(&pool).await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        build_date: Some("2015-05-15T20:26:15+02:00"),
        ..Default::default()
    });

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
        Some("validation of product TestProduct failed: Build is older than 6 months"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_product_not_accepting(pool: PgPool) {
    let (app, store, boundary, _worker, body, token) = setup(&pool).await;

    let product = ProductRepo::get_by_name(&pool, "TestProduct")
        .await
        .expect("Failed to retrieve product")
        .expect("Product not found");

    ProductRepo::update(
        &pool,
        Product {
            accepting_crashes: false,
            ..product
        },
    )
    .await
    .expect("Failed to update product");

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
        Some("product TestProduct not accepting crashes"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_per_product_validation_script(pool: PgPool) {
    // Create settings with per-product validation scripts using the new format
    let mut settings = create_settings();
    // Set up validation scripts with product-specific rules
    settings.minidumps.validation_scripts = Some(vec![
        common::settings::ValidationScript::ProductSpecific {
            product: "^TestProduct$".to_string(),
            script: "scripts/test_product_specific.rhai".to_string(),
        },
        common::settings::ValidationScript::ProductSpecific {
            product: "^OtherProduct$".to_string(),
            script: "scripts/other_product_specific.rhai".to_string(),
        },
    ]);
    let settings = Arc::new(settings);

    let repo = Repo::new(pool.clone());
    let worker = Arc::new(TestMinidumpProcessor::new());
    let store = Arc::new(object_store::memory::InMemory::new());

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

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary,
        ..Default::default()
    });

    let product =
        create_test_product_with_details(&pool, "TestProduct", "Test product description").await;
    let (token, _) =
        create_test_token(&pool, "Test Token", Some(product.id), None, &["minidump-upload"]).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // For a successful per-product validation, this should return OK
    assert_response_ok(response).await;

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_per_product_validation_script_missing(pool: PgPool) {
    // Create settings with per-product validation scripts, but not for the product we'll use
    let mut settings = create_settings();
    // Set up validation scripts only for a different product
    settings.minidumps.validation_scripts =
        Some(vec![common::settings::ValidationScript::ProductSpecific {
            product: "^SomeOtherProduct$".to_string(),
            script: "scripts/other_product_specific.rhai".to_string(),
        }]);

    let repo = Repo::new(pool.clone());
    let worker = Arc::new(TestMinidumpProcessor::new());
    let store = Arc::new(object_store::memory::InMemory::new());

    let settings = Arc::new(settings);
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

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary,
        ..Default::default()
    });

    // Use a product that doesn't have validation scripts
    let product = create_test_product_with_details(
        &pool,
        "UnknownProduct",
        "Product without validation scripts",
    )
    .await;
    let (token, _) =
        create_test_token(&pool, "Test Token", Some(product.id), None, &["minidump-upload"]).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // When no validation scripts are found for a product, it should still succeed
    // (based on our implementation which only runs scripts if found)
    assert_response_ok(response).await;

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_upload_validation_script_regex_patterns(pool: PgPool) {
    // Test various regex patterns for product matching
    let mut settings = create_settings();
    settings.minidumps.validation_scripts = Some(vec![
        // Global script for all products
        common::settings::ValidationScript::Global("scripts/product_validation.rhai".to_string()),
        // Exact match for TestProduct
        common::settings::ValidationScript::ProductSpecific {
            product: "^TestProduct$".to_string(),
            script: "scripts/test_product_specific.rhai".to_string(),
        },
        // Pattern for any product starting with "Test"
        common::settings::ValidationScript::ProductSpecific {
            product: "^Test.*".to_string(),
            script: "scripts/test_product_specific.rhai".to_string(),
        },
        // Pattern for any product containing "workrave" (case insensitive would be "(?i)workrave")
        common::settings::ValidationScript::ProductSpecific {
            product: ".*workrave.*".to_string(),
            script: "scripts/workrave_validation.rhai".to_string(),
        },
    ]);
    let settings = Arc::new(settings);

    let repo = Repo::new(pool.clone());
    let worker = Arc::new(TestMinidumpProcessor::new());
    let store = Arc::new(object_store::memory::InMemory::new());

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

    // Test with a product that matches multiple patterns (TestSomething)
    let product = create_test_product_with_details(
        &pool,
        "TestSomething",
        "Test product with multiple pattern matches",
    )
    .await;
    let (token, _) =
        create_test_token(&pool, "Test Token", Some(product.id), None, &["minidump-upload"]).await;

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary,
        product: Some("TestSomething"), // Match the authorized product
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should succeed - global script + Test.* pattern script both run
    assert_response_ok(response).await;

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}
