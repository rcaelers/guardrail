#![cfg(test)]

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::Json;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use serde_json::{Value, json};
use tower::ServiceExt;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};

use testware::mockall_object_store::MockObjectStoreWrapper;
use testware::setup::TestSetup;
use testware::{create_settings, create_test_product, create_test_token, create_test_user};

use crate::access::SESSION_KEY;
use crate::auth_cache::AuthCache;
use crate::auth_user::{AuthenticatedUser, User};
use crate::routes::{auth, db_api, home, impersonation, invite};
use crate::state::AppState;
use repos::Repo;
use uuid::Uuid;

type Db = surrealdb::Surreal<surrealdb::engine::any::Any>;

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/// Injects an AuthenticatedUser into a tower session.
/// Supports impersonation via optional real_user_id / real_user_name / real_user_is_admin fields.
async fn test_login_handler(session: Session, Json(body): Json<Value>) -> StatusCode {
    let user_id = body["user_id"].as_str().unwrap_or("").to_string();
    let name = body["name"].as_str().unwrap_or("Test").to_string();
    let is_admin = body["is_admin"].as_bool().unwrap_or(false);
    let real_user = body["real_user_id"].as_str().map(|rid| User {
        id: rid.to_string(),
        name: body["real_user_name"].as_str().unwrap_or("Real User").to_string(),
        is_admin: body["real_user_is_admin"].as_bool().unwrap_or(true),
        avatar: None,
    });
    let _ = session
        .insert(
            SESSION_KEY,
            AuthenticatedUser {
                user: Some(User { id: user_id, name, is_admin, avatar: None }),
                real_user,
            },
        )
        .await;
    StatusCode::OK
}

struct TestApp {
    db: Db,
    router: Router,
}

impl TestApp {
    async fn new() -> Self {
        TestSetup::init();
        let db = TestSetup::create_db().await;
        // Match the ns/db in JWTs to the in-memory test DB (ns=test, db=test).
        let mut settings = create_settings();
        settings.database.namespace = "test".to_string();
        settings.database.database = "test".to_string();
        let settings = Arc::new(settings);
        let storage: Arc<dyn object_store::ObjectStore> = Arc::new(MockObjectStoreWrapper::new());
        let state = AppState {
            repo: Arc::new(Repo::new(db.clone())),
            settings,
            http_client: reqwest::Client::new(),
            provisioner: None,
            storage,
            auth_cache: AuthCache::default(),
        };
        let session_store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(session_store)
            .with_name("guardrail")
            .with_same_site(SameSite::Lax)
            .with_expiry(Expiry::OnInactivity(time::Duration::hours(4)))
            .with_secure(false);
        let router = Router::new()
            .merge(home::router())
            .merge(auth::router())
            .merge(impersonation::router())
            .merge(db_api::router())
            .merge(invite::api_router())
            .merge(invite::router())
            .route("/test/login", post(test_login_handler))
            .layer(session_layer)
            .with_state(state);
        Self { db, router }
    }

