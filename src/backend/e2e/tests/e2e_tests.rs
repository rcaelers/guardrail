#![cfg(test)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use futures::TryStreamExt;
use object_store::path::Path;
use object_store::ObjectStore;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tower::ServiceExt;

use testware::setup::TestSetup;
use testware::{
    create_e2e_settings, create_test_product_with_details, create_test_token, create_test_user,
};

use common::QueryParams;
use repos::annotation::AnnotationsRepo;
use repos::crash::CrashRepo;
use repos::symbols::SymbolsRepo;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Shared test infrastructure. Each test gets an isolated SurrealDB database.
struct TestHarness {
    /// Direct SurrealDB handle for verification queries.
    db: Surreal<Any>,
    /// Direct S3 handle for verification.
    storage: Arc<dyn ObjectStore>,
    /// Product created during setup.
    product_id: uuid::Uuid,
    /// Product name created during setup.
    product_name: String,
    /// API bearer token for authenticated requests.
    api_token: String,
    /// Ingestion router (fully bootstrapped from settings).
    ingestion: ingestion::app::GuardrailIngestionApp,
    /// API router (fully bootstrapped from settings).
    api: api::app::GuardrailApiApp,
    /// Background worker shutdown handles.
    _proc_shutdown: tokio::sync::oneshot::Sender<()>,
    _proc_handle: tokio::task::JoinHandle<()>,
    _cur_shutdown: tokio::sync::oneshot::Sender<()>,
    _cur_handle: tokio::task::JoinHandle<()>,
}

static TEST_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));
static NEXT_VALKEY_DB: AtomicUsize = AtomicUsize::new(1);
const PIPELINE_TIMEOUT: Duration = Duration::from_secs(60);

impl TestHarness {
    async fn try_new() -> Option<Self> {
        TestSetup::init();

        let mut settings = create_e2e_settings();
        let valkey_db = NEXT_VALKEY_DB.fetch_add(1, Ordering::Relaxed);
        settings.valkey.uri = format!("{}/{}", settings.valkey.uri.trim_end_matches('/'), valkey_db);
        let settings = Arc::new(settings);

        // Verify Docker services are reachable
        if apalis_redis::connect(settings.valkey.uri.clone())
            .await
            .is_err()
        {
            eprintln!("SKIP: Valkey not available");
            return None;
        }

        Self::flush_valkey(&settings).await;

        // ── 1. Init SurrealDB schema and test data ──────────────────────
        let db = Self::init_db(&settings).await;

        let product_name = format!("TestProduct_{}", uuid::Uuid::new_v4().simple());
        let product = create_test_product_with_details(&db, &product_name, "E2E Test Product").await;
        let user = create_test_user(&db, "e2e_tester", false).await;
        let (token, _api_token) = create_test_token(
            &db,
            "E2E Token",
            Some(product.id),
            Some(user.id),
            &["symbol-upload", "token"],
        )
        .await;

        // ── 2. Start curator (syncs products → Valkey, runs import workers)
        let (cur_shutdown, cur_handle) = Self::spawn_curator(settings.clone());

        // Give curator time to sync products to Valkey
        tokio::time::sleep(Duration::from_millis(500)).await;

        // ── 3. Start processor workers ──────────────────────────────────
        let (proc_shutdown, proc_handle) = Self::spawn_processor(settings.clone());

        // ── 4. Build service routers from settings (black-box) ──────────
        let ingestion =
            ingestion::app::GuardrailIngestionApp::from_settings(settings.clone()).await;
        let api = api::app::GuardrailApiApp::from_settings(settings.clone()).await;

        let storage = common::init_s3_object_store(settings.clone()).await;

        Some(Self {
            db,
            storage,
            product_id: product.id,
            product_name,
            api_token: token,
            ingestion,
            api,
            _proc_shutdown: proc_shutdown,
            _proc_handle: proc_handle,
            _cur_shutdown: cur_shutdown,
            _cur_handle: cur_handle,
        })
    }

