use super::common::*;

// ---------------------------------------------------------------------------
// Tests: product API tokens (product-maintainer)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                             |
// | ------ | --------------------------------- |
// | GET    | /products/{product_id}/api-tokens |
// Cases:
// | Auth/product role                      | Expected |
// | -------------------------------------- | -------- |
// | no_session                             | 403      |
// | admin                                  | 200      |
// | imp_admin                              | 200      |
// | non_admin or imp_non_admin: no access  | 403      |
// | non_admin or imp_non_admin: read-only  | 403      |
// | non_admin or imp_non_admin: read-write | 403      |
// | non_admin or imp_non_admin: maintainer | 200      |
#[tokio::test]
async fn test_list_product_api_tokens_all_contexts() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_product_maintainer(
        &app,
        &f,
        "GET",
        |pid| format!("/products/{pid}/api-tokens"),
        |_| None,
        StatusCode::OK,
    )
    .await;
}

// API calls:
// | Method | Route                             |
// | ------ | --------------------------------- |
// | POST   | /products/{product_id}/api-tokens |
// Cases:
// | Auth/product role                      | Expected |
// | -------------------------------------- | -------- |
// | no_session                             | 403      |
// | admin                                  | 200      |
// | imp_admin                              | 200      |
// | non_admin or imp_non_admin: no access  | 403      |
// | non_admin or imp_non_admin: read-only  | 403      |
// | non_admin or imp_non_admin: read-write | 403      |
// | non_admin or imp_non_admin: maintainer | 200      |
#[tokio::test]
async fn test_create_product_api_token_all_contexts() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_product_maintainer(
        &app,
        &f,
        "POST",
        |pid| format!("/products/{pid}/api-tokens"),
        |_| Some(json!({"description": "test token"})),
        StatusCode::OK,
    )
    .await;
}

// API calls:
// | Method | Route                                        |
// | ------ | -------------------------------------------- |
// | DELETE | /products/{product_id}/api-tokens/{token_id} |
// Cases:
// | Auth/product role                      | Expected |
// | -------------------------------------- | -------- |
// | no_session                             | 403      |
// | admin                                  | 204      |
// | imp_admin                              | 204      |
// | non_admin or imp_non_admin: no access  | 403      |
// | non_admin or imp_non_admin: read-only  | 403      |
// | non_admin or imp_non_admin: read-write | 403      |
// | non_admin or imp_non_admin: maintainer | 204      |
#[tokio::test]
async fn test_delete_product_api_token_all_contexts() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    for p in &f.products {
        // Create a fresh token per product for each check (5 contexts × create + delete)
        // no_session and non-maintainer contexts only need to fail at auth, no token needed
        // We need a real token for the success paths (admin, imp_admin, and maint for p_maint)
        let (_, tok1) =
            create_test_token(&app.db, "del_tok", Some(p.id.clone()), None, &["token"]).await;
        let uri_tok1 = format!("/products/{}/api-tokens/{}", p.id, tok1.id);

        assert_eq!(
            app.call("DELETE", &uri_tok1, None, None).await,
            StatusCode::FORBIDDEN,
            "no_session product {}",
            p.id
        );
        assert_eq!(
            app.call("DELETE", &uri_tok1, None, Some(&f.non_admin))
                .await,
            if p.non_admin_maintainer {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::FORBIDDEN
            },
            "non_admin product {}",
            p.id
        );
        assert_eq!(
            app.call("DELETE", &uri_tok1, None, Some(&f.imp_non_admin))
                .await,
            if p.non_admin_maintainer {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::FORBIDDEN
            },
            "imp_non_admin product {}",
            p.id
        );

        // Admin contexts need their own tokens (non_admin/imp_non_admin may have consumed above)
        let (_, tok2) =
            create_test_token(&app.db, "del_tok2", Some(p.id.clone()), None, &["token"]).await;
        let uri_tok2 = format!("/products/{}/api-tokens/{}", p.id, tok2.id);
        assert_eq!(
            app.call("DELETE", &uri_tok2, None, Some(&f.admin)).await,
            StatusCode::NO_CONTENT,
            "admin product {}",
            p.id
        );

        let (_, tok3) =
            create_test_token(&app.db, "del_tok3", Some(p.id.clone()), None, &["token"]).await;
        let uri_tok3 = format!("/products/{}/api-tokens/{}", p.id, tok3.id);
        assert_eq!(
            app.call("DELETE", &uri_tok3, None, Some(&f.imp_admin))
                .await,
            StatusCode::NO_CONTENT,
            "imp_admin product {}",
            p.id
        );
    }
}

