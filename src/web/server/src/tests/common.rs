use std::sync::Arc;

use axum::Router;
pub(super) use axum::body::Body;
use axum::body::Bytes;
use axum::extract::Json;
pub(super) use axum::http::{Request, StatusCode};
use axum::routing::post;
use serde_json::Value;
pub(super) use serde_json::json;
use tower::ServiceExt;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};

use object_store::memory::InMemory;
use testware::setup::TestSetup;
pub(super) use testware::{
    create_test_attachment, create_test_product, create_test_token, create_test_user,
};

use crate::access::SESSION_KEY;
use crate::auth_cache::AuthCache;
use crate::auth_user::{AuthenticatedUser, User};
use crate::routes::{auth, db_api, home, impersonation, invite};
use crate::state::AppState;
use repos::Repo;
use uuid::Uuid;

pub(super) type Db = surrealdb::Surreal<surrealdb::engine::any::Any>;

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/// Injects an AuthenticatedUser into a tower session.
/// Supports impersonation via optional real_user_id / real_user_name / real_user_is_admin fields.
/// If `user_id` is JSON null, sets `user: None` (unauthenticated active slot).
async fn test_login_handler(session: Session, Json(body): Json<Value>) -> StatusCode {
    let user = if body["user_id"].is_null() {
        None
    } else {
        let user_id = body["user_id"].as_str().unwrap_or("").to_string();
        let name = body["name"].as_str().unwrap_or("Test").to_string();
        let is_admin = body["is_admin"].as_bool().unwrap_or(false);
        Some(User {
            id: user_id,
            name,
            is_admin,
            avatar: None,
        })
    };
    let real_user = body["real_user_id"].as_str().map(|rid| User {
        id: rid.to_string(),
        name: body["real_user_name"]
            .as_str()
            .unwrap_or("Real User")
            .to_string(),
        is_admin: body["real_user_is_admin"].as_bool().unwrap_or(true),
        avatar: None,
    });
    let _ = session
        .insert(SESSION_KEY, AuthenticatedUser { user, real_user })
        .await;
    StatusCode::OK
}

pub(super) struct TestApp {
    pub(super) db: Db,
    router: Router,
    pub(super) storage: Arc<InMemory>,
}

// ---------------------------------------------------------------------------
// Mock provisioner (for invite tests that require a configured provisioner)
// ---------------------------------------------------------------------------

struct MockProvisioner;

#[async_trait::async_trait]
impl crate::provisioner::IdentityProvisioner for MockProvisioner {
    async fn create_user(
        &self,
        req: crate::provisioner::CreateUserRequest,
    ) -> Result<crate::provisioner::ProvisionedUser, crate::provisioner::ProvisionerError> {
        Ok(crate::provisioner::ProvisionedUser {
            external_id: format!("ext-{}", req.username),
            setup_url: url::Url::parse("https://example.com/setup").ok(),
        })
    }

    async fn create_setup_url(
        &self,
        _external_id: &str,
    ) -> Result<Option<url::Url>, crate::provisioner::ProvisionerError> {
        Ok(Some(
            url::Url::parse("https://example.com/setup")
                .map_err(|e| crate::provisioner::ProvisionerError::ApiError(e.to_string()))?,
        ))
    }

    async fn find_user_id(
        &self,
        _email: &str,
        username: &str,
    ) -> Result<Option<String>, crate::provisioner::ProvisionerError> {
        Ok(Some(format!("ext-{username}")))
    }

    async fn create_recovery_url(
        &self,
        _external_id: &str,
    ) -> Result<url::Url, crate::provisioner::ProvisionerError> {
        url::Url::parse("https://example.com/recovery")
            .map_err(|e| crate::provisioner::ProvisionerError::ApiError(e.to_string()))
    }
}

struct FailingMockProvisioner;

#[async_trait::async_trait]
impl crate::provisioner::IdentityProvisioner for FailingMockProvisioner {
    async fn create_user(
        &self,
        _req: crate::provisioner::CreateUserRequest,
    ) -> Result<crate::provisioner::ProvisionedUser, crate::provisioner::ProvisionerError> {
        Err(crate::provisioner::ProvisionerError::ApiError(
            "simulated create_user failure".to_string(),
        ))
    }

    async fn create_setup_url(
        &self,
        _external_id: &str,
    ) -> Result<Option<url::Url>, crate::provisioner::ProvisionerError> {
        Err(crate::provisioner::ProvisionerError::ApiError(
            "simulated setup_url failure".to_string(),
        ))
    }

    async fn find_user_id(
        &self,
        _email: &str,
        _username: &str,
    ) -> Result<Option<String>, crate::provisioner::ProvisionerError> {
        Err(crate::provisioner::ProvisionerError::ApiError(
            "simulated find_user_id failure".to_string(),
        ))
    }

