#![cfg(test)]

use sqlx::PgPool;
use uuid::Uuid;

use common::{QueryParams, SortOrder};
use data::user::*;
use repos::user::*;

use testware::create_test_user;

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let username = "testuser1";
    let is_admin = false;
    let _inserted_user = create_test_user(&pool, username, is_admin).await;

    let username = "testuser2";
    let is_admin = true;
    let inserted_user = create_test_user(&pool, username, is_admin).await;

    let found_user = UserRepo::get_by_id(&pool, inserted_user.id)
        .await
        .expect("Failed to get user by ID");

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.id, inserted_user.id);
    assert_eq!(found_user.username, username);
    assert_eq!(found_user.is_admin, is_admin);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = UserRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let username = "testuser_error";
    let is_admin = false;
    let inserted_user = create_test_user(&pool, username, is_admin).await;

    pool.close().await;

    let result = UserRepo::get_by_id(&pool, inserted_user.id).await;
    assert!(result.is_err(), "Expected an error when getting user by ID with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name(pool: PgPool) {
    let username = "testuser1";
    let is_admin = false;
    create_test_user(&pool, username, is_admin).await;

    let username = "testuser2";
    let is_admin = true;
    create_test_user(&pool, username, is_admin).await;

    let found_user = UserRepo::get_by_name(&pool, username)
        .await
        .expect("Failed to get user by name");

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.username, username);
    assert_eq!(found_user.is_admin, is_admin);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name_not_found(pool: PgPool) {
    let non_existent_name = "nonexistentuser";
    let not_found = UserRepo::get_by_name(&pool, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name_error(pool: PgPool) {
    let username = "getbynameuser_error";
    let is_admin = false;
    create_test_user(&pool, username, is_admin).await;

    pool.close().await;

    let result = UserRepo::get_by_name(&pool, username).await;
    assert!(result.is_err(), "Expected an error when getting user by name with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names(pool: PgPool) {
    let usernames = vec!["user1", "user2", "user3"];
    for username in &usernames {
        create_test_user(&pool, username, false).await;
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
async fn test_get_all_names_error(pool: PgPool) {
    create_test_user(&pool, "names_user1_error", false).await;
    create_test_user(&pool, "names_user2_error", false).await;

    pool.close().await;

    let result = UserRepo::get_all_names(&pool).await;
    assert!(result.is_err(), "Expected an error when getting all names with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let usernames = vec!["user3", "user1", "user2"];
    for username in &usernames {
        create_test_user(&pool, username, false).await;
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
async fn test_get_all_error(pool: PgPool) {
    create_test_user(&pool, "get_all_user1_error", false).await;
    create_test_user(&pool, "get_all_user2_error", false).await;

    pool.close().await;

    let query_params = QueryParams::default();
    let result = UserRepo::get_all(&pool, query_params).await;
    assert!(result.is_err(), "Expected an error when getting all users with closed pool");
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
async fn test_create_error(pool: PgPool) {
    let new_user = NewUser {
        username: "createuser_error".to_string(),
        is_admin: false,
    };

    pool.close().await;

    let result = UserRepo::create(&pool, new_user).await;
    assert!(result.is_err(), "Expected an error when creating user with closed pool");
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
async fn test_create_with_id_error(pool: PgPool) {
    let user_id = Uuid::new_v4();
    let username = "userwithid_error";

    pool.close().await;

    let result = UserRepo::create_with_id(&pool, user_id, username).await;
    assert!(result.is_err(), "Expected an error when creating user with ID and closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut user = create_test_user(&pool, "updateuser", false).await;

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
async fn test_update_error(pool: PgPool) {
    let mut user = create_test_user(&pool, "updateuser_error", false).await;

    user.username = "updated_username_error".to_string();
    user.is_admin = true;

    pool.close().await;

    let result = UserRepo::update(&pool, user.clone()).await;
    assert!(result.is_err(), "Expected an error when updating user with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let user = create_test_user(&pool, "removeuser", false).await;

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
async fn test_remove_error(pool: PgPool) {
    let user = create_test_user(&pool, "removeuser_error", false).await;

    pool.close().await;

    let result = UserRepo::remove(&pool, user.id).await;
    assert!(result.is_err(), "Expected an error when removing user with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let count = UserRepo::count(&pool).await.expect("Failed to count users");
    assert_eq!(count, 0);

    for i in 0..3 {
        create_test_user(&pool, &format!("countuser{i}"), false).await;
    }

    let count = UserRepo::count(&pool).await.expect("Failed to count users");
    assert_eq!(count, 3);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count_error(pool: PgPool) {
    create_test_user(&pool, "count_user1_error", false).await;
    create_test_user(&pool, "count_user2_error", false).await;

    pool.close().await;

    let result = UserRepo::count(&pool).await;
    assert!(result.is_err(), "Expected an error when counting users with closed pool");
}
