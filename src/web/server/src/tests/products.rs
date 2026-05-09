use super::common::*;

// ---------------------------------------------------------------------------
// Tests: product management
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route     |
// | ------ | --------- |
// | POST   | /products |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 403      |
// | imp_admin     | 200      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_create_product() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Use unique names per admin context to avoid unique-index violations.
    assert_eq!(
        app.call("POST", "/products", Some(json!({"name": "Blocked"})), None)
            .await,
        StatusCode::FORBIDDEN,
        "no_session"
    );
    assert_eq!(
        app.call("POST", "/products", Some(json!({"name": "Blocked"})), Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "non_admin"
    );
    assert_eq!(
        app.call("POST", "/products", Some(json!({"name": "Blocked"})), Some(&f.imp_non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "imp_non_admin"
    );
    assert_eq!(
        app.call(
            "POST",
            "/products",
            Some(json!({"name": "Admin Created Product"})),
            Some(&f.admin)
        )
        .await,
        StatusCode::OK,
        "admin"
    );
    assert_eq!(
        app.call(
            "POST",
            "/products",
            Some(json!({"name": "ImpAdmin Created Product"})),
            Some(&f.imp_admin)
        )
        .await,
        StatusCode::OK,
        "imp_admin"
    );
}

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | DELETE | /products/{product_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 204      |
// | non_admin     | 403      |
// | imp_admin     | 204      |
// | imp_non_admin | 403      |
#[tokio::test]
async fn test_delete_product() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // Use fixture's p_none as first candidate (non_admin has no access, so no FK issues)
    let uri = format!("/products/{}", f.products[3].id);
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

    // admin and imp_admin succeed (use different products to avoid double-delete)
    let extra1 = create_test_product(&app.db).await;
    let extra2 = create_test_product(&app.db).await;
    assert_eq!(
        app.call("DELETE", &format!("/products/{}", extra1.id), None, Some(&f.admin))
            .await,
        StatusCode::NO_CONTENT,
        "admin"
    );
    assert_eq!(
        app.call("DELETE", &format!("/products/{}", extra2.id), None, Some(&f.imp_admin))
            .await,
        StatusCode::NO_CONTENT,
        "imp_admin"
    );
}

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | POST   | /products/{product_id} |
// Cases:
// | Auth/product role                      | Expected                              |
// | -------------------------------------- | ------------------------------------- |
// | no_session                             | 403                                   |
// | admin                                  | 200                                   |
// | imp_admin                              | 200                                   |
// | non_admin or imp_non_admin: no access  | 403                                   |
// | non_admin or imp_non_admin: read-only  | 403                                   |
// | non_admin or imp_non_admin: read-write | 403                                   |
// | non_admin or imp_non_admin: maintainer | 404 (guard passes, RLS blocks update) |
#[tokio::test]
async fn test_update_product_all_contexts() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // products RLS: `FOR update WHERE fn::auth::is_admin()` — admin only at DB level.
    // The auth guard (require_product_maintainer) is checked first:
    //   - no role or readonly/readwrite → 403 from guard
    //   - maintainer → guard passes, but RLS blocks the UPDATE → 0 rows → 404
    //   - admin → guard passes and RLS allows UPDATE → 200

    for p in &f.products {
        let uri = format!("/products/{}", p.id);
        let name = format!("Updated-{}", p.slug);
        let body = Some(json!({"name": name, "slug": p.slug, "description": ""}));

        // no_session always 403
        assert_eq!(
            app.call("POST", &uri, body.clone(), None).await,
            StatusCode::FORBIDDEN,
            "no_session {}",
            p.id
        );
        // admin: guard passes, RLS allows → 200
        assert_eq!(
            app.call("POST", &uri, body.clone(), Some(&f.admin)).await,
            StatusCode::OK,
            "admin {}",
            p.id
        );
        // imp_admin: same as admin
        assert_eq!(
            app.call("POST", &uri, body.clone(), Some(&f.imp_admin))
                .await,
            StatusCode::OK,
            "imp_admin {}",
            p.id
        );

        // non_admin: depends on their product role
        let (non_admin_expected, label) = if p.non_admin_maintainer {
            // guard passes (maintainer), but RLS blocks UPDATE → 0 rows → 404
            (StatusCode::NOT_FOUND, "non_admin maintainer (RLS blocks)")
        } else {
            // guard rejects (no maintainer role) → 403
            (StatusCode::FORBIDDEN, "non_admin non-maintainer")
        };
        assert_eq!(
            app.call("POST", &uri, body.clone(), Some(&f.non_admin))
                .await,
            non_admin_expected,
            "{label} {}",
            p.id
        );
        assert_eq!(
            app.call("POST", &uri, body, Some(&f.imp_non_admin)).await,
            non_admin_expected,
            "imp_{label} {}",
            p.id
        );
    }
}

// ---------------------------------------------------------------------------
// Tests: member management (product-maintainer)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                    |
// | ------ | ---------------------------------------- |
// | POST   | /products/{product_id}/members/{user_id} |
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
async fn test_grant_access_all_contexts() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target = create_test_user(&app.db, "grant_tgt", false).await;
    assert_product_maintainer(
        &app,
        &f,
        "POST",
        |pid| format!("/products/{pid}/members/{}", target.id),
        |_| Some(json!({"role": "readonly"})),
        StatusCode::NO_CONTENT,
    )
    .await;
}

