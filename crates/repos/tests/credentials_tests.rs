#![cfg(all(test, feature = "ssr"))]

use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use repos::credentials::*;

async fn create_test_user(pool: &PgPool) -> Uuid {
    sqlx::query_scalar!(
        r#"
        INSERT INTO guardrail.users (username, is_admin)
        VALUES ($1, false)
        RETURNING id
        "#,
        format!("testuser_{}", Uuid::new_v4())
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create test user")
}

async fn insert_test_credential(
    pool: &PgPool,
    name: &str,
    data: serde_json::Value,
    user_id: Option<Uuid>,
) -> Credential {
    let user_id = match user_id {
        Some(id) => id,
        None => create_test_user(pool).await,
    };
    let now = Utc::now().naive_utc();

    sqlx::query_as!(
        Credential,
        r#"
        INSERT INTO guardrail.credentials (
            user_id, name, data, last_used
        )
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, name, data, created_at, updated_at, last_used
        "#,
        user_id,
        name,
        data,
        now
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test credential")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let name = "github";
    let data = json!({"token": "fake-token-123", "scope": "repo"});

    let inserted_credential = insert_test_credential(&pool, name, data.clone(), None).await;

    let found_credential = CredentialRepo::get_by_id(&pool, inserted_credential.id)
        .await
        .expect("Failed to get credential by ID");

    assert!(found_credential.is_some());
    let found_credential = found_credential.unwrap();
    assert_eq!(found_credential.id, inserted_credential.id);
    assert_eq!(found_credential.name, name);
    assert_eq!(found_credential.data, data);

    let non_existent_id = Uuid::new_v4();
    let not_found = CredentialRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_by_user_id(pool: PgPool) {
    let user_id = create_test_user(&pool).await;

    let credentials_data = vec![
        ("github", json!({"token": "github-token", "scope": "repo"})),
        ("gitlab", json!({"token": "gitlab-token", "scope": "api"})),
        ("npm", json!({"token": "npm-token"})),
    ];

    for (name, data) in &credentials_data {
        insert_test_credential(&pool, name, data.clone(), Some(user_id)).await;
    }

    // Create another user with different credentials
    let other_user_id = create_test_user(&pool).await;
    insert_test_credential(
        &pool,
        "other-cred",
        json!({"token": "other-token"}),
        Some(other_user_id),
    )
    .await;

    let user_credentials = CredentialRepo::get_all_by_user_id(&pool, user_id)
        .await
        .expect("Failed to get credentials by user ID");

    assert_eq!(user_credentials.len(), credentials_data.len());

    for credential in user_credentials {
        assert_eq!(credential.user_id, user_id);
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_by_name(pool: PgPool) {
    let name = "docker";

    let user_ids = [
        create_test_user(&pool).await,
        create_test_user(&pool).await,
        create_test_user(&pool).await,
    ];

    for (i, user_id) in user_ids.iter().enumerate() {
        insert_test_credential(
            &pool,
            name,
            json!({"registry": "docker.io", "token": format!("docker-token-{}", i)}),
            Some(*user_id),
        )
        .await;
    }

    // Insert a credential with a different name
    insert_test_credential(
        &pool,
        "different-name",
        json!({"token": "different-token"}),
        Some(user_ids[0]),
    )
    .await;

    let name_credentials = CredentialRepo::get_all_by_name(&pool, name)
        .await
        .expect("Failed to get credentials by name");

    assert_eq!(name_credentials.len(), user_ids.len());

    for credential in name_credentials {
        assert_eq!(credential.name, name);
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let user_id = create_test_user(&pool).await;
    let data = json!({
        "username": "test_username",
        "password": "test_password",
        "url": "https://example.com"
    });

    let credential_id = CredentialRepo::create(&pool, user_id, data.clone())
        .await
        .expect("Failed to create credential");

    let created_credential = CredentialRepo::get_by_id(&pool, credential_id)
        .await
        .expect("Failed to get created credential")
        .expect("Created credential not found");

    assert_eq!(created_credential.user_id, user_id);
    assert_eq!(created_credential.data, data);
    // Note: The name is hardcoded to "fixme" in the implementation
    assert_eq!(created_credential.name, "fixme");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_data(pool: PgPool) {
    let credential =
        insert_test_credential(&pool, "ssh-key", json!({"key": "original-key-data"}), None).await;

    let updated_data = json!({
        "key": "updated-key-data",
        "comment": "Added comment"
    });

    let updated_id = CredentialRepo::update_data(&pool, credential.id, updated_data.clone())
        .await
        .expect("Failed to update credential")
        .expect("Credential not found when updating");

    assert_eq!(updated_id, credential.id);

    let updated_credential = CredentialRepo::get_by_id(&pool, credential.id)
        .await
        .expect("Failed to get updated credential")
        .expect("Updated credential not found");

    assert_eq!(updated_credential.data, updated_data);
    // Verify that last_used timestamp was updated
    assert!(updated_credential.last_used >= credential.last_used);
}
