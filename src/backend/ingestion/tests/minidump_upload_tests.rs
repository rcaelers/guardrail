#![cfg(test)]

use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use bytes::Bytes;
use chrono::Utc;
use common::product_info::ProductInfo;
use futures::TryStreamExt;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;

use ingestion::product_cache::ProductCache;
use ingestion::routes::routes;
use ingestion::state::AppState;
use ingestion::worker::TestWorker;

use testware::create_settings;

fn create_test_product_cache() -> ProductCache {
    let product_id = uuid::Uuid::new_v4();
    let mut products = HashMap::new();
    products.insert(
        "TestProduct".to_string(),
        ProductInfo {
            id: product_id,
            name: "TestProduct".to_string(),
            accepting_crashes: true,
        },
    );
    ProductCache::from_map(products)
}

fn create_test_product_cache_with(entries: Vec<(&str, bool)>) -> ProductCache {
    let mut products = HashMap::new();
    for (name, accepting) in entries {
        products.insert(
            name.to_string(),
            ProductInfo {
                id: uuid::Uuid::new_v4(),
                name: name.to_string(),
                accepting_crashes: accepting,
            },
        );
    }
    ProductCache::from_map(products)
}

async fn setup() -> (Router, Arc<dyn ObjectStore>, String, Arc<TestWorker>, String) {
    setup_with_storage(Arc::new(object_store::memory::InMemory::new())).await
}

async fn setup_with_storage(
    store: Arc<dyn ObjectStore>,
) -> (Router, Arc<dyn ObjectStore>, String, Arc<TestWorker>, String) {
    setup_with_storage_and_cache(store, create_test_product_cache()).await
}

async fn setup_with_storage_and_cache(
    store: Arc<dyn ObjectStore>,
    product_cache: ProductCache,
) -> (Router, Arc<dyn ObjectStore>, String, Arc<TestWorker>, String) {
    let settings = create_settings();

    let worker = Arc::new(TestWorker::new());

    let settings = Arc::new(settings);
    let state = AppState {
        product_cache,
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

    (app, store, boundary.to_owned(), worker, body)
}

#[derive(Debug, Clone)]
pub struct MinidumpBodyConfig<'a> {
    pub boundary: &'a str,
    pub product: Option<&'a str>,
    pub version: Option<&'a str>,
    pub build_date: Option<String>,
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
            build_date: Some(Utc::now().to_rfc3339()),
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

    if let Some(build_date) = &config.build_date {
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

#[tokio::test]
async fn test_minidump_upload_ok() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let config = MinidumpBodyConfig {
        boundary: &boundary,
        ..Default::default()
    };
    let expected_build_date = config.build_date.clone().unwrap();
    let body = create_body_from_config(&config);
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 6);
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
        expected_build_date
    );
    assert_eq!(crash_info["attachments"].as_array().unwrap().len(), 0);
    assert_eq!(
        crash_info["minidump"]["filename"].as_str().unwrap(),
        "test.dmp"
    );

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