// API calls:
// | Method | Route                                    |
// | ------ | ---------------------------------------- |
// | DELETE | /products/{product_id}/members/{user_id} |
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
async fn test_revoke_access_all_contexts() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target = create_test_user(&app.db, "revoke_tgt", false).await;
    // Pre-grant access for all products so revoke has something to act on
    for p in &f.products {
        grant_product_role(&app.db, &target.id, &p.id, "readonly").await;
    }
    assert_product_maintainer(
        &app,
        &f,
        "DELETE",
        |pid| format!("/products/{pid}/members/{}", target.id),
        |_| None,
        StatusCode::NO_CONTENT,
    )
    .await;
}

// ---------------------------------------------------------------------------
// Tests: product read endpoints
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | GET    | /products/{product_id} |
// Cases:
// | Case                                  | Expected |
// | ------------------------------------- | -------- |
// | no_session with private product       | 404      |
// | no_session with public product        | 200      |
// | admin with private product            | 200      |
// | non_admin with read-only product role | 200      |
#[tokio::test]
async fn test_get_product() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let private_uri = format!("/products/{}", f.products[0].id);

    assert_eq!(
        app.call("GET", &private_uri, None, None).await,
        StatusCode::NOT_FOUND,
        "anonymous cannot read private product"
    );
    assert_eq!(
        app.call("GET", &private_uri, None, Some(&f.admin)).await,
        StatusCode::OK,
        "admin can read private product"
    );
    assert_eq!(
        app.call("GET", &private_uri, None, Some(&f.non_admin))
            .await,
        StatusCode::OK,
        "read-only product member can read product"
    );

    let public_product = create_test_product(&app.db).await;
    app.db
        .query("UPDATE type::record('products', $pid) SET public = true")
        .bind(("pid", public_product.id.clone()))
        .await
        .expect("mark public product failed");
    let public_uri = format!("/products/{}", public_product.id);
    assert_eq!(
        app.call("GET", &public_uri, None, None).await,
        StatusCode::OK,
        "anonymous can read public product"
    );
}

// API calls:
// | Method | Route                               |
// | ------ | ----------------------------------- |
// | GET    | /products                           |
// | GET    | /products?scope=mine&user={user_id} |
// | GET    | /products?scope=mine                |
// | GET    | /products?scope=public              |
// Cases:
// | Case                                    | Expected |
// | --------------------------------------- | -------- |
// | default listing: no_session             | 200 with only public products |
// | default listing: admin                  | 200      |
// | scope=mine with explicit non_admin user | 200      |
// | scope=mine without user as admin        | 200      |
// | scope=public without session            | 200 with only public products |
#[tokio::test]
async fn test_list_products() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let public_product = create_test_product(&app.db).await;
    app.db
        .query("UPDATE type::record('products', $pid) SET public = true")
        .bind(("pid", public_product.id.clone()))
        .await
        .expect("mark public product failed");

    // Default scope (no query): all contexts get a 200
    assert_eq!(app.call("GET", "/products", None, Some(&f.admin)).await, StatusCode::OK);
    let (status, anonymous_products) = app.call_json("GET", "/products", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let anonymous_ids: Vec<_> = anonymous_products
        .as_array()
        .expect("products response should be an array")
        .iter()
        .filter_map(|p| p.get("id").and_then(|id| id.as_str()))
        .collect();
    assert!(
        anonymous_ids.contains(&public_product.id.as_str()),
        "anonymous list should include public product: {anonymous_products}"
    );
    assert!(
        !anonymous_ids.contains(&f.products[0].id.as_str()),
        "anonymous list must not include private product: {anonymous_products}"
    );

    // scope=mine with explicit user
    let mine = format!("/products?scope=mine&user={}", f.non_admin_id);
    assert_eq!(app.call("GET", &mine, None, Some(&f.non_admin)).await, StatusCode::OK);

    // scope=mine without user → empty 200
    assert_eq!(
        app.call("GET", "/products?scope=mine", None, Some(&f.admin))
            .await,
        StatusCode::OK
    );

    // scope=public → 200 (no session needed)
    let (status, public_products) = app
        .call_json("GET", "/products?scope=public", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let public_ids: Vec<_> = public_products
        .as_array()
        .expect("public products response should be an array")
        .iter()
        .filter_map(|p| p.get("id").and_then(|id| id.as_str()))
        .collect();
    assert!(public_ids.contains(&public_product.id.as_str()));
    assert!(!public_ids.contains(&f.products[0].id.as_str()));
}

// API calls:
// | Method | Route                          |
// | ------ | ------------------------------ |
// | GET    | /products/{product_id}/members |
// Cases:
// | Auth context | Expected |
// | ------------ | -------- |
// | no_session   | 200      |
// | admin        | 200      |
// | non_admin    | 200      |
#[tokio::test]
async fn test_list_members() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let uri = format!("/products/{}/members", f.products[0].id);
    // list_members has no auth guard — RLS scopes results
    assert_eq!(app.call("GET", &uri, None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &uri, None, Some(&f.non_admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &uri, None, None).await, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Tests: db_api – update_product with public flag
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                  |
// | ------ | ---------------------- |
// | POST   | /products/{product_id} |
// Cases:
// | Case                    | Expected |
// | ----------------------- | -------- |
// | admin sets public=true  | 200      |
// | admin sets public=false | 200      |
#[tokio::test]
async fn test_update_product_public_flag() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;
    // Flip product to public (must include slug)
    let body = json!({"name": "Updated", "slug": "updated", "description": "desc", "public": true});
    assert_eq!(
        app.call("POST", &format!("/products/{pid}"), Some(body.clone()), Some(&f.admin))
            .await,
        StatusCode::OK,
    );
    // Flip back to private
    let body2 =
        json!({"name": "Updated", "slug": "updated", "description": "desc", "public": false});
    assert_eq!(
        app.call("POST", &format!("/products/{pid}"), Some(body2), Some(&f.admin))
            .await,
        StatusCode::OK,
    );
}
