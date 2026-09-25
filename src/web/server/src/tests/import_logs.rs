use super::common::*;
use common::import_failure::{ImportFailure, ImportFailureStatus, ImportKind};
use object_store::{ObjectStore, ObjectStoreExt, PutPayload, path::Path};

async fn put_json(storage: &dyn ObjectStore, path: &str, value: &serde_json::Value) {
    storage
        .put(&Path::from(path), PutPayload::from(serde_json::to_vec(value).unwrap()))
        .await
        .unwrap();
}

#[tokio::test]
async fn import_logs_require_product_maintainer() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_product_maintainer(
        &app,
        &f,
        "GET",
        |pid| format!("/products/{pid}/import-logs"),
        |_| None,
        StatusCode::OK,
    )
    .await;
    assert_product_maintainer(
        &app,
        &f,
        "DELETE",
        |pid| format!("/products/{pid}/import-logs"),
        |_| Some(json!({"imports": []})),
        StatusCode::BAD_REQUEST,
    )
    .await;
}

#[tokio::test]
async fn failed_import_can_be_listed_and_retry_requested() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let product_id = &f.products[2].id;
    let upload_id = "upload123";
    let source = json!({
        "product_id": product_id,
        "module_id": "workrave.pdb",
        "build_id": "ABC123",
        "version": "1.11.4"
    });
    put_json(app.storage.as_ref(), &ImportKind::Symbol.source_path(upload_id), &source).await;
    let marker = ImportFailure {
        id: upload_id.to_string(),
        kind: ImportKind::Symbol,
        product_id: product_id.to_string(),
        subject: "workrave.pdb".to_string(),
        error: "Session not found".to_string(),
        attempts: 6,
        first_failed_at: "2026-09-25T09:00:00Z".to_string(),
        last_failed_at: "2026-09-25T09:05:00Z".to_string(),
        status: ImportFailureStatus::Failed,
    };
    put_json(
        app.storage.as_ref(),
        &ImportKind::Symbol.failure_path(upload_id),
        &serde_json::to_value(marker).unwrap(),
    )
    .await;

    let uri = format!("/products/{product_id}/import-logs");
    let (status, body) = app.call_json("GET", &uri, None, Some(&f.non_admin)).await;
    assert_eq!(status, StatusCode::OK);
    let logs = body.as_array().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["id"], upload_id);
    assert_eq!(logs[0]["kind"], "symbol");
    assert_eq!(logs[0]["version"], "1.11.4");
    assert_eq!(logs[0]["status"], "failed");
    assert_eq!(logs[0]["attempts"], 6);
    assert_eq!(logs[0]["retryable"], true);

    let (status, body) = app
        .call_json(
            "POST",
            &uri,
            Some(json!({"imports": [{"kind": "symbol", "id": upload_id}]})),
            Some(&f.non_admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["queued"], 1);

    let (status, body) = app.call_json("GET", &uri, None, Some(&f.non_admin)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["status"], "failed");
    assert_eq!(body[0]["retryable"], true);

    let bytes = app
        .storage
        .get(&Path::from(ImportKind::Symbol.failure_path(upload_id)))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let marker: ImportFailure = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        marker.status,
        ImportFailureStatus::Failed,
        "the web server must not mutate curator-owned retry state"
    );
}

#[tokio::test]
async fn retained_import_can_be_deleted() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let product_id = &f.products[2].id;
    let upload_id = "delete-upload";
    put_json(
        app.storage.as_ref(),
        &ImportKind::Symbol.source_path(upload_id),
        &json!({
            "product_id": product_id,
            "module_id": "obsolete.pdb",
            "version": "1.10.0"
        }),
    )
    .await;
    let marker = ImportFailure {
        id: upload_id.to_string(),
        kind: ImportKind::Symbol,
        product_id: product_id.to_string(),
        subject: "obsolete.pdb".to_string(),
        error: "database unavailable".to_string(),
        attempts: 2,
        first_failed_at: "2026-09-25T09:00:00Z".to_string(),
        last_failed_at: "2026-09-25T09:05:00Z".to_string(),
        status: ImportFailureStatus::Failed,
    };
    put_json(
        app.storage.as_ref(),
        &ImportKind::Symbol.failure_path(upload_id),
        &serde_json::to_value(marker).unwrap(),
    )
    .await;

    let uri = format!("/products/{product_id}/import-logs");
    let (status, body) = app
        .call_json(
            "DELETE",
            &uri,
            Some(json!({"imports": [{"kind": "symbol", "id": upload_id}]})),
            Some(&f.non_admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["deleted"], 1);
    assert!(
        app.storage
            .get(&Path::from(ImportKind::Symbol.source_path(upload_id)))
            .await
            .is_err()
    );
    assert!(
        app.storage
            .get(&Path::from(ImportKind::Symbol.failure_path(upload_id)))
            .await
            .is_err()
    );

    let (status, body) = app.call_json("GET", &uri, None, Some(&f.non_admin)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[tokio::test]
async fn retry_rejects_an_import_from_another_product() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let maintained_id = &f.products[2].id;
    let other_id = &f.products[0].id;
    put_json(
        app.storage.as_ref(),
        &ImportKind::Crash.source_path("crash123"),
        &json!({
            "crash_info": {"product_id": other_id},
            "report": {"title": "Wrong product"}
        }),
    )
    .await;

    assert_eq!(
        app.call(
            "POST",
            &format!("/products/{maintained_id}/import-logs"),
            Some(json!({"imports": [{"kind": "crash", "id": "crash123"}]})),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::FORBIDDEN
    );

    assert_eq!(
        app.call(
            "DELETE",
            &format!("/products/{maintained_id}/import-logs"),
            Some(json!({"imports": [{"kind": "crash", "id": "crash123"}]})),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert!(
        app.storage
            .get(&Path::from(ImportKind::Crash.source_path("crash123")))
            .await
            .is_ok()
    );
}
