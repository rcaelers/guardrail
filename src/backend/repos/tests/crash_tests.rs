#![cfg(test)]

use testware::setup::TestSetup;
use uuid::Uuid;

use common::{QueryParams, SortOrder};
use data::crash::*;
use repos::crash::*;

use testware::{create_test_crash, create_test_product};

// get_by_id tests

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let signature = "Test Crash";
    let inserted_crash = create_test_crash(&db, Some(signature), None).await;

    let found_crash = CrashRepo::get_by_id(&db, inserted_crash.id)
        .await
        .expect("Failed to get crash by ID");

    assert!(found_crash.is_some());
    let found_crash = found_crash.unwrap();
    assert_eq!(found_crash.id, inserted_crash.id);
    assert_eq!(found_crash.signature, Some(signature.to_string()));

    let non_existent_id = Uuid::new_v4();
    let not_found = CrashRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

// get_all tests

#[tokio::test]
async fn test_get_all() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;

    let test_crash_data = vec![("Crash A"), ("Crash B"), ("Crash C")];

    for signature in &test_crash_data {
        create_test_crash(&db, Some(signature), Some(product.id)).await;
    }

    let query_params = QueryParams::default();
    let all_crashes = CrashRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get all crashes");

    assert!(all_crashes.len() >= test_crash_data.len());

    let mut query_params = QueryParams::default();
    query_params
        .sorting
        .push_back(("signature".to_string(), SortOrder::Ascending));

    let sorted_crashes = CrashRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get sorted crashes");

    for i in 1..sorted_crashes.len() {
        if sorted_crashes[i - 1].signature == sorted_crashes[i].signature {
            continue;
        }
        assert!(sorted_crashes[i - 1].signature <= sorted_crashes[i].signature);
    }

    let query_params = QueryParams {
        filter: Some("Crash B".to_string()),
        ..QueryParams::default()
    };

    let filtered_crashes = CrashRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get filtered crashes");

    for crash in filtered_crashes {
        assert!(
            crash
                .signature
                .as_ref()
                .unwrap_or(&String::new())
                .contains("Crash B")
        );
    }
}

// create tests

#[tokio::test]
async fn test_create() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;

    let new_crash = NewCrash {
        id: None,
        signature: Some("Test Crash Signature".to_string()),
        product_id: product.id,
        minidump: Some(Uuid::new_v4()),
        report: Some(serde_json::json!({
            "error": "Division by zero",
            "stack_trace": "at main",
        })),
    };

    let crash_id = CrashRepo::create(&db, new_crash.clone())
        .await
        .expect("Failed to create crash");

    let created_crash = CrashRepo::get_by_id(&db, crash_id)
        .await
        .expect("Failed to get created crash")
        .expect("Created crash not found");

    assert_eq!(created_crash.signature, new_crash.signature);
    assert_eq!(created_crash.product_id, new_crash.product_id);
}

// update tests

#[tokio::test]
async fn test_update() {
    let db = TestSetup::create_db().await;
    let mut crash = create_test_crash(&db, Some("Original Crash"), None).await;

    crash.signature = Some("Updated Crash".to_string());

    let updated_id = CrashRepo::update(&db, crash.clone())
        .await
        .expect("Failed to update crash")
        .expect("Crash not found when updating");

    assert_eq!(updated_id, crash.id);

    let updated_crash = CrashRepo::get_by_id(&db, crash.id)
        .await
        .expect("Failed to get updated crash")
        .expect("Updated crash not found");

    assert_eq!(updated_crash.signature, Some("Updated Crash".to_string()));
}

// remove tests

#[tokio::test]
async fn test_remove() {
    let db = TestSetup::create_db().await;
    let crash = create_test_crash(&db, Some("Crash to Delete"), None).await;

    CrashRepo::remove(&db, crash.id)
        .await
        .expect("Failed to remove crash");

    let deleted_crash = CrashRepo::get_by_id(&db, crash.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_crash.is_none());
}

// count tests

#[tokio::test]
async fn test_count() {
    let db = TestSetup::create_db().await;
    let initial_count = CrashRepo::count(&db)
        .await
        .expect("Failed to count initial crashes");

    let product = create_test_product(&db).await;

    let test_crashes = vec![("Count Crash 1"), ("Count Crash 2"), ("Count Crash 3")];

    for signature in &test_crashes {
        create_test_crash(&db, Some(signature), Some(product.id)).await;
    }

    let new_count = CrashRepo::count(&db)
        .await
        .expect("Failed to count crashes after insertion");

    assert_eq!(new_count, initial_count + test_crashes.len() as i64);
}
