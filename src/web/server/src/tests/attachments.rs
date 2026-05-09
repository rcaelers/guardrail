use super::common::*;

// ---------------------------------------------------------------------------
// Tests: db_api – download_attachment
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                 |
// | ------ | ------------------------------------- |
// | GET    | /attachments/{attachment_id}/download |
// Cases:
// | Case                                           | Expected              |
// | ---------------------------------------------- | --------------------- |
// | admin with missing attachment                  | 404                   |
// | admin with existing private-product attachment | 200 with stored bytes |
// | read-only member with private-product file     | 200 with stored bytes |
// | anonymous user with private-product file       | 404                   |
#[tokio::test]
async fn test_download_attachment() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Nonexistent attachment → 404
    assert_eq!(
        app.call("GET", "/attachments/nosuchattachment/download", None, Some(&f.admin))
            .await,
        StatusCode::NOT_FOUND,
    );

    // Create a crash then an attachment for it (bypassing product RLS via root DB)
    let crash = testware::create_test_crash(&app.db, Some("fp"), Some(pid.clone())).await;
    let att = create_test_attachment(
        &app.db,
        "test-file",
        "text/plain",
        5,
        "hello.txt",
        Some(pid.clone()),
        Some(crash.id.clone()),
    )
    .await;

    // Pre-populate the in-memory object store at the attachment's storage path
    let content = b"hello";
    {
        use object_store::ObjectStore as _;
        (&*app.storage)
            .put_opts(
                &object_store::path::Path::from(att.storage_path.as_str()),
                object_store::PutPayload::from_static(content),
                Default::default(),
            )
            .await
            .expect("put failed");
    }

    // Admin should be able to download (product is non-public but admin bypasses RLS)
    let (status, body) = app
        .call_full("GET", &format!("/attachments/{}/download", att.id), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), content);

    let (status, body) = app
        .call_full("GET", &format!("/attachments/{}/download", att.id), None, Some(&f.non_admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), content);

    assert_eq!(
        app.call("GET", &format!("/attachments/{}/download", att.id), None, None)
            .await,
        StatusCode::NOT_FOUND
    );
}
