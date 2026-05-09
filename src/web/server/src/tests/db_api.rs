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
    // user_db queries the DB for "nonexistent-user-id", finds nothing → line 103
    // (return self.anon_db().await), then the query runs with anonymous credentials.
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
    // GET /products (no scope) calls user_db → session has user: None → line 83 → anon_db.
    let (status, _) = app
        .call_json("GET", "/products", None, Some(&no_user))
        .await;
    assert_ne!(status, StatusCode::INTERNAL_SERVER_ERROR);
}