    async fn send(&self, req: Request<Body>) -> (StatusCode, Bytes, Option<String>) {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let cookie = resp
            .headers()
            .get("set-cookie")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or("").trim().to_string());
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
        (status, body, cookie)
    }

    async fn make_session(&self, payload: Value) -> String {
        let req = Request::builder()
            .method("POST")
            .uri("/test/login")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();
        let (status, _, cookie) = self.send(req).await;
        assert_eq!(status, StatusCode::OK, "test login failed");
        cookie.expect("no session cookie set")
    }

    async fn call(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> StatusCode {
        let (status, _) = self.call_full(method, uri, body, cookie).await;
        status
    }

    async fn call_full(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> (StatusCode, Bytes) {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        if let Some(c) = cookie {
            b = b.header("cookie", c);
        }
        let body_bytes =
            body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
        let (status, bytes, _) = self.send(b.body(body_bytes).unwrap()).await;
        (status, bytes)
    }

    /// Call an endpoint and parse the response body as JSON.
    async fn call_json(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> (StatusCode, Value) {
        let (status, bytes) = self.call_full(method, uri, body, cookie).await;
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }
}

async fn grant_product_role(db: &Db, user_id: &str, product_id: &str, role: &str) {
    let uid = user_id.to_string();
    let pid = product_id.to_string();
    let role_s = role.to_string();
    db.query(
        "CREATE user_access CONTENT {
            user_id:    type::record('users',    $uid),
            product_id: type::record('products', $pid),
            role:       $role,
            created_at: time::now(),
            updated_at: time::now()
        }",
    )
    .bind(("uid", uid))
    .bind(("pid", pid))
    .bind(("role", role_s))
    .await
    .expect("grant_product_role failed");
}

// ---------------------------------------------------------------------------
// Extra helpers
// ---------------------------------------------------------------------------

/// Create an invitation via the API; return (id, code).
async fn api_create_invitation(app: &TestApp, cookie: &str, body: Value) -> (String, String) {
    let (status, v) = app.call_json("POST", "/invitations", Some(body), Some(cookie)).await;
    assert_eq!(status, StatusCode::OK, "api_create_invitation failed");
    let id = v["id"].as_str().expect("no id").to_string();
    let code = v["code"].as_str().expect("no code").to_string();
    (id, code)
}

/// Insert a minimal crash_group directly into the DB (bypasses ingestion).
/// Uses a no-hyphen alphanumeric ID so that SurrealDB's meta::id() returns it
/// without backtick escaping, which would break `WHERE meta::id(id) = $id`.
async fn create_test_crash_group(db: &Db, product_id: &str) -> String {
    let gid = Uuid::new_v4().to_string().replace('-', "");
    db.query(
        "CREATE type::record('crash_groups', $gid) CONTENT {
            product_id: type::record('products', $pid),
            fingerprint: 'test-fp',
            signal: 'SIGSEGV',
            count: 0,
            status: 'new',
            assignee: NONE,
            first_seen: time::now(),
            last_seen: time::now(),
            created_at: time::now(),
            updated_at: time::now()
        }",
    )
    .bind(("gid", gid.clone()))
    .bind(("pid", product_id.to_string()))
    .await
    .expect("create_test_crash_group failed");
    gid
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

/// One product entry: (id, slug, non_admin_has_maintainer_role)
struct ProductInfo {
    id: String,
    slug: String,
    non_admin_maintainer: bool,
}

/// Shared test fixture: users, products with defined roles, and 5 session contexts.
///
/// Auth contexts:
///   1. no_session    – no cookie at all
///   2. admin         – admin user (is_admin=true)
///   3. non_admin     – regular user with roles: ro/rw/maintainer/none across 4 products
///   4. imp_admin     – real_user=real_admin, effective=admin   → same rights as admin
///   5. imp_non_admin – real_user=real_admin, effective=non_admin → same rights as non_admin
struct Fixture {
    admin_id: String,
    non_admin_id: String,
    /// Products in order: [ro, rw, maint, none]
    products: [ProductInfo; 4],
    // session cookies
    admin: String,
    non_admin: String,
    imp_admin: String,
    imp_non_admin: String,
}

impl Fixture {
    async fn setup(app: &TestApp) -> Self {
        let admin_u = create_test_user(&app.db, "fx_admin", true).await;
        let non_admin_u = create_test_user(&app.db, "fx_nonadmin", false).await;
        let real_u = create_test_user(&app.db, "fx_real", true).await;

        let p_ro = create_test_product(&app.db).await;
        let p_rw = create_test_product(&app.db).await;
        let p_maint = create_test_product(&app.db).await;
        let p_none = create_test_product(&app.db).await;

        // non_admin roles
        grant_product_role(&app.db, &non_admin_u.id, &p_ro.id, "readonly").await;
        grant_product_role(&app.db, &non_admin_u.id, &p_rw.id, "readwrite").await;
        grant_product_role(&app.db, &non_admin_u.id, &p_maint.id, "maintainer").await;
        // admin also has roles (admin bypasses, but existence is realistic)
        grant_product_role(&app.db, &admin_u.id, &p_ro.id, "readonly").await;
        grant_product_role(&app.db, &admin_u.id, &p_rw.id, "readwrite").await;
        grant_product_role(&app.db, &admin_u.id, &p_maint.id, "maintainer").await;

        let admin = app
            .make_session(json!({"user_id": admin_u.id, "name": "Admin", "is_admin": true}))
            .await;
        let non_admin = app
            .make_session(json!({"user_id": non_admin_u.id, "name": "NonAdmin", "is_admin": false}))
            .await;
        let imp_admin = app
            .make_session(json!({
                "user_id": admin_u.id, "name": "Admin", "is_admin": true,
                "real_user_id": real_u.id, "real_user_name": "Real", "real_user_is_admin": true
            }))
            .await;
        let imp_non_admin = app
            .make_session(json!({
                "user_id": non_admin_u.id, "name": "NonAdmin", "is_admin": false,
                "real_user_id": real_u.id, "real_user_name": "Real", "real_user_is_admin": true
            }))
            .await;

        Fixture {
            admin_id: admin_u.id,
            non_admin_id: non_admin_u.id,
            products: [
                ProductInfo { id: p_ro.id, slug: p_ro.slug, non_admin_maintainer: false },
                ProductInfo { id: p_rw.id, slug: p_rw.slug, non_admin_maintainer: false },
                ProductInfo { id: p_maint.id, slug: p_maint.slug, non_admin_maintainer: true },
                ProductInfo { id: p_none.id, slug: p_none.slug, non_admin_maintainer: false },
            ],
            admin,
            non_admin,
            imp_admin,
            imp_non_admin,
        }
    }

    /// All 5 session contexts: (cookie, label)
    fn sessions(&self) -> [(Option<&str>, &str); 5] {
        [
            (None, "no_session"),
            (Some(&self.admin), "admin"),
            (Some(&self.non_admin), "non_admin"),
            (Some(&self.imp_admin), "imp_admin"),
            (Some(&self.imp_non_admin), "imp_non_admin"),
        ]
    }
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Asserts all 5 contexts against expected: [no_session, admin, non_admin, imp_admin, imp_non_admin]
async fn assert_all(
    app: &TestApp,
    f: &Fixture,
    method: &str,
    uri: &str,
    body: Option<Value>,
    expected: [StatusCode; 5],
) {
    let sessions = f.sessions();
    for (i, (cookie, label)) in sessions.iter().enumerate() {
        let got = app.call(method, uri, body.clone(), *cookie).await;
        assert_eq!(got, expected[i], "{label}: {method} {uri}");
    }
}

/// Admin-only: no_session=403, admin=ok, non_admin=403, imp_admin=ok, imp_non_admin=403
async fn assert_admin_only(
    app: &TestApp,
    f: &Fixture,
    method: &str,
    uri: &str,
    body: Option<Value>,
    ok: StatusCode,
) {
    assert_all(app, f, method, uri, body, [
        StatusCode::FORBIDDEN,
        ok,
        StatusCode::FORBIDDEN,
        ok,
        StatusCode::FORBIDDEN,
    ])
    .await;
}

/// Session-only: no_session=403, all others=not-403
async fn assert_session_only_not_forbidden(
    app: &TestApp,
    f: &Fixture,
    method: &str,
    uri: &str,
    body: Option<Value>,
) {
    let sessions = f.sessions();
    for (i, (cookie, label)) in sessions.iter().enumerate() {
        let got = app.call(method, uri, body.clone(), *cookie).await;
        if i == 0 {
            assert_eq!(got, StatusCode::FORBIDDEN, "{label}: {method} {uri}");
        } else {
            assert_ne!(got, StatusCode::FORBIDDEN, "{label}: {method} {uri} should not be 403");
        }
    }
}

/// Product-maintainer check across all 4 products × 5 contexts.
/// `uri_fn(product_id)` → URI; `body_fn(slug)` → request body.
async fn assert_product_maintainer(
    app: &TestApp,
    f: &Fixture,
    method: &str,
    uri_fn: impl Fn(&str) -> String,
    body_fn: impl Fn(&str) -> Option<Value>,
    ok: StatusCode,
) {
    for p in &f.products {
        let uri = uri_fn(&p.id);
        let body = body_fn(&p.slug);
        let exp_non_admin = if p.non_admin_maintainer { ok } else { StatusCode::FORBIDDEN };
        assert_all(app, f, method, &uri, body, [
            StatusCode::FORBIDDEN,
            ok,
            exp_non_admin,
            ok,
            exp_non_admin,
        ])
        .await;
    }
}

// ---------------------------------------------------------------------------
// Tests: user management (admin-only)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_users() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_admin_only(&app, &f, "GET", "/users", None, StatusCode::OK).await;
}

#[tokio::test]
async fn test_create_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Use unique emails per admin context to avoid duplicate-ID errors.
    assert_eq!(app.call("POST", "/users", Some(json!({"email": "x@x.com"})), None).await, StatusCode::FORBIDDEN, "no_session");
    assert_eq!(app.call("POST", "/users", Some(json!({"email": "x@x.com"})), Some(&f.non_admin)).await, StatusCode::FORBIDDEN, "non_admin");
    assert_eq!(app.call("POST", "/users", Some(json!({"email": "x@x.com"})), Some(&f.imp_non_admin)).await, StatusCode::FORBIDDEN, "imp_non_admin");
    assert_eq!(app.call("POST", "/users", Some(json!({"email": "admin.created@test.com"})), Some(&f.admin)).await, StatusCode::OK, "admin");
    assert_eq!(app.call("POST", "/users", Some(json!({"email": "imp.admin.created@test.com"})), Some(&f.imp_admin)).await, StatusCode::OK, "imp_admin");
}

