#![cfg(test)]
//! Token-authenticated crash API.
//!
//! The security-relevant properties are that a token cannot reach another
//! product's crashes, cannot act without the matching entitlement, and does not
//! leak reporter-identifying fields unless explicitly granted.

use axum::http::{Request, StatusCode};
use axum::{Router, body::Body};
use object_store::ObjectStore;
use serde_json::{Value, json};
use std::sync::Arc;
use testware::setup::TestSetup;
use tower::ServiceExt;

use api::routes::routes;
use api::state::AppState;
use api::worker::TestWorker;
use repos::Repo;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use testware::{create_test_product_with_details, create_test_token};

async fn app_for(db: &Surreal<Any>) -> Router {
    let state = AppState {
        repo: Repo::new(db.clone()),
        settings: Arc::new(api::settings::Settings::default()),
        storage: Arc::new(object_store::memory::InMemory::new()) as Arc<dyn ObjectStore>,
        worker: Arc::new(TestWorker::new()),
    };
    Router::new().nest("/api", routes(state.clone()).await).with_state(state)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let req = if let Some(b) = body {
        req.header("Content-Type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap()
    } else {
        req.body(Body::empty()).unwrap()
    };
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

/// A crash whose report carries both diagnostic and reporter-identifying parts.
async fn seed_crash(db: &Surreal<Any>, product_id: &str) -> (String, String) {
    let gid = uuid::Uuid::new_v4().to_string();
    let cid = uuid::Uuid::new_v4().to_string();
    db.query(
        "CREATE type::record('crash_groups', $gid) CONTENT {
            product_id: type::record('products', $pid), fingerprint: 'fp-analyse',
            signal: 'SIGSEGV', count: 1, status: 'new',
            first_seen: time::now(), last_seen: time::now(),
            created_at: time::now(), updated_at: time::now()
         };
         CREATE type::record('crashes', $cid) CONTENT {
            product_id: type::record('products', $pid),
            group_id: type::record('crash_groups', $gid),
            fingerprint: 'fp-analyse',
            report: {
                title: 'boom', version: '1.2.3', os: 'linux', platform: 'linux',
                threads: [{ thread_id: 1, frames: [{ function: 'main', file: 'main.c', line: 4 }] }],
                modules: [{ filename: 'app', version: '1.2.3' }],
                handles: [{ handle: 7, object_name: '/home/someone/secret.txt' }],
                lsb_release: 'Ubuntu 24.04'
            },
            created_at: time::now(), updated_at: time::now()
         };
         CREATE annotations CONTENT {
            source: 'submission', key: 'guid', value: 'install-abc-123',
            crash_id: type::record('crashes', $cid),
            product_id: type::record('products', $pid),
            created_at: time::now(), updated_at: time::now()
         };
         CREATE annotations CONTENT {
            source: 'submission', key: 'commit_hash', value: 'deadbeef',
            crash_id: type::record('crashes', $cid),
            product_id: type::record('products', $pid),
            created_at: time::now(), updated_at: time::now()
         };",
    )
    .bind(("gid", gid.clone()))
    .bind(("cid", cid.clone()))
    .bind(("pid", product_id.to_string()))
    .await
    .expect("seed crash failed");
    (gid, cid)
}

#[tokio::test]
async fn crash_api_enforces_entitlements_scope_and_redaction() {
    let db = &TestSetup::create_db().await;
    let app = app_for(db).await;

    let product = create_test_product_with_details(db, "Analysed", "under analysis").await;
    let other = create_test_product_with_details(db, "Other", "not yours").await;
    let (gid, cid) = seed_crash(db, &product.id).await;

    let (reader, _) =
        create_test_token(db, "reader", Some(product.id.clone()), None, &["crash-read"]).await;
    let (annotator, _) = create_test_token(
        db,
        "analysis-bot",
        Some(product.id.clone()),
        None,
        &["crash-read", "crash-annotate"],
    )
    .await;
    let (full, _) = create_test_token(
        db,
        "full",
        Some(product.id.clone()),
        None,
        &["crash-read", "crash-read-full"],
    )
    .await;
    let (foreign, _) =
        create_test_token(db, "foreign", Some(other.id.clone()), None, &["crash-read"]).await;

    let pid = &product.id;

    // --- authentication ---
    let (status, _) = call(&app, "GET", &format!("/api/crashes?productId={pid}"), None, None).await;
    assert_ne!(status, StatusCode::OK, "no token must not be accepted");

    // --- entitlement ---
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/crashes/{gid}/status"),
        Some(&reader),
        Some(json!({"productId": pid, "status": "triaged"})),
    )
    .await;
    assert_ne!(status, StatusCode::OK, "crash-read alone must not write");

    // --- product scope ---
    let (status, _) = call(
        &app,
        "GET",
        &format!("/api/crashes?productId={pid}"),
        Some(&foreign),
        None,
    )
    .await;
    assert_ne!(status, StatusCode::OK, "a token must not reach another product");

    // --- listing ---
    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/crashes?productId={pid}"),
        Some(&reader),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["groups"][0]["id"].as_str(), Some(gid.as_str()));

    // --- redaction ---
    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/crashes/by-crash/{cid}?productId={pid}"),
        Some(&reader),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["redacted"].as_bool(), Some(true));
    assert!(
        body["report"]["threads"].is_array(),
        "the diagnostic parts must survive redaction"
    );
    assert_eq!(
        body["annotations"]["commit_hash"].as_str(),
        Some("deadbeef"),
        "build metadata is what makes the analysis useful"
    );
    assert!(body["report"]["handles"].is_null(), "handles name the reporter's files");
    assert!(body["report"]["lsb_release"].is_null());
    assert!(
        body["annotations"]["guid"].is_null(),
        "the install identifier must not leak under crash-read"
    );
    let serialised = body.to_string();
    assert!(!serialised.contains("install-abc-123"));
    assert!(!serialised.contains("secret.txt"));

    // --- full read is a separate grant ---
    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/crashes/by-crash/{cid}?productId={pid}"),
        Some(&full),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["redacted"].as_bool(), Some(false));
    assert_eq!(body["annotations"]["guid"].as_str(), Some("install-abc-123"));
    assert!(body["report"]["handles"].is_array());

    // --- writes ---
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/crashes/{gid}/notes"),
        Some(&annotator),
        Some(json!({"productId": pid, "body": "  null deref in main  "})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/crashes/{gid}/status"),
        Some(&annotator),
        Some(json!({"productId": pid, "status": "triaged"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/crashes/{gid}?productId={pid}"),
        Some(&reader),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["group"]["status"].as_str(), Some("triaged"));
    assert_eq!(body["notes"][0]["body"].as_str(), Some("null deref in main"));
    assert_eq!(
        body["notes"][0]["author"].as_str(),
        Some("analysis-bot"),
        "a note records which integration wrote it"
    );
    assert_eq!(body["crashes"][0]["id"].as_str(), Some(cid.as_str()));

    // --- rejected input ---
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/crashes/{gid}/status"),
        Some(&annotator),
        Some(json!({"productId": pid, "status": "wontfix"})),
    )
    .await;
    assert_ne!(status, StatusCode::OK, "unknown status must be rejected");

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/crashes/{gid}/notes"),
        Some(&annotator),
        Some(json!({"productId": pid, "body": "   "})),
    )
    .await;
    assert_ne!(status, StatusCode::OK, "an empty note must be rejected");
}
