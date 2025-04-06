#![cfg(all(test, feature = "ssr"))]

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use repos::api_token::*;

// Updated to use argon2 instead of sha256
fn hash_token(token: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(token.as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string()
}

async fn insert_test_api_token(
    pool: &PgPool,
    description: &str,
    token: &str,
    product_id: Option<Uuid>,
    user_id: Option<Uuid>,
    entitlements: &[&str],
) -> ApiToken {
    let product_id = match product_id {
        Some(id) => id,
        None => {
            // Create a test product first
            sqlx::query_scalar!(
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
            .expect("Failed to insert test product")
        }
    };

    // Convert entitlements to a SQL array
    let entitlements: Vec<String> = entitlements.iter().map(|&s| s.to_string()).collect();

    // Hash the token using argon2
    let token_hash = hash_token(token);

    // Create the API token
    sqlx::query_as!(
        ApiToken,
        r#"
        INSERT INTO guardrail.api_tokens (
            description, token_hash, product_id, user_id, entitlements, is_active
        )
        VALUES ($1, $2, $3, $4, $5, true)
        RETURNING id, description, token_hash, product_id, user_id, entitlements, is_active, last_used_at, expires_at, created_at, updated_at
        "#,
        description,
        token_hash,
        product_id,
        user_id,
        &entitlements as &[String],
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test API token")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let description = "Test Token";
    let inserted_token =
        insert_test_api_token(&pool, description, "test_token", None, None, &["symbol-upload"])
            .await;

    let found_token = ApiTokenRepo::get_by_id(&pool, inserted_token.id)
        .await
        .expect("Failed to get token by ID");

    assert!(found_token.is_some());
    let found_token = found_token.unwrap();
    assert_eq!(found_token.id, inserted_token.id);
    assert_eq!(found_token.description, description);

    let non_existent_id = Uuid::new_v4();
    let not_found = ApiTokenRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_token_hash(pool: PgPool) {
    let description = "Test Token";
    let token_hash = format!("unique_hash_{}", Uuid::new_v4());

    let new_token = NewApiToken {
        description: description.to_string(),
        token_hash: token_hash.clone(),
        product_id: None,
        user_id: None,
        entitlements: vec!["symbol-upload".to_string()],
        expires_at: None,
    };

    ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to create API token");

    let found_token = ApiTokenRepo::get_by_token_hash(&pool, &token_hash)
        .await
        .expect("Failed to get token by hash");

    assert!(found_token.is_some());
    let found_token = found_token.unwrap();
    assert_eq!(found_token.token_hash, token_hash);

    let non_existent_hash = "non-existent-hash";
    let not_found = ApiTokenRepo::get_by_token_hash(&pool, non_existent_hash)
        .await
        .expect("Failed to query with non-existent hash");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_last_used(pool: PgPool) {
    let token = insert_test_api_token(
        &pool,
        "Update Last Used",
        "update_token",
        None,
        None,
        &["symbol-upload"],
    )
    .await;

    assert!(token.last_used_at.is_none());

    ApiTokenRepo::update_last_used(&pool, token.id)
        .await
        .expect("Failed to update last used timestamp");

    let updated_token = ApiTokenRepo::get_by_id(&pool, token.id)
        .await
        .expect("Failed to get token after update")
        .expect("Token not found after update");

    assert!(updated_token.last_used_at.is_some());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_product_id(pool: PgPool) {
    // First create a product in the database
    let product_id = sqlx::query_scalar!(
        r#"
        INSERT INTO guardrail.products (name, description)
        VALUES ($1, $2)
        RETURNING id
        "#,
        format!("TestProduct_{}", Uuid::new_v4()),
        "Test Product Description"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create test product");

    insert_test_api_token(
        &pool,
        "Product Token 1",
        "product_token_1",
        Some(product_id),
        None,
        &["symbol-upload"],
    )
    .await;
    insert_test_api_token(
        &pool,
        "Product Token 2",
        "product_token_2",
        Some(product_id),
        None,
        &["minidump-upload"],
    )
    .await;
    insert_test_api_token(&pool, "Other Token", "other_token", None, None, &["token"]).await;

    let product_tokens = ApiTokenRepo::get_by_product_id(&pool, product_id)
        .await
        .expect("Failed to get tokens by product ID");

    assert_eq!(product_tokens.len(), 2);
    for token in product_tokens {
        assert_eq!(token.product_id, Some(product_id));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_user_id(pool: PgPool) {
    let user_id = Uuid::new_v4();

    // First, create a user since we have foreign key constraints
    sqlx::query!(
        "INSERT INTO guardrail.users (id, username, is_admin) VALUES ($1, $2, false)",
        user_id,
        format!("user_{}", Uuid::new_v4())
    )
    .execute(&pool)
    .await
    .expect("Failed to create test user");

    insert_test_api_token(
        &pool,
        "User Token 1",
        "user_token_1",
        None,
        Some(user_id),
        &["symbol-upload"],
    )
    .await;
    insert_test_api_token(
        &pool,
        "User Token 2",
        "user_token_2",
        None,
        Some(user_id),
        &["minidump-upload"],
    )
    .await;
    insert_test_api_token(&pool, "Other Token", "other_token", None, None, &["token"]).await;

    let user_tokens = ApiTokenRepo::get_by_user_id(&pool, user_id)
        .await
        .expect("Failed to get tokens by user ID");

    assert_eq!(user_tokens.len(), 2);
    for token in user_tokens {
        assert_eq!(token.user_id, Some(user_id));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let description = "New API Token";
    let token_hash = format!("create_hash_{}", Uuid::new_v4());
    let entitlements = vec!["symbol-upload".to_string(), "minidump-upload".to_string()];

    let new_token = NewApiToken {
        description: description.to_string(),
        token_hash: token_hash.clone(),
        product_id: None,
        user_id: None,
        entitlements: entitlements.clone(),
        expires_at: None,
    };

    let token_id = ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to create API token");

    let token = ApiTokenRepo::get_by_id(&pool, token_id)
        .await
        .expect("Failed to get created token")
        .expect("Created token not found");

    assert_eq!(token.description, description);
    assert_eq!(token.token_hash, token_hash);
    assert_eq!(token.entitlements, entitlements);
    assert!(token.is_active);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut token = insert_test_api_token(
        &pool,
        "Update Token",
        "update_token",
        None,
        None,
        &["symbol-upload"],
    )
    .await;

    token.description = "Updated Description".to_string();
    token.entitlements = vec!["symbol-upload".to_string(), "token".to_string()];
    token.is_active = false;

    ApiTokenRepo::update(&pool, token.clone())
        .await
        .expect("Failed to update API token");

    let updated_token = ApiTokenRepo::get_by_id(&pool, token.id)
        .await
        .expect("Failed to get updated token")
        .expect("Updated token not found");

    assert_eq!(updated_token.description, "Updated Description");
    assert_eq!(updated_token.entitlements, vec!["symbol-upload", "token"]);
    assert!(!updated_token.is_active);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_revoke(pool: PgPool) {
    let token = insert_test_api_token(
        &pool,
        "Revoke Token",
        "revoke_token",
        None,
        None,
        &["symbol-upload"],
    )
    .await;

    assert!(token.is_active);

    ApiTokenRepo::revoke(&pool, token.id)
        .await
        .expect("Failed to revoke API token");

    let updated_token = ApiTokenRepo::get_by_id(&pool, token.id)
        .await
        .expect("Failed to get revoked token")
        .expect("Revoked token not found");

    assert!(!updated_token.is_active);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_delete(pool: PgPool) {
    let token = insert_test_api_token(
        &pool,
        "Delete Token",
        "delete_token",
        None,
        None,
        &["symbol-upload"],
    )
    .await;

    ApiTokenRepo::delete(&pool, token.id)
        .await
        .expect("Failed to delete API token");

    let deleted_token = ApiTokenRepo::get_by_id(&pool, token.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_token.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_has_entitlement(pool: PgPool) {
    let token = insert_test_api_token(
        &pool,
        "Entitlement Token",
        "entitlement_token",
        None,
        None,
        &["symbol-upload", "minidump-upload"],
    )
    .await;

    // Test exact entitlement match
    assert!(ApiTokenRepo::has_entitlement(&token, "symbol-upload"));
    assert!(ApiTokenRepo::has_entitlement(&token, "minidump-upload"));
    assert!(!ApiTokenRepo::has_entitlement(&token, "non-existent"));

    // Create token with the special "token" entitlement that grants all permissions
    let super_token =
        insert_test_api_token(&pool, "Super Token", "super_token", None, None, &["token"]).await;

    assert!(ApiTokenRepo::has_entitlement(&super_token, "symbol-upload"));
    assert!(ApiTokenRepo::has_entitlement(&super_token, "minidump-upload"));
    assert!(ApiTokenRepo::has_entitlement(&super_token, "anything"));

    // Test expired token
    let mut expired_token = insert_test_api_token(
        &pool,
        "Expired Token",
        "expired_token",
        None,
        None,
        &["symbol-upload"],
    )
    .await;
    expired_token.expires_at = Some(Utc::now().naive_utc() - Duration::hours(1));

    ApiTokenRepo::update(&pool, expired_token.clone())
        .await
        .expect("Failed to update token expiry");

    assert!(!ApiTokenRepo::has_entitlement(&expired_token, "symbol-upload"));

    // Test inactive token
    let mut inactive_token = insert_test_api_token(
        &pool,
        "Inactive Token",
        "inactive_token",
        None,
        None,
        &["symbol-upload"],
    )
    .await;
    inactive_token.is_active = false;

    ApiTokenRepo::update(&pool, inactive_token.clone())
        .await
        .expect("Failed to update token active status");

    assert!(!ApiTokenRepo::has_entitlement(&inactive_token, "symbol-upload"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let initial_tokens = ApiTokenRepo::get_all(&pool)
        .await
        .expect("Failed to get initial tokens");

    let initial_count = initial_tokens.len();

    insert_test_api_token(&pool, "All Token 1", "all_token_1", None, None, &["symbol-upload"])
        .await;
    insert_test_api_token(&pool, "All Token 2", "all_token_2", None, None, &["minidump-upload"])
        .await;
    insert_test_api_token(&pool, "All Token 3", "all_token_3", None, None, &["token"]).await;

    let all_tokens = ApiTokenRepo::get_all(&pool)
        .await
        .expect("Failed to get all tokens");

    assert_eq!(all_tokens.len(), initial_count + 3);
}
