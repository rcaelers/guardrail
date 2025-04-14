#![cfg(test)]

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use common::QueryParams;
use data::attachment::*;
use data::crash::NewCrash;
use repos::attachment::*;
use repos::crash::CrashRepo;

use testware::{create_test_attachment, setup_test_dependencies};

// get_by_id tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let name = "log.txt";
    let mime_type = "text/plain";
    let size = 1024;
    let filename = "log_2024_04_06.txt";

    let inserted_attachment =
        create_test_attachment(&pool, name, mime_type, size, filename, None, None).await;

    let found_attachment = AttachmentsRepo::get_by_id(&pool, inserted_attachment.id)
        .await
        .expect("Failed to get attachment by ID");

    assert!(found_attachment.is_some());
    let found_attachment = found_attachment.unwrap();
    assert_eq!(found_attachment.id, inserted_attachment.id);
    assert_eq!(found_attachment.name, "log.txt");
    assert_eq!(found_attachment.mime_type, "text/plain");
    assert_eq!(found_attachment.size, 1024);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let inserted_attachment =
        create_test_attachment(&pool, "test.log", "text/plain", 1024, "test_file.log", None, None)
            .await;

    pool.close().await;

    let result = AttachmentsRepo::get_by_id(&pool, inserted_attachment.id).await;
    assert!(result.is_err(), "Expected an error when getting attachment by ID with closed pool");
}

// get_by_id_not_found test

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = AttachmentsRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

