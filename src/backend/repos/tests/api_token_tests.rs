use chrono::{Duration, Utc};
use testware::setup::TestSetup;
use uuid::Uuid;

use data::api_token::NewApiToken;
use repos::api_token::*;

use testware::{create_test_product, create_test_token, create_test_user};

// get_by_id tests

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;

    let description = "Test Token";
    let (_, inserted_token) =
        create_test_token(&db, description, Some(product.id), None, &["symbol-upload"]).await;

    let found_token = ApiTokenRepo::get_by_id(&db, inserted_token.id)
        .await
        .expect("Failed to get token by ID");

    assert!(found_token.is_some());
    let found_token = found_token.unwrap();
    assert_eq!(found_token.id, inserted_token.id);
    assert_eq!(found_token.description, description);
    assert_eq!(found_token.product_id, Some(product.id));
    assert_eq!(found_token.entitlements, vec!["symbol-upload"]);
}

#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4();
    let not_found = ApiTokenRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

// get_by_token_id tests

#[tokio::test]
async fn test_get_by_token_id() {
    let db = TestSetup::create_db().await;
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

    ApiTokenRepo::create(&db, new_token)
        .await
        .expect("Failed to create API token");

    let found_token = ApiTokenRepo::get_by_token_id(&db, token_id)
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

#[tokio::test]
async fn test_get_by_token_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4();
    let not_found = ApiTokenRepo::get_by_token_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent hash");

    assert!(not_found.is_none());
}

// update_last_used tests

#[tokio::test]
async fn test_update_last_used() {
    let db = TestSetup::create_db().await;
    let (_, token) =
        create_test_token(&db, "Update Last Used", None, None, &["symbol-upload"]).await;

    assert!(token.last_used_at.is_none());

    ApiTokenRepo::update_last_used(&db, token.id)
        .await
        .expect("Failed to update last used timestamp");

    let updated_token = ApiTokenRepo::get_by_id(&db, token.id)
        .await
        .expect("Failed to get token after update")
        .expect("Token not found after update");

    assert!(updated_token.last_used_at.is_some());
}

// get_by_product_id tests

#[tokio::test]
async fn test_get_by_product_id() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;

    create_test_token(&db, "Product Token 1", Some(product.id), None, &["symbol-upload"]).await;
    create_test_token(&db, "Product Token 2", Some(product.id), None, &["minidump-upload"]).await;
    create_test_token(&db, "Other Token", None, None, &["token"]).await;

    let product_tokens = ApiTokenRepo::get_by_product_id(&db, product.id)
        .await
        .expect("Failed to get tokens by product ID");

    assert_eq!(product_tokens.len(), 2);
    for token in product_tokens {
        assert_eq!(token.product_id, Some(product.id));
    }
}

#[tokio::test]
async fn test_get_by_user_id() {
    let db = TestSetup::create_db().await;
    let username = format!("user_{}", Uuid::new_v4());
    let user = create_test_user(&db, &username, true).await;

    create_test_token(&db, "User Token 1", None, Some(user.id), &["symbol-upload"]).await;
    create_test_token(&db, "User Token 2", None, Some(user.id), &["minidump-upload"]).await;
    create_test_token(&db, "Other Token", None, None, &["token"]).await;

    let user_tokens = ApiTokenRepo::get_by_user_id(&db, user.id)
        .await
        .expect("Failed to get tokens by user ID");

    assert_eq!(user_tokens.len(), 2);
    for token in user_tokens {
        assert_eq!(token.user_id, Some(user.id));
    }
}

#[tokio::test]
async fn test_create() {
    let db = TestSetup::create_db().await;
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

    let token_id = ApiTokenRepo::create(&db, new_token)
        .await
        .expect("Failed to create API token");

    let token = ApiTokenRepo::get_by_id(&db, token_id)
        .await
        .expect("Failed to get created token")
        .expect("Created token not found");

    assert_eq!(token.description, description);
    assert_eq!(token.token_hash, token_hash);
    assert_eq!(token.entitlements, entitlements);
    assert!(token.is_active);
}