#[tokio::test]
async fn test_get_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let uri = format!("/users/{}", f.non_admin_id);
    assert_admin_only(&app, &f, "GET", &uri, None, StatusCode::OK).await;
}

#[tokio::test]
async fn test_update_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target = create_test_user(&app.db, "update_target", false).await;
    let uri = format!("/users/{}", target.id);
    let body = json!({"email": "updated@example.com", "name": "Updated"});
    assert_admin_only(&app, &f, "POST", &uri, Some(body), StatusCode::OK).await;
}

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

#[tokio::test]
async fn test_get_me() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // no_session → 403; all sessions → 200 (users exist in DB)
    assert_all(&app, &f, "GET", "/me", None, [
        StatusCode::FORBIDDEN,
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::OK,
    ])
    .await;
}

#[tokio::test]
async fn test_find_user_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Searching for "fx_admin" (name set by create_test_user).
    // Exact result varies; auth layer must pass for sessions, block for no_session.
    assert_session_only_not_forbidden(&app, &f, "GET", "/users/find?q=fx_admin", None).await;
}

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
    assert_eq!(app.call("GET", &non_admin_uri, None, Some(&f.non_admin)).await, StatusCode::OK);
    // non_admin cannot read someone else's
    assert_eq!(
        app.call("GET", &admin_uri, None, Some(&f.non_admin)).await,
        StatusCode::FORBIDDEN
    );

    // imp_admin acts as admin
    assert_eq!(app.call("GET", &non_admin_uri, None, Some(&f.imp_admin)).await, StatusCode::OK);

    // imp_non_admin acts as non_admin: can read self, blocked from others
    assert_eq!(
        app.call("GET", &non_admin_uri, None, Some(&f.imp_non_admin)).await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &admin_uri, None, Some(&f.imp_non_admin)).await,
        StatusCode::FORBIDDEN
    );
}