// get_all tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let new_crash = NewCrash {
        summary: "Test Crash".to_string(),
        report: json!({"test": "data"}),
        version_id,
        product_id,
    };

    let crash_id = CrashRepo::create(&pool, new_crash)
        .await
        .expect("Failed to insert test crash");

    let test_attachment_data = vec![
        ("screenshot.png", "image/png", 20480, "crash_screenshot.png"),
        ("config.json", "application/json", 2048, "app_config.json"),
        ("core.dump", "application/octet-stream", 1048576, "core.dump"),
    ];

    for (name, mime_type, size, filename) in &test_attachment_data {
        create_test_attachment(
            &pool,
            name,
            mime_type,
            *size,
            filename,
            Some(product_id),
            Some(crash_id),
        )
        .await;
    }

    let query_params = QueryParams::default();
    let all_attachments = AttachmentsRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all attachments");

    assert!(all_attachments.len() >= test_attachment_data.len());

    let query_params = QueryParams {
        filter: Some("config".to_string()),
        ..QueryParams::default()
    };

    let filtered_attachments = AttachmentsRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get filtered attachments");

    assert!(!filtered_attachments.is_empty());
    for attachment in &filtered_attachments {
        assert!(
            attachment.name.contains("config")
                || attachment.filename.contains("config")
                || attachment.mime_type.contains("config")
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_error(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let new_crash = NewCrash {
        summary: "Test Crash".to_string(),
        report: json!({"test": "data"}),
        version_id,
        product_id,
    };

    let crash_id = CrashRepo::create(&pool, new_crash)
        .await
        .expect("Failed to insert test crash");

    create_test_attachment(
        &pool,
        "screenshot.png",
        "image/png",
        20480,
        "crash_screenshot.png",
        Some(product_id),
        Some(crash_id),
    )
    .await;

    pool.close().await;

    let query_params = QueryParams::default();
    let result = AttachmentsRepo::get_all(&pool, query_params).await;
    assert!(result.is_err(), "Expected an error when getting all attachments with closed pool");
}

// create tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let new_crash = NewCrash {
        summary: "Test Crash".to_string(),
        report: json!({"test": "data"}),
        version_id,
        product_id,
    };

    let crash_id = CrashRepo::create(&pool, new_crash)
        .await
        .expect("Failed to insert test crash");

    let new_attachment = NewAttachment {
        name: "stacktrace.txt".to_string(),
        mime_type: "text/plain".to_string(),
        size: 5120,
        filename: "stack_trace_full.txt".to_string(),
        crash_id,
        product_id,
    };

    let attachment_id = AttachmentsRepo::create(&pool, new_attachment.clone())
        .await
        .expect("Failed to create attachment");

    let created_attachment = AttachmentsRepo::get_by_id(&pool, attachment_id)
        .await
        .expect("Failed to get created attachment")
        .expect("Created attachment not found");

    assert_eq!(created_attachment.name, new_attachment.name);
    assert_eq!(created_attachment.mime_type, new_attachment.mime_type);
    assert_eq!(created_attachment.size, new_attachment.size);
    assert_eq!(created_attachment.filename, new_attachment.filename);
    assert_eq!(created_attachment.crash_id, new_attachment.crash_id);
    assert_eq!(created_attachment.product_id, new_attachment.product_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_error(pool: PgPool) {
    let (product_id, crash_id) = setup_test_dependencies(&pool).await;

    let new_attachment = NewAttachment {
        filename: "test_file.txt".to_string(),
        mime_type: "text/plain".to_string(),
        size: 123,
        name: "/path/to/file.txt".to_string(),
        crash_id,
        product_id,
    };

    pool.close().await;

    let result = AttachmentsRepo::create(&pool, new_attachment).await;
    assert!(result.is_err(), "Expected an error when creating attachment with closed pool");
}

// update tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut attachment = create_test_attachment(
        &pool,
        "original.log",
        "text/plain",
        2048,
        "original_log.txt",
        None,
        None,
    )
    .await;

    attachment.name = "updated.log".to_string();
    attachment.mime_type = "text/plain; charset=utf-8".to_string();
    attachment.size = 3072;
    attachment.filename = "updated_log.txt".to_string();

    let updated_id = AttachmentsRepo::update(&pool, attachment.clone())
        .await
        .expect("Failed to update attachment")
        .expect("Attachment not found when updating");

    assert_eq!(updated_id, attachment.id);

    let updated_attachment = AttachmentsRepo::get_by_id(&pool, attachment.id)
        .await
        .expect("Failed to get updated attachment")
        .expect("Updated attachment not found");

    assert_eq!(updated_attachment.name, "updated.log");
    assert_eq!(updated_attachment.mime_type, "text/plain; charset=utf-8");
    assert_eq!(updated_attachment.size, 3072);
    assert_eq!(updated_attachment.filename, "updated_log.txt");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_error(pool: PgPool) {
    let mut attachment = create_test_attachment(
        &pool,
        "original.log",
        "text/plain",
        2048,
        "original_log.txt",
        None,
        None,
    )
    .await;

    attachment.name = "updated.log".to_string();
    attachment.mime_type = "text/plain; charset=utf-8".to_string();
    attachment.size = 3072;
    attachment.filename = "updated_log.txt".to_string();

    pool.close().await;

    let result = AttachmentsRepo::update(&pool, attachment.clone()).await;
    assert!(result.is_err(), "Expected an error when updating attachment with closed pool");
}

// remove tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let attachment = create_test_attachment(
        &pool,
        "delete_me.log",
        "text/plain",
        1024,
        "file_to_delete.log",
        None,
        None,
    )
    .await;

    AttachmentsRepo::remove(&pool, attachment.id)
        .await
        .expect("Failed to remove attachment");

    let deleted_attachment = AttachmentsRepo::get_by_id(&pool, attachment.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_attachment.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove_error(pool: PgPool) {
    let attachment = create_test_attachment(
        &pool,
        "delete_me.log",
        "text/plain",
        1024,
        "file_to_delete.log",
        None,
        None,
    )
    .await;

    pool.close().await;

    let result = AttachmentsRepo::remove(&pool, attachment.id).await;
    assert!(result.is_err(), "Expected an error when removing attachment with closed pool");
}

// count tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = AttachmentsRepo::count(&pool)
        .await
        .expect("Failed to count initial attachments");

    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let new_crash = NewCrash {
        summary: "Test Crash".to_string(),
        report: json!({"test": "data"}),
        version_id,
        product_id,
    };

    let crash_id = CrashRepo::create(&pool, new_crash)
        .await
        .expect("Failed to insert test crash");

    let test_attachments = vec![
        ("count1.txt", "text/plain", 100, "count_file1.txt"),
        ("count2.jpg", "image/jpeg", 200, "count_image2.jpg"),
        ("count3.pdf", "application/pdf", 300, "count_doc3.pdf"),
    ];

    for (name, mime_type, size, filename) in &test_attachments {
        create_test_attachment(
            &pool,
            name,
            mime_type,
            *size,
            filename,
            Some(product_id),
            Some(crash_id),
        )
        .await;
    }

    let new_count = AttachmentsRepo::count(&pool)
        .await
        .expect("Failed to count attachments after insertion");

    assert_eq!(new_count, initial_count + test_attachments.len() as i64);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count_error(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let new_crash = NewCrash {
        summary: "Test Crash".to_string(),
        report: json!({"test": "data"}),
        version_id,
        product_id,
    };

    let crash_id = CrashRepo::create(&pool, new_crash)
        .await
        .expect("Failed to insert test crash");

    create_test_attachment(
        &pool,
        "count_test.txt",
        "text/plain",
        1024,
        "count_test_file.txt",
        Some(product_id),
        Some(crash_id),
    )
    .await;

    pool.close().await;

    let result = AttachmentsRepo::count(&pool).await;
    assert!(result.is_err(), "Expected an error when counting attachments with closed pool");
}