    async fn create_recovery_url(
        &self,
        _external_id: &str,
    ) -> Result<url::Url, crate::provisioner::ProvisionerError> {
        Err(crate::provisioner::ProvisionerError::ApiError(
            "simulated create_recovery_url failure".to_string(),
        ))
    }
}

impl TestApp {
    async fn with_options(
        provisioner: Option<Arc<dyn crate::provisioner::IdentityProvisioner>>,
        mutate_settings: impl FnOnce(&mut crate::settings::Settings),
    ) -> Self {
        TestSetup::init();
        let db = TestSetup::create_db().await;
        // Match the ns/db in JWTs to the in-memory test DB (ns=test, db=test).
        let mut settings = crate::settings::Settings::test_default();
        settings.database.namespace = "test".to_string();
        settings.database.database = "test".to_string();
        mutate_settings(&mut settings);
        let settings = Arc::new(settings);
        let storage_inner = Arc::new(InMemory::new());
        let storage: Arc<dyn object_store::ObjectStore> = storage_inner.clone();
        let state = AppState {
            repo: Arc::new(Repo::new(db.clone())),
            settings,
            http_client: reqwest::Client::new(),
            provisioner,
            email_sender: None,
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
        Self {
            db,
            router,
            storage: storage_inner,
        }
    }

    pub(super) async fn new() -> Self {
        Self::with_options(None, |_| {}).await
    }

    pub(super) async fn new_with_provisioner() -> Self {
        Self::with_options(Some(Arc::new(MockProvisioner)), |_| {}).await
    }

    pub(super) async fn new_with_failing_provisioner() -> Self {
        Self::with_options(Some(Arc::new(FailingMockProvisioner)), |_| {}).await
    }

    pub(super) async fn new_with_invalid_jwt_key() -> Self {
        Self::with_options(None, |settings| {
            settings.database.jwk.private_key = "not a pem key".to_string();
        })
        .await
    }

    /// A valid Ed25519 key that is NOT the one trusted by the test SurrealDB instance.
    /// JWT generation succeeds, but SurrealDB rejects the JWT on authenticate().
    /// Simulates the production failure mode: "There was a problem with authentication".
    pub(super) async fn new_with_mismatched_jwt_key() -> Self {
        Self::with_options(None, |settings| {
            // Throwaway key generated for this test — never used anywhere else.
            settings.database.jwk.private_key = "-----BEGIN PRIVATE KEY-----\
                MC4CAQAwBQYDK2VwBCIEID5zKZ0YKMIEwKSsTpAKwrhLSd9U9+8NdB4JFgx89hSQ\
                -----END PRIVATE KEY-----"
                .to_string();
        })
        .await
    }

    pub(super) async fn send(&self, req: Request<Body>) -> (StatusCode, Bytes, Option<String>) {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let cookie = resp
            .headers()
            .get("set-cookie")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or("").trim().to_string());
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap();
        (status, body, cookie)
    }

