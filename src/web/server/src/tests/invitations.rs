use super::common::*;

// ---------------------------------------------------------------------------
// Tests: invitation endpoints
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route        |
// | ------ | ------------ |
// | GET    | /invitations |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | 200      |
// | imp_admin     | 200      |
// | imp_non_admin | 200      |
#[tokio::test]
async fn test_list_invitations_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // GET /invitations uses require_session → empty list for all sessions is fine
    assert_all(
        &app,
        &f,
        "GET",
        "/invitations",
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
// | Method | Route        |
// | ------ | ------------ |
// | POST   | /invitations |
// Cases:
// | Auth context                                           | Expected |
// | ------------------------------------------------------ | -------- |
// | no_session                                             | 403      |
// | admin creating non-admin invitation without grants     | 200      |
// | imp_admin creating non-admin invitation without grants | 200      |
#[tokio::test]
async fn test_create_invitation_admin() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    let body = json!({"is_admin": false, "grants": []});

    // no_session → 403
    assert_eq!(
        app.call("POST", "/invitations", Some(body.clone()), None)
            .await,
        StatusCode::FORBIDDEN,
        "no_session"
    );
    // admin → 200
    assert_eq!(
        app.call("POST", "/invitations", Some(body.clone()), Some(&f.admin))
            .await,
        StatusCode::OK,
        "admin"
    );
    // imp_admin → 200
    assert_eq!(
        app.call("POST", "/invitations", Some(body.clone()), Some(&f.imp_admin))
            .await,
        StatusCode::OK,
        "imp_admin"
    );
}

