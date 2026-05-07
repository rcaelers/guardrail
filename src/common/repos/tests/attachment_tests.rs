#![cfg(test)]

use testware::setup::TestSetup;
use uuid::Uuid;

use common::QueryParams;
use data::attachment::*;
use repos::attachment::*;

use testware::{create_test_attachment, create_test_crash, create_test_product};

// get_by_id tests

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let name = "log.txt";
    let mime_type = "text/plain";
    let size = 1024;
    let filename = "log_2024_04_06.txt";

    let inserted_attachment =
        create_test_attachment(&db, name, mime_type, size, filename, None, None).await;

    let found_attachment = AttachmentsRepo::get_by_id(&db, inserted_attachment.id.clone())
        .await
        .expect("Failed to get attachment by ID");

    assert!(found_attachment.is_some());
    let found_attachment = found_attachment.unwrap();
    assert_eq!(found_attachment.id, inserted_attachment.id);
    assert_eq!(found_attachment.name, "log.txt");
    assert_eq!(found_attachment.mime_type, "text/plain");
    assert_eq!(found_attachment.size, 1024);
}

// get_by_id_not_found test

#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4().to_string();
    let not_found = AttachmentsRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

// get_all tests

#[tokio::test]
async fn test_get_all() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;
    let crash = create_test_crash(&db, None, Some(product.id.clone())).await;

    let test_attachment_data = vec![
        ("screenshot.png", "image/png", 20480, "crash_screenshot.png"),
        ("config.json", "application/json", 2048, "app_config.json"),
        ("core.dump", "application/octet-stream", 1048576, "core.dump"),
    ];

    for (name, mime_type, size, filename) in &test_attachment_data {
        create_test_attachment(
            &db,
            name,
            mime_type,
            *size,
            filename,
            Some(product.id.clone()),
            Some(crash.id.clone()),
        )
        .await;
    }

    let query_params = QueryParams::default();
    let all_attachments = AttachmentsRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get all attachments");

    assert!(all_attachments.len() >= test_attachment_data.len());

    let query_params = QueryParams {
        filter: Some("config".to_string()),
        ..QueryParams::default()
    };

    let filtered_attachments = AttachmentsRepo::get_all(&db, query_params)
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

// create tests

#[tokio::test]
async fn test_create() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;
    let crash = create_test_crash(&db, None, Some(product.id.clone())).await;

    let new_attachment = NewAttachment {
        name: "stacktrace.txt".to_string(),
        mime_type: "text/plain".to_string(),
        size: 5120,
        filename: "stack_trace_full.txt".to_string(),
        crash_id: crash.id.clone(),
        product_id: product.id.clone(),
        storage_path: "s3://bucket/path/to/stack_trace_full.txt".to_string(),
    };

    let attachment_id = AttachmentsRepo::create(&db, new_attachment.clone())
        .await
        .expect("Failed to create attachment");

    let created_attachment = AttachmentsRepo::get_by_id(&db, attachment_id)
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

// update tests

#[tokio::test]
async fn test_update() {
    let db = TestSetup::create_db().await;
    let mut attachment = create_test_attachment(
        &db,
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

    let updated_id = AttachmentsRepo::update(&db, attachment.clone())
        .await
        .expect("Failed to update attachment")
        .expect("Attachment not found when updating");

    assert_eq!(updated_id, attachment.id);

    let updated_attachment = AttachmentsRepo::get_by_id(&db, attachment.id.clone())
        .await
        .expect("Failed to get updated attachment")
        .expect("Updated attachment not found");

    assert_eq!(updated_attachment.name, "updated.log");
    assert_eq!(updated_attachment.mime_type, "text/plain; charset=utf-8");
    assert_eq!(updated_attachment.size, 3072);
    assert_eq!(updated_attachment.filename, "updated_log.txt");
}

// remove tests

#[tokio::test]
async fn test_remove() {
    let db = TestSetup::create_db().await;
    let attachment = create_test_attachment(
        &db,
        "delete_me.log",
        "text/plain",
        1024,
        "file_to_delete.log",
        None,
        None,
    )
    .await;

    AttachmentsRepo::remove(&db, attachment.id.clone())
        .await
        .expect("Failed to remove attachment");

    let deleted_attachment = AttachmentsRepo::get_by_id(&db, attachment.id.clone())
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_attachment.is_none());
}

// count tests

#[tokio::test]
async fn test_count() {
    let db = TestSetup::create_db().await;
    let initial_count = AttachmentsRepo::count(&db)
        .await
        .expect("Failed to count initial attachments");

    let product = create_test_product(&db).await;
    let crash = create_test_crash(&db, None, Some(product.id.clone())).await;

    let test_attachments = vec![
        ("count1.txt", "text/plain", 100, "count_file1.txt"),
        ("count2.jpg", "image/jpeg", 200, "count_image2.jpg"),
        ("count3.pdf", "application/pdf", 300, "count_doc3.pdf"),
    ];

    for (name, mime_type, size, filename) in &test_attachments {
        create_test_attachment(
            &db,
            name,
            mime_type,
            *size,
            filename,
            Some(product.id.clone()),
            Some(crash.id.clone()),
        )
        .await;
    }

    let new_count = AttachmentsRepo::count(&db)
        .await
        .expect("Failed to count attachments after insertion");

    assert_eq!(new_count, initial_count + test_attachments.len() as i64);
}
