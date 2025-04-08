use repos::annotation::AnnotationKind;

#[cfg(feature = "ssr")]
use {
    chrono::{NaiveDateTime, Utc},
    repos::QueryParams,
    repos::annotation::ssr::AnnotationRepo,
    repos::annotation::{Annotation, NewAnnotation},
    repos::error::RepoError,
    sqlx::{Error as SqlxError, Pool, Postgres},
    uuid::Uuid,
};

#[test]
fn test_annotation_kind_as_str() {
    assert_eq!(AnnotationKind::System.as_str(), "system");
    assert_eq!(AnnotationKind::User.as_str(), "user");
}

#[test]
fn test_annotation_kind_try_from_valid_str() {
    let system_kind = AnnotationKind::try_from("system").expect("Failed to parse 'system'");
    let user_kind = AnnotationKind::try_from("user").expect("Failed to parse 'user'");

    assert!(matches!(system_kind, AnnotationKind::System));
    assert!(matches!(user_kind, AnnotationKind::User));
}

#[test]
fn test_annotation_kind_try_from_with_casing() {
    let system_kind = AnnotationKind::try_from("SYSTEM").expect("Failed to parse 'SYSTEM'");
    let user_kind = AnnotationKind::try_from("User").expect("Failed to parse 'User'");

    assert!(matches!(system_kind, AnnotationKind::System));
    assert!(matches!(user_kind, AnnotationKind::User));
}

#[test]
fn test_annotation_kind_try_from_invalid_str() {
    let result = AnnotationKind::try_from("invalid");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid annotation kind: invalid");
}

#[cfg(feature = "ssr")]
mod ssr_tests {
    use super::*;
    use repos::product::ssr::ProductRepo;
    use repos::product::NewProduct;
    use repos::crash::ssr::CrashRepo;
    use repos::crash::NewCrash;
    use repos::version::ssr::VersionRepo;
    use repos::version::NewVersion;

    // Helper to create a test product
    async fn setup_test_product(pool: &Pool<Postgres>) -> Uuid {
        let product = NewProduct {
            name: format!("test_product_{}", Uuid::new_v4()),
            description: "Test product for annotation tests".to_string(),
        };

        ProductRepo::create(pool, product)
            .await
            .expect("Failed to create test product")
    }

    // Helper to create a test version
    async fn setup_test_version(pool: &Pool<Postgres>, product_id: Uuid) -> Uuid {
        let version = NewVersion {
            name: format!("test_version_{}", Uuid::new_v4()),
            hash: format!("hash_{}", Uuid::new_v4()),
            tag: format!("tag_{}", Uuid::new_v4()),
            product_id,
        };

        VersionRepo::create(pool, version)
            .await
            .expect("Failed to create test version")
    }

    // Helper to create a test crash
    async fn setup_test_crash(pool: &Pool<Postgres>, product_id: Uuid, version_id: Uuid) -> Uuid {
        let crash = NewCrash {
            summary: "Test crash for annotation tests".to_string(),
            report: serde_json::json!({
                "crash_info": {
                    "type": "Test",
                    "address": "0x0",
                    "crashing_thread": 0
                }
            }),
            version_id,
            product_id,
        };

        CrashRepo::create(pool, crash)
            .await
            .expect("Failed to create test crash")
    }