// API calls:
// | Method | Route        |
// | ------ | ------------ |
// | POST   | /invitations |
// Cases:
// | Case                                           | Expected |
// | ---------------------------------------------- | -------- |
// | non_admin creates admin invitation             | 403      |
// | imp_non_admin creates admin invitation         | 403      |
// | non_admin grants unmaintained product access   | 403      |
// | non_admin grants maintained product access     | 200      |
// | imp_non_admin grants maintained product access | 200      |
#[tokio::test]
async fn test_create_invitation_non_admin_restrictions() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // non_admin creating admin invitation → 403
    let admin_body = json!({"is_admin": true, "grants": []});
    assert_eq!(
        app.call("POST", "/invitations", Some(admin_body.clone()), Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "non_admin cannot create admin invitation"
    );
    assert_eq!(
        app.call("POST", "/invitations", Some(admin_body), Some(&f.imp_non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "imp_non_admin cannot create admin invitation"
    );

    // non_admin granting access to product they don't maintain → 403
    let bad_grant = json!({
        "is_admin": false,
        "grants": [{"product_id": f.products[3].id, "role": "readonly"}]
    });
    assert_eq!(
        app.call("POST", "/invitations", Some(bad_grant.clone()), Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "non_admin cannot grant access to unowned product"
    );

    // non_admin granting access to product they maintain → 200
    let good_grant = json!({
        "is_admin": false,
        "grants": [{"product_id": f.products[2].id, "role": "readonly"}]
    });
    assert_eq!(
        app.call("POST", "/invitations", Some(good_grant.clone()), Some(&f.non_admin))
            .await,
        StatusCode::OK,
        "non_admin can grant access to maintained product"
    );
    assert_eq!(
        app.call("POST", "/invitations", Some(good_grant), Some(&f.imp_non_admin))
            .await,
        StatusCode::OK,
        "imp_non_admin can grant access to maintained product"
    );
}

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | PUT    | /invitations/{invitation_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | not 403  |
// | non_admin     | not 403  |
// | imp_admin     | not 403  |
// | imp_non_admin | not 403  |
#[tokio::test]
async fn test_update_invitation_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let body = json!({"is_admin": false, "grants": []});
    // No existing invitation → 404 for sessions (auth passes), 403 for no_session
    assert_eq!(
        app.call("PUT", "/invitations/nonexistent", Some(body.clone()), None)
            .await,
        StatusCode::FORBIDDEN,
        "no_session"
    );
    // With any session: auth passes, invitation not found → not 403
    for (cookie, label) in &[
        (Some(f.admin.as_str()), "admin"),
        (Some(f.non_admin.as_str()), "non_admin"),
        (Some(f.imp_admin.as_str()), "imp_admin"),
        (Some(f.imp_non_admin.as_str()), "imp_non_admin"),
    ] {
        let got = app
            .call("PUT", "/invitations/nonexistent", Some(body.clone()), *cookie)
            .await;
        assert_ne!(got, StatusCode::FORBIDDEN, "{label} should not be 403");
    }
}

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | DELETE | /invitations/{invitation_id} |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | 200      |
// | non_admin     | not 403  |
// | imp_admin     | 200      |
// | imp_non_admin | not 403  |
#[tokio::test]
async fn test_revoke_invitation_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    assert_eq!(
        app.call("DELETE", "/invitations/nonexistent", None, None)
            .await,
        StatusCode::FORBIDDEN,
        "no_session"
    );
    // admin: no existence check → 200 (revoke is idempotent for admins)
    assert_eq!(
        app.call("DELETE", "/invitations/nonexistent", None, Some(&f.admin))
            .await,
        StatusCode::OK,
        "admin"
    );
    assert_eq!(
        app.call("DELETE", "/invitations/nonexistent", None, Some(&f.imp_admin))
            .await,
        StatusCode::OK,
        "imp_admin"
    );
    // non_admin: checks existence first → 404
    let non_admin_got = app
        .call("DELETE", "/invitations/nonexistent", None, Some(&f.non_admin))
        .await;
    assert_ne!(non_admin_got, StatusCode::FORBIDDEN, "non_admin should not be 403");
    let imp_non_admin_got = app
        .call("DELETE", "/invitations/nonexistent", None, Some(&f.imp_non_admin))
        .await;
    assert_ne!(imp_non_admin_got, StatusCode::FORBIDDEN, "imp_non_admin should not be 403");
}

// ---------------------------------------------------------------------------
// Tests: invitation redemption flow
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | GET    | /invitations/redeem/{code} |
// Cases:
// | Case         | Expected |
// | ------------ | -------- |
// | invalid code | 404      |
// | valid code   | 200      |
#[tokio::test]
async fn test_get_invite_info() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // invalid code → 404
    assert_eq!(
        app.call("GET", "/invitations/redeem/no-such-code", None, None)
            .await,
        StatusCode::NOT_FOUND
    );

    // valid code → { valid: true }
    let (_, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(
        app.call("GET", &format!("/invitations/redeem/{code}"), None, None)
            .await,
        StatusCode::OK
    );
}

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /invitations/redeem/{code} |
// Cases:
// | Case                           | Expected |
// | ------------------------------ | -------- |
// | invalid code                   | 404      |
// | valid code without provisioner | 400      |
// | max_uses exceeded              | 404      |
#[tokio::test]
async fn test_redeem_invite_json_no_provisioner() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // invalid code → 404
    let body = json!({"username": "u", "email": "u@e.com"});
    assert_eq!(
        app.call("POST", "/invitations/redeem/no-such-code", Some(body.clone()), None)
            .await,
        StatusCode::NOT_FOUND,
    );

    // valid code, no provisioner configured → 400
    let (_, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(
        app.call("POST", &format!("/invitations/redeem/{code}"), Some(body.clone()), None)
            .await,
        StatusCode::BAD_REQUEST,
    );

    // max_uses exceeded → 404 ("Invitation has been fully used")
    let (id_lim, code_lim) = api_create_invitation(
        &app,
        &f.admin,
        json!({"is_admin": false, "grants": [], "max_uses": 1}),
    )
    .await;
    app.db
        .query("UPDATE type::record('invitations', $id) SET use_count = 1")
        .bind(("id", id_lim))
        .await
        .unwrap();
    assert_eq!(
        app.call("POST", &format!("/invitations/redeem/{code_lim}"), Some(body), None)
            .await,
        StatusCode::NOT_FOUND,
    );
}

// API calls:
// | Method | Route          |
// | ------ | -------------- |
// | GET    | /invite/{code} |
// Cases:
// | Case                           | Expected |
// | ------------------------------ | -------- |
// | invalid code                   | 404      |
// | valid code without provisioner | 200      |
// | valid code with error query    | 200      |
#[tokio::test]
async fn test_show_invite_form() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // invalid code → 404
    assert_eq!(app.call("GET", "/invite/no-such-code", None, None).await, StatusCode::NOT_FOUND);

    // valid code, no provisioner → renders form (200)
    let (_, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(
        app.call("GET", &format!("/invite/{code}"), None, None)
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("/invite/{code}?error=oops"), None, None)
            .await,
        StatusCode::OK
    );
}

// ---------------------------------------------------------------------------
// Tests: invitation update / revoke success paths
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | PUT    | /invitations/{invitation_id} |
// Cases:
// | Case                                                | Expected |
// | --------------------------------------------------- | -------- |
// | admin updates any invitation                        | 200      |
// | non_admin updates admin-created invitation          | 403      |
// | non_admin updates own maintained-product invitation | 200      |
// | non_admin adds unmaintained-product grant           | 403      |
#[tokio::test]
async fn test_update_invitation_success() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // Admin can update any invitation
    let (id, _) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    let uri = format!("/invitations/{id}");
    let update = json!({"is_admin": false, "grants": []});
    assert_eq!(
        app.call("PUT", &uri, Some(update.clone()), Some(&f.admin))
            .await,
        StatusCode::OK
    );

    // Non-admin cannot update an admin-created invitation (no overlap) → 403
    assert_eq!(
        app.call("PUT", &uri, Some(update), Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN
    );

    // Non-admin can update an invitation they created (created_by overlap)
    let (id2, _) = api_create_invitation(
        &app,
        &f.non_admin,
        json!({"is_admin": false, "grants": [{"product_id": f.products[2].id, "role": "readonly"}]}),
    )
    .await;
    let uri2 = format!("/invitations/{id2}");
    let update2 = json!({"is_admin": false, "grants": [{"product_id": f.products[2].id, "role": "readonly"}]});
    assert_eq!(
        app.call("PUT", &uri2, Some(update2), Some(&f.non_admin))
            .await,
        StatusCode::OK
    );

    // Non-admin cannot add grants for products they don't maintain → 403
    let bad_update = json!({"is_admin": false, "grants": [{"product_id": f.products[3].id, "role": "readonly"}]});
    assert_eq!(
        app.call("PUT", &uri2, Some(bad_update), Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN
    );
}

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | DELETE | /invitations/{invitation_id} |
// Cases:
// | Case                                              | Expected |
// | ------------------------------------------------- | -------- |
// | admin revokes any invitation                      | 200      |
// | non_admin revokes maintained-product invitation   | 200      |
// | non_admin revokes unmaintained-product invitation | 403      |
#[tokio::test]
async fn test_revoke_invitation_existing() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // Admin can revoke any invitation
    let (id, _) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(
        app.call("DELETE", &format!("/invitations/{id}"), None, Some(&f.admin))
            .await,
        StatusCode::OK,
    );

    // Non-admin can revoke invitation for a maintained product
    let (id2, _) = api_create_invitation(
        &app,
        &f.admin,
        json!({"is_admin": false, "grants": [{"product_id": f.products[2].id, "role": "readonly"}]}),
    )
    .await;
    assert_eq!(
        app.call("DELETE", &format!("/invitations/{id2}"), None, Some(&f.non_admin))
            .await,
        StatusCode::OK,
        "non_admin maintainer can revoke",
    );

    // Non-admin cannot revoke invitation for a product they don't maintain → 403
    let (id3, _) = api_create_invitation(
        &app,
        &f.admin,
        json!({"is_admin": false, "grants": [{"product_id": f.products[3].id, "role": "readonly"}]}),
    )
    .await;
    assert_eq!(
        app.call("DELETE", &format!("/invitations/{id3}"), None, Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "non_admin cannot revoke unowned product invitation",
    );
}

#[tokio::test]
async fn test_used_invitation_only_allows_delete() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    let (id, _) = api_create_invitation(
        &app,
        &f.admin,
        json!({"is_admin": false, "grants": [], "max_uses": 1}),
    )
    .await;
    app.db
        .query("UPDATE type::record('invitations', $id) SET use_count = 1, status = 'Exhausted'")
        .bind(("id", id.clone()))
        .await
        .unwrap();

    assert_eq!(
        app.call(
            "PUT",
            &format!("/invitations/{id}"),
            Some(json!({"is_admin": false, "grants": [], "max_uses": 1})),
            Some(&f.admin)
        )
        .await,
        StatusCode::BAD_REQUEST,
    );
    assert_eq!(
        app.call(
            "POST",
            &format!("/invitations/{id}/send"),
            Some(json!({"to": "user@example.com"})),
            Some(&f.admin)
        )
        .await,
        StatusCode::BAD_REQUEST,
    );
    assert_eq!(
        app.call("POST", &format!("/invitations/{id}/revoke"), None, Some(&f.admin))
            .await,
        StatusCode::BAD_REQUEST,
    );
    assert_eq!(
        app.call("DELETE", &format!("/invitations/{id}"), None, Some(&f.admin))
            .await,
        StatusCode::OK,
    );
}

// ---------------------------------------------------------------------------
// Tests: invite – create_invitation via API token (Principal::Token path)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                       |
// | ------ | --------------------------- |
// | POST   | /invitations (Bearer token) |
// Cases:
// | Case                                      | Expected |
// | ----------------------------------------- | -------- |
// | global token creates non-admin invitation | 200      |
// | scoped token creates admin invitation     | 403      |
// | scoped token grants outside product       | 403      |
// | scoped token grants own product           | 200      |
#[tokio::test]
async fn test_create_invitation_via_api_token() {
    let app = TestApp::new().await;

    // Global token with invitation-create entitlement
    let (raw_token, _) =
        create_test_token(&app.db, "invite-token", None, None, &["invitation-create"]).await;

    // Token can create a non-admin invitation
    let req = Request::builder()
        .method("POST")
        .uri("/invitations")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {raw_token}"))
        .body(Body::from(json!({"is_admin": false, "grants": []}).to_string()))
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::OK);

    // Product-scoped token: trying to create admin invitation → 403
    let p = create_test_product(&app.db).await;
    let (scoped_token, _) = create_test_token(
        &app.db,
        "scoped-invite-token",
        Some(p.id.clone()),
        None,
        &["invitation-create"],
    )
    .await;
    let req2 = Request::builder()
        .method("POST")
        .uri("/invitations")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {scoped_token}"))
        .body(Body::from(json!({"is_admin": true, "grants": []}).to_string()))
        .unwrap();
    let (status2, _, _) = app.send(req2).await;
    assert_eq!(status2, StatusCode::FORBIDDEN);

    // Product-scoped token: trying to grant access outside its product → 403
    let other = create_test_product(&app.db).await;
    let req3 = Request::builder()
        .method("POST")
        .uri("/invitations")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {scoped_token}"))
        .body(Body::from(
            json!({"is_admin": false, "grants": [{"product_id": other.id, "role": "readonly"}]})
                .to_string(),
        ))
        .unwrap();
    let (status3, _, _) = app.send(req3).await;
    assert_eq!(status3, StatusCode::FORBIDDEN);

    // Product-scoped token: grant within its own product → 200
    let req4 = Request::builder()
        .method("POST")
        .uri("/invitations")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {scoped_token}"))
        .body(Body::from(
            json!({"is_admin": false, "grants": [{"product_id": p.id, "role": "readonly"}]})
                .to_string(),
        ))
        .unwrap();
    let (status4, _, _) = app.send(req4).await;
    assert_eq!(status4, StatusCode::OK);
}