// ---------------------------------------------------------------------------
// Tests: admin API tokens
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route       |
// | ------ | ----------- |
// | GET    | /api-tokens |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_list_all_api_tokens() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_admin_only(&app, &f, "GET", "/api-tokens", None, StatusCode::OK).await;
}

// API calls:
// | Method | Route                    |
// | ------ | ------------------------ |
// | GET    | /api-tokens/entitlements |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_list_entitlements() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_admin_only(&app, &f, "GET", "/api-tokens/entitlements", None, StatusCode::OK).await;
}

// API calls:
// | Method | Route       |
// | ------ | ----------- |
// | POST   | /api-tokens |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_create_admin_api_token() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let body = json!({"description": "global token"});
    assert_admin_only(&app, &f, "POST", "/api-tokens", Some(body), StatusCode::OK).await;
}

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | PATCH  | /api-tokens/{token_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 204      |
// | non_admin     | 403      |
// | imp_admin     | 204      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_update_admin_api_token() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let (_, tok) = create_test_token(&app.db, "upd_tok", None, None, &["token"]).await;
    let uri = format!("/api-tokens/{}", tok.id);
    let body = json!({
        "description": "updated", "isActive": true,
        "entitlements": ["token"], "productId": null, "userId": null
    });
    assert_admin_only(&app, &f, "PATCH", &uri, Some(body), StatusCode::NO_CONTENT).await;
}

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | DELETE | /api-tokens/{token_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 204      |
// | non_admin     | 403      |
// | imp_admin     | 204      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_delete_admin_api_token() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // non_admin and no_session fail on a fresh token
    let (_, tok_a) = create_test_token(&app.db, "del_admin_tok", None, None, &["token"]).await;
    let uri_a = format!("/api-tokens/{}", tok_a.id);
    assert_eq!(app.call("DELETE", &uri_a, None, None).await, StatusCode::FORBIDDEN, "no_session");
    assert_eq!(
        app.call("DELETE", &uri_a, None, Some(&f.non_admin)).await,
        StatusCode::FORBIDDEN,
        "non_admin"
    );
    assert_eq!(
        app.call("DELETE", &uri_a, None, Some(&f.imp_non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "imp_non_admin"
    );

    let (_, tok_b) = create_test_token(&app.db, "del_admin_tok2", None, None, &["token"]).await;
    assert_eq!(
        app.call("DELETE", &format!("/api-tokens/{}", tok_b.id), None, Some(&f.admin))
            .await,
        StatusCode::NO_CONTENT,
        "admin"
    );

    let (_, tok_c) = create_test_token(&app.db, "del_admin_tok3", None, None, &["token"]).await;
    assert_eq!(
        app.call("DELETE", &format!("/api-tokens/{}", tok_c.id), None, Some(&f.imp_admin))
            .await,
        StatusCode::NO_CONTENT,
        "imp_admin"
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – empty description validation in API token handlers
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                             |
// | ------ | --------------------------------- |
// | POST   | /products/{product_id}/api-tokens |
// | POST   | /api-tokens                       |
// | PATCH  | /api-tokens/{token_id}            |
// Cases:
// | Case                                                  | Expected |
// | ----------------------------------------------------- | -------- |
// | product-scoped create with blank description as admin | 400      |
// | admin-token create with blank description as admin    | 400      |
// | admin-token update with blank description as admin    | 400      |
#[tokio::test]
async fn test_api_token_empty_description() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[2].id;

    // create_api_token (product-scoped): empty description → 400
    assert_eq!(
        app.call(
            "POST",
            &format!("/products/{pid}/api-tokens"),
            Some(json!({"description": "  "})),
            Some(&f.admin),
        )
        .await,
        StatusCode::BAD_REQUEST,
    );

    // create_admin_api_token: empty description → 400
    assert_eq!(
        app.call("POST", "/api-tokens", Some(json!({"description": ""})), Some(&f.admin),)
            .await,
        StatusCode::BAD_REQUEST,
    );

    // update_admin_api_token: create one first, then try empty description
    let (_, v) = app
        .call_json("POST", "/api-tokens", Some(json!({"description": "valid"})), Some(&f.admin))
        .await;
    let token_id = v["id"].as_str().expect("no id").to_string();
    assert_eq!(
        app.call(
            "PATCH",
            &format!("/api-tokens/{token_id}"),
            Some(json!({"description": "", "isActive": true, "entitlements": []})),
            Some(&f.admin),
        )
        .await,
        StatusCode::BAD_REQUEST,
    );
}
