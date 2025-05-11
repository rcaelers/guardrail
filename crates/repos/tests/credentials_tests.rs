#![cfg(test)]

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use data::credentials::*;
use repos::credentials::*;

use testware::{create_random_test_user, create_test_credential};

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let data = json!({"token": "fake-token-123", "scope": "repo"});

    let inserted_credential = create_test_credential(&pool, data.clone(), None).await;

    let found_credential = CredentialsRepo::get_by_id(&pool, inserted_credential.id)
        .await
        .expect("Failed to get credential by ID");

    assert!(found_credential.is_some());
    let found_credential = found_credential.unwrap();
    assert_eq!(found_credential.id, inserted_credential.id);
    assert_eq!(found_credential.data, data);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = CredentialsRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_by_user_id(pool: PgPool) {
    let user_id = create_random_test_user(&pool).await;

    let credentials_data = vec![
        (json!({"token": "github-token", "scope": "repo"})),
        (json!({"token": "gitlab-token", "scope": "api"})),
        (json!({"token": "npm-token"})),
    ];

    for data in &credentials_data {
        create_test_credential(&pool, data.clone(), Some(user_id)).await;
    }

    let other_user_id = create_random_test_user(&pool).await;
    create_test_credential(&pool, json!({"token": "other-token"}), Some(other_user_id)).await;

    let user_credentials = CredentialsRepo::get_all_by_user_id(&pool, user_id)
        .await
        .expect("Failed to get credentials by user ID");

    assert_eq!(user_credentials.len(), credentials_data.len());

    for credential in user_credentials {
        assert_eq!(credential.user_id, user_id);
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let user_id = create_random_test_user(&pool).await;
    let data = json!({
        "username": "test_username",
        "password": "test_password",
        "url": "https://example.com"
    });

    let credential_id = CredentialsRepo::create(
        &pool,
        NewCredential {
            user_id,
            data: data.clone(),
        },
    )
    .await
    .expect("Failed to create credential");

    let created_credential = CredentialsRepo::get_by_id(&pool, credential_id)
        .await
        .expect("Failed to get created credential")
        .expect("Created credential not found");

    assert_eq!(created_credential.user_id, user_id);
    assert_eq!(created_credential.data, data);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_data(pool: PgPool) {
    let credential = create_test_credential(&pool, json!({"key": "original-key-data"}), None).await;

    let updated_data = json!({
        "key": "updated-key-data",
        "comment": "Added comment"
    });

    let updated_id = CredentialsRepo::update_data(&pool, credential.id, updated_data.clone())
        .await
        .expect("Failed to update credential")
        .expect("Credential not found when updating");

    assert_eq!(updated_id, credential.id);

    let updated_credential = CredentialsRepo::get_by_id(&pool, credential.id)
        .await
        .expect("Failed to get updated credential")
        .expect("Updated credential not found");

    assert_eq!(updated_credential.data, updated_data);
    assert!(updated_credential.last_used >= credential.last_used);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_data_error(pool: PgPool) {
    let credential =
        create_test_credential(&pool, json!({"key": "original-key-data-error"}), None).await;

    let updated_data = json!({
        "key": "should-fail-key-data",
        "comment": "This update should fail"
    });

    pool.close().await;

    let result = CredentialsRepo::update_data(&pool, credential.id, updated_data.clone()).await;
    assert!(result.is_err(), "Expected an error when updating credential data with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_error(pool: PgPool) {
    let user_id = create_random_test_user(&pool).await;

    let new_credential = NewCredential {
        user_id,
        data: json!({"username": "test", "password": "password123"}),
    };

    pool.close().await;

    let result = CredentialsRepo::create(&pool, new_credential).await;
    assert!(result.is_err(), "Expected an error when creating credential with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let credential = create_test_credential(&pool, json!({"key": "value"}), None).await;

    pool.close().await;

    let result = CredentialsRepo::get_by_id(&pool, credential.id).await;
    assert!(result.is_err(), "Expected an error when getting credential by ID with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_by_user_id_error(pool: PgPool) {
    let user_id = create_random_test_user(&pool).await;

    create_test_credential(&pool, json!({"token": "github-token"}), Some(user_id)).await;

    pool.close().await;

    let result = CredentialsRepo::get_all_by_user_id(&pool, user_id).await;
    assert!(
        result.is_err(),
        "Expected an error when getting credentials by user ID with closed pool"
    );
}