    async fn flush_valkey(settings: &common::settings::Settings) {
        let client = redis::Client::open(settings.valkey.uri.as_str())
            .expect("Failed to create Redis client for test isolation");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect to Valkey for test isolation");
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut conn)
            .await
            .expect("Failed to flush Valkey test database");
    }

    /// Connect to real SurrealDB, apply schema, define JWT access.
    async fn init_db(settings: &common::settings::Settings) -> Surreal<Any> {
        use surrealdb::opt::auth::Root;

        let db = surrealdb::engine::any::connect(&settings.database.endpoint)
            .await
            .expect("Failed to connect to SurrealDB");

        db.signin(Root {
            username: settings.database.username.clone(),
            password: settings.database.password.clone(),
        })
        .await
        .expect("Failed to sign in to SurrealDB");

        db.use_ns(&settings.database.namespace)
            .use_db(&settings.database.database)
            .await
            .expect("Failed to select namespace/database");

        // Apply schema
        let schema = include_str!("../../../../database/schema/guardrail.surql");
        db.query(schema)
            .await
            .expect("Failed to apply SurrealDB schema");

        // Define JWT access method
        let public_key = &settings.auth.jwk.public_key;
        db.query(format!(
            r#"DEFINE ACCESS OVERWRITE guardrail_api ON DATABASE TYPE RECORD
                WITH JWT ALGORITHM EDDSA KEY '{public_key}'
                DURATION FOR SESSION 1h"#
        ))
        .await
        .expect("Failed to define JWT access method");

        db
    }

    fn spawn_processor(
        settings: Arc<common::settings::Settings>,
    ) -> (tokio::sync::oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            let app = processor::app::GuardrailProcessorApp::from_settings(settings).await;
            app.run(async move {
                let _ = rx.await;
                Ok(())
            })
            .await;
        });
        (tx, handle)
    }

    fn spawn_curator(
        settings: Arc<common::settings::Settings>,
    ) -> (tokio::sync::oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            let app = curator::app::GuardrailCuratorApp::from_settings(settings).await;
            app.run(async move {
                let _ = rx.await;
                Ok(())
            })
            .await;
        });
        (tx, handle)
    }
}

fn fixture_path(relative_path: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../../../{relative_path}")
}

/// Read the real minidump fixture used for end-to-end decoding.
fn read_test_minidump() -> Vec<u8> {
    let path = fixture_path("dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp");
    std::fs::read(&path).expect("failed to read test minidump from dev/")
}

/// Read the real Breakpad symbol file that matches the minidump fixture.
fn read_test_symbol_file() -> Vec<u8> {
    let path = fixture_path("dev/crash.sym");
    std::fs::read(&path).expect("failed to read test symbols from dev/crash.sym")
}

/// Read the expected decoded report for the sample crash.
fn read_expected_crash_report() -> Value {
    let path = fixture_path("dev/crash.json");
    let bytes = std::fs::read(&path).expect("failed to read expected crash report");
    serde_json::from_slice(&bytes).expect("failed to parse expected crash report")
}