    pub(super) async fn make_session(&self, payload: Value) -> String {
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

    pub(super) async fn call(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> StatusCode {
        let (status, _) = self.call_full(method, uri, body, cookie).await;
        status
    }

    pub(super) async fn call_full(
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
        let body_bytes = body
            .map(|v| Body::from(v.to_string()))
            .unwrap_or_else(Body::empty);
        let (status, bytes, _) = self.send(b.body(body_bytes).unwrap()).await;
        (status, bytes)
    }

    pub(super) async fn call_bearer(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        token: &str,
    ) -> StatusCode {
        let mut b = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"));
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        let body_bytes = body
            .map(|v| Body::from(v.to_string()))
            .unwrap_or_else(Body::empty);
        let (status, _, _) = self.send(b.body(body_bytes).unwrap()).await;
        status
    }

    /// Call an endpoint and parse the response body as JSON.
    pub(super) async fn call_json(
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

pub(super) async fn grant_product_role(db: &Db, user_id: &str, product_id: &str, role: &str) {
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

/// Insert a pending_access row directly (bypasses RLS).
pub(super) async fn create_pending_access(db: &Db, invitation_id: &str, sub: &str) {
    db.query(
        "CREATE type::record('pending_access', $id) CONTENT {
            sub: $sub,
            invitation_id: $inv_id,
            is_admin: false,
            grants: []
        }",
    )
    .bind(("id", Uuid::new_v4().to_string().replace('-', "")))
    .bind(("sub", sub.to_string()))
    .bind(("inv_id", invitation_id.to_string()))
    .await
    .expect("create_pending_access failed");
}

/// Create an invitation via the API; return (id, code).
pub(super) async fn api_create_invitation(
    app: &TestApp,
    cookie: &str,
    body: Value,
) -> (String, String) {
    let (status, v) = app
        .call_json("POST", "/invitations", Some(body), Some(cookie))
        .await;
    assert_eq!(status, StatusCode::OK, "api_create_invitation failed");
    let id = v["id"].as_str().expect("no id").to_string();
    let code = v["code"].as_str().expect("no code").to_string();
    (id, code)
}

/// Insert a minimal crash_group directly into the DB (bypasses ingestion).
/// Uses a no-hyphen alphanumeric ID so that SurrealDB's meta::id() returns it
/// without backtick escaping, which would break `WHERE meta::id(id) = $id`.
pub(super) async fn create_test_crash_group(db: &Db, product_id: &str) -> String {
    let gid = Uuid::new_v4().to_string().replace('-', "");
    // Use the gid as fingerprint to satisfy the UNIQUE(product_id, fingerprint) index.
    db.query(
        "CREATE type::record('crash_groups', $gid) CONTENT {
            product_id: type::record('products', $pid),
            fingerprint: $gid,
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
pub(super) struct ProductInfo {
    pub(super) id: String,
    pub(super) slug: String,
    pub(super) non_admin_maintainer: bool,
}

/// Shared test fixture: users, products with defined roles, and 5 session contexts.
///
/// Auth contexts:
///   1. no_session    – no cookie at all
///   2. admin         – admin user (is_admin=true)
///   3. non_admin     – regular user with roles: ro/rw/maintainer/none across 4 products
///   4. imp_admin     – real_user=real_admin, effective=admin   → same rights as admin
///   5. imp_non_admin – real_user=real_admin, effective=non_admin → same rights as non_admin
pub(super) struct Fixture {
    pub(super) admin_id: String,
    pub(super) non_admin_id: String,
    /// Products in order: [ro, rw, maint, none]
    pub(super) products: [ProductInfo; 4],
    // session cookies
    pub(super) admin: String,
    pub(super) non_admin: String,
    pub(super) imp_admin: String,
    pub(super) imp_non_admin: String,
}

impl Fixture {
    pub(super) async fn setup(app: &TestApp) -> Self {
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
                ProductInfo {
                    id: p_ro.id,
                    slug: p_ro.slug,
                    non_admin_maintainer: false,
                },
                ProductInfo {
                    id: p_rw.id,
                    slug: p_rw.slug,
                    non_admin_maintainer: false,
                },
                ProductInfo {
                    id: p_maint.id,
                    slug: p_maint.slug,
                    non_admin_maintainer: true,
                },
                ProductInfo {
                    id: p_none.id,
                    slug: p_none.slug,
                    non_admin_maintainer: false,
                },
            ],
            admin,
            non_admin,
            imp_admin,
            imp_non_admin,
        }
    }

    /// All 5 session contexts: (cookie, label)
    pub(super) fn sessions(&self) -> [(Option<&str>, &str); 5] {
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
pub(super) async fn assert_all(
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
pub(super) async fn assert_admin_only(
    app: &TestApp,
    f: &Fixture,
    method: &str,
    uri: &str,
    body: Option<Value>,
    ok: StatusCode,
) {
    assert_all(
        app,
        f,
        method,
        uri,
        body,
        [
            StatusCode::FORBIDDEN,
            ok,
            StatusCode::FORBIDDEN,
            ok,
            StatusCode::FORBIDDEN,
        ],
    )
    .await;
}

/// Session-only: no_session=403, all others=not-403
pub(super) async fn assert_session_only_not_forbidden(
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
pub(super) async fn assert_product_maintainer(
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
        let exp_non_admin = if p.non_admin_maintainer {
            ok
        } else {
            StatusCode::FORBIDDEN
        };
        assert_all(
            app,
            f,
            method,
            &uri,
            body,
            [StatusCode::FORBIDDEN, ok, exp_non_admin, ok, exp_non_admin],
        )
        .await;
    }
}

/// Create a crash in the DB linked to an existing crash group.
pub(super) async fn create_test_crash_in_group(
    db: &Db,
    product_id: &str,
    group_id: &str,
) -> String {
    let cid = Uuid::new_v4().to_string().replace('-', "");
    db.query(
        "CREATE type::record('crashes', $cid) CONTENT {
            product_id: type::record('products', $pid),
            group_id:   type::record('crash_groups', $gid),
            fingerprint: 'test-fp',
            report: {
                title:    'Test crash',
                topFrame: 'main()',
                version:  '1.2.3',
                platform: 'linux'
            },
            created_at: time::now(),
            updated_at: time::now()
        }",
    )
    .bind(("cid", cid.clone()))
    .bind(("pid", product_id.to_string()))
    .bind(("gid", group_id.to_string()))
    .await
    .expect("create_test_crash_in_group failed");
    cid
}