    // Helper to create a test annotation with valid references
    async fn create_test_annotation(pool: &Pool<Postgres>) -> (NewAnnotation, Uuid, Uuid) {
        // Create required records
        let product_id = setup_test_product(pool).await;
        let version_id = setup_test_version(pool, product_id).await;
        let crash_id = setup_test_crash(pool, product_id, version_id).await;

        let new_annotation = NewAnnotation {
            key: format!("test_key_{}", Uuid::new_v4()),
            kind: "system".to_string(),
            value: "test_value".to_string(),
            crash_id,
            product_id,
        };

        (new_annotation, product_id, crash_id)
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_annotation(pool: Pool<Postgres>) {
        let (new_annotation, _, _) = create_test_annotation(&pool).await;

        let annotation_id = AnnotationRepo::create(&pool, new_annotation.clone())
            .await
            .expect("Failed to create annotation");

        // Verify the annotation was created
        let annotation = AnnotationRepo::get_by_id(&pool, annotation_id)
            .await
            .expect("Failed to get annotation by ID")
            .expect("Annotation not found");

        assert_eq!(annotation.key, new_annotation.key);
        assert_eq!(annotation.kind, new_annotation.kind);
        assert_eq!(annotation.value, new_annotation.value);
        assert_eq!(annotation.crash_id, new_annotation.crash_id);
        assert_eq!(annotation.product_id, new_annotation.product_id);

        // Clean up
        AnnotationRepo::remove(&pool, annotation_id)
            .await
            .expect("Failed to remove test annotation");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_annotation_with_invalid_kind(pool: Pool<Postgres>) {
        let (mut new_annotation, _, _) = create_test_annotation(&pool).await;
        new_annotation.kind = "invalid".to_string();

        let result = AnnotationRepo::create(&pool, new_annotation).await;

        assert!(result.is_err());
        match result {
            Err(RepoError::InvalidColumn(_)) => (),
            _ => panic!("Expected InvalidColumn error, got: {:?}", result),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_annotation(pool: Pool<Postgres>) {
        // Create test annotation
        let (new_annotation, _, _) = create_test_annotation(&pool).await;
        let annotation_id = AnnotationRepo::create(&pool, new_annotation)
            .await
            .expect("Failed to create test annotation");

        // Get the created annotation
        let mut annotation = AnnotationRepo::get_by_id(&pool, annotation_id)
            .await
            .expect("Failed to get annotation")
            .expect("Annotation not found");

        // Update the annotation
        annotation.key = "updated_key".to_string();
        annotation.value = "updated_value".to_string();

        let result = AnnotationRepo::update(&pool, annotation.clone())
            .await
            .expect("Failed to update annotation")
            .expect("No rows were updated");

        // Verify the update
        assert_eq!(result, annotation_id);

        // Get the updated annotation
        let updated_annotation = AnnotationRepo::get_by_id(&pool, annotation_id)
            .await
            .expect("Failed to get updated annotation")
            .expect("Updated annotation not found");

        assert_eq!(updated_annotation.key, "updated_key");
        assert_eq!(updated_annotation.value, "updated_value");

        // Clean up
        AnnotationRepo::remove(&pool, annotation_id)
            .await
            .expect("Failed to remove test annotation");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_annotation_with_invalid_kind(pool: Pool<Postgres>) {
        // Create test annotation
        let (new_annotation, _, _) = create_test_annotation(&pool).await;
        let annotation_id = AnnotationRepo::create(&pool, new_annotation)
            .await
            .expect("Failed to create test annotation");

        // Get the created annotation
        let mut annotation = AnnotationRepo::get_by_id(&pool, annotation_id)
            .await
            .expect("Failed to get annotation")
            .expect("Annotation not found");

        // Try to update with invalid kind
        annotation.kind = "invalid".to_string();

        let result = AnnotationRepo::update(&pool, annotation).await;

        assert!(result.is_err());
        match result {
            Err(RepoError::InvalidColumn(_)) => (),
            _ => panic!("Expected InvalidColumn error, got: {:?}", result),
        }

        // Clean up
        AnnotationRepo::remove(&pool, annotation_id)
            .await
            .expect("Failed to remove test annotation");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_get_all_annotations(pool: Pool<Postgres>) {
        // Create some test annotations
        let (new_annotation1, _, _) = create_test_annotation(&pool).await;
        let (new_annotation2, _, _) = create_test_annotation(&pool).await;

        let id1 = AnnotationRepo::create(&pool, new_annotation1)
            .await
            .expect("Failed to create test annotation 1");

        let id2 = AnnotationRepo::create(&pool, new_annotation2)
            .await
            .expect("Failed to create test annotation 2");

        // Get all annotations
        let params = QueryParams::default();
        let annotations = AnnotationRepo::get_all(&pool, params)
            .await
            .expect("Failed to get all annotations");

        // At least the two we added should be there
        assert!(annotations.len() >= 2);
        assert!(annotations.iter().any(|a| a.id == id1));
        assert!(annotations.iter().any(|a| a.id == id2));

        // Clean up
        AnnotationRepo::remove(&pool, id1)
            .await
            .expect("Failed to remove test annotation 1");

        AnnotationRepo::remove(&pool, id2)
            .await
            .expect("Failed to remove test annotation 2");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_count_annotations(pool: Pool<Postgres>) {
        // Get initial count
        let initial_count = AnnotationRepo::count(&pool)
            .await
            .expect("Failed to count annotations");

        // Create test annotations
        let (new_annotation, _, _) = create_test_annotation(&pool).await;
        let id = AnnotationRepo::create(&pool, new_annotation)
            .await
            .expect("Failed to create test annotation");

        // Count should increase by 1
        let new_count = AnnotationRepo::count(&pool)
            .await
            .expect("Failed to count annotations");

        assert_eq!(new_count, initial_count + 1);

        // Clean up
        AnnotationRepo::remove(&pool, id)
            .await
            .expect("Failed to remove test annotation");

        // Count should be back to initial
        let final_count = AnnotationRepo::count(&pool)
            .await
            .expect("Failed to count annotations");

        assert_eq!(final_count, initial_count);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_get_by_crash_id(pool: Pool<Postgres>) {
        // Create a product and version first
        let product_id = setup_test_product(&pool).await;
        let version_id = setup_test_version(&pool, product_id).await;

        // Create a crash ID that we'll use for multiple annotations
        let crash_id = setup_test_crash(&pool, product_id, version_id).await;

        // Create test annotations with the same crash_id
        let mut new_annotation1 = NewAnnotation {
            key: "key1".to_string(),
            kind: "system".to_string(),
            value: "test_value1".to_string(),
            crash_id,
            product_id,
        };

        let mut new_annotation2 = NewAnnotation {
            key: "key2".to_string(),
            kind: "system".to_string(),
            value: "test_value2".to_string(),
            crash_id,
            product_id,
        };

        // Create a different crash for the third annotation
        let different_crash_id = setup_test_crash(&pool, product_id, version_id).await;

        // Different crash_id
        let mut new_annotation3 = NewAnnotation {
            key: "key3".to_string(),
            kind: "system".to_string(),
            value: "test_value3".to_string(),
            crash_id: different_crash_id,
            product_id,
        };

        let id1 = AnnotationRepo::create(&pool, new_annotation1)
            .await
            .expect("Failed to create test annotation 1");

        let id2 = AnnotationRepo::create(&pool, new_annotation2)
            .await
            .expect("Failed to create test annotation 2");

        let id3 = AnnotationRepo::create(&pool, new_annotation3)
            .await
            .expect("Failed to create test annotation 3");

        // Get annotations by crash_id
        let params = QueryParams::default();
        let annotations = AnnotationRepo::get_by_crash_id(&pool, crash_id, params)
            .await
            .expect("Failed to get annotations by crash_id");

        // Should be 2 annotations with the same crash_id
        assert_eq!(annotations.len(), 2);
        assert!(annotations.iter().any(|a| a.key == "key1"));
        assert!(annotations.iter().any(|a| a.key == "key2"));
        assert!(!annotations.iter().any(|a| a.key == "key3"));

        // Clean up
        AnnotationRepo::remove(&pool, id1)
            .await
            .expect("Failed to remove test annotation 1");

        AnnotationRepo::remove(&pool, id2)
            .await
            .expect("Failed to remove test annotation 2");

        AnnotationRepo::remove(&pool, id3)
            .await
            .expect("Failed to remove test annotation 3");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_remove_annotation(pool: Pool<Postgres>) {
        // Create test annotation
        let (new_annotation, _, _) = create_test_annotation(&pool).await;
        let id = AnnotationRepo::create(&pool, new_annotation)
            .await
            .expect("Failed to create test annotation");

        // Verify it exists
        let annotation = AnnotationRepo::get_by_id(&pool, id)
            .await
            .expect("Failed to get annotation")
            .expect("Annotation not found");

        assert_eq!(annotation.id, id);

        // Remove it
        AnnotationRepo::remove(&pool, id)
            .await
            .expect("Failed to remove annotation");

        // Verify it no longer exists
        let result = AnnotationRepo::get_by_id(&pool, id)
            .await
            .expect("Failed to query for annotation");

        assert!(result.is_none(), "Annotation still exists after removal");
    }
}
