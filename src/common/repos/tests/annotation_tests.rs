use testware::setup::TestSetup;
use uuid::Uuid;

use common::QueryParams;
use data::annotation::AnnotationSource;
use data::annotation::NewAnnotation;
use repos::{annotation::AnnotationsRepo, error::RepoError};
use testware::create_test_crash;
use testware::create_test_product;

// AnnotationKind tests

#[test]
fn test_annotation_source_as_str() {
    assert_eq!(AnnotationSource::Submission.as_str(), "submission");
    assert_eq!(AnnotationSource::User.as_str(), "user");
    assert_eq!(AnnotationSource::Script.as_str(), "script");
}

#[test]
fn test_annotation_source_try_from_valid_str() {
    let submission_source =
        AnnotationSource::try_from("submission").expect("Failed to parse 'submission'");
    let user_source = AnnotationSource::try_from("user").expect("Failed to parse 'user'");
    let script_source = AnnotationSource::try_from("script").expect("Failed to parse 'script'");

    assert!(matches!(submission_source, AnnotationSource::Submission));
    assert!(matches!(user_source, AnnotationSource::User));
    assert!(matches!(script_source, AnnotationSource::Script));
}

#[test]
fn test_annotation_source_try_from_with_casing() {
    let system_source =
        AnnotationSource::try_from("SUBMISSION").expect("Failed to parse 'SUBMISSION'");
    let user_source = AnnotationSource::try_from("User").expect("Failed to parse 'User'");
    let script_source = AnnotationSource::try_from("SCRIPT").expect("Failed to parse 'SCRIPT'");

    assert!(matches!(system_source, AnnotationSource::Submission));
    assert!(matches!(user_source, AnnotationSource::User));
    assert!(matches!(script_source, AnnotationSource::Script));
}

#[test]
fn test_annotation_source_try_from_invalid_str() {
    let result = AnnotationSource::try_from("invalid");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid annotation source: invalid");
}

async fn create_test_annotation(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
) -> (NewAnnotation, String, String) {
    let product = create_test_product(db).await;
    let crash = create_test_crash(db, None, Some(product.id.clone())).await;

    let new_annotation = NewAnnotation {
        key: format!("test_key_{}", Uuid::new_v4()),
        source: "submission".to_string(),
        value: "test_value".to_string(),
        crash_id: crash.id.clone(),
        product_id: product.id.clone(),
    };

    (new_annotation, product.id, crash.id)
}

// get_by_id tests

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let (new_annotation, _, _) = create_test_annotation(&db).await;
    let annotation_id = AnnotationsRepo::create(&db, new_annotation.clone())
        .await
        .expect("Failed to create test annotation");

    let found_annotation = AnnotationsRepo::get_by_id(&db, annotation_id.clone())
        .await
        .expect("Failed to get annotation by ID");

    assert!(found_annotation.is_some());
    let found_annotation = found_annotation.unwrap();
    assert_eq!(found_annotation.id, annotation_id);
    assert_eq!(found_annotation.key, new_annotation.key);
    assert_eq!(found_annotation.value, new_annotation.value);
    assert_eq!(found_annotation.source, new_annotation.source);
}

#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4().to_string();
    let not_found = AnnotationsRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

// get_by_crash_id tests

#[tokio::test]
async fn test_get_by_crash_id() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;
    let crash = create_test_crash(&db, None, Some(product.id.clone())).await;

    let new_annotation1 = NewAnnotation {
        key: "key1".to_string(),
        source: "submission".to_string(),
        value: "test_value1".to_string(),
        crash_id: crash.id.clone(),
        product_id: product.id.clone(),
    };

    let new_annotation2 = NewAnnotation {
        key: "key2".to_string(),
        source: "submission".to_string(),
        value: "test_value2".to_string(),
        crash_id: crash.id.clone(),
        product_id: product.id.clone(),
    };

    let different_crash = create_test_crash(&db, None, Some(product.id.clone())).await;

    let new_annotation3 = NewAnnotation {
        key: "key3".to_string(),
        source: "submission".to_string(),
        value: "test_value3".to_string(),
        crash_id: different_crash.id.clone(),
        product_id: product.id.clone(),
    };

    let _id1 = AnnotationsRepo::create(&db, new_annotation1)
        .await
        .expect("Failed to create test annotation 1");

    let _id2 = AnnotationsRepo::create(&db, new_annotation2)
        .await
        .expect("Failed to create test annotation 2");

    let _id3 = AnnotationsRepo::create(&db, new_annotation3)
        .await
        .expect("Failed to create test annotation 3");

    let params = QueryParams::default();
    let annotations = AnnotationsRepo::get_by_crash_id(&db, crash.id.clone(), params)
        .await
        .expect("Failed to get annotations by crash_id");

    assert_eq!(annotations.len(), 2);
    assert!(annotations.iter().any(|a| a.key == "key1"));
    assert!(annotations.iter().any(|a| a.key == "key2"));
    assert!(!annotations.iter().any(|a| a.key == "key3"));
}

// get_all tests