// ---------------------------------------------------------------------------
// Tests: product management
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_product() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Use unique names per admin context to avoid unique-index violations.
    assert_eq!(app.call("POST", "/products", Some(json!({"name": "Blocked"})), None).await, StatusCode::FORBIDDEN, "no_session");
    assert_eq!(app.call("POST", "/products", Some(json!({"name": "Blocked"})), Some(&f.non_admin)).await, StatusCode::FORBIDDEN, "non_admin");
    assert_eq!(app.call("POST", "/products", Some(json!({"name": "Blocked"})), Some(&f.imp_non_admin)).await, StatusCode::FORBIDDEN, "imp_non_admin");
    assert_eq!(app.call("POST", "/products", Some(json!({"name": "Admin Created Product"})), Some(&f.admin)).await, StatusCode::OK, "admin");
    assert_eq!(app.call("POST", "/products", Some(json!({"name": "ImpAdmin Created Product"})), Some(&f.imp_admin)).await, StatusCode::OK, "imp_admin");
}

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
        app.call("DELETE", &format!("/products/{}", extra1.id), None, Some(&f.admin)).await,
        StatusCode::NO_CONTENT,
        "admin"
    );
    assert_eq!(
        app.call("DELETE", &format!("/products/{}", extra2.id), None, Some(&f.imp_admin)).await,
        StatusCode::NO_CONTENT,
        "imp_admin"
    );
}

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
        assert_eq!(app.call("POST", &uri, body.clone(), None).await, StatusCode::FORBIDDEN, "no_session {}", p.id);
        // admin: guard passes, RLS allows → 200
        assert_eq!(app.call("POST", &uri, body.clone(), Some(&f.admin)).await, StatusCode::OK, "admin {}", p.id);
        // imp_admin: same as admin
        assert_eq!(app.call("POST", &uri, body.clone(), Some(&f.imp_admin)).await, StatusCode::OK, "imp_admin {}", p.id);

        // non_admin: depends on their product role
        let (non_admin_expected, label) = if p.non_admin_maintainer {
            // guard passes (maintainer), but RLS blocks UPDATE → 0 rows → 404
            (StatusCode::NOT_FOUND, "non_admin maintainer (RLS blocks)")
        } else {
            // guard rejects (no maintainer role) → 403
            (StatusCode::FORBIDDEN, "non_admin non-maintainer")
        };
        assert_eq!(app.call("POST", &uri, body.clone(), Some(&f.non_admin)).await, non_admin_expected, "{label} {}", p.id);
        assert_eq!(app.call("POST", &uri, body, Some(&f.imp_non_admin)).await, non_admin_expected, "imp_{label} {}", p.id);
    }
}

