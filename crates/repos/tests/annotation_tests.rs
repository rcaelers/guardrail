use common::QueryParams;
use data::annotation::AnnotationKind;
use data::annotation::NewAnnotation;
use repos::{annotation::AnnotationsRepo, error::RepoError};
use sqlx::{Pool, Postgres};
use testware::create_test_crash;
use testware::create_test_product;
use testware::create_test_version;
use uuid::Uuid;

// AnnotationKind tests

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

async fn create_test_annotation(pool: &Pool<Postgres>) -> (NewAnnotation, Uuid, Uuid) {
    let product = create_test_product(pool).await;
    let version =
        create_test_version(pool, "1.0.0", "Test Version", "test-platform", Some(product.id)).await;
    let crash = create_test_crash(pool, None, Some(product.id), Some(version.id)).await;

    let new_annotation = NewAnnotation {
        key: format!("test_key_{}", Uuid::new_v4()),
        kind: "system".to_string(),
        value: "test_value".to_string(),
        crash_id: crash.id,
        product_id: product.id,
    };

    (new_annotation, product.id, crash.id)
}

// get_by_id tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let annotation_id = AnnotationsRepo::create(&pool, new_annotation.clone())
        .await
        .expect("Failed to create test annotation");

    let found_annotation = AnnotationsRepo::get_by_id(&pool, annotation_id)
        .await
        .expect("Failed to get annotation by ID");

    assert!(found_annotation.is_some());
    let found_annotation = found_annotation.unwrap();
    assert_eq!(found_annotation.id, annotation_id);
    assert_eq!(found_annotation.key, new_annotation.key);
    assert_eq!(found_annotation.value, new_annotation.value);
    assert_eq!(found_annotation.kind, new_annotation.kind);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: Pool<Postgres>) {
    let non_existent_id = Uuid::new_v4();
    let not_found = AnnotationsRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let annotation_id = AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    pool.close().await;

    let result = AnnotationsRepo::get_by_id(&pool, annotation_id).await;
    assert!(result.is_err(), "Expected an error when getting annotation by ID with closed pool");
}

