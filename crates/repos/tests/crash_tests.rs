#![cfg(test)]

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use common::{QueryParams, SortOrder};
use data::crash::*;
use repos::crash::*;

mod testcommon;
use testcommon::{create_test_crash as insert_test_crash, setup_test_dependencies};

// get_by_id tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let summary = "Test Crash";
    let report = json!({
        "crash_info": {
            "type": "SIGSEGV",
            "address": "0x0",
            "crashing_thread": 0
        },
        "threads": [
            { "frames": [{"module": "test", "function": "main", "offset": 0}] }
        ]
    });

    let inserted_crash = insert_test_crash(&pool, summary, report.clone(), None, None).await;

    let found_crash = CrashRepo::get_by_id(&pool, inserted_crash.id)
        .await
        .expect("Failed to get crash by ID");

    assert!(found_crash.is_some());
    let found_crash = found_crash.unwrap();
    assert_eq!(found_crash.id, inserted_crash.id);
    assert_eq!(found_crash.summary, summary);
    assert_eq!(found_crash.report, report);

    let non_existent_id = Uuid::new_v4();
    let not_found = CrashRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

// get_all tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let test_crash_data = vec![
        ("Crash A", json!({"type": "null pointer", "address": "0x0"})),
        ("Crash B", json!({"type": "stack overflow", "address": "0xFFF"})),
        ("Crash C", json!({"type": "assertion failure", "condition": "x > 0"})),
    ];

    for (summary, report) in &test_crash_data {
        insert_test_crash(&pool, summary, report.clone(), Some(product_id), Some(version_id)).await;
    }

    let query_params = QueryParams::default();
    let all_crashes = CrashRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all crashes");

    assert!(all_crashes.len() >= test_crash_data.len());

    let mut query_params = QueryParams::default();
    query_params
        .sorting
        .push_back(("summary".to_string(), SortOrder::Ascending));

    let sorted_crashes = CrashRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get sorted crashes");

    for i in 1..sorted_crashes.len() {
        if sorted_crashes[i - 1].summary == sorted_crashes[i].summary {
            continue;
        }
        assert!(sorted_crashes[i - 1].summary <= sorted_crashes[i].summary);
    }

    let query_params = QueryParams {
        filter: Some("Crash B".to_string()),
        ..QueryParams::default()
    };

    let filtered_crashes = CrashRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get filtered crashes");

    for crash in filtered_crashes {
        assert!(crash.summary.contains("Crash B"));
    }
}

// create tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let report_data = json!({
        "crash_reason": "division by zero",
        "cpu": "x86_64",
        "os": "Linux",
        "frames": [
            { "function": "calculate", "offset": 123 },
            { "function": "main", "offset": 456 }
        ]
    });

    let new_crash = NewCrash {
        summary: "Calculation Error".to_string(),
        report: report_data.clone(),
        version_id,
        product_id,
    };

    let crash_id = CrashRepo::create(&pool, new_crash.clone())
        .await
        .expect("Failed to create crash");

    let created_crash = CrashRepo::get_by_id(&pool, crash_id)
        .await
        .expect("Failed to get created crash")
        .expect("Created crash not found");

    assert_eq!(created_crash.summary, new_crash.summary);
    assert_eq!(created_crash.report, new_crash.report);
    assert_eq!(created_crash.version_id, new_crash.version_id);
    assert_eq!(created_crash.product_id, new_crash.product_id);
}

// update tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut crash =
        insert_test_crash(&pool, "Original Crash", json!({"original": "data"}), None, None).await;

    crash.summary = "Updated Crash".to_string();
    crash.report = json!({"updated": "data", "with": "more information"});

    let updated_id = CrashRepo::update(&pool, crash.clone())
        .await
        .expect("Failed to update crash")
        .expect("Crash not found when updating");

    assert_eq!(updated_id, crash.id);

    let updated_crash = CrashRepo::get_by_id(&pool, crash.id)
        .await
        .expect("Failed to get updated crash")
        .expect("Updated crash not found");

    assert_eq!(updated_crash.summary, "Updated Crash");
    assert_eq!(updated_crash.report, json!({"updated": "data", "with": "more information"}));
}

// remove tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let crash =
        insert_test_crash(&pool, "Crash to Delete", json!({"delete": "me"}), None, None).await;

    CrashRepo::remove(&pool, crash.id)
        .await
        .expect("Failed to remove crash");

    let deleted_crash = CrashRepo::get_by_id(&pool, crash.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_crash.is_none());
}

// count tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = CrashRepo::count(&pool)
        .await
        .expect("Failed to count initial crashes");

    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let test_crashes = vec![
        ("Count Crash 1", json!({"count": 1})),
        ("Count Crash 2", json!({"count": 2})),
        ("Count Crash 3", json!({"count": 3})),
    ];

    for (summary, report) in &test_crashes {
        insert_test_crash(&pool, summary, report.clone(), Some(product_id), Some(version_id)).await;
    }

    let new_count = CrashRepo::count(&pool)
        .await
        .expect("Failed to count crashes after insertion");

    assert_eq!(new_count, initial_count + test_crashes.len() as i64);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_error(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let new_crash = NewCrash {
        summary: "Test crash with closed pool".to_string(),
        report: json!({
            "crash_info": {
                "type": "SIGSEGV",
                "address": "0x0"
            }
        }),
        product_id,
        version_id,
    };

    pool.close().await;

    let result = CrashRepo::create(&pool, new_crash).await;
    assert!(result.is_err(), "Expected an error when creating crash with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let crash =
        insert_test_crash(&pool, "Test crash for closed pool", json!({"test": "data"}), None, None)
            .await;

    pool.close().await;

    let result = CrashRepo::get_by_id(&pool, crash.id).await;
    assert!(result.is_err(), "Expected an error when getting crash by ID with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_error(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    insert_test_crash(
        &pool,
        "Test crash for get_all with closed pool",
        json!({"test": "data"}),
        Some(product_id),
        Some(version_id),
    )
    .await;

    pool.close().await;

    let query_params = QueryParams::default();
    let result = CrashRepo::get_all(&pool, query_params).await;
    assert!(result.is_err(), "Expected an error when getting all crashes with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_error(pool: PgPool) {
    let mut crash = insert_test_crash(
        &pool,
        "Original Crash for Update Test",
        json!({"original": "data"}),
        None,
        None,
    )
    .await;

    crash.summary = "Updated Crash With Closed Pool".to_string();
    crash.report = json!({"updated": "data", "with": "closed pool"});

    pool.close().await;

    let result = CrashRepo::update(&pool, crash.clone()).await;
    assert!(result.is_err(), "Expected an error when updating crash with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove_error(pool: PgPool) {
    let crash = insert_test_crash(
        &pool,
        "Crash to Delete with Error",
        json!({"delete": "me with error"}),
        None,
        None,
    )
    .await;

    pool.close().await;

    let result = CrashRepo::remove(&pool, crash.id).await;
    assert!(result.is_err(), "Expected an error when removing crash with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count_error(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    insert_test_crash(
        &pool,
        "Test crash for count with closed pool",
        json!({"test": "data"}),
        Some(product_id),
        Some(version_id),
    )
    .await;

    pool.close().await;

    let result = CrashRepo::count(&pool).await;
    assert!(result.is_err(), "Expected an error when counting crashes with closed pool");
}