// API calls:
// | Method | Route        |
// | ------ | ------------ |
// | POST   | /invitations |
// Cases:
// | Bearer token                                 | Expected |
// | -------------------------------------------- | -------- |
// | global token missing invitation-create grant | 403      |
#[tokio::test]
async fn test_create_invitation_via_api_token_missing_entitlement() {
    let app = TestApp::new().await;
    let (raw_token, _) =
        create_test_token(&app.db, "invite-token-no-entitlement", None, None, &["token"]).await;

    let req = Request::builder()
        .method("POST")
        .uri("/invitations")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {raw_token}"))
        .body(Body::from(json!({"is_admin": false, "grants": []}).to_string()))
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// Tests: invite – non-admin with no maintained products → 403
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route        |
// | ------ | ------------ |
// | POST   | /invitations |
// Cases:
// | Case                                                                         | Expected |
// | ---------------------------------------------------------------------------- | -------- |
// | non_admin with no maintained products creates grantless non-admin invitation | 403      |
#[tokio::test]
async fn test_create_invitation_no_maintainer_role() {
    let app = TestApp::new().await;
    // Create a fresh user with NO roles at all
    let user = create_test_user(&app.db, "no_role_user", false).await;
    let cookie = app
        .make_session(json!({"user_id": user.id, "name": "NoRole", "is_admin": false}))
        .await;

    // No maintained products → 403
    assert_eq!(
        app.call(
            "POST",
            "/invitations",
            Some(json!({"is_admin": false, "grants": []})),
            Some(&cookie)
        )
        .await,
        StatusCode::FORBIDDEN,
    );
}

// ---------------------------------------------------------------------------
// Tests: invite – redeem_invite form (POST /invite/{code})
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route          |
// | ------ | -------------- |
// | POST   | /invite/{code} |
// Cases:
// | Case                           | Expected |
// | ------------------------------ | -------- |
// | invalid code                   | 404      |
// | valid code without provisioner | 400      |
// | max_uses exceeded              | 404      |
#[tokio::test]
async fn test_redeem_invite_form() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // Invalid code → 404
    let form_body = "username=user&email=u%40e.com&first_name=First&last_name=Last";
    let req = Request::builder()
        .method("POST")
        .uri("/invite/no-such-code")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form_body))
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Valid code but no provisioner → 400 (Failure)
    let (_, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/invite/{code}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form_body))
        .unwrap();
    let (status2, _, _) = app.send(req2).await;
    assert_eq!(status2, StatusCode::BAD_REQUEST);

    // max_uses exceeded → 404
    let (id_lim, code_lim) = api_create_invitation(
        &app,
        &f.admin,
        json!({"is_admin": false, "grants": [], "max_uses": 1}),
    )
    .await;
    app.db
        .query("UPDATE type::record('invitations', $id) SET use_count = 1")
        .bind(("id", id_lim))
        .await
        .unwrap();
    let req3 = Request::builder()
        .method("POST")
        .uri(format!("/invite/{code_lim}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form_body))
        .unwrap();
    let (status3, _, _) = app.send(req3).await;
    assert_eq!(status3, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tests: invite – provisioner paths (get_invite_info, redeem_invite_json,
//                                    show_invite_form, redeem_invite form)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | GET    | /invitations/redeem/{code} |
// Cases:
// | Case                                                 | Expected                  |
// | ---------------------------------------------------- | ------------------------- |
// | valid invitation with pending access and provisioner | 200 with needs_refresh=true |
#[tokio::test]
async fn test_get_invite_info_with_pending_access() {
    let app = TestApp::new_with_provisioner().await;
    let f = Fixture::setup(&app).await;

    // Create an invitation and a matching pending_access record
    let (inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    create_pending_access(&app.db, &inv_id, "ext-testuser").await;

    // GET /invitations/redeem/{code} with pending → no token created, frontend polls via refresh
    let (status, body) = app
        .call_json("GET", &format!("/invitations/redeem/{code}"), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.get("needs_refresh").and_then(|v| v.as_bool()), Some(true), "expected needs_refresh=true; got {body}");
    assert!(body.get("setup_url").is_none(), "setup_url must not be returned on GET; got {body}");
}

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /invitations/redeem/{code} |
// Cases:
// | Case                                                         | Expected              |
// | ------------------------------------------------------------ | --------------------- |
// | JSON redemption with existing pending access and provisioner | 200 with redirect_url |
#[tokio::test]
async fn test_redeem_invite_json_existing_pending() {
    let app = TestApp::new_with_provisioner().await;
    let f = Fixture::setup(&app).await;

    let (inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    create_pending_access(&app.db, &inv_id, "ext-existing").await;

    // POST with JSON: pending_access exists → provisioner re-issues setup URL
    let body = json!({"username": "existing", "email": "existing@x.com"});
    let (status, resp) = app
        .call_json("POST", &format!("/invitations/redeem/{code}"), Some(body), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.get("setup_url").is_some(), "expected setup_url; got {resp}");
}

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /invitations/redeem/{code} |
// Cases:
// | Case                                                            | Expected              |
// | --------------------------------------------------------------- | --------------------- |
// | JSON redemption for new user with provisioner and product grant | 200 with redirect_url |
#[tokio::test]
async fn test_redeem_invite_json_new_user() {
    let app = TestApp::new_with_provisioner().await;
    let f = Fixture::setup(&app).await;

    // Use non-empty grants so the grants mapping closure (lines 321-323) is exercised.
    let pid = &f.products[0].id;
    let (inv_id, code) = api_create_invitation(
        &app,
        &f.admin,
        json!({"is_admin": false, "grants": [{"product_id": pid, "role": "readonly"}]}),
    )
    .await;

    // POST with JSON: no pending_access → provisioner creates user
    let body = json!({"username": "newuser", "email": "newuser@x.com",
                      "first_name": "New", "last_name": "User"});
    let (status, resp) = app
        .call_json("POST", &format!("/invitations/redeem/{code}"), Some(body), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.get("setup_url").is_some(), "expected setup_url; got {resp}");

    let invitation = repos::invitation::InvitationRepo::get_by_id(&app.db, inv_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invitation.accepted_username.as_deref(), Some("newuser"));
    assert_eq!(invitation.accepted_email.as_deref(), Some("newuser@x.com"));
}

// API calls:
// | Method | Route          |
// | ------ | -------------- |
// | GET    | /invite/{code} |
// Cases:
// | Case                                          | Expected |
// | --------------------------------------------- | -------- |
// | form view with pending access and provisioner | 303      |
#[tokio::test]
async fn test_show_invite_form_with_pending_access() {
    let app = TestApp::new_with_provisioner().await;
    let f = Fixture::setup(&app).await;

    let (inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    create_pending_access(&app.db, &inv_id, "ext-pending").await;

    // GET /invite/{code}: provisioner + pending → 303 redirect
    let status = app
        .call("GET", &format!("/invite/{code}"), None, None)
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

// API calls:
// | Method | Route          |
// | ------ | -------------- |
// | POST   | /invite/{code} |
// Cases:
// | Case                                                            | Expected |
// | --------------------------------------------------------------- | -------- |
// | form redemption for new user with provisioner and product grant | 303      |
#[tokio::test]
async fn test_redeem_invite_form_with_provisioner() {
    let app = TestApp::new_with_provisioner().await;
    let f = Fixture::setup(&app).await;

    // Use non-empty grants so the grants mapping closure (lines 439-441) is exercised.
    let pid = &f.products[0].id;
    let (inv_id, code) = api_create_invitation(
        &app,
        &f.admin,
        json!({"is_admin": false, "grants": [{"product_id": pid, "role": "readonly"}]}),
    )
    .await;

    // POST /invite/{code} with form: provisioner creates user → redirect
    // Also exercises non_empty() with both empty and non-empty optional fields.
    let form_body =
        "username=formuser&email=formuser%40x.com&first_name=Form&last_name=".to_string();
    let req = Request::builder()
        .method("POST")
        .uri(format!("/invite/{code}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form_body))
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let invitation = repos::invitation::InvitationRepo::get_by_id(&app.db, inv_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invitation.accepted_username.as_deref(), Some("formuser"));
    assert_eq!(invitation.accepted_email.as_deref(), Some("formuser@x.com"));
}

// ---------------------------------------------------------------------------
// Tests: invite – provisioner error paths (map_err closures)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                       |
// | ------ | ------------------------------------------- |
// | POST   | /invitations/redeem/{code}/setup-url        |
// Cases:
// | Case                             | Expected |
// | -------------------------------- | -------- |
// | pending access setup URL failure | 400      |
#[tokio::test]
async fn test_refresh_setup_url_error() {
    // Covers the map_err closure in refresh_setup_url:
    // provisioner.create_setup_url fails → 400 Failure response.
    let app = TestApp::new_with_failing_provisioner().await;
    let f = Fixture::setup(&app).await;

    let (inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    create_pending_access(&app.db, &inv_id, "ext-testuser").await;

    let (status, _) = app
        .call_json("POST", &format!("/invitations/redeem/{code}/setup-url"), None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /invitations/redeem/{code} |
// Cases:
// | Case                                               | Expected |
// | -------------------------------------------------- | -------- |
// | JSON redemption existing pending setup URL failure | 400      |
#[tokio::test]
async fn test_redeem_invite_json_existing_pending_setup_url_error() {
    // Covers the map_err closure in redeem_invite_json for existing pending (lines 292-294):
    // provisioner.create_setup_url fails → 400 Failure.
    let app = TestApp::new_with_failing_provisioner().await;
    let f = Fixture::setup(&app).await;

    let (inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    create_pending_access(&app.db, &inv_id, "ext-existing").await;

    let body = json!({"username": "existing", "email": "existing@x.com"});
    let (status, _) = app
        .call_json("POST", &format!("/invitations/redeem/{code}"), Some(body), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /invitations/redeem/{code} |
// Cases:
// | Case                                | Expected |
// | ----------------------------------- | -------- |
// | JSON redemption create_user failure | 400      |
#[tokio::test]
async fn test_redeem_invite_json_create_user_error() {
    // Covers the map_err closure in redeem_invite_json for new user (lines 307-309):
    // provisioner.create_user fails → 400 Failure.
    let app = TestApp::new_with_failing_provisioner().await;
    let f = Fixture::setup(&app).await;

    let (_inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;

    let body = json!({"username": "newuser", "email": "newuser@x.com"});
    let (status, _) = app
        .call_json("POST", &format!("/invitations/redeem/{code}"), Some(body), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// API calls:
// | Method | Route          |
// | ------ | -------------- |
// | GET    | /invite/{code} |
// Cases:
// | Case                          | Expected |
// | ----------------------------- | -------- |
// | invite form setup URL failure | 400      |
#[tokio::test]
async fn test_show_invite_form_setup_url_error() {
    // Covers the map_err closure in show_invite_form (lines 375-377):
    // provisioner.create_setup_url fails → 400 Failure.
    let app = TestApp::new_with_failing_provisioner().await;
    let f = Fixture::setup(&app).await;

    let (inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    create_pending_access(&app.db, &inv_id, "ext-pending").await;

    let status = app
        .call("GET", &format!("/invite/{code}"), None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// API calls:
// | Method | Route          |
// | ------ | -------------- |
// | POST   | /invite/{code} |
// Cases:
// | Case                                | Expected |
// | ----------------------------------- | -------- |
// | form redemption create_user failure | 400      |
#[tokio::test]
async fn test_redeem_invite_form_create_user_error() {
    // Covers the map_err closure in redeem_invite (lines 425-427):
    // provisioner.create_user fails → 400 Failure.
    let app = TestApp::new_with_failing_provisioner().await;
    let f = Fixture::setup(&app).await;

    let (_inv_id, code) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;

    let form_body = "username=failuser&email=fail%40x.com&first_name=&last_name=".to_string();
    let req = Request::builder()
        .method("POST")
        .uri(format!("/invite/{code}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form_body))
        .unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
