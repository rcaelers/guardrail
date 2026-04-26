#![cfg(test)]

use serde_json::json;
use testware::setup::TestSetup;
use uuid::Uuid;

use data::credentials::*;
use repos::credentials::*;

use testware::{create_random_test_user, create_test_credential};

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let data = json!({"token": "fake-token-123", "scope": "repo"});

    let inserted_credential = create_test_credential(&db, data.clone(), None).await;

    let found_credential = CredentialsRepo::get_by_id(&db, inserted_credential.id.clone())
        .await
        .expect("Failed to get credential by ID");

    assert!(found_credential.is_some());
    let found_credential = found_credential.unwrap();
    assert_eq!(found_credential.id, inserted_credential.id);
    assert_eq!(found_credential.data, data);
}

#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4().to_string();
    let not_found = CredentialsRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_get_all_by_user_id() {
    let db = TestSetup::create_db().await;
    let user_id = create_random_test_user(&db).await;

    let credentials_data = vec![
        (json!({"token": "github-token", "scope": "repo"})),
        (json!({"token": "gitlab-token", "scope": "api"})),
        (json!({"token": "npm-token"})),
    ];

    for data in &credentials_data {
        create_test_credential(&db, data.clone(), Some(user_id.clone())).await;
    }

    let other_user_id = create_random_test_user(&db).await;
    create_test_credential(&db, json!({"token": "other-token"}), Some(other_user_id)).await;

    let user_credentials = CredentialsRepo::get_all_by_user_id(&db, user_id.clone())
        .await
        .expect("Failed to get credentials by user ID");

    assert_eq!(user_credentials.len(), credentials_data.len());

    for credential in user_credentials {
        assert_eq!(credential.user_id, user_id);
    }
}

#[tokio::test]
async fn test_create() {
    let db = TestSetup::create_db().await;
    let user_id = create_random_test_user(&db).await;
    let data = json!({
        "username": "test_username",
        "password": "test_password",
        "url": "https://example.com"
    });

    let credential_id = CredentialsRepo::create(
        &db,
        NewCredential {
            user_id: user_id.clone(),
            data: data.clone(),
        },
    )
    .await
    .expect("Failed to create credential");

    let created_credential = CredentialsRepo::get_by_id(&db, credential_id)
        .await
        .expect("Failed to get created credential")
        .expect("Created credential not found");

    assert_eq!(created_credential.user_id, user_id);
    assert_eq!(created_credential.data, data);
}

#[tokio::test]
async fn test_update_data() {
    let db = TestSetup::create_db().await;
    let credential = create_test_credential(&db, json!({"key": "original-key-data"}), None).await;

    let updated_data = json!({
        "key": "updated-key-data",
        "comment": "Added comment"
    });

    let updated_id = CredentialsRepo::update_data(&db, credential.id.clone(), updated_data.clone())
        .await
        .expect("Failed to update credential")
        .expect("Credential not found when updating");

    assert_eq!(updated_id, credential.id);

    let updated_credential = CredentialsRepo::get_by_id(&db, credential.id.clone())
        .await
        .expect("Failed to get updated credential")
        .expect("Updated credential not found");

    assert_eq!(updated_credential.data, updated_data);
    assert!(updated_credential.last_used >= credential.last_used);
}
