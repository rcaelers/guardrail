use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use data::api_token::NewApiToken;
use repos::api_token::*;

use testware::{create_test_product, create_test_token, create_test_user};

// get_by_id tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let description = "Test Token";
    let (_, inserted_token) =
        create_test_token(&pool, description, Some(product.id), None, &["symbol-upload"]).await;

    let found_token = ApiTokenRepo::get_by_id(&pool, inserted_token.id)
        .await
        .expect("Failed to get token by ID");

    assert!(found_token.is_some());
    let found_token = found_token.unwrap();
    assert_eq!(found_token.id, inserted_token.id);
    assert_eq!(found_token.description, description);
    assert_eq!(found_token.product_id, Some(product.id));
    assert_eq!(found_token.entitlements, vec!["symbol-upload"]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = ApiTokenRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let description = "Test Token";
    let (_, inserted_token) =
        create_test_token(&pool, description, None, None, &["symbol-upload"]).await;

    pool.close().await;

    let result = ApiTokenRepo::get_by_id(&pool, inserted_token.id).await;
    assert!(result.is_err(), "Expected an error when getting token by ID with closed pool");
}

// get_by_token_id tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_token_id(pool: PgPool) {
    let description = "Test Token";
    let token_hash = format!("unique_hash_{}", Uuid::new_v4());
    let token_id = Uuid::new_v4();

    let new_token = NewApiToken {
        description: description.to_string(),
        token_id,
        token_hash: token_hash.clone(),
        product_id: None,
        user_id: None,
        entitlements: vec!["symbol-upload".to_string()],
        expires_at: None,
        is_active: true,
    };

    ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to create API token");

    let found_token = ApiTokenRepo::get_by_token_id(&pool, token_id)
        .await
        .expect("Failed to get token by hash");

    assert!(found_token.is_some());
    let found_token = found_token.unwrap();
    assert_eq!(found_token.token_hash, token_hash);
    assert_eq!(found_token.token_id, token_id);
    assert_eq!(found_token.description, description);
    assert_eq!(found_token.entitlements, vec!["symbol-upload"]);
    assert!(found_token.is_active);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_token_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = ApiTokenRepo::get_by_token_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent hash");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_token_id_error(pool: PgPool) {
    let description = "Test Token";
    let token_id = Uuid::new_v4();
    let token_hash = format!("unique_hash_{}", Uuid::new_v4());

    let new_token = NewApiToken {
        description: description.to_string(),
        token_id,
        token_hash: token_hash.clone(),
        product_id: None,
        user_id: None,
        entitlements: vec!["symbol-upload".to_string()],
        expires_at: None,
        is_active: true,
    };

    ApiTokenRepo::create(&pool, new_token)
        .await
        .expect("Failed to create API token");

    pool.close().await;

    let result = ApiTokenRepo::get_by_token_id(&pool, token_id).await;
    assert!(result.is_err(), "Expected an error when getting token by hash with closed pool");
}

// update_last_used tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_last_used(pool: PgPool) {
    let (_, token) =
        create_test_token(&pool, "Update Last Used", None, None, &["symbol-upload"]).await;

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
async fn test_update_last_used_error(pool: PgPool) {
    let (_, token) =
        create_test_token(&pool, "Update Last Used Error", None, None, &["symbol-upload"]).await;

    pool.close().await;

    let result = ApiTokenRepo::update_last_used(&pool, token.id).await;
    assert!(
        result.is_err(),
        "Expected an error when updating last used timestamp with closed pool"
    );
}

