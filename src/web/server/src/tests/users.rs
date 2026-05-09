use super::common::*;

// ---------------------------------------------------------------------------
// Tests: user management (admin-only)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route  |
// | ------ | ------ |
// | GET    | /users |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_list_users() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_admin_only(&app, &f, "GET", "/users", None, StatusCode::OK).await;
}

// API calls:
// | Method | Route  |
// | ------ | ------ |
// | POST   | /users |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_create_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Use unique emails per admin context to avoid duplicate-ID errors.
    assert_eq!(
        app.call("POST", "/users", Some(json!({"email": "x@x.com"})), None)
            .await,
        StatusCode::FORBIDDEN,
        "no_session"
    );
    assert_eq!(
        app.call("POST", "/users", Some(json!({"email": "x@x.com"})), Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "non_admin"
    );
    assert_eq!(
        app.call("POST", "/users", Some(json!({"email": "x@x.com"})), Some(&f.imp_non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "imp_non_admin"
    );
    assert_eq!(
        app.call(
            "POST",
            "/users",
            Some(json!({"email": "admin.created@test.com"})),
            Some(&f.admin)
        )
        .await,
        StatusCode::OK,
        "admin"
    );
    assert_eq!(
        app.call(
            "POST",
            "/users",
            Some(json!({"email": "imp.admin.created@test.com"})),
            Some(&f.imp_admin)
        )
        .await,
        StatusCode::OK,
        "imp_admin"
    );
}

// API calls:
// | Method | Route            |
// | ------ | ---------------- |
// | GET    | /users/{user_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_get_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let uri = format!("/users/{}", f.non_admin_id);
    assert_admin_only(&app, &f, "GET", &uri, None, StatusCode::OK).await;
}

// API calls:
// | Method | Route            |
// | ------ | ---------------- |
// | POST   | /users/{user_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_update_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target = create_test_user(&app.db, "update_target", false).await;
    let uri = format!("/users/{}", target.id);
    let body = json!({"email": "updated@example.com", "name": "Updated"});
    assert_admin_only(&app, &f, "POST", &uri, Some(body), StatusCode::OK).await;
}

// API calls:
// | Method | Route            |
// | ------ | ---------------- |
// | DELETE | /users/{user_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 204      |
// | non_admin     | 403      |
// | imp_admin     | 204      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_delete_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // non_admin and no_session must be rejected
    let victim = create_test_user(&app.db, "del_victim", false).await;
    let uri = format!("/users/{}", victim.id);
    assert_eq!(app.call("DELETE", &uri, None, None).await, StatusCode::FORBIDDEN, "no_session");
    assert_eq!(
        app.call("DELETE", &uri, None, Some(&f.non_admin)).await,
        StatusCode::FORBIDDEN,
        "non_admin"
    );
    assert_eq!(
        app.call("DELETE", &uri, None, Some(&f.imp_non_admin)).await,
        StatusCode::FORBIDDEN,
        "imp_non_admin"
    );

    // imp_admin succeeds (creates victim2)
    let victim2 = create_test_user(&app.db, "del_victim2", false).await;
    let uri2 = format!("/users/{}", victim2.id);
    assert_eq!(
        app.call("DELETE", &uri2, None, Some(&f.imp_admin)).await,
        StatusCode::NO_CONTENT,
        "imp_admin"
    );

    // admin succeeds (deletes original victim)
    assert_eq!(
        app.call("DELETE", &uri, None, Some(&f.admin)).await,
        StatusCode::NO_CONTENT,
        "admin"
    );
}

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | POST   | /users/{user_id}/admin |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 204      |
// | non_admin     | 403      |
// | imp_admin     | 204      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_set_admin() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target = create_test_user(&app.db, "promote_target", false).await;
    let uri = format!("/users/{}/admin", target.id);
    let body = json!({"isAdmin": true});
    assert_admin_only(&app, &f, "POST", &uri, Some(body), StatusCode::NO_CONTENT).await;
}

// ---------------------------------------------------------------------------
// Tests: user self-service (session-only)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route |
// | ------ | ----- |
// | GET    | /me   |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 200      |
// | imp_admin     | 200      |
// | imp_non_admin | 200      |
#[tokio::test]
async fn test_get_me() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // no_session → 403; all sessions → 200 (users exist in DB)
    assert_all(
        &app,
        &f,
        "GET",
        "/me",
        None,
        [
            StatusCode::FORBIDDEN,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
        ],
    )
    .await;
}

