use super::common::*;

// ---------------------------------------------------------------------------
// Tests: auth routes
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route        |
// | ------ | ------------ |
// | POST   | /auth/logout |
// Cases:
// | Auth context         | Expected |
// | -------------------- | -------- |
// | admin session logout | 303      |
// | no_session logout    | 303      |
#[tokio::test]
async fn test_logout() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Logout always succeeds → 303 redirect to /
    let req = Request::builder()
        .method("POST")
        .uri("/auth/logout")
        .header("cookie", &f.admin)
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    // Works without a session too
    let req = Request::builder()
        .method("POST")
        .uri("/auth/logout")
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

// API calls:
// | Method | Route           |
// | ------ | --------------- |
// | GET    | /auth/real-user |
// Cases:
// | Auth context                | Expected |
// | --------------------------- | -------- |
// | no_session                  | 403      |
// | admin not impersonating     | 404      |
// | non_admin not impersonating | 404      |
// | imp_admin                   | 200      |
// | imp_non_admin               | 200      |
#[tokio::test]
async fn test_get_real_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // no session → 403
    assert_eq!(app.call("GET", "/auth/real-user", None, None).await, StatusCode::FORBIDDEN);
    // session but not impersonating → 404
    assert_eq!(
        app.call("GET", "/auth/real-user", None, Some(&f.admin))
            .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.call("GET", "/auth/real-user", None, Some(&f.non_admin))
            .await,
        StatusCode::NOT_FOUND
    );
    // impersonating → 200 (real user exists in DB)
    assert_eq!(
        app.call("GET", "/auth/real-user", None, Some(&f.imp_admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", "/auth/real-user", None, Some(&f.imp_non_admin))
            .await,
        StatusCode::OK
    );
}

// ---------------------------------------------------------------------------
// Tests: impersonation routes
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                       |
// | ------ | --------------------------- |
// | POST   | /auth/impersonate/{user_id} |
// Cases:
// | Auth context            | Expected |
// | ----------------------- | -------- |
// | no_session              | 403      |
// | non_admin               | 403      |
// | already impersonating   | 400      |
// | admin impersonates self | 400      |
// | admin missing target    | 404      |
// | admin valid target      | 303      |
#[tokio::test]
async fn test_start_impersonation() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target_uri = format!("/auth/impersonate/{}", f.non_admin_id);

    // no session → 403
    assert_eq!(app.call("POST", &target_uri, None, None).await, StatusCode::FORBIDDEN);
    // non-admin → 403
    assert_eq!(
        app.call("POST", &target_uri, None, Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN
    );
    // already impersonating → 400 (AppError::failure)
    assert_eq!(
        app.call("POST", &target_uri, None, Some(&f.imp_admin))
            .await,
        StatusCode::BAD_REQUEST
    );
    // impersonate self → 400
    let self_uri = format!("/auth/impersonate/{}", f.admin_id);
    assert_eq!(app.call("POST", &self_uri, None, Some(&f.admin)).await, StatusCode::BAD_REQUEST);
    // target not found → 404
    assert_eq!(
        app.call("POST", "/auth/impersonate/nonexistent-user-id", None, Some(&f.admin))
            .await,
        StatusCode::NOT_FOUND,
    );
    // success: admin impersonates non_admin → 303
    let req = Request::builder()
        .method("POST")
        .uri(&target_uri)
        .header("cookie", &f.admin)
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | POST   | /auth/impersonate/stop |
// Cases:
// | Auth context                | Expected |
// | --------------------------- | -------- |
// | no_session                  | 403      |
// | admin not impersonating     | 400      |
// | non_admin not impersonating | 400      |
// | imp_admin                   | 303      |
// | imp_non_admin               | 303      |
#[tokio::test]
async fn test_stop_impersonation() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // no session → 403
    assert_eq!(
        app.call("POST", "/auth/impersonate/stop", None, None).await,
        StatusCode::FORBIDDEN
    );
    // not impersonating → 400 (AppError::failure)
    assert_eq!(
        app.call("POST", "/auth/impersonate/stop", None, Some(&f.admin))
            .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.call("POST", "/auth/impersonate/stop", None, Some(&f.non_admin))
            .await,
        StatusCode::BAD_REQUEST
    );
    // impersonating → 303
    let req = Request::builder()
        .method("POST")
        .uri("/auth/impersonate/stop")
        .header("cookie", &f.imp_admin)
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let req = Request::builder()
        .method("POST")
        .uri("/auth/impersonate/stop")
        .header("cookie", &f.imp_non_admin)
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}