// ---------------------------------------------------------------------------
// Tests: member management (product-maintainer)
// ---------------------------------------------------------------------------

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
// Tests: product API tokens (product-maintainer)
// ---------------------------------------------------------------------------

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
            "no_session product {}", p.id
        );
        assert_eq!(
            app.call("DELETE", &uri_tok1, None, Some(&f.non_admin)).await,
            if p.non_admin_maintainer { StatusCode::NO_CONTENT } else { StatusCode::FORBIDDEN },
            "non_admin product {}", p.id
        );
        assert_eq!(
            app.call("DELETE", &uri_tok1, None, Some(&f.imp_non_admin)).await,
            if p.non_admin_maintainer { StatusCode::NO_CONTENT } else { StatusCode::FORBIDDEN },
            "imp_non_admin product {}", p.id
        );

        // Admin contexts need their own tokens (non_admin/imp_non_admin may have consumed above)
        let (_, tok2) =
            create_test_token(&app.db, "del_tok2", Some(p.id.clone()), None, &["token"]).await;
        let uri_tok2 = format!("/products/{}/api-tokens/{}", p.id, tok2.id);
        assert_eq!(
            app.call("DELETE", &uri_tok2, None, Some(&f.admin)).await,
            StatusCode::NO_CONTENT,
            "admin product {}", p.id
        );

        let (_, tok3) =
            create_test_token(&app.db, "del_tok3", Some(p.id.clone()), None, &["token"]).await;
        let uri_tok3 = format!("/products/{}/api-tokens/{}", p.id, tok3.id);
        assert_eq!(
            app.call("DELETE", &uri_tok3, None, Some(&f.imp_admin)).await,
            StatusCode::NO_CONTENT,
            "imp_admin product {}", p.id
        );
    }
}

// ---------------------------------------------------------------------------
// Tests: admin API tokens
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_all_api_tokens() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_admin_only(&app, &f, "GET", "/api-tokens", None, StatusCode::OK).await;
}

#[tokio::test]
async fn test_list_entitlements() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_admin_only(&app, &f, "GET", "/api-tokens/entitlements", None, StatusCode::OK).await;
}

#[tokio::test]
async fn test_create_admin_api_token() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let body = json!({"description": "global token"});
    assert_admin_only(&app, &f, "POST", "/api-tokens", Some(body), StatusCode::OK).await;
}

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
        app.call("DELETE", &uri_a, None, Some(&f.imp_non_admin)).await,
        StatusCode::FORBIDDEN,
        "imp_non_admin"
    );

    let (_, tok_b) = create_test_token(&app.db, "del_admin_tok2", None, None, &["token"]).await;
    assert_eq!(
        app.call("DELETE", &format!("/api-tokens/{}", tok_b.id), None, Some(&f.admin)).await,
        StatusCode::NO_CONTENT,
        "admin"
    );

    let (_, tok_c) = create_test_token(&app.db, "del_admin_tok3", None, None, &["token"]).await;
    assert_eq!(
        app.call("DELETE", &format!("/api-tokens/{}", tok_c.id), None, Some(&f.imp_admin)).await,
        StatusCode::NO_CONTENT,
        "imp_admin"
    );
}

// ---------------------------------------------------------------------------
// Tests: symbol upload (product-maintainer)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_upload_symbol_all_contexts() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_product_maintainer(
        &app,
        &f,
        "POST",
        |pid| format!("/products/{pid}/symbols"),
        |_| Some(json!({"name": "crash.pdb", "arch": "x86_64"})),
        StatusCode::OK,
    )
    .await;
}

// ---------------------------------------------------------------------------
// Tests: crash / symbol session-only endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_set_crash_status_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // Crash doesn't exist; auth is checked before DB access → 403 without session
    // With session → not 403 (will be 404 or 204 depending on RLS/crash existence)
    assert_session_only_not_forbidden(
        &app,
        &f,
        "POST",
        "/crashes/nonexistent/status",
        Some(json!({"status": "resolved"})),
    )
    .await;
}

#[tokio::test]
async fn test_add_note_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_session_only_not_forbidden(
        &app,
        &f,
        "POST",
        "/crashes/nonexistent/notes",
        Some(json!({"body": "a note", "author": "tester"})),
    )
    .await;
}

#[tokio::test]
async fn test_merge_groups_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_session_only_not_forbidden(
        &app,
        &f,
        "POST",
        "/crashes/some-group/merge",
        Some(json!({"mergedId": "other-group"})),
    )
    .await;
}

#[tokio::test]
async fn test_delete_symbol_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_session_only_not_forbidden(&app, &f, "DELETE", "/symbols/nonexistent", None).await;
}

// ---------------------------------------------------------------------------
// Tests: invitation endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_invitations_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // GET /invitations uses require_session → empty list for all sessions is fine
    assert_all(&app, &f, "GET", "/invitations", None, [
        StatusCode::FORBIDDEN,
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::OK,
    ])
    .await;
}

#[tokio::test]
async fn test_create_invitation_admin() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    let body = json!({"is_admin": false, "grants": []});

    // no_session → 403
    assert_eq!(
        app.call("POST", "/invitations", Some(body.clone()), None).await,
        StatusCode::FORBIDDEN,
        "no_session"
    );
    // admin → 200
    assert_eq!(
        app.call("POST", "/invitations", Some(body.clone()), Some(&f.admin)).await,
        StatusCode::OK,
        "admin"
    );
    // imp_admin → 200
    assert_eq!(
        app.call("POST", "/invitations", Some(body.clone()), Some(&f.imp_admin)).await,
        StatusCode::OK,
        "imp_admin"
    );
}