// get_by_crash_id tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_crash_id(pool: Pool<Postgres>) {
    let product = create_test_product(&pool).await;
    let version =
        create_test_version(&pool, "1.0.0", "Test Version", "test-platform", Some(product.id))
            .await;
    let crash = create_test_crash(&pool, None, Some(product.id), Some(version.id)).await;

    let new_annotation1 = NewAnnotation {
        key: "key1".to_string(),
        kind: "system".to_string(),
        value: "test_value1".to_string(),
        crash_id: crash.id,
        product_id: product.id,
    };

    let new_annotation2 = NewAnnotation {
        key: "key2".to_string(),
        kind: "system".to_string(),
        value: "test_value2".to_string(),
        crash_id: crash.id,
        product_id: product.id,
    };

    let different_crash = create_test_crash(&pool, None, Some(product.id), Some(version.id)).await;

    let new_annotation3 = NewAnnotation {
        key: "key3".to_string(),
        kind: "system".to_string(),
        value: "test_value3".to_string(),
        crash_id: different_crash.id,
        product_id: product.id,
    };

    let _id1 = AnnotationsRepo::create(&pool, new_annotation1)
        .await
        .expect("Failed to create test annotation 1");

    let _id2 = AnnotationsRepo::create(&pool, new_annotation2)
        .await
        .expect("Failed to create test annotation 2");

    let _id3 = AnnotationsRepo::create(&pool, new_annotation3)
        .await
        .expect("Failed to create test annotation 3");

    let params = QueryParams::default();
    let annotations = AnnotationsRepo::get_by_crash_id(&pool, crash.id, params)
        .await
        .expect("Failed to get annotations by crash_id");

    assert_eq!(annotations.len(), 2);
    assert!(annotations.iter().any(|a| a.key == "key1"));
    assert!(annotations.iter().any(|a| a.key == "key2"));
    assert!(!annotations.iter().any(|a| a.key == "key3"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_crash_id_error(pool: Pool<Postgres>) {
    let product = create_test_product(&pool).await;
    let version =
        create_test_version(&pool, "1.0.0", "Test Version", "test-platform", Some(product.id))
            .await;
    let crash = create_test_crash(&pool, None, Some(product.id), Some(version.id)).await;

    let new_annotation = NewAnnotation {
        key: "key1".to_string(),
        kind: "system".to_string(),
        value: "test_value1".to_string(),
        crash_id: crash.id,
        product_id: product.id,
    };

    AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    pool.close().await;

    let result = AnnotationsRepo::get_by_crash_id(&pool, crash.id, QueryParams::default()).await;
    assert!(
        result.is_err(),
        "Expected an error when getting annotations by crash_id with closed pool"
    );
}

// get_all tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: Pool<Postgres>) {
    let (new_annotation1, _, _) = create_test_annotation(&pool).await;
    let (new_annotation2, _, _) = create_test_annotation(&pool).await;

    let id1 = AnnotationsRepo::create(&pool, new_annotation1)
        .await
        .expect("Failed to create test annotation 1");

    let id2 = AnnotationsRepo::create(&pool, new_annotation2)
        .await
        .expect("Failed to create test annotation 2");

    let params = QueryParams::default();
    let annotations = AnnotationsRepo::get_all(&pool, params)
        .await
        .expect("Failed to get all annotations");
    assert!(annotations.len() >= 2);
    assert!(annotations.iter().any(|a| a.id == id1));
    assert!(annotations.iter().any(|a| a.id == id2));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_error(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    pool.close().await;

    let result = AnnotationsRepo::get_all(&pool, QueryParams::default()).await;
    assert!(result.is_err(), "Expected an error when getting all annotations with closed pool");
}

// create tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;

    let annotation_id = AnnotationsRepo::create(&pool, new_annotation.clone())
        .await
        .expect("Failed to create annotation");

    let annotation = AnnotationsRepo::get_by_id(&pool, annotation_id)
        .await
        .expect("Failed to get annotation by ID")
        .expect("Annotation not found");

    assert_eq!(annotation.key, new_annotation.key);
    assert_eq!(annotation.kind, new_annotation.kind);
    assert_eq!(annotation.value, new_annotation.value);
    assert_eq!(annotation.crash_id, new_annotation.crash_id);
    assert_eq!(annotation.product_id, new_annotation.product_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_with_invalid_kind(pool: Pool<Postgres>) {
    let (mut new_annotation, _, _) = create_test_annotation(&pool).await;
    new_annotation.kind = "invalid".to_string();

    let result = AnnotationsRepo::create(&pool, new_annotation).await;

    assert!(result.is_err());
    match result {
        Err(RepoError::InvalidColumn(_)) => (),
        _ => panic!("Expected InvalidColumn error, got: {:?}", result),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_error(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;

    pool.close().await;

    let result = AnnotationsRepo::create(&pool, new_annotation.clone()).await;
    assert!(result.is_err(), "Expected an error when creating annotation with closed pool");
}

// update tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let annotation_id = AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let mut annotation = AnnotationsRepo::get_by_id(&pool, annotation_id)
        .await
        .expect("Failed to get annotation")
        .expect("Annotation not found");

    annotation.key = "updated_key".to_string();
    annotation.value = "updated_value".to_string();

    let result = AnnotationsRepo::update(&pool, annotation.clone())
        .await
        .expect("Failed to update annotation")
        .expect("No rows were updated");

    assert_eq!(result, annotation_id);

    let updated_annotation = AnnotationsRepo::get_by_id(&pool, annotation_id)
        .await
        .expect("Failed to get updated annotation")
        .expect("Updated annotation not found");

    assert_eq!(updated_annotation.key, "updated_key");
    assert_eq!(updated_annotation.value, "updated_value");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_with_invalid_kind(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let annotation_id = AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let mut annotation = AnnotationsRepo::get_by_id(&pool, annotation_id)
        .await
        .expect("Failed to get annotation")
        .expect("Annotation not found");

    annotation.kind = "invalid".to_string();

    let result = AnnotationsRepo::update(&pool, annotation).await;

    assert!(result.is_err());
    match result {
        Err(RepoError::InvalidColumn(_)) => (),
        _ => panic!("Expected InvalidColumn error, got: {:?}", result),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_error(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let annotation_id = AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let mut annotation = AnnotationsRepo::get_by_id(&pool, annotation_id)
        .await
        .expect("Failed to get annotation")
        .expect("Annotation not found");

    annotation.key = "updated_key".to_string();
    annotation.value = "updated_value".to_string();

    pool.close().await;

    let result = AnnotationsRepo::update(&pool, annotation.clone()).await;
    assert!(result.is_err(), "Expected an error when updating annotation with closed pool");
}

// count tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: Pool<Postgres>) {
    let initial_count = AnnotationsRepo::count(&pool)
        .await
        .expect("Failed to count annotations");

    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let id = AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let new_count = AnnotationsRepo::count(&pool)
        .await
        .expect("Failed to count annotations");

    assert_eq!(new_count, initial_count + 1);

    AnnotationsRepo::remove(&pool, id)
        .await
        .expect("Failed to remove test annotation");

    let final_count = AnnotationsRepo::count(&pool)
        .await
        .expect("Failed to count annotations");

    assert_eq!(final_count, initial_count);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count_error(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    pool.close().await;

    let result = AnnotationsRepo::count(&pool).await;
    assert!(result.is_err(), "Expected an error when counting annotations with closed pool");
}

// remove tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let id = AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let annotation = AnnotationsRepo::get_by_id(&pool, id)
        .await
        .expect("Failed to get annotation")
        .expect("Annotation not found");

    assert_eq!(annotation.id, id);

    AnnotationsRepo::remove(&pool, id)
        .await
        .expect("Failed to remove annotation");

    let result = AnnotationsRepo::get_by_id(&pool, id)
        .await
        .expect("Failed to query for annotation");

    assert!(result.is_none(), "Annotation still exists after removal");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove_annotation_error(pool: Pool<Postgres>) {
    let (new_annotation, _, _) = create_test_annotation(&pool).await;
    let id = AnnotationsRepo::create(&pool, new_annotation)
        .await
        .expect("Failed to create test annotation");

    pool.close().await;

    let result = AnnotationsRepo::remove(&pool, id).await;
    assert!(result.is_err(), "Expected an error when removing annotation with closed pool");
}