#[tokio::test]
async fn test_update() {
    let db = TestSetup::create_db().await;
    let (_, mut token) =
        create_test_token(&db, "Update Token", None, None, &["symbol-upload"]).await;

    token.description = "Updated Description".to_string();
    token.entitlements = vec!["symbol-upload".to_string(), "token".to_string()];
    token.is_active = false;

    ApiTokenRepo::update(&db, token.clone())
        .await
        .expect("Failed to update API token");

    let updated_token = ApiTokenRepo::get_by_id(&db, token.id)
        .await
        .expect("Failed to get updated token")
        .expect("Updated token not found");

    assert_eq!(updated_token.description, "Updated Description");
    assert_eq!(updated_token.entitlements, vec!["symbol-upload", "token"]);
    assert!(!updated_token.is_active);
}

#[tokio::test]
async fn test_revoke() {
    let db = TestSetup::create_db().await;
    let (_, token) = create_test_token(&db, "Revoke Token", None, None, &["symbol-upload"]).await;

    assert!(token.is_active);

    ApiTokenRepo::revoke(&db, token.id)
        .await
        .expect("Failed to revoke API token");

    let updated_token = ApiTokenRepo::get_by_id(&db, token.id)
        .await
        .expect("Failed to get revoked token")
        .expect("Revoked token not found");

    assert!(!updated_token.is_active);
}

#[tokio::test]
async fn test_delete() {
    let db = TestSetup::create_db().await;
    let (_, token) = create_test_token(&db, "Delete Token", None, None, &["symbol-upload"]).await;

    ApiTokenRepo::delete(&db, token.id)
        .await
        .expect("Failed to delete API token");

    let deleted_token = ApiTokenRepo::get_by_id(&db, token.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_token.is_none());
}

#[tokio::test]
async fn test_has_entitlement() {
    let db = TestSetup::create_db().await;
    let (_, token) = create_test_token(
        &db,
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

    let (_, super_token) = create_test_token(&db, "Super Token", None, None, &["token"]).await;

    assert!(!super_token.has_entitlement("symbol-upload"));
    assert!(!super_token.has_entitlement("minidump-upload"));
    assert!(!super_token.has_entitlement("anything"));
    assert!(super_token.has_entitlement("token"));
    assert!(super_token.is_valid());

    let (_, mut expired_token) =
        create_test_token(&db, "Expired Token", None, None, &["symbol-upload"]).await;
    expired_token.expires_at = Some(Utc::now() - Duration::hours(1));

    ApiTokenRepo::update(&db, expired_token.clone())
        .await
        .expect("Failed to update token expiry");

    assert!(expired_token.has_entitlement("symbol-upload"));
    assert!(!expired_token.is_valid());

    let (_, mut inactive_token) =
        create_test_token(&db, "Inactive Token", None, None, &["symbol-upload"]).await;
    inactive_token.is_active = false;

    ApiTokenRepo::update(&db, inactive_token.clone())
        .await
        .expect("Failed to update token active status");

    assert!(inactive_token.has_entitlement("symbol-upload"));
    assert!(!inactive_token.is_valid());
}

#[tokio::test]
async fn test_get_all() {
    let db = TestSetup::create_db().await;
    let initial_tokens = ApiTokenRepo::get_all(&db)
        .await
        .expect("Failed to get initial tokens");

    let initial_count = initial_tokens.len();

    create_test_token(&db, "All Token 1", None, None, &["symbol-upload"]).await;
    create_test_token(&db, "All Token 2", None, None, &["minidump-upload"]).await;
    create_test_token(&db, "All Token 3", None, None, &["token"]).await;

    let all_tokens = ApiTokenRepo::get_all(&db)
        .await
        .expect("Failed to get all tokens");

    assert_eq!(all_tokens.len(), initial_count + 3);
}