#[tokio::test]
async fn test_create_invitation_non_admin_restrictions() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // non_admin creating admin invitation → 403
    let admin_body = json!({"is_admin": true, "grants": []});
    assert_eq!(
        app.call("POST", "/invitations", Some(admin_body.clone()), Some(&f.non_admin)).await,
        StatusCode::FORBIDDEN,
        "non_admin cannot create admin invitation"
    );
    assert_eq!(
        app.call("POST", "/invitations", Some(admin_body), Some(&f.imp_non_admin)).await,
        StatusCode::FORBIDDEN,
        "imp_non_admin cannot create admin invitation"
    );

    // non_admin granting access to product they don't maintain → 403
    let bad_grant = json!({
        "is_admin": false,
        "grants": [{"product_id": f.products[3].id, "role": "readonly"}]
    });
    assert_eq!(
        app.call("POST", "/invitations", Some(bad_grant.clone()), Some(&f.non_admin)).await,
        StatusCode::FORBIDDEN,
        "non_admin cannot grant access to unowned product"
    );

    // non_admin granting access to product they maintain → 200
    let good_grant = json!({
        "is_admin": false,
        "grants": [{"product_id": f.products[2].id, "role": "readonly"}]
    });
    assert_eq!(
        app.call("POST", "/invitations", Some(good_grant.clone()), Some(&f.non_admin)).await,
        StatusCode::OK,
        "non_admin can grant access to maintained product"
    );
    assert_eq!(
        app.call("POST", "/invitations", Some(good_grant), Some(&f.imp_non_admin)).await,
        StatusCode::OK,
        "imp_non_admin can grant access to maintained product"
    );
}

#[tokio::test]
async fn test_update_invitation_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let body = json!({"is_admin": false, "grants": []});
    // No existing invitation → 404 for sessions (auth passes), 403 for no_session
    assert_eq!(
        app.call("PUT", "/invitations/nonexistent", Some(body.clone()), None).await,
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
        let got = app.call("PUT", "/invitations/nonexistent", Some(body.clone()), *cookie).await;
        assert_ne!(got, StatusCode::FORBIDDEN, "{label} should not be 403");
    }
}

#[tokio::test]
async fn test_revoke_invitation_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    assert_eq!(
        app.call("DELETE", "/invitations/nonexistent", None, None).await,
        StatusCode::FORBIDDEN,
        "no_session"
    );
    // admin: no existence check → 200 (revoke is idempotent for admins)
    assert_eq!(
        app.call("DELETE", "/invitations/nonexistent", None, Some(&f.admin)).await,
        StatusCode::OK,
        "admin"
    );
    assert_eq!(
        app.call("DELETE", "/invitations/nonexistent", None, Some(&f.imp_admin)).await,
        StatusCode::OK,
        "imp_admin"
    );
    // non_admin: checks existence first → 404
    let non_admin_got =
        app.call("DELETE", "/invitations/nonexistent", None, Some(&f.non_admin)).await;
    assert_ne!(non_admin_got, StatusCode::FORBIDDEN, "non_admin should not be 403");
    let imp_non_admin_got =
        app.call("DELETE", "/invitations/nonexistent", None, Some(&f.imp_non_admin)).await;
    assert_ne!(imp_non_admin_got, StatusCode::FORBIDDEN, "imp_non_admin should not be 403");
}

