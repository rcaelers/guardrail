#![cfg(test)]

use std::path::PathBuf;
use std::sync::Arc;

use apalis::prelude::{Context, Data, Worker, WorkerId};
use common::QueryParams;
use data::symbols::NewSymbols;
use jobs::error::JobError;
use object_store::ObjectStore;
use object_store::PutPayload;
use object_store::path::Path;
use repos::annotation::AnnotationsRepo;
use repos::symbols::SymbolsRepo;
use repos::{attachment::AttachmentsRepo, crash::CrashRepo};
use serde_json::json;
use sqlx::PgPool;
use testware::create_test_product_with_details;
use uuid::Uuid;

use common::settings::Settings;
use jobs::jobs::MinidumpJob;
use jobs::minidump::MinidumpProcessor;
use jobs::state::AppState;
use repos::Repo;

async fn upload(path: String, dest: String, store: Arc<dyn ObjectStore>) {
    let payload = tokio::fs::read(path)
        .await
        .map(PutPayload::from)
        .expect("Failed to read symbol file");
    store
        .put(&Path::from(dest), payload)
        .await
        .expect("Failed to put symbols");
}

async fn setup(pool: &PgPool, store: Arc<dyn ObjectStore>) -> (uuid::Uuid, uuid::Uuid) {
    // TestSetup::init();

    let product =
        create_test_product_with_details(pool, "TestProduct", "Test product description").await;

    let workspace_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("Failed to get current directory"))
        .ancestors()
        .nth(2)
        .expect("Failed to find workspace root")
        .to_path_buf();

    let path = workspace_dir.join("dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp");
    let minidump_path = "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string();
    upload(path.to_string_lossy().to_string(), minidump_path.clone(), store.clone()).await;

    let module_id = "crash.pdb".to_string();
    let build_id = "EE9E2672A6863B084C4C44205044422E1".to_string();
    let symbols_path = format!("symbols/{module_id}-{build_id}");

    let data = NewSymbols {
        build_id,
        module_id,
        storage_path: symbols_path.clone(),
        product_id: product.id,
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
    };

    let path = workspace_dir.join("dev/crash.sym");
    upload(path.to_string_lossy().to_string(), symbols_path.clone(), store.clone()).await;

    let path = workspace_dir.join("dev/init.sh");
    let attachment_id = uuid::Uuid::new_v4();
    let attachment_path = format!("attachments/{attachment_id}");
    upload(path.to_string_lossy().to_string(), attachment_path.clone(), store.clone()).await;

    let _symbol_id = SymbolsRepo::create(pool, data)
        .await
        .expect("Failed to create symbol");
    (product.id, attachment_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_full_minidump_processing_flow(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_ok());

    let crash = CrashRepo::get_by_id(&pool, crash_id)
        .await
        .expect("Failed to get crash")
        .expect("Crash not found");
    assert_eq!(
        crash.minidump,
        Some(Uuid::parse_str("6fda4029-be94-43ea-90b6-32fe2a78074a").unwrap())
    );
    assert_eq!(crash.product_id, product_id);

    let report = crash.report.expect("Report should be present");
    assert!(report.is_object());
    assert!(report["crashing_thread"].is_object());
    assert!(report["crashing_thread"]["frames"].is_array());
    assert!(report["crashing_thread"]["frames"][0]["missing_symbols"].is_boolean());
    assert!(
        !report["crashing_thread"]["frames"][0]["missing_symbols"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        report["crashing_thread"]["frames"][0]["module"]
            .as_str()
            .unwrap(),
        "crash.exe"
    );
    assert_eq!(
        report["crashing_thread"]["frames"][0]["function"]
            .as_str()
            .unwrap(),
        "crash2()"
    );
    assert_eq!(
        report["crashing_thread"]["frames"][4]["function"]
            .as_str()
            .unwrap(),
        "main(int, char**)"
    );

    let attachments = AttachmentsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Crash not found");
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "init.sh");
    assert_eq!(attachments[0].storage_path, format!("attachments/{attachment_id}"));
    assert_eq!(attachments[0].mime_type, "application/octet-stream");
    assert_eq!(attachments[0].size, 1234);
    assert_eq!(attachments[0].crash_id, crash_id);
    assert_eq!(attachments[0].product_id, product_id);

    let annotations = AnnotationsRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Crash not found");
    assert_eq!(annotations.len(), 2);
    // Check that both annotations exist, regardless of order
    let mut found_session_id = false;
    let mut found_user_id = false;

    for annotation in &annotations {
        match annotation.key.as_str() {
            "session_id" => {
                found_session_id = true;
                assert_eq!(annotation.value, "67890");
            }
            "user_id" => {
                found_user_id = true;
                assert_eq!(annotation.value, "12345");
            }
            _ => panic!("Unexpected annotation key: {}", annotation.key),
        }
    }

    assert!(found_session_id, "session_id annotation not found");
    assert!(found_user_id, "user_id annotation not found");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_missing_crash_id(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());
    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "crash_id is missing");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_invalid_crash_id(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": "invalid-uuid".to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());
    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "invalid crash_id format");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_not_found(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/8fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "8fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());
    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "Failed to retrieve minidump");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_invalid_product_id(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (_product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": "nonexistent-product-id".to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());
    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "invalid product_id format");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_product_id_not_found(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (_product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": "56be53ac-adfe-4f50-91a6-70ecbe6e7d0c",
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());
    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "no such product 56be53ac-adfe-4f50-91a6-70ecbe6e7d0c");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_storage_id_missing(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "minidump_id is missing");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_storage_path_missing(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "no minidump found");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_missing(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "no minidump found");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_storage_id_invald(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "Xfda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "invalid minidump_id format");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_atachment_missing_filename(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "attachment filename is missing");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_attachment_missing_content_type(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "content_type": "application/octet-stream".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "attachment content_type is missing");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_attachment_missing_size(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "size": 1024,
            "content_type": "application/octet-stream".to_string(),
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "attachment size is missing");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_attachment_missing_storage_path(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "attachment storage path is missing");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_annotation_value_wrong_type(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": 12345,
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "annotation value is missing");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_invalid(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    let crash_id = Uuid::new_v4();

    let minidump_path = "minidumps/afda4029-be94-43ea-90b6-32fe2a78074a".to_string();
    store
        .put(&Path::from(minidump_path), "Hello World".into())
        .await
        .expect("Failed to put broken minidump  ");

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "size": 1024,
            "storage_path": "minidumps/afda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "afda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "failed to read minidump");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_attachment_db_failure(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    sqlx::query("ALTER TABLE guardrail.attachments ADD COLUMN foo TEXT NOT NULL")
        .execute(&pool)
        .await
        .expect("Failed to add column");

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "failed to create attachment");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_annotation_db_failure(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    sqlx::query("ALTER TABLE guardrail.annotations ADD COLUMN foo TEXT NOT NULL")
        .execute(&pool)
        .await
        .expect("Failed to add column");

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "failed to create annotation");
    } else {
        panic!("Expected JobError::Failure");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_minidump_crash_db_failure(pool: PgPool) {
    let store = Arc::new(object_store::memory::InMemory::new());
    let (product_id, attachment_id) = setup(&pool, store.clone()).await;

    sqlx::query("ALTER TABLE guardrail.crashes ADD COLUMN foo TEXT NOT NULL")
        .execute(&pool)
        .await
        .expect("Failed to add column");

    let crash_id = Uuid::new_v4();

    let crash_info = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z".to_string(),
        "authorized_product": "TestProduct".to_string(),
        "product_id": product_id.to_string(),
        "product": "TestProduct".to_string(),
        "version": "1.0.0".to_string(),
        "channel": "stable".to_string(),
        "commit": "abcdef1234567890".to_string(),
        "build_id": "EE9E2672A6863B084C4C44205044422E1".to_string(),
        "minidump": {
            "filename": "test.dmp".to_string(),
            "size": 1024,
            "storage_path": "minidumps/6fda4029-be94-43ea-90b6-32fe2a78074a".to_string(),
            "storage_id": "6fda4029-be94-43ea-90b6-32fe2a78074a".to_string()
        },
        "attachments": [
            {
                "filename": "init.sh".to_string(),
                "content_type": "application/octet-stream".to_string(),
                "size": 1234,
                "storage_path": format!("attachments/{attachment_id}"),
                "storage_id": attachment_id.to_string()
            }

        ],
        "annotations": {
          "user_id": "12345",
          "session_id": "67890"
        },
    });

    let crash_path = format!("crashes/{crash_id}");
    store
        .put(&Path::from(crash_path), serde_json::to_vec(&crash_info).unwrap().into())
        .await
        .expect("Failed to put symbols");

    let job = MinidumpJob { crash: crash_info };

    let app_state = Data::new(AppState {
        repo: Repo::new(pool.clone()),
        storage: store.clone(),
        settings: Arc::new(Settings::default()),
    });

    let worker = Worker::new(WorkerId::new("test-worker"), Context::default());

    let result = MinidumpProcessor::process(job, worker, app_state).await;

    assert!(result.is_err());
    if let Err(JobError::Failure(msg)) = result {
        assert_eq!(msg, "failed to store crash report");
    } else {
        panic!("Expected JobError::Failure");
    }
}
