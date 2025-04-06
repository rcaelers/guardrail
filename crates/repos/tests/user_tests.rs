#![cfg(all(test, feature = "ssr"))]

use sqlx::PgPool;
use uuid::Uuid;

use repos::user::*;
use repos::{QueryParams, SortOrder};

async fn insert_test_user(pool: &PgPool, username: &str, is_admin: bool) -> User {
    // Insert directly through SQL to avoid depending on the function we're testing
    sqlx::query_as!(
        User,
        r#"
        INSERT INTO guardrail.users (username, is_admin)
        VALUES ($1, $2)
        RETURNING id, username, is_admin, created_at, updated_at, last_login_at
    "#,
        username,
        is_admin
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test user")
}

// Use the migrations parameter to have SQLx automatically run migrations
#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let username = "testuser";
    let is_admin = false;
    let inserted_user = insert_test_user(&pool, username, is_admin).await;

    let found_user = UserRepo::get_by_id(&pool, inserted_user.id)
        .await
        .expect("Failed to get user by ID");

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.id, inserted_user.id);
    assert_eq!(found_user.username, username);
    assert_eq!(found_user.is_admin, is_admin);

    let non_existent_id = Uuid::new_v4();
    let not_found = UserRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name(pool: PgPool) {
    let username = "testuser";
    let is_admin = false;
    insert_test_user(&pool, username, is_admin).await;

    let found_user = UserRepo::get_by_name(&pool, username)
        .await
        .expect("Failed to get user by name");

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.username, username);
    assert_eq!(found_user.is_admin, is_admin);

    let non_existent_name = "nonexistentuser";
    let not_found = UserRepo::get_by_name(&pool, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names(pool: PgPool) {
    let usernames = vec!["user1", "user2", "user3"];
    for username in &usernames {
        insert_test_user(&pool, username, false).await;
    }

    let user_names = UserRepo::get_all_names(&pool)
        .await
        .expect("Failed to get all user names");

    assert_eq!(user_names.len(), usernames.len());
    for username in usernames {
        assert!(user_names.contains(username));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let usernames = vec!["user3", "user1", "user2"];
    for username in &usernames {
        insert_test_user(&pool, username, false).await;
    }

    let query_params = QueryParams::default();
    let all_users = UserRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all users");

    assert_eq!(all_users.len(), usernames.len());

    let mut query_params = QueryParams::default();
    query_params
        .sorting
        .push_back(("username".to_string(), SortOrder::Ascending));

    let sorted_users = UserRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get sorted users");

    assert_eq!(sorted_users.len(), usernames.len());
    let mut expected = usernames.clone();
    expected.sort();
    for (i, expected_name) in expected.iter().enumerate() {
        assert_eq!(sorted_users[i].username, *expected_name);
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_user(pool: PgPool) {
    let new_user = NewUser {
        username: "newuser".to_string(),
        is_admin: false,
    };

    let user_id = UserRepo::create(&pool, new_user.clone())
        .await
        .expect("Failed to create user");

    let found_user = UserRepo::get_by_id(&pool, user_id)
        .await
        .expect("Failed to get user by ID")
        .expect("User not found after creation");

    assert_eq!(found_user.username, new_user.username);
    assert!(!found_user.is_admin);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_with_id(pool: PgPool) {
    let user_id = Uuid::new_v4();
    let username = "userwithid";

    let created_id = UserRepo::create_with_id(&pool, user_id, username)
        .await
        .expect("Failed to create user with ID");

    assert_eq!(created_id, user_id);

    let found_user = UserRepo::get_by_id(&pool, user_id)
        .await
        .expect("Failed to get user by ID")
        .expect("User not found after creation");

    assert_eq!(found_user.id, user_id);
    assert_eq!(found_user.username, username);
    assert!(!found_user.is_admin);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut user = insert_test_user(&pool, "updateuser", false).await;

    user.username = "updated_username".to_string();
    user.is_admin = true;

    let updated_id = UserRepo::update(&pool, user.clone())
        .await
        .expect("Failed to update user")
        .expect("User not found when updating");

    assert_eq!(updated_id, user.id);

    let found_user = UserRepo::get_by_id(&pool, user.id)
        .await
        .expect("Failed to get user by ID")
        .expect("User not found after update");

    assert_eq!(found_user.username, "updated_username");
    assert!(found_user.is_admin);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let user = insert_test_user(&pool, "removeuser", false).await;

    let found_user = UserRepo::get_by_id(&pool, user.id)
        .await
        .expect("Failed to get user by ID");

    assert!(found_user.is_some());

    UserRepo::remove(&pool, user.id)
        .await
        .expect("Failed to remove user");

    let not_found = UserRepo::get_by_id(&pool, user.id)
        .await
        .expect("Failed to query after removal");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let count = UserRepo::count(&pool).await.expect("Failed to count users");
    assert_eq!(count, 0);

    for i in 0..3 {
        insert_test_user(&pool, &format!("countuser{}", i), false).await;
    }

    let count = UserRepo::count(&pool).await.expect("Failed to count users");
    assert_eq!(count, 3);
}