// API calls:
// | Method | Route                 |
// | ------ | --------------------- |
// | GET    | /users/find?q={query} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | not 403  |
// | non_admin     | not 403  |
// | imp_admin     | not 403  |
// | imp_non_admin | not 403  |
#[tokio::test]
async fn test_find_user_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Searching for "fx_admin" (name set by create_test_user).
    // Exact result varies; auth layer must pass for sessions, block for no_session.
    assert_session_only_not_forbidden(&app, &f, "GET", "/users/find?q=fx_admin", None).await;
}

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | GET    | /users/{user_id}/memberships |
// Cases:
// | Case                                               | Expected |
// | -------------------------------------------------- | -------- |
// | no_session: admin target                           | 403      |
// | no_session: own target                             | 403      |
// | admin or imp_admin: any target                     | 200      |
// | non_admin or imp_non_admin: own memberships        | 200      |
// | non_admin or imp_non_admin: other user memberships | 403      |
#[tokio::test]
async fn test_memberships_self_or_admin() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    let admin_uri = format!("/users/{}/memberships", f.admin_id);
    let non_admin_uri = format!("/users/{}/memberships", f.non_admin_id);

    // no_session → always 403
    assert_eq!(app.call("GET", &admin_uri, None, None).await, StatusCode::FORBIDDEN);
    assert_eq!(app.call("GET", &non_admin_uri, None, None).await, StatusCode::FORBIDDEN);

    // admin can read anyone's memberships
    assert_eq!(app.call("GET", &admin_uri, None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &non_admin_uri, None, Some(&f.admin)).await, StatusCode::OK);

    // non_admin can read their own
    assert_eq!(
        app.call("GET", &non_admin_uri, None, Some(&f.non_admin))
            .await,
        StatusCode::OK
    );
    // non_admin cannot read someone else's
    assert_eq!(app.call("GET", &admin_uri, None, Some(&f.non_admin)).await, StatusCode::FORBIDDEN);

    // imp_admin acts as admin
    assert_eq!(
        app.call("GET", &non_admin_uri, None, Some(&f.imp_admin))
            .await,
        StatusCode::OK
    );

    // imp_non_admin acts as non_admin: can read self, blocked from others
    assert_eq!(
        app.call("GET", &non_admin_uri, None, Some(&f.imp_non_admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &admin_uri, None, Some(&f.imp_non_admin))
            .await,
        StatusCode::FORBIDDEN
    );
}

// API calls:
// | Method | Route                         |
// | ------ | ----------------------------- |
// | GET    | /users/{user_id}/memberships  |
// Cases:
// | Bearer token         | Expected |
// | -------------------- | -------- |
// | global token         | 200      |
// | product-scoped token | 403      |
#[tokio::test]
async fn test_memberships_with_bearer_tokens() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let product = create_test_product(&app.db).await;

    let (global_token, _) =
        create_test_token(&app.db, "memberships-global-token", None, None, &["token"]).await;
    let (scoped_token, _) =
        create_test_token(&app.db, "memberships-scoped-token", Some(product.id), None, &["token"])
            .await;

    let uri = format!("/users/{}/memberships", f.non_admin_id);
    assert_eq!(app.call_bearer("GET", &uri, None, &global_token).await, StatusCode::OK);
    assert_eq!(app.call_bearer("GET", &uri, None, &scoped_token).await, StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// Tests: db_api – bad request helpers (bad() function)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route  |
// | ------ | ------ |
// | POST   | /users |
// Cases:
// | Case                                  | Expected |
// | ------------------------------------- | -------- |
// | admin with empty email                | 400      |
// | admin with empty name and valid email | 200      |
#[tokio::test]
async fn test_create_user_missing_email() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // create_user: empty email → bad("Email required.")
    let body = json!({"name": "Test", "email": "", "is_admin": false});
    assert_eq!(
        app.call("POST", "/users", Some(body), Some(&f.admin)).await,
        StatusCode::BAD_REQUEST,
    );
    // empty name falls back to using email as name, so it succeeds (200)
    let body2 = json!({"name": "", "email": "newuser@example.com", "is_admin": false});
    assert_eq!(
        app.call("POST", "/users", Some(body2), Some(&f.admin))
            .await,
        StatusCode::OK,
    );
}

// API calls:
// | Method | Route            |
// | ------ | ---------------- |
// | POST   | /users/{user_id} |
// Cases:
// | Case                                 | Expected |
// | ------------------------------------ | -------- |
// | admin updating user with empty email | 400      |
#[tokio::test]
async fn test_update_user_missing_email() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target = create_test_user(&app.db, "upd_email_target", false).await;
    let body = json!({"name": "X", "email": ""});
    assert_eq!(
        app.call("POST", &format!("/users/{}", target.id), Some(body), Some(&f.admin))
            .await,
        StatusCode::BAD_REQUEST,
    );
}
