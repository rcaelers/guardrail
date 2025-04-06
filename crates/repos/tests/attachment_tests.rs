#![cfg(all(test, feature = "ssr"))]

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use repos::QueryParams;
use repos::attachment::*;

async fn setup_test_dependencies(pool: &PgPool) -> (Uuid, Uuid) {
    // Create product first
    let product_id = sqlx::query_scalar!(
        r#"
        INSERT INTO guardrail.products (name, description)
        VALUES ($1, $2)
        RETURNING id
        "#,
        format!("TestProduct_{}", Uuid::new_v4()),
        "Test Product Description"
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test product");

    // Create version
    let version_id = sqlx::query_scalar!(
        r#"
        INSERT INTO guardrail.versions (name, hash, tag, product_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        format!("Version_{}", Uuid::new_v4()),
        format!("Hash_{}", Uuid::new_v4()),
        format!("Tag_{}", Uuid::new_v4()),
        product_id
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test version");

    // Create crash
    let crash_id = sqlx::query_scalar!(
        r#"
        INSERT INTO guardrail.crashes (summary, report, version_id, product_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        "Test Crash",
        json!({"test": "data"}),
        version_id,
        product_id
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test crash");

    (product_id, crash_id)
}

async fn insert_test_attachment(
    pool: &PgPool,
    name: &str,
    mime_type: &str,
    size: i64,
    filename: &str,
    product_id: Option<Uuid>,
    crash_id: Option<Uuid>,
) -> Attachment {
    let (product_id, crash_id) = match (product_id, crash_id) {
        (Some(p), Some(c)) => (p, c),
        _ => setup_test_dependencies(pool).await,
    };

    sqlx::query_as!(
        Attachment,
        r#"
        INSERT INTO guardrail.attachments (
            name, mime_type, size, filename, crash_id, product_id
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, name, mime_type, size, filename, crash_id, product_id, created_at, updated_at
        "#,
        name,
        mime_type,
        size,
        filename,
        crash_id,
        product_id
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test attachment")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let name = "log.txt";
    let mime_type = "text/plain";
    let size = 1024;
    let filename = "log_2024_04_06.txt";

    let inserted_attachment =
        insert_test_attachment(&pool, name, mime_type, size, filename, None, None).await;

    let found_attachment = AttachmentRepo::get_by_id(&pool, inserted_attachment.id)
        .await
        .expect("Failed to get attachment by ID");

    assert!(found_attachment.is_some());
    let found_attachment = found_attachment.unwrap();
    assert_eq!(found_attachment.id, inserted_attachment.id);
    assert_eq!(found_attachment.name, name);
    assert_eq!(found_attachment.mime_type, mime_type);
    assert_eq!(found_attachment.size, size);
    assert_eq!(found_attachment.filename, filename);

    let non_existent_id = Uuid::new_v4();
    let not_found = AttachmentRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let (product_id, crash_id) = setup_test_dependencies(&pool).await;

    let test_attachment_data = vec![
        ("screenshot.png", "image/png", 20480, "crash_screenshot.png"),
        ("config.json", "application/json", 2048, "app_config.json"),
        ("core.dump", "application/octet-stream", 1048576, "core.dump"),
    ];

    for (name, mime_type, size, filename) in &test_attachment_data {
        insert_test_attachment(
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

    // Test get_all with no params
    let query_params = QueryParams::default();
    let all_attachments = AttachmentRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all attachments");

    assert!(all_attachments.len() >= test_attachment_data.len());

    // Test with filtering - use a filter that exists in the test data
    let query_params = QueryParams {
        filter: Some("config".to_string()),
        ..QueryParams::default()
    };

    let filtered_attachments = AttachmentRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get filtered attachments");

    // Verify at least one result with the filter
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
async fn test_create(pool: PgPool) {
    let (product_id, crash_id) = setup_test_dependencies(&pool).await;

    let new_attachment = NewAttachment {
        name: "stacktrace.txt".to_string(),
        mime_type: "text/plain".to_string(),
        size: 5120,
        filename: "stack_trace_full.txt".to_string(),
        crash_id,
        product_id,
    };

    let attachment_id = AttachmentRepo::create(&pool, new_attachment.clone())
        .await
        .expect("Failed to create attachment");

    let created_attachment = AttachmentRepo::get_by_id(&pool, attachment_id)
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
async fn test_update(pool: PgPool) {
    let mut attachment = insert_test_attachment(
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

    let updated_id = AttachmentRepo::update(&pool, attachment.clone())
        .await
        .expect("Failed to update attachment")
        .expect("Attachment not found when updating");

    assert_eq!(updated_id, attachment.id);

    let updated_attachment = AttachmentRepo::get_by_id(&pool, attachment.id)
        .await
        .expect("Failed to get updated attachment")
        .expect("Updated attachment not found");

    assert_eq!(updated_attachment.name, "updated.log");
    assert_eq!(updated_attachment.mime_type, "text/plain; charset=utf-8");
    assert_eq!(updated_attachment.size, 3072);
    assert_eq!(updated_attachment.filename, "updated_log.txt");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let attachment = insert_test_attachment(
        &pool,
        "delete_me.log",
        "text/plain",
        1024,
        "file_to_delete.log",
        None,
        None,
    )
    .await;

    AttachmentRepo::remove(&pool, attachment.id)
        .await
        .expect("Failed to remove attachment");

    let deleted_attachment = AttachmentRepo::get_by_id(&pool, attachment.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_attachment.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = AttachmentRepo::count(&pool)
        .await
        .expect("Failed to count initial attachments");

    let (product_id, crash_id) = setup_test_dependencies(&pool).await;

    let test_attachments = vec![
        ("count1.txt", "text/plain", 100, "count_file1.txt"),
        ("count2.jpg", "image/jpeg", 200, "count_image2.jpg"),
        ("count3.pdf", "application/pdf", 300, "count_doc3.pdf"),
    ];

    for (name, mime_type, size, filename) in &test_attachments {
        insert_test_attachment(
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

    let new_count = AttachmentRepo::count(&pool)
        .await
        .expect("Failed to count attachments after insertion");

    assert_eq!(new_count, initial_count + test_attachments.len() as i64);
}