#[tokio::test]
async fn test_get_all() {
    let db = TestSetup::create_db().await;
    let (new_annotation1, _, _) = create_test_annotation(&db).await;
    let (new_annotation2, _, _) = create_test_annotation(&db).await;

    let id1 = AnnotationsRepo::create(&db, new_annotation1)
        .await
        .expect("Failed to create test annotation 1");

    let id2 = AnnotationsRepo::create(&db, new_annotation2)
        .await
        .expect("Failed to create test annotation 2");

    let params = QueryParams::default();
    let annotations = AnnotationsRepo::get_all(&db, params)
        .await
        .expect("Failed to get all annotations");
    assert!(annotations.len() >= 2);
    assert!(annotations.iter().any(|a| a.id == id1));
    assert!(annotations.iter().any(|a| a.id == id2));
}

// create tests

#[tokio::test]
async fn test_create() {
    let db = TestSetup::create_db().await;
    let (new_annotation, _, _) = create_test_annotation(&db).await;

    let annotation_id = AnnotationsRepo::create(&db, new_annotation.clone())
        .await
        .expect("Failed to create annotation");

    let annotation = AnnotationsRepo::get_by_id(&db, annotation_id.clone())
        .await
        .expect("Failed to get annotation by ID")
        .expect("Annotation not found");

    assert_eq!(annotation.key, new_annotation.key);
    assert_eq!(annotation.source, new_annotation.source);
    assert_eq!(annotation.value, new_annotation.value);
    assert_eq!(annotation.crash_id, new_annotation.crash_id);
    assert_eq!(annotation.product_id, new_annotation.product_id);
}

#[tokio::test]
async fn test_create_with_invalid_source() {
    let db = TestSetup::create_db().await;
    let (mut new_annotation, _, _) = create_test_annotation(&db).await;
    new_annotation.source = "invalid".to_string();

    let result = AnnotationsRepo::create(&db, new_annotation).await;

    assert!(result.is_err());
    match result {
        Err(RepoError::InvalidColumn(_)) => (),
        _ => panic!("Expected InvalidColumn error, got: {result:?}"),
    }
}

// update tests

#[tokio::test]
async fn test_update() {
    let db = TestSetup::create_db().await;
    let (new_annotation, _, _) = create_test_annotation(&db).await;
    let annotation_id = AnnotationsRepo::create(&db, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let mut annotation = AnnotationsRepo::get_by_id(&db, annotation_id.clone())
        .await
        .expect("Failed to get annotation")
        .expect("Annotation not found");

    annotation.key = "updated_key".to_string();
    annotation.value = "updated_value".to_string();

    let result = AnnotationsRepo::update(&db, annotation.clone())
        .await
        .expect("Failed to update annotation")
        .expect("No rows were updated");

    assert_eq!(result, annotation_id);

    let updated_annotation = AnnotationsRepo::get_by_id(&db, annotation_id.clone())
        .await
        .expect("Failed to get updated annotation")
        .expect("Updated annotation not found");

    assert_eq!(updated_annotation.key, "updated_key");
    assert_eq!(updated_annotation.value, "updated_value");
}

#[tokio::test]
async fn test_update_with_invalid_source() {
    let db = TestSetup::create_db().await;
    let (new_annotation, _, _) = create_test_annotation(&db).await;
    let annotation_id = AnnotationsRepo::create(&db, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let mut annotation = AnnotationsRepo::get_by_id(&db, annotation_id.clone())
        .await
        .expect("Failed to get annotation")
        .expect("Annotation not found");

    annotation.source = "invalid".to_string();

    let result = AnnotationsRepo::update(&db, annotation).await;

    assert!(result.is_err());
    match result {
        Err(RepoError::InvalidColumn(_)) => (),
        _ => panic!("Expected InvalidColumn error, got: {result:?}"),
    }
}

// count tests

#[tokio::test]
async fn test_count() {
    let db = TestSetup::create_db().await;
    let initial_count = AnnotationsRepo::count(&db)
        .await
        .expect("Failed to count annotations");

    let (new_annotation, _, _) = create_test_annotation(&db).await;
    let id = AnnotationsRepo::create(&db, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let new_count = AnnotationsRepo::count(&db)
        .await
        .expect("Failed to count annotations");

    assert_eq!(new_count, initial_count + 1);

    AnnotationsRepo::remove(&db, id)
        .await
        .expect("Failed to remove test annotation");

    let final_count = AnnotationsRepo::count(&db)
        .await
        .expect("Failed to count annotations");

    assert_eq!(final_count, initial_count);
}

// remove tests

#[tokio::test]
async fn test_remove() {
    let db = TestSetup::create_db().await;
    let (new_annotation, _, _) = create_test_annotation(&db).await;
    let id = AnnotationsRepo::create(&db, new_annotation)
        .await
        .expect("Failed to create test annotation");

    let annotation = AnnotationsRepo::get_by_id(&db, id.clone())
        .await
        .expect("Failed to get annotation")
        .expect("Annotation not found");

    assert_eq!(annotation.id, id);

    AnnotationsRepo::remove(&db, id.clone())
        .await
        .expect("Failed to remove annotation");

    let result = AnnotationsRepo::get_by_id(&db, id.clone())
        .await
        .expect("Failed to query for annotation");

    assert!(result.is_none(), "Annotation still exists after removal");
}
