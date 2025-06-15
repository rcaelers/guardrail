#![cfg(test)]

use object_store::{ObjectStore, path::Path};
use serde_json::json;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

use jobs::jobs::MinidumpJob;
use jobs::maintenance::JobCleaner;

#[tokio::test]
async fn test_extract_crash_id_from_job() {
    let crash_id = Uuid::new_v4();
    let crash_data = json!({
        "crash_id": crash_id.to_string(),
        "submission_timestamp": "2023-10-01T12:00:00Z",
        "product": "TestProduct",
        "version": "1.0.0"
    });

    let job = MinidumpJob { crash: crash_data };

    let extracted_crash_id = JobCleaner::extract_crash_id_from_job(&job);
    assert_eq!(extracted_crash_id, Some(crash_id));
}

#[tokio::test]
async fn test_extract_crash_id_from_job_missing_crash_id() {
    let crash_data = json!({
        "submission_timestamp": "2023-10-01T12:00:00Z",
        "product": "TestProduct",
        "version": "1.0.0"
        // Missing crash_id
    });

    let job = MinidumpJob { crash: crash_data };

    let extracted_crash_id = JobCleaner::extract_crash_id_from_job(&job);
    assert_eq!(extracted_crash_id, None);
}

#[tokio::test]
async fn test_extract_crash_id_from_job_invalid_crash_id() {
    let crash_data = json!({
        "crash_id": "invalid-uuid",
        "submission_timestamp": "2023-10-01T12:00:00Z",
        "product": "TestProduct",
        "version": "1.0.0"
    });

    let job = MinidumpJob { crash: crash_data };

    let extracted_crash_id = JobCleaner::extract_crash_id_from_job(&job);
    assert_eq!(extracted_crash_id, None);
}

#[tokio::test]
async fn test_remove_crash_info_files() {
    let store = Arc::new(object_store::memory::InMemory::new());

    // Create test crash IDs
    let crash_id_1 = Uuid::new_v4();
    let crash_id_2 = Uuid::new_v4();
    let crash_id_3 = Uuid::new_v4(); // This one won't have a file

    let mut crash_ids = HashSet::new();
    crash_ids.insert(crash_id_1);
    crash_ids.insert(crash_id_2);
    crash_ids.insert(crash_id_3);

    // Create crash_info files for crash_id_1 and crash_id_2
    let crash_info_1 = json!({
        "crash_id": crash_id_1.to_string(),
        "status": "processed"
    });
    let crash_info_2 = json!({
        "crash_id": crash_id_2.to_string(),
        "status": "processed"
    });

    store
        .put(
            &Path::from(format!("crashes/{crash_id_1}.json")),
            serde_json::to_vec(&crash_info_1).unwrap().into(),
        )
        .await
        .expect("Failed to put crash_info_1");

    store
        .put(
            &Path::from(format!("crashes/{crash_id_2}.json")),
            serde_json::to_vec(&crash_info_2).unwrap().into(),
        )
        .await
        .expect("Failed to put crash_info_2");

    // Also create an unrelated file that shouldn't be touched
    let unrelated_crash_id = Uuid::new_v4();
    let unrelated_crash_info = json!({
        "crash_id": unrelated_crash_id.to_string(),
        "status": "processing"
    });
    store
        .put(
            &Path::from(format!("crashes/{unrelated_crash_id}.json")),
            serde_json::to_vec(&unrelated_crash_info).unwrap().into(),
        )
        .await
        .expect("Failed to put unrelated crash_info");

    // Verify files exist before cleanup
    assert!(
        store
            .get(&Path::from(format!("crashes/{crash_id_1}.json")))
            .await
            .is_ok()
    );
    assert!(
        store
            .get(&Path::from(format!("crashes/{crash_id_2}.json")))
            .await
            .is_ok()
    );
    assert!(
        store
            .get(&Path::from(format!("crashes/{unrelated_crash_id}.json")))
            .await
            .is_ok()
    );

    // Call the remove_crash_info_files method directly
    let deleted_count = JobCleaner::remove_crash_info_files(&*store, &crash_ids)
        .await
        .expect("Failed to remove crash_info files");

    // Should have deleted 2 files (crash_id_1 and crash_id_2, but not crash_id_3 since it didn't exist)
    assert_eq!(deleted_count, 2);

    // Verify files are deleted
    assert!(
        store
            .get(&Path::from(format!("crashes/{crash_id_1}.json")))
            .await
            .is_err()
    );
    assert!(
        store
            .get(&Path::from(format!("crashes/{crash_id_2}.json")))
            .await
            .is_err()
    );

    // Verify unrelated file is still there
    assert!(
        store
            .get(&Path::from(format!("crashes/{unrelated_crash_id}.json")))
            .await
            .is_ok()
    );
}