#[tokio::test]
async fn test_minidump_upload_ok_without_filename() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let config = MinidumpBodyConfig {
        boundary: &boundary,
        minidump_filename: None,
        ..Default::default()
    };
    let body = create_body_from_config(&config);
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 6);
    assert_eq!(
        crash_info["minidump"]["filename"].as_str().unwrap(),
        "unnamed_minidump"
    );

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[tokio::test]
async fn test_minidump_upload_with_attachments_ok() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let attachment1_content = "LOG DATA 1";
    let attachment2_content = "LOG DATA 2";
    let config = MinidumpBodyConfig {
        boundary: &boundary,
        extra: Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"attachment1\"; filename=\"log.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment1_content}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"attachment2\"; filename=\"log2.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n{attachment2_content}\r\n"
        )),
        ..Default::default()
    };
    let expected_build_date = config.build_date.clone().unwrap();
    let body = create_body_from_config(&config);

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

    assert_eq!(
        crash_info["annotations"]["product"]["value"]
            .as_str()
            .unwrap(),
        "TestProduct"
    );
    assert_eq!(
        crash_info["annotations"]["version"]["value"]
            .as_str()
            .unwrap(),
        "1.0.0"
    );
    assert_eq!(
        crash_info["annotations"]["build_date"]["value"]
            .as_str()
            .unwrap(),
        expected_build_date
    );
    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 6);
    assert_eq!(crash_info["attachments"].as_array().unwrap().len(), 2);

    let attachment = crash_info["attachments"].as_array().unwrap();
    for (i, att) in attachment.iter().enumerate() {
        let storage_path = att["storage_path"]
            .as_str()
            .expect("storage_path is missing");
        let filename = att["filename"].as_str().expect("filename is missing");
        assert_eq!(filename, vec!["log.txt", "log2.txt"][i]);
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

#[tokio::test]
async fn test_minidump_upload_with_attachments_no_name() {
    let (app, store, boundary, _worker, _body) = setup().await;

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

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("general failure: name field for attachment is missing"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
}

#[tokio::test]
async fn test_minidump_upload_with_annotations_ok() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let config = MinidumpBodyConfig {
        boundary: &boundary,
        extra: Some(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"features\"; \r\nContent-Type: text/plain\r\n\r\ntracing\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"ui\"; \r\nContent-Type: text/plain\r\n\r\nQt\r\n"
        )),
        ..Default::default()
    };
    let expected_build_date = config.build_date.clone().unwrap();
    let body = create_body_from_config(&config);

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

    assert_eq!(crash_info["annotations"].as_object().unwrap().len(), 8);
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
    assert_eq!(
        crash_info["annotations"]["ui"]["value"].as_str().unwrap(),
        "Qt"
    );
    assert_eq!(
        crash_info["annotations"]["ui"]["source"]
            .as_str()
            .unwrap(),
        "submission"
    );
    assert_eq!(
        crash_info["annotations"]["build_date"]["value"]
            .as_str()
            .unwrap(),
        expected_build_date
    );

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[tokio::test]
async fn test_minidump_upload_empty_version() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        version: Some(""),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_empty_product() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        product: Some(""),
        ..Default::default()
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_annotation_no_name() {
    let (app, store, boundary, _worker, _body) = setup().await;

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
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_annotation_no_value() {
    let (app, store, boundary, _worker, _body) = setup().await;

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
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_invalid_content_type() {
    let (app, store, boundary, _worker, body) = setup().await;

    let body = body.replace("application/octet-stream", "text/octet-stream");

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_invalid_multipart() {
    let (app, store, boundary, _worker, _body) = setup().await;

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
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_invalid_boundary() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let boundary2 = "----WebKitFormBoundaryX7MA4YWxkTrZu0gW";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: boundary2,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_symbol_no_version() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        product: Some("TestProduct"),
        version: None,
        ..Default::default()
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload?product=TestProduct")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_symbol_no_product() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        product: None,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_empty() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_wrong_name() {
    let (app, store, boundary, _worker, body) = setup().await;

    let body = body.replace("upload_file_minidump", "xupload_file_minidump");

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_product_too_old() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        build_date: Some("2015-05-15T20:26:15+02:00".to_string()),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_product_not_accepting() {
    let product_cache = create_test_product_cache_with(vec![("TestProduct", false)]);
    let store: Arc<dyn ObjectStore> = Arc::new(object_store::memory::InMemory::new());
    let (app, store, _boundary, _worker, _body) =
        setup_with_storage_and_cache(store, product_cache).await;

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary,
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
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

#[tokio::test]
async fn test_minidump_upload_unknown_product() {
    let (app, store, boundary, _worker, _body) = setup().await;

    let body = create_body_from_config(&MinidumpBodyConfig {
        boundary: &boundary,
        product: Some("UnknownProduct"),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_error(
        response,
        StatusCode::BAD_REQUEST,
        Some("product UnknownProduct not found"),
    )
    .await;

    assert_count_crashes(store.clone(), 0).await;
    assert_count_minidumps(store.clone(), 0).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[tokio::test]
async fn test_minidump_upload_per_product_validation_script() {
    let mut settings = create_settings();
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

    let product_cache = create_test_product_cache();
    let store: Arc<dyn ObjectStore> = Arc::new(object_store::memory::InMemory::new());

    let worker = Arc::new(TestWorker::new());
    let settings = Arc::new(settings);
    let state = AppState {
        product_cache,
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

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_ok(response).await;

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[tokio::test]
async fn test_minidump_upload_per_product_validation_script_missing() {
    let mut settings = create_settings();
    settings.minidumps.validation_scripts =
        Some(vec![common::settings::ValidationScript::ProductSpecific {
            product: "^SomeOtherProduct$".to_string(),
            script: "scripts/other_product_specific.rhai".to_string(),
        }]);

    let product_cache = create_test_product_cache_with(vec![("UnknownProduct", true)]);
    let store: Arc<dyn ObjectStore> = Arc::new(object_store::memory::InMemory::new());

    let worker = Arc::new(TestWorker::new());
    let settings = Arc::new(settings);
    let state = AppState {
        product_cache,
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
        product: Some("UnknownProduct"),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_ok(response).await;

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}

#[tokio::test]
async fn test_minidump_upload_validation_script_regex_patterns() {
    let mut settings = create_settings();
    settings.minidumps.validation_scripts = Some(vec![
        common::settings::ValidationScript::Global("scripts/product_validation.rhai".to_string()),
        common::settings::ValidationScript::ProductSpecific {
            product: "^TestProduct$".to_string(),
            script: "scripts/test_product_specific.rhai".to_string(),
        },
        common::settings::ValidationScript::ProductSpecific {
            product: "^Test.*".to_string(),
            script: "scripts/test_product_specific.rhai".to_string(),
        },
        common::settings::ValidationScript::ProductSpecific {
            product: ".*workrave.*".to_string(),
            script: "scripts/workrave_validation.rhai".to_string(),
        },
    ]);

    let product_cache = create_test_product_cache_with(vec![("TestSomething", true)]);
    let store: Arc<dyn ObjectStore> = Arc::new(object_store::memory::InMemory::new());

    let worker = Arc::new(TestWorker::new());
    let settings = Arc::new(settings);
    let state = AppState {
        product_cache,
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
        product: Some("TestSomething"),
        ..Default::default()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_response_ok(response).await;

    assert_count_crashes(store.clone(), 1).await;
    assert_count_minidumps(store.clone(), 1).await;
    assert_count_attachments(store.clone(), 0).await;
}