// ---------------------------------------------------------------------------
// Tests: home page
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_home_page() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // / is public; renders HTML for all contexts
    assert_eq!(app.call("GET", "/", None, None).await, StatusCode::OK);
    assert_eq!(app.call("GET", "/", None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", "/", None, Some(&f.non_admin)).await, StatusCode::OK);
    // with query params
    assert_eq!(app.call("GET", "/?next=/dashboard&error=login+failed", None, None).await, StatusCode::OK);
    assert_eq!(app.call("GET", "/?error=oops", None, None).await, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Tests: auth routes
// ---------------------------------------------------------------------------

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
    let req = Request::builder().method("POST").uri("/auth/logout").body(Body::empty()).unwrap();
    let (status, _, _) = app.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn test_get_real_user() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    // no session → 403
    assert_eq!(app.call("GET", "/auth/real-user", None, None).await, StatusCode::FORBIDDEN);
    // session but not impersonating → 404
    assert_eq!(app.call("GET", "/auth/real-user", None, Some(&f.admin)).await, StatusCode::NOT_FOUND);
    assert_eq!(app.call("GET", "/auth/real-user", None, Some(&f.non_admin)).await, StatusCode::NOT_FOUND);
    // impersonating → 200 (real user exists in DB)
    assert_eq!(app.call("GET", "/auth/real-user", None, Some(&f.imp_admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", "/auth/real-user", None, Some(&f.imp_non_admin)).await, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Tests: impersonation routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_start_impersonation() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let target_uri = format!("/auth/impersonate/{}", f.non_admin_id);

    // no session → 403
    assert_eq!(app.call("POST", &target_uri, None, None).await, StatusCode::FORBIDDEN);
    // non-admin → 403
    assert_eq!(app.call("POST", &target_uri, None, Some(&f.non_admin)).await, StatusCode::FORBIDDEN);
    // already impersonating → 400 (AppError::failure)
    assert_eq!(app.call("POST", &target_uri, None, Some(&f.imp_admin)).await, StatusCode::BAD_REQUEST);
    // impersonate self → 400
    let self_uri = format!("/auth/impersonate/{}", f.admin_id);
    assert_eq!(app.call("POST", &self_uri, None, Some(&f.admin)).await, StatusCode::BAD_REQUEST);
    // target not found → 404
    assert_eq!(
        app.call("POST", "/auth/impersonate/nonexistent-user-id", None, Some(&f.admin)).await,
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

#[tokio::test]
async fn test_stop_impersonation() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // no session → 403
    assert_eq!(app.call("POST", "/auth/impersonate/stop", None, None).await, StatusCode::FORBIDDEN);
    // not impersonating → 400 (AppError::failure)
    assert_eq!(app.call("POST", "/auth/impersonate/stop", None, Some(&f.admin)).await, StatusCode::BAD_REQUEST);
    assert_eq!(app.call("POST", "/auth/impersonate/stop", None, Some(&f.non_admin)).await, StatusCode::BAD_REQUEST);
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

// ---------------------------------------------------------------------------
// Tests: product read endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_product() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let uri = format!("/products/{}", f.products[0].id);
    // GET /products/{id} is unguarded — all contexts return 200
    for (cookie, label) in &f.sessions() {
        assert_eq!(app.call("GET", &uri, None, *cookie).await, StatusCode::OK, "{label}");
    }
    assert_eq!(app.call("GET", &uri, None, None).await, StatusCode::OK);
}

#[tokio::test]
async fn test_list_products() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // Default scope (no query): all contexts get a 200
    assert_eq!(app.call("GET", "/products", None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", "/products", None, None).await, StatusCode::OK);

    // scope=mine with explicit user
    let mine = format!("/products?scope=mine&user={}", f.non_admin_id);
    assert_eq!(app.call("GET", &mine, None, Some(&f.non_admin)).await, StatusCode::OK);

    // scope=mine without user → empty 200
    assert_eq!(app.call("GET", "/products?scope=mine", None, Some(&f.admin)).await, StatusCode::OK);

    // scope=public → 200 (no session needed)
    assert_eq!(app.call("GET", "/products?scope=public", None, None).await, StatusCode::OK);
}

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
// Tests: symbol read endpoint with filters
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_symbols() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[2].id; // maintainer product

    // Seed one symbol
    app.call(
        "POST",
        &format!("/products/{pid}/symbols"),
        Some(json!({"name": "app.pdb", "arch": "x86_64"})),
        Some(&f.admin),
    )
    .await;

    let base = format!("/products/{pid}/symbols");
    // plain list
    assert_eq!(app.call("GET", &base, None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &base, None, None).await, StatusCode::OK);
    // search filter
    assert_eq!(app.call("GET", &format!("{base}?search=app"), None, Some(&f.admin)).await, StatusCode::OK);
    // arch filter
    assert_eq!(app.call("GET", &format!("{base}?arch=x86_64"), None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("{base}?arch=all"), None, Some(&f.admin)).await, StatusCode::OK);
    // sort variants
    assert_eq!(app.call("GET", &format!("{base}?sort=name"), None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("{base}?sort=size"), None, Some(&f.admin)).await, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Tests: crash group endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_groups() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Empty product: basic list
    let base = format!("/crashes?productId={pid}");
    assert_eq!(app.call("GET", &base, None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &base, None, None).await, StatusCode::OK);

    // Seed a crash group to exercise the merge/filter/sort paths
    create_test_crash_group(&app.db, pid).await;
    assert_eq!(app.call("GET", &base, None, Some(&f.admin)).await, StatusCode::OK);

    // filters
    assert_eq!(app.call("GET", &format!("{base}&status=unresolved"), None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("{base}&status=all"), None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("{base}&version=1.0"), None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("{base}&search=test"), None, Some(&f.admin)).await, StatusCode::OK);
    // sort variants
    assert_eq!(app.call("GET", &format!("{base}&sort=recent"), None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("{base}&sort=similarity"), None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("{base}&sort=version"), None, Some(&f.admin)).await, StatusCode::OK);
    // pagination
    assert_eq!(app.call("GET", &format!("{base}&limit=5&offset=0"), None, Some(&f.admin)).await, StatusCode::OK);
}

#[tokio::test]
async fn test_get_group() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Nonexistent group → 404
    assert_eq!(app.call("GET", "/crashes/nonexistent-group", None, Some(&f.admin)).await, StatusCode::NOT_FOUND);

    // Real group → 200 for admin and non_admin (products[0] grants readonly to non_admin)
    // No session → 404 because products[0] is non-public
    let gid = create_test_crash_group(&app.db, pid).await;
    let uri = format!("/crashes/{gid}");
    assert_eq!(app.call("GET", &uri, None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &uri, None, Some(&f.non_admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &uri, None, None).await, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tests: invitation redemption flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_invite_info() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // invalid code → 404
    assert_eq!(app.call("GET", "/invitations/redeem/no-such-code", None, None).await, StatusCode::NOT_FOUND);

    // valid code → { valid: true }
    let (_, code) = api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(app.call("GET", &format!("/invitations/redeem/{code}"), None, None).await, StatusCode::OK);
}

#[tokio::test]
async fn test_redeem_invite_json_no_provisioner() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // invalid code → 404
    let body = json!({"username": "u", "email": "u@e.com"});
    assert_eq!(
        app.call("POST", "/invitations/redeem/no-such-code", Some(body.clone()), None).await,
        StatusCode::NOT_FOUND,
    );

    // valid code, no provisioner configured → 400
    let (_, code) = api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(
        app.call("POST", &format!("/invitations/redeem/{code}"), Some(body.clone()), None).await,
        StatusCode::BAD_REQUEST,
    );

    // max_uses exceeded → 404 ("Invitation has been fully used")
    let (id_lim, code_lim) =
        api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": [], "max_uses": 1})).await;
    app.db
        .query("UPDATE type::record('invitations', $id) SET use_count = 1")
        .bind(("id", id_lim))
        .await
        .unwrap();
    assert_eq!(
        app.call("POST", &format!("/invitations/redeem/{code_lim}"), Some(body), None).await,
        StatusCode::NOT_FOUND,
    );
}

#[tokio::test]
async fn test_show_invite_form() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // invalid code → 404
    assert_eq!(app.call("GET", "/invite/no-such-code", None, None).await, StatusCode::NOT_FOUND);

    // valid code, no provisioner → renders form (200)
    let (_, code) = api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(app.call("GET", &format!("/invite/{code}"), None, None).await, StatusCode::OK);
    assert_eq!(app.call("GET", &format!("/invite/{code}?error=oops"), None, None).await, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Tests: invitation update / revoke success paths
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_update_invitation_success() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // Admin can update any invitation
    let (id, _) = api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    let uri = format!("/invitations/{id}");
    let update = json!({"is_admin": false, "grants": []});
    assert_eq!(app.call("PUT", &uri, Some(update.clone()), Some(&f.admin)).await, StatusCode::OK);

    // Non-admin cannot update an admin-created invitation (no overlap) → 403
    assert_eq!(app.call("PUT", &uri, Some(update), Some(&f.non_admin)).await, StatusCode::FORBIDDEN);

    // Non-admin can update an invitation they created (created_by overlap)
    let (id2, _) = api_create_invitation(
        &app,
        &f.non_admin,
        json!({"is_admin": false, "grants": [{"product_id": f.products[2].id, "role": "readonly"}]}),
    )
    .await;
    let uri2 = format!("/invitations/{id2}");
    let update2 =
        json!({"is_admin": false, "grants": [{"product_id": f.products[2].id, "role": "readonly"}]});
    assert_eq!(app.call("PUT", &uri2, Some(update2), Some(&f.non_admin)).await, StatusCode::OK);

    // Non-admin cannot add grants for products they don't maintain → 403
    let bad_update = json!({"is_admin": false, "grants": [{"product_id": f.products[3].id, "role": "readonly"}]});
    assert_eq!(app.call("PUT", &uri2, Some(bad_update), Some(&f.non_admin)).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_revoke_invitation_existing() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    // Admin can revoke any invitation
    let (id, _) = api_create_invitation(&app, &f.admin, json!({"is_admin": false, "grants": []})).await;
    assert_eq!(
        app.call("DELETE", &format!("/invitations/{id}"), None, Some(&f.admin)).await,
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
        app.call("DELETE", &format!("/invitations/{id2}"), None, Some(&f.non_admin)).await,
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
        app.call("DELETE", &format!("/invitations/{id3}"), None, Some(&f.non_admin)).await,
        StatusCode::FORBIDDEN,
        "non_admin cannot revoke unowned product invitation",
    );
}
