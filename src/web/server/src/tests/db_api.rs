use super::common::*;

// ---------------------------------------------------------------------------
// Tests: db_api – user_db fallback (ghost session)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route     |
// | ------ | --------- |
// | GET    | /me       |
// | GET    | /products |
// Cases:
// | Case                                                | Expected |
// | --------------------------------------------------- | -------- |
// | ghost session /me                                   | not 500  |
// | ghost session /products after anonymous DB fallback | not 500  |
#[tokio::test]
async fn test_user_db_ghost_session() {
    let app = TestApp::new().await;
    // A session whose user_id does not exist in the DB → user_db falls back to
    // anon_db; the request proceeds as anonymous (public products visible, private not).
    let ghost = app
        .make_session(json!({"user_id": "nonexistent-user-id", "name": "Ghost", "is_admin": false}))
        .await;
    // /me requires a session but hits require_session before user_db, so expect 200 (session exists)
    // but the DB lookup for the user will fail → anon_db is used for subsequent DB queries.
    // The handler still reads the session successfully for user info even if user_db falls back.
    let status = app.call("GET", "/me", None, Some(&ghost)).await;
    // require_session passes (session exists); the user info comes from the session, not user_db.
    assert_ne!(status, StatusCode::INTERNAL_SERVER_ERROR);

    // GET /products (no scope) calls user_db without a prior DB access check.
    // user_db queries the DB for "nonexistent-user-id", finds nothing → anon_db.
    let (status, _) = app.call_json("GET", "/products", None, Some(&ghost)).await;
    assert_ne!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

// API calls:
// | Method | Route     |
// | ------ | --------- |
// | GET    | /products |
// Cases:
// | Case                                                           | Expected |
// | -------------------------------------------------------------- | -------- |
// | null active user session /products after anonymous DB fallback | not 500  |
#[tokio::test]
async fn test_user_db_null_user_session() {
    let app = TestApp::new().await;
    // A session with user: None (AuthenticatedUser { user: None }) triggers line 83
    // in user_db (the `let Some(active) = session_user.user.as_ref() else` branch).
    let no_user = app
        .make_session(json!({"user_id": null, "name": "Nobody", "is_admin": false}))
        .await;
    // GET /products (no scope) calls user_db → session has user: None → anon_db.
    let (status, _) = app
        .call_json("GET", "/products", None, Some(&no_user))
        .await;
    assert_ne!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

// ---------------------------------------------------------------------------
// Tests: db_api – JWT failure returns 503, never root access
// ---------------------------------------------------------------------------
//
// These tests would have caught the root-session fallback security bug.
//
// The bug: when JWT auth failed (either generation or SurrealDB authentication),
// the code called `root_db_fallback()` which returned `self.repo.db` — the root
// admin connection that bypasses all SurrealDB row-level security.
//
// The assert_ne!(status, 500) tests below were the original assertions, which
// passed even with root access (root returns 200, not 500). The fix asserts
// 503 SERVICE_UNAVAILABLE, which only passes when no fallback to root occurs.

// API calls:
// | Method | Route     |
// | ------ | --------- |
// | GET    | /products |
// Cases:
// | Case                                                     | Expected |
// | -------------------------------------------------------- | -------- |
// | no session, JWT generation fails (unparseable key)       | 503      |
// | valid session, JWT generation fails for user and anon    | 503      |
#[tokio::test]
async fn test_jwt_generation_failure_returns_503() {
    let app = TestApp::new_with_invalid_jwt_key().await;

    // No session: anon JWT generation fails → 503, not root fallback.
    let (status, _) = app.call_json("GET", "/products", None, None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    // Session with real user: user JWT generation fails → falls back to anon JWT
    // generation, which also fails → 503.
    let user = create_test_user(&app.db, "jwt_failure_user", false).await;
    let session = app
        .make_session(json!({"user_id": user.id, "name": "Jwt Failure", "is_admin": false}))
        .await;
    let (status, _) = app
        .call_json("GET", "/products", None, Some(&session))
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// API calls:
// | Method | Route     |
// | ------ | --------- |
// | GET    | /products |
// Cases:
// | Case                                                              | Expected |
// | ----------------------------------------------------------------- | -------- |
// | no session, JWT generation OK but SurrealDB rejects it            | 503      |
// | valid session, user JWT rejected by SurrealDB, anon also rejected | 503      |
//
// This is the exact production failure mode logged as:
//   "There was a problem with authentication"
//   "Anonymous JWT auth failed, falling back to root session"
#[tokio::test]
async fn test_jwt_auth_failure_returns_503() {
    let app = TestApp::new_with_mismatched_jwt_key().await;

    // No session: anon JWT generation succeeds (key is valid) but SurrealDB
    // rejects it (wrong key, not in DEFINE ACCESS) → 503.
    let (status, _) = app.call_json("GET", "/products", None, None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    // Session with real user: user JWT rejected → falls back to anon JWT,
    // also rejected → 503.
    let user = create_test_user(&app.db, "mismatch_user", false).await;
    let session = app
        .make_session(json!({"user_id": user.id, "name": "Mismatch", "is_admin": false}))
        .await;
    let (status, _) = app
        .call_json("GET", "/products", None, Some(&session))
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// API calls:
// | Method | Route     |
// | ------ | --------- |
// | GET    | /products |
// Cases:
// | Case                                                    | Expected          |
// | ------------------------------------------------------- | ----------------- |
// | JWT auth failure, private product in DB                 | 503, no data leak |
//
// This test directly demonstrates the old root-fallback bug:
//   Old code: GET /products → root session → SELECT FROM products (no RLS) →
//             200 with private product in response body → data leak.
//   New code: GET /products → 503 → no response body with product data.
#[tokio::test]
async fn test_jwt_auth_failure_does_not_leak_private_data() {
    let app = TestApp::new_with_mismatched_jwt_key().await;

    // Insert a private product directly via the root DB connection.
    // Products are private (public: false) by default.
    let product = create_test_product(&app.db).await;

    // With the old bug: root fallback → SELECT FROM products (bypasses RLS) →
    // 200 with product.id in the body.
    // With the fix: 503 → product.id never appears in the response.
    let (status, body) = app.call_json("GET", "/products", None, None).await;

    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "JWT auth failure must return 503, not silently serve data via root access"
    );
    assert!(
        !body.to_string().contains(&product.id),
        "private product id must not appear in response when JWT auth fails"
    );
}
