use super::common::*;

// ---------------------------------------------------------------------------
// Tests: home page
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                         |
// | ------ | ----------------------------- |
// | GET    | /                             |
// | GET    | /?next={path}&error={message} |
// | GET    | /?error={message}             |
// Cases:
// | Case              | Expected |
// | ----------------- | -------- |
// | no_session        | 200      |
// | admin             | 200      |
// | non_admin         | 200      |
// | query: next+error | 200      |
// | query: error only | 200      |
#[tokio::test]
async fn test_home_page() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // / is public; renders HTML for all contexts
    assert_eq!(app.call("GET", "/", None, None).await, StatusCode::OK);
    assert_eq!(app.call("GET", "/", None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", "/", None, Some(&f.non_admin)).await, StatusCode::OK);
    // with query params
    assert_eq!(
        app.call("GET", "/?next=/dashboard&error=login+failed", None, None)
            .await,
        StatusCode::OK
    );
    assert_eq!(app.call("GET", "/?error=oops", None, None).await, StatusCode::OK);
}
