#![cfg(test)]

use testware::setup::TestSetup;
use uuid::Uuid;

use common::{QueryParams, SortOrder};
use data::user::*;
use repos::user::*;

use testware::create_test_user;

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let username = "testuser1";
    let is_admin = false;
    let _inserted_user = create_test_user(&db, username, is_admin).await;

    let username = "testuser2";
    let is_admin = true;
    let inserted_user = create_test_user(&db, username, is_admin).await;

    let found_user = UserRepo::get_by_id(&db, inserted_user.id.clone())
        .await
        .expect("Failed to get user by ID");

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.id, inserted_user.id);
    assert_eq!(found_user.username, username);
    assert_eq!(found_user.is_admin, is_admin);
}

#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4().to_string();
    let not_found = UserRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_get_by_name() {
    let db = TestSetup::create_db().await;
    let username = "testuser1";
    let is_admin = false;
    create_test_user(&db, username, is_admin).await;

    let username = "testuser2";
    let is_admin = true;
    create_test_user(&db, username, is_admin).await;

    let found_user = UserRepo::get_by_name(&db, username)
        .await
        .expect("Failed to get user by name");

    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.username, username);
    assert_eq!(found_user.is_admin, is_admin);
}

#[tokio::test]
async fn test_get_by_name_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_name = "nonexistentuser";
    let not_found = UserRepo::get_by_name(&db, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");

    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_get_all_names() {
    let db = TestSetup::create_db().await;
    let usernames = vec!["user1", "user2", "user3"];
    for username in &usernames {
        create_test_user(&db, username, false).await;
    }

    let user_names = UserRepo::get_all_names(&db)
        .await
        .expect("Failed to get all user names");

    assert_eq!(user_names.len(), usernames.len());
    for username in usernames {
        assert!(user_names.contains(username));
    }
}

#[tokio::test]
async fn test_get_all() {
    let db = TestSetup::create_db().await;
    let usernames = vec!["user3", "user1", "user2"];
    for username in &usernames {
        create_test_user(&db, username, false).await;
    }

    let query_params = QueryParams::default();
    let all_users = UserRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get all users");

    assert_eq!(all_users.len(), usernames.len());

    let mut query_params = QueryParams::default();
    query_params
        .sorting
        .push_back(("username".to_string(), SortOrder::Ascending));

    let sorted_users = UserRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get sorted users");

    assert_eq!(sorted_users.len(), usernames.len());
    let mut expected = usernames.clone();
    expected.sort();
    for (i, expected_name) in expected.iter().enumerate() {
        assert_eq!(sorted_users[i].username, *expected_name);
    }
}

#[tokio::test]
async fn test_create_user() {
    let db = TestSetup::create_db().await;
    let new_user = NewUser {
        username: "newuser".to_string(),
        email: None,
        is_admin: false,
    };

    let user_id = UserRepo::create(&db, new_user.clone())
        .await
        .expect("Failed to create user");

    let found_user = UserRepo::get_by_id(&db, user_id)
        .await
        .expect("Failed to get user by ID")
        .expect("User not found after creation");

    assert_eq!(found_user.username, new_user.username);
    assert!(!found_user.is_admin);
}

#[tokio::test]
async fn test_create_with_id() {
    let db = TestSetup::create_db().await;
    let user_id = Uuid::new_v4().to_string();
    let username = "userwithid";

    let created_id = UserRepo::create_with_id(&db, user_id.clone(), username)
        .await
        .expect("Failed to create user with ID");

    assert_eq!(created_id, user_id);

    let found_user = UserRepo::get_by_id(&db, user_id.clone())
        .await
        .expect("Failed to get user by ID")
        .expect("User not found after creation");

    assert_eq!(found_user.id, user_id);
    assert_eq!(found_user.username, username);
    assert!(!found_user.is_admin);
}

#[tokio::test]
async fn test_update() {
    let db = TestSetup::create_db().await;
    let mut user = create_test_user(&db, "updateuser", false).await;

    user.username = "updated_username".to_string();
    user.is_admin = true;

    let updated_id = UserRepo::update(&db, user.clone())
        .await
        .expect("Failed to update user")
        .expect("User not found when updating");

    assert_eq!(updated_id, user.id);

    let found_user = UserRepo::get_by_id(&db, user.id.clone())
        .await
        .expect("Failed to get user by ID")
        .expect("User not found after update");

    assert_eq!(found_user.username, "updated_username");
    assert!(found_user.is_admin);
}

#[tokio::test]
async fn test_remove() {
    let db = TestSetup::create_db().await;
    let user = create_test_user(&db, "removeuser", false).await;

    let found_user = UserRepo::get_by_id(&db, user.id.clone())
        .await
        .expect("Failed to get user by ID");

    assert!(found_user.is_some());

    UserRepo::remove(&db, user.id.clone())
        .await
        .expect("Failed to remove user");

    let not_found = UserRepo::get_by_id(&db, user.id.clone())
        .await
        .expect("Failed to query after removal");

    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_count() {
    let db = TestSetup::create_db().await;
    let count = UserRepo::count(&db).await.expect("Failed to count users");
    assert_eq!(count, 0);

    for i in 0..3 {
        create_test_user(&db, &format!("countuser{i}"), false).await;
    }

    let count = UserRepo::count(&db).await.expect("Failed to count users");
    assert_eq!(count, 3);
}