// get_by_product_id tests

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_product_id(pool: PgPool) {
    let product = create_test_product(&pool).await;

    create_test_token(&pool, "Product Token 1", Some(product.id), None, &["symbol-upload"]).await;
    create_test_token(&pool, "Product Token 2", Some(product.id), None, &["minidump-upload"]).await;
    create_test_token(&pool, "Other Token", None, None, &["token"]).await;

    let product_tokens = ApiTokenRepo::get_by_product_id(&pool, product.id)
        .await
        .expect("Failed to get tokens by product ID");

    assert_eq!(product_tokens.len(), 2);
    for token in product_tokens {
        assert_eq!(token.product_id, Some(product.id));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_product_id_error(pool: PgPool) {
    let product = create_test_product(&pool).await;

    pool.close().await;

    let result = ApiTokenRepo::get_by_product_id(&pool, product.id).await;
    assert!(
        result.is_err(),
        "Expected an error when getting tokens by product ID with closed pool"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_user_id(pool: PgPool) {
    let username = format!("user_{}", Uuid::new_v4());
    let user = create_test_user(&pool, &username, true).await;

    create_test_token(&pool, "User Token 1", None, Some(user.id), &["symbol-upload"]).await;
    create_test_token(&pool, "User Token 2", None, Some(user.id), &["minidump-upload"]).await;
    create_test_token(&pool, "Other Token", None, None, &["token"]).await;

    let user_tokens = ApiTokenRepo::get_by_user_id(&pool, user.id)
        .await
        .expect("Failed to get tokens by user ID");

    assert_eq!(user_tokens.len(), 2);
    for token in user_tokens {
        assert_eq!(token.user_id, Some(user.id));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_user_id_error(pool: PgPool) {
    let username = format!("user_{}", Uuid::new_v4());
    let user = create_test_user(&pool, &username, true).await;

    create_test_token(&pool, "User Token 1", None, Some(user.id), &["symbol-upload"]).await;

    pool.close().await;

    let result = ApiTokenRepo::get_by_user_id(&pool, user.id).await;
    assert!(result.is_err(), "Expected an error when getting tokens by user ID with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let description = "New API Token";
    let token_hash = format!("create_hash_{}", Uuid::new_v4());
    let entitlements = vec!["symbol-upload".to_string(), "minidump-upload".to_string()];

    let new_token = NewApiToken {
        description: description.to_string(),
        token_id: Uuid::new_v4(),
        token_hash: token_hash.clone(),
        product_id: None,
        user_id: None,
        entitlements: entitlements.clone(),
        expires_at: None,
        is_active: true,
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
async fn test_create_error(pool: PgPool) {
    let description = "New API Token Error";
    let token_hash = format!("create_error_hash_{}", Uuid::new_v4());
    let entitlements = vec!["symbol-upload".to_string(), "minidump-upload".to_string()];

    let new_token = NewApiToken {
        description: description.to_string(),
        token_id: Uuid::new_v4(),
        token_hash: token_hash.clone(),
        product_id: None,
        user_id: None,
        entitlements: entitlements.clone(),
        expires_at: None,
        is_active: true,
    };

    pool.close().await;

    let result = ApiTokenRepo::create(&pool, new_token).await;
    assert!(result.is_err(), "Expected an error when creating API token with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let (_, mut token) =
        create_test_token(&pool, "Update Token", None, None, &["symbol-upload"]).await;

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
async fn test_update_error(pool: PgPool) {
    let (_, mut token) =
        create_test_token(&pool, "Update Token Error", None, None, &["symbol-upload"]).await;

    token.description = "Updated Description".to_string();
    token.entitlements = vec!["symbol-upload".to_string(), "token".to_string()];

    pool.close().await;

    let result = ApiTokenRepo::update(&pool, token.clone()).await;
    assert!(result.is_err(), "Expected an error when updating API token with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_revoke(pool: PgPool) {
    let (_, token) = create_test_token(&pool, "Revoke Token", None, None, &["symbol-upload"]).await;

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
async fn test_revoke_error(pool: PgPool) {
    let (_, token) =
        create_test_token(&pool, "Revoke Token Error", None, None, &["symbol-upload"]).await;

    assert!(token.is_active);

    pool.close().await;

    let result = ApiTokenRepo::revoke(&pool, token.id).await;
    assert!(result.is_err(), "Expected an error when revoking API token with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_delete(pool: PgPool) {
    let (_, token) = create_test_token(&pool, "Delete Token", None, None, &["symbol-upload"]).await;

    ApiTokenRepo::delete(&pool, token.id)
        .await
        .expect("Failed to delete API token");

    let deleted_token = ApiTokenRepo::get_by_id(&pool, token.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_token.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_delete_error(pool: PgPool) {
    let (_, token) =
        create_test_token(&pool, "Delete Token Error", None, None, &["symbol-upload"]).await;

    pool.close().await;

    let result = ApiTokenRepo::delete(&pool, token.id).await;
    assert!(result.is_err(), "Expected an error when deleting API token with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_has_entitlement(pool: PgPool) {
    let (_, token) = create_test_token(
        &pool,
        "Entitlement Token",
        None,
        None,
        &["symbol-upload", "minidump-upload"],
    )
    .await;

    assert!(token.has_entitlement("symbol-upload"));
    assert!(token.has_entitlement("minidump-upload"));
    assert!(!token.has_entitlement("non-existent"));
    assert!(token.is_valid());

    let (_, super_token) = create_test_token(&pool, "Super Token", None, None, &["token"]).await;

    assert!(!super_token.has_entitlement("symbol-upload"));
    assert!(!super_token.has_entitlement("minidump-upload"));
    assert!(!super_token.has_entitlement("anything"));
    assert!(super_token.has_entitlement("token"));
    assert!(super_token.is_valid());

    let (_, mut expired_token) =
        create_test_token(&pool, "Expired Token", None, None, &["symbol-upload"]).await;
    expired_token.expires_at = Some(Utc::now().naive_utc() - Duration::hours(1));

    ApiTokenRepo::update(&pool, expired_token.clone())
        .await
        .expect("Failed to update token expiry");

    assert!(expired_token.has_entitlement("symbol-upload"));
    assert!(!expired_token.is_valid());

    let (_, mut inactive_token) =
        create_test_token(&pool, "Inactive Token", None, None, &["symbol-upload"]).await;
    inactive_token.is_active = false;

    ApiTokenRepo::update(&pool, inactive_token.clone())
        .await
        .expect("Failed to update token active status");

    assert!(inactive_token.has_entitlement("symbol-upload"));
    assert!(!inactive_token.is_valid());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let initial_tokens = ApiTokenRepo::get_all(&pool)
        .await
        .expect("Failed to get initial tokens");

    let initial_count = initial_tokens.len();

    create_test_token(&pool, "All Token 1", None, None, &["symbol-upload"]).await;
    create_test_token(&pool, "All Token 2", None, None, &["minidump-upload"]).await;
    create_test_token(&pool, "All Token 3", None, None, &["token"]).await;

    let all_tokens = ApiTokenRepo::get_all(&pool)
        .await
        .expect("Failed to get all tokens");

    assert_eq!(all_tokens.len(), initial_count + 3);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_error(pool: PgPool) {
    create_test_token(&pool, "All Token 1", None, None, &["symbol-upload"]).await;
    create_test_token(&pool, "All Token 2", None, None, &["minidump-upload"]).await;
    create_test_token(&pool, "All Token 3", None, None, &["token"]).await;

    pool.close().await;

    let all_tokens = ApiTokenRepo::get_all(&pool).await;
    assert!(all_tokens.is_err(), "Expected an error when getting all tokens with closed pool");
}