/// Build a multipart body with binary file data and text fields.
fn build_multipart_body(
    boundary: &str,
    field_name: &str,
    filename: &str,
    content_type: &str,
    file_data: &[u8],
    text_fields: &[(&str, &str)],
) -> Vec<u8> {
    let mut body = Vec::new();

    // File part
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n\
             Content-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file_data);
    body.extend_from_slice(b"\r\n");

    // Text fields
    for (name, value) in text_fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\n\
                 Content-Disposition: form-data; name=\"{name}\"\r\n\
                 Content-Type: text/plain\r\n\r\n\
                 {value}\r\n"
            )
            .as_bytes(),
        );
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// Poll until a condition is met or timeout expires.
async fn poll_until<F, Fut, T>(check: F, timeout: Duration) -> T
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let start = Instant::now();
    loop {
        if let Some(result) = check().await {
            return result;
        }
        if start.elapsed() > timeout {
            panic!("Timed out waiting for condition after {:?}", timeout);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn upload_test_symbols(harness: &TestHarness) {
    let router = harness.api.router().await;
    let boundary = "----E2ESymbolBoundary";
    let body = build_multipart_body(
        boundary,
        "upload_file_symbols",
        "crash.sym",
        "application/octet-stream",
        &read_test_symbol_file(),
        &[
            ("product", harness.product_name.as_str()),
            ("version", "1.0.0"),
            ("channel", "stable"),
            ("commit", "abc123"),
            ("build_id", "EE9E2672A6863B084C4C44205044422E1"),
        ],
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/symbols/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("Authorization", format!("Bearer {}", harness.api_token))
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let response_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "symbol upload failed: {}",
        String::from_utf8_lossy(&response_bytes)
    );

    let response_json: Value = serde_json::from_slice(&response_bytes).unwrap();
    assert_eq!(response_json["result"], "ok");
}

async fn wait_for_uploaded_symbol(harness: &TestHarness) {
    let db = harness.db.clone();
    let product_id = harness.product_id;

    let symbols = poll_until(
        move || {
            let db = db.clone();
            async move {
                let syms = SymbolsRepo::get_all(&db, QueryParams::default())
                    .await
                    .ok()?;
                let matches: Vec<_> = syms
                    .into_iter()
                    .filter(|symbol| symbol.product_id == product_id)
                    .collect();
                if matches.is_empty() {
                    None
                } else {
                    Some(matches)
                }
            }
        },
        PIPELINE_TIMEOUT,
    )
    .await;

    assert_eq!(symbols.len(), 1, "expected exactly one symbol record");

    let symbol = &symbols[0];
    assert_eq!(symbol.product_id, harness.product_id);
    assert_eq!(symbol.os, "windows");
    assert_eq!(symbol.arch, "x86_64");
    assert_eq!(symbol.build_id, "EE9E2672A6863B084C4C44205044422E1");
    assert_eq!(symbol.module_id, "crash.pdb");
}

fn assert_decoded_report_matches_expected(report: &Value, signature: Option<&str>) {
    let expected = read_expected_crash_report();

    assert_eq!(report["crash_info"]["type"], expected["crash_info"]["type"]);
    assert_eq!(report["crash_info"]["address"], expected["crash_info"]["address"]);
    assert_eq!(report["system_info"]["os"], expected["system_info"]["os"]);
    assert_eq!(
        report["system_info"]["cpu_arch"],
        expected["system_info"]["cpu_arch"]
    );
    assert_eq!(report["thread_count"], expected["thread_count"]);
    assert_eq!(
        report["crashing_thread"]["frame_count"],
        expected["crashing_thread"]["frame_count"]
    );

    let actual_main_module = report["modules"]
        .as_array()
        .and_then(|modules| modules.first())
        .expect("expected at least one module in crash report");
    let expected_main_module = expected["modules"]
        .as_array()
        .and_then(|modules| modules.first())
        .expect("expected at least one module in expected report");

    assert_eq!(actual_main_module["filename"], expected_main_module["filename"]);
    assert_eq!(actual_main_module["debug_file"], expected_main_module["debug_file"]);
    assert_eq!(actual_main_module["debug_id"], expected_main_module["debug_id"]);
    assert_eq!(actual_main_module["loaded_symbols"], Value::Bool(true));
    assert_eq!(actual_main_module["missing_symbols"], Value::Bool(false));

    let actual_frames = report["crashing_thread"]["frames"]
        .as_array()
        .expect("expected crashing_thread.frames in crash report");
    let expected_frames = expected["crashing_thread"]["frames"]
        .as_array()
        .expect("expected crashing_thread.frames in expected report");

    for frame_index in 0..5 {
        assert_eq!(
            actual_frames[frame_index]["function"],
            expected_frames[frame_index]["function"],
            "unexpected function at crashing_thread.frames[{frame_index}]"
        );
        assert_eq!(
            actual_frames[frame_index]["missing_symbols"],
            Value::Bool(false),
            "expected symbols for crashing_thread.frames[{frame_index}]"
        );
    }

    let signature = signature.expect("expected a crash signature");
    assert_ne!(signature, "NONE", "expected a decoded signature");
    assert!(
        signature.contains("crash.exe!crash2")
            && signature.contains("crash.exe!crash1")
            && signature.contains("crash.exe!crash"),
        "unexpected signature: {signature}"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// End-to-end crash flow:
///   ingestion upload → Valkey → processor → curator import → DB verification
///
/// Requires Docker services: SurrealDB, Valkey, MinIO.
#[tokio::test]
async fn test_e2e_crash_flow() {
    let _guard = TEST_LOCK.lock().await;
    let harness = match TestHarness::try_new().await {
        Some(h) => h,
        None => return,
    };

    upload_test_symbols(&harness).await;
    wait_for_uploaded_symbol(&harness).await;

    // ── 1. Upload a REAL minidump via ingestion HTTP ────────────────────
    let router = harness.ingestion.router().await;
    let boundary = "----E2ETestBoundary";
    let minidump_bytes = read_test_minidump();
    let build_date = Utc::now().to_rfc3339();

    let body = build_multipart_body(
        boundary,
        "upload_file_minidump",
        "test.dmp",
        "application/octet-stream",
        &minidump_bytes,
        &[
            ("product", harness.product_name.as_str()),
            ("version", "1.0.0"),
            ("channel", "stable"),
            ("commit", "abc123"),
            ("build_date", &build_date),
        ],
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

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();
    assert_eq!(response_json["result"], "ok");

    let crash_id: uuid::Uuid = response_json["crash_id"]
        .as_str()
        .expect("crash_id missing from response")
        .parse()
        .expect("invalid crash_id UUID");

    // ── 2. Wait for crash to appear in DB (processor → curator pipeline) ─
    let db = harness.db.clone();
    let crash = poll_until(
        || {
            let db = db.clone();
            async move { CrashRepo::get_by_id(&db, crash_id).await.ok().flatten() }
        },
        PIPELINE_TIMEOUT,
    )
    .await;

    // ── 3. Verify crash data ────────────────────────────────────────────
    assert_eq!(crash.id, crash_id);
    assert_eq!(crash.product_id, harness.product_id);
    assert!(crash.signature.is_some(), "expected a crash signature");
    assert!(crash.report.is_some(), "expected a crash report");
    assert_decoded_report_matches_expected(
        crash.report.as_ref().expect("expected a crash report"),
        crash.signature.as_deref(),
    );

    // Verify annotations
    let annotations =
        AnnotationsRepo::get_by_crash_id(&harness.db, crash_id, QueryParams::default())
            .await
            .expect("annotation query failed");

    assert!(
        !annotations.is_empty(),
        "expected annotations to be created"
    );

    let product_annotation = annotations.iter().find(|a| a.key == "product");
    assert!(
        product_annotation.is_some(),
        "expected 'product' annotation"
    );
    assert_eq!(product_annotation.unwrap().value, harness.product_name);
}

/// End-to-end crash flow with attachments.
#[tokio::test]
async fn test_e2e_crash_flow_with_attachments() {
    let _guard = TEST_LOCK.lock().await;
    let harness = match TestHarness::try_new().await {
        Some(h) => h,
        None => return,
    };

    upload_test_symbols(&harness).await;
    wait_for_uploaded_symbol(&harness).await;

    // ── 1. Upload minidump with attachments via ingestion HTTP ──────────
    let router = harness.ingestion.router().await;
    let boundary = "----E2EAttachBoundary";
    let minidump_bytes = read_test_minidump();
    let build_date = Utc::now().to_rfc3339();

    // Build multipart body with both minidump and a log attachment
    let mut body = Vec::new();

    // Minidump file part
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"crash.dmp\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&minidump_bytes);
    body.extend_from_slice(b"\r\n");

    // Text fields
    for (name, value) in &[
        ("product", harness.product_name.as_str()),
        ("version", "2.0.0"),
        ("channel", "beta"),
        ("commit", "def456"),
        ("build_date", build_date.as_str()),
    ] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\n\
                 Content-Disposition: form-data; name=\"{name}\"\r\n\
                 Content-Type: text/plain\r\n\r\n\
                 {value}\r\n"
            )
            .as_bytes(),
        );
    }

    // Log attachment
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"log_attachment\"; filename=\"app.log\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n\
             LOG LINE 1\nLOG LINE 2\r\n"
        )
        .as_bytes(),
    );

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let request = Request::builder()
        .method("POST")
        .uri("/api/minidump/upload")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();
    let crash_id: uuid::Uuid = response_json["crash_id"].as_str().unwrap().parse().unwrap();

    // ── 2. Wait for crash to appear in DB ───────────────────────────────
    let db = harness.db.clone();
    let crash = poll_until(
        || {
            let db = db.clone();
            async move { CrashRepo::get_by_id(&db, crash_id).await.ok().flatten() }
        },
        PIPELINE_TIMEOUT,
    )
    .await;

    // ── 3. Verify ───────────────────────────────────────────────────────
    assert!(crash.signature.is_some(), "expected a crash signature");
    assert_decoded_report_matches_expected(
        crash.report.as_ref().expect("expected a crash report"),
        crash.signature.as_deref(),
    );

    // Verify annotations
    let annotations =
        AnnotationsRepo::get_by_crash_id(&harness.db, crash_id, QueryParams::default())
            .await
            .unwrap();

    let version_ann = annotations.iter().find(|a| a.key == "version");
    assert!(version_ann.is_some());
    assert_eq!(version_ann.unwrap().value, "2.0.0");

    // Verify attachment files in S3
    let prefix = &Path::from("attachments/");
    let attachment_objects: Vec<_> = harness
        .storage
        .list(Some(prefix))
        .map_ok(|meta| meta.location)
        .try_collect()
        .await
        .unwrap();
    assert!(
        !attachment_objects.is_empty(),
        "expected attachment files in S3"
    );
}
