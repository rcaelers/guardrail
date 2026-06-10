// Regression tests for the auth-hardening work:
//   - the `username = "admin"` RLS backdoor is gone (admin is the flag only),
//   - SurrealDB RLS isolates crash data between products,
//   - public products expose crash metadata but gate attachments (PII).

use super::common::*;
use object_store::ObjectStoreExt as _;
use testware::create_test_crash;

/// Set a product's `public` flag via the root connection (bypasses RLS).
async fn set_product_public(db: &Db, product_id: &str) {
    db.query("UPDATE type::record('products', $id) SET public = true")
        .bind(("id", product_id.to_string()))
        .await
        .expect("set public failed");
}

// A user literally named "admin" who is NOT a DB admin must gain no privilege:
// neither admin-only endpoints nor RLS-protected private data. This is the
// regression guard for the removed `fn::auth::username() = "admin"` shortcut.
#[tokio::test]
async fn username_admin_grants_no_privilege() {
    let app = TestApp::new().await;
    let private = create_test_product(&app.db).await; // public defaults to false
    let evil = create_test_user(&app.db, "admin", false).await;
    let session = app
        .make_session(json!({ "user_id": evil.id, "name": "admin", "is_admin": false }))
        .await;

    // Admin-only endpoint is refused (require_admin uses the persisted flag).
    assert_eq!(
        app.call("GET", "/users", None, Some(&session)).await,
        StatusCode::FORBIDDEN
    );

    // RLS must hide a private product from this user (with the old backdoor the
    // "admin" username would have made it visible).
    assert_eq!(
        app.call("GET", &format!("/products/{}", private.id), None, Some(&session))
            .await,
        StatusCode::NOT_FOUND
    );
}

// A user with a role on product A cannot read product B's product record or
// crashes through the RLS-scoped handlers.
#[tokio::test]
async fn rls_isolates_crash_data_between_products() {
    let app = TestApp::new().await;
    let prod_a = create_test_product(&app.db).await;
    let prod_b = create_test_product(&app.db).await;
    let user = create_test_user(&app.db, "iso_user", false).await;
    grant_product_role(&app.db, &user.id, &prod_a.id, "readonly").await;

    let crash_b = create_test_crash(&app.db, Some("fp-b"), Some(prod_b.id.clone())).await;
    let session = app
        .make_session(json!({ "user_id": user.id, "is_admin": false }))
        .await;

    // Can read the product it has a role on...
    assert_eq!(
        app.call("GET", &format!("/products/{}", prod_a.id), None, Some(&session))
            .await,
        StatusCode::OK
    );
    // ...but not the other product, nor that product's crash.
    assert_eq!(
        app.call("GET", &format!("/products/{}", prod_b.id), None, Some(&session))
            .await,
        StatusCode::NOT_FOUND
    );
    assert_ne!(
        app.call("GET", &format!("/crashes/by-crash/{}", crash_b.id), None, Some(&session))
            .await,
        StatusCode::OK
    );
}

// Even when a product is public, attachments (which can carry PII) are gated to
// product members: anonymous gets 404, a readonly member gets the object.
#[tokio::test]
async fn public_product_gates_attachments_from_anonymous() {
    let app = TestApp::new().await;
    let product = create_test_product(&app.db).await;
    set_product_public(&app.db, &product.id).await;

    let crash = create_test_crash(&app.db, Some("fp-pub"), Some(product.id.clone())).await;
    let att = create_test_attachment(
        &app.db,
        "log",
        "text/plain",
        5,
        "log.txt",
        Some(product.id.clone()),
        Some(crash.id.clone()),
    )
    .await;
    app.storage
        .put(
            &object_store::path::Path::from(att.storage_path.as_str()),
            object_store::PutPayload::from_static(b"hello"),
        )
        .await
        .expect("seed object");

    // Anonymous is gated even though the product is public (404, not disclosing
    // existence).
    assert_eq!(
        app.call("GET", &format!("/attachments/{}/download", att.id), None, None)
            .await,
        StatusCode::NOT_FOUND
    );

    // A readonly member of the product can download it — proving the gate is
    // role-based, not a blanket block.
    let member = create_test_user(&app.db, "ro_member", false).await;
    grant_product_role(&app.db, &member.id, &product.id, "readonly").await;
    let session = app
        .make_session(json!({ "user_id": member.id, "is_admin": false }))
        .await;
    assert_eq!(
        app.call("GET", &format!("/attachments/{}/download", att.id), None, Some(&session))
            .await,
        StatusCode::OK
    );
}
