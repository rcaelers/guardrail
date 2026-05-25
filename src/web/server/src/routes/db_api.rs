// REST API backed by SurrealDB. Returns JSON shapes consumed by
// the SvelteKit http adapter.
//
// Handlers generate a short-lived JWT from the authenticated tower session so
// SurrealDB row-level security (RLS) rules are enforced on every query.  When
// no session is present, an anonymous JWT is used, which grants access only to
// public data.

use std::sync::Arc;

use crate::auth_user::AuthenticatedUser;
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use object_store::{ObjectStoreExt, path::Path as ObjectPath};
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tower_sessions::Session;

use crate::AppState;

const ANON_CACHE_KEY: &str = "__anon__";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api-tokens", get(list_all_api_tokens).post(create_admin_api_token))
        .route("/api-tokens/{id}", patch(update_admin_api_token).delete(delete_admin_api_token))
        .route("/api-tokens/entitlements", get(list_entitlements_handler))
        .route("/attachments/{id}/download", get(download_attachment))
        .route("/crashes", get(list_groups))
        .route("/crashes/{id}", get(get_group).delete(delete_group))
        .route("/crashes/{id}/merge", post(merge_groups))
        .route("/crashes/{id}/notes", post(add_note))
        .route("/crashes/{id}/status", post(set_status))
        .route("/crashes/by-crash/{crash_id}", get(get_crash).delete(delete_crash))
        .route("/products", get(list_products).post(create_product))
        .route("/products/{id}", get(get_product).post(update_product).delete(delete_product))
        .route(
            "/products/{id}/email-settings",
            get(get_product_email_settings).post(update_product_email_settings),
        )
        .route(
            "/products/{id}/processor-settings",
            get(get_product_processor_settings).post(update_product_processor_settings),
        )
        .route(
            "/products/{id}/minidump-settings",
            get(get_product_minidump_settings).post(update_product_minidump_settings),
        )
        .route(
            "/products/{id}/validation-scripts",
            get(list_validation_scripts).post(upload_validation_script),
        )
        .route(
            "/products/{id}/validation-scripts/{sid}",
            get(get_validation_script).delete(delete_validation_script),
        )
        .route("/products/{id}/product-token", post(update_product_token))
        .route("/products/{pid}/api-tokens", get(list_api_tokens).post(create_api_token))
        .route("/products/{pid}/api-tokens/{id}", delete(delete_api_token))
        .route("/products/{pid}/members", get(list_members))
        .route("/products/{pid}/members/{uid}", post(grant_access).delete(revoke_access))
        .route("/products/{pid}/symbols", get(list_symbols).post(upload_symbol))
        .route("/symbols/{id}", delete(delete_symbol))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user).post(update_user).delete(delete_user))
        .route("/users/{id}/admin", post(set_admin))
        .route("/users/{id}/memberships", get(memberships_for))
        .route("/users/find", get(find_user))
        .route("/me", get(get_me))
        .route("/settings/email", get(get_app_email_settings).post(update_app_email_settings))
}

// --------------------------------------------------------------------
// per-request authenticated DB
// --------------------------------------------------------------------

impl AppState {
    /// Returns a SurrealDB handle authenticated as the current tower session
    /// user. Falls back to an anonymous JWT (public data only) when no session
    /// user exists or the user cannot be found. Returns 503 if the anonymous
    /// JWT itself cannot be issued or authenticated — never falls back to root.
    pub async fn user_db(
        &self,
        session: &Session,
    ) -> Result<Arc<Surreal<Any>>, (StatusCode, String)> {
        let session_user = session
            .get::<AuthenticatedUser>(crate::access::SESSION_KEY)
            .await
            .ok()
            .flatten();
        let Some(session_user) = session_user else {
            return self.anon_db().await;
        };
        let Some(active) = session_user.user.as_ref() else {
            return self.anon_db().await;
        };
        let uid = repos::record_key(&active.id);
        if let Some(cached) = self.auth_cache.get(&uid).await {
            tracing::trace!(user_id = %uid, "db_auth: using cached connection");
            return Ok(cached);
        }

        let user_row = self
            .repo
            .db
            .query(
                "SELECT username, is_admin, meta::id(id) AS uid \
                 FROM ONLY type::record('users', $id)",
            )
            .bind(("id", uid.clone()))
            .await
            .ok()
            .and_then(|mut r| r.take::<Option<Value>>(0).ok().flatten());

        let Some(row) = user_row else {
            tracing::debug!(user_id = %uid, "db_auth: user not found in DB, falling back to anon");
            return self.anon_db().await;
        };

        let username = row
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("anonymous");
        let is_admin = row
            .get("is_admin")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let user_id = row.get("uid").and_then(|v| v.as_str()).map(String::from);

        let jwt = match crate::jwt::make_jwt(username, user_id.as_deref(), is_admin, &self.settings)
        {
            Ok(jwt) => jwt,
            Err(_) => {
                tracing::error!(user_id = %uid, "db_auth: JWT generation failed, falling back to anon");
                return self.anon_db().await;
            }
        };

        tracing::debug!(user_id = %uid, username, is_admin, "db_auth: issuing new JWT for SurrealDB");

        match self.repo.authenticated(&jwt).await {
            Ok(db) => {
                let handle = Arc::new(db);
                self.auth_cache
                    .insert(uid.to_string(), Arc::clone(&handle))
                    .await;
                Ok(handle)
            }
            Err(e) => {
                tracing::error!(user_id = %uid, "db_auth: SurrealDB authentication failed: {e}, falling back to anon");
                self.anon_db().await
            }
        }
    }

    /// Returns an anonymous (public-only) SurrealDB handle. Returns 503 if the
    /// anonymous JWT cannot be created or authenticated — never falls back to root.
    async fn anon_db(&self) -> Result<Arc<Surreal<Any>>, (StatusCode, String)> {
        if let Some(cached) = self.auth_cache.get(ANON_CACHE_KEY).await {
            tracing::trace!("db_auth: using cached anonymous connection");
            return Ok(cached);
        }

        tracing::debug!("db_auth: issuing new anonymous JWT for SurrealDB");

        let jwt = crate::jwt::make_anon_jwt(&self.settings).map_err(|e| {
            tracing::error!("db_auth: anonymous JWT generation failed: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "database authentication unavailable".to_string(),
            )
        })?;

        match self.repo.authenticated(&jwt).await {
            Ok(db) => {
                let handle = Arc::new(db);
                self.auth_cache
                    .insert(ANON_CACHE_KEY.to_string(), Arc::clone(&handle))
                    .await;
                Ok(handle)
            }
            Err(e) => {
                tracing::error!("db_auth: anonymous SurrealDB authentication failed: {e}");
                Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    "database authentication unavailable".to_string(),
                ))
            }
        }
    }
}

// --------------------------------------------------------------------
// helpers
// --------------------------------------------------------------------

fn bad(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.into())
}
fn not_found(what: &str) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("not found: {what}"))
}
fn server_error(e: impl std::fmt::Display) -> (StatusCode, String) {
    tracing::error!("db_api: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, format!("db error: {e}"))
}

/// Converts an `access` module error into the `(StatusCode, String)` tuple
/// used by db_api handlers.
fn access_err(e: crate::error::AppError) -> (StatusCode, String) {
    use crate::error::AppError;
    match e {
        AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
        AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string()),
    }
}

fn storage_error(err: object_store::Error) -> (StatusCode, String) {
    match err {
        object_store::Error::NotFound { .. } => not_found("attachment object"),
        other => {
            tracing::error!("db_api storage: {other}");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("storage error: {other}"))
        }
    }
}

async fn product_id_for_crash_group(
    db: &Surreal<Any>,
    group_id: &str,
) -> Result<String, (StatusCode, String)> {
    let rows = run_value(
        db,
        "SELECT meta::id(product_id) AS productId
         FROM ONLY type::record('crash_groups', $id)",
        vec![("id", Value::String(group_id.to_string()))],
    )
    .await?;
    rows.into_iter()
        .next()
        .filter(|v| !v.is_null())
        .and_then(|row| {
            row.get("productId")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .ok_or_else(|| not_found(group_id))
}

async fn crash_product_and_group(
    db: &Surreal<Any>,
    crash_id: &str,
) -> Result<(String, Option<String>), (StatusCode, String)> {
    let rows = run_value(
        db,
        "SELECT meta::id(product_id) AS productId,
                IF group_id != NONE THEN meta::id(group_id) ELSE NONE END AS groupId
         FROM ONLY type::record('crashes', $id)",
        vec![("id", Value::String(crash_id.to_string()))],
    )
    .await?;
    rows.into_iter()
        .next()
        .filter(|v| !v.is_null())
        .and_then(|row| {
            let product_id = row
                .get("productId")
                .and_then(|v| v.as_str())
                .map(String::from)?;
            let group_id = row
                .get("groupId")
                .and_then(|v| v.as_str())
                .map(String::from);
            Some((product_id, group_id))
        })
        .ok_or_else(|| not_found(crash_id))
}

async fn product_id_for_symbol(
    db: &Surreal<Any>,
    symbol_id: &str,
) -> Result<String, (StatusCode, String)> {
    let rows = run_value(
        db,
        "SELECT meta::id(product_id) AS productId
         FROM ONLY type::record('symbols', $id)",
        vec![("id", Value::String(symbol_id.to_string()))],
    )
    .await?;
    rows.into_iter()
        .next()
        .filter(|v| !v.is_null())
        .and_then(|row| {
            row.get("productId")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .ok_or_else(|| not_found(symbol_id))
}

fn avatar_initials(name: &str) -> String {
    let avatar = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    if avatar.is_empty() {
        "U".to_string()
    } else {
        avatar
    }
}

async fn run_value(
    db: &Surreal<Any>,
    sql: &str,
    binds: Vec<(&'static str, Value)>,
) -> Result<Vec<Value>, (StatusCode, String)> {
    let mut q = db.query(sql);
    for (k, v) in binds {
        q = q.bind((k, v));
    }
    let mut resp = q.await.map_err(server_error)?;
    let out: Vec<Value> = resp.take(0).map_err(server_error)?;
    Ok(out)
}

const USER_PROJ: &str =
    "meta::id(id) AS id, username, email, name, avatar, is_admin AS isAdmin, created_at AS joinedAt";
const PRODUCT_PROJ: &str =
    "meta::id(id) AS id, name, slug, description, color, public, product_token AS productToken";
const SYMBOL_PROJ: &str = "meta::id(id) AS id, meta::id(product_id) AS productId, \
    module_id AS name, version, arch, 'Breakpad' AS format, '' AS size, \
    build_id AS debugId, '' AS codeId, channel, commit, build_tag AS buildTag, \
    created_at AS uploadedAt, '' AS uploadedBy, 0 AS referencedBy";

const GROUP_BASE_SELECT: &str = "
    SELECT
        meta::id(id)         AS id,
        meta::id(product_id) AS productId,
        fingerprint,
        signal,
        count,
        status,
        IF assignee != NONE THEN meta::id(assignee) ELSE NONE END AS assignee,
        first_seen           AS firstSeen,
        last_seen            AS lastSeen
    FROM crash_groups
";

fn extract_short_id(s: &str) -> String {
    let after = s.split_once(':').map(|(_, r)| r).unwrap_or(s);
    after
        .trim_matches(|c: char| c == '⟨' || c == '⟩' || c == '`')
        .to_string()
}

fn apply_rep(mut group: Value, rep: Option<&Value>) -> Value {
    if let (Some(obj), Some(rep_obj)) = (group.as_object_mut(), rep.and_then(|v| v.as_object())) {
        for key in [
            "title",
            "topFrame",
            "file",
            "line",
            "address",
            "platform",
            "version",
            "build",
            "exceptionType",
            "exceptionTypeShort",
            "similarity",
        ] {
            if !obj.contains_key(key)
                && let Some(val) = rep_obj.get(key)
            {
                obj.insert(key.into(), val.clone());
            }
        }
    }
    group
}

fn attachment_filename(name: Option<&str>, filename: Option<&str>) -> String {
    filename
        .filter(|value| !value.is_empty())
        .or(name.filter(|value| !value.is_empty()))
        .unwrap_or("attachment.bin")
        .to_string()
}

fn quoted_header_value(prefix: &str, value: &str) -> Option<HeaderValue> {
    let sanitized = value.replace(['"', '\n', '\r'], "_");
    HeaderValue::from_str(&format!("{prefix}\"{sanitized}\"")).ok()
}

async fn load_attachment_rows(
    db: &Surreal<Any>,
    crash_id: &str,
) -> Result<Vec<Value>, (StatusCode, String)> {
    run_value(
        db,
        "SELECT
            meta::id(id) AS id,
            name,
            mime_type AS mimeType,
            size,
            filename,
            storage_path AS storagePath,
            created_at AS createdAt
         FROM attachments
         WHERE crash_id = type::record('crashes', $cid)
         ORDER BY created_at",
        vec![("cid", Value::String(crash_id.to_string()))],
    )
    .await
}

// Returns attachment metadata for the crash. "user-text" attachments are split
// out and returned as the second element (metadata only, no S3 fetch). Their
// content is served on-demand via the download-attachment endpoint.
fn split_crash_attachments(rows: Vec<Value>) -> (Vec<Value>, Option<Value>) {
    let mut attachments = Vec::new();
    let mut user_text = None;

    for row in rows {
        let name = row.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        if name == "user-text" {
            if user_text.is_none() {
                user_text = Some(json!({
                    "attachmentId": row.get("id").and_then(|v| v.as_str()).map(extract_short_id).unwrap_or_default(),
                    "filename": attachment_filename(
                        row.get("name").and_then(|v| v.as_str()),
                        row.get("filename").and_then(|v| v.as_str())
                    ),
                    "createdAt": row.get("createdAt").cloned().unwrap_or(Value::Null),
                }));
            }
            continue;
        }

        attachments.push(json!({
            "id": row.get("id").and_then(|v| v.as_str()).map(extract_short_id).unwrap_or_default(),
            "name": name,
            "filename": attachment_filename(
                row.get("name").and_then(|v| v.as_str()),
                row.get("filename").and_then(|v| v.as_str())
            ),
            "mimeType": row.get("mimeType").cloned().unwrap_or(Value::String("application/octet-stream".into())),
            "size": row.get("size").cloned().unwrap_or(Value::Null),
            "createdAt": row.get("createdAt").cloned().unwrap_or(Value::Null),
        }));
    }

    (attachments, user_text)
}

// --------------------------------------------------------------------
// auth / users
// --------------------------------------------------------------------

async fn get_me(
    State(s): State<AppState>,
    session: Session,
) -> Result<Json<Value>, (StatusCode, String)> {
    let user = crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let rows = run_value(
        &s.repo.db,
        &format!("SELECT {USER_PROJ} FROM ONLY type::record('users', $id)"),
        vec![("id", Value::String(user.active().id.clone()))],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "not authenticated".into()))
}

async fn list_users(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let rows =
        run_value(&db, &format!("SELECT {USER_PROJ} FROM users ORDER BY created_at"), vec![])
            .await?;
    Ok(Json(Value::Array(rows)))
}

async fn get_user(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let rows = run_value(
        &db,
        &format!("SELECT {USER_PROJ} FROM ONLY type::record('users', $id)"),
        vec![("id", Value::String(id.clone()))],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| not_found(&id))
}

#[derive(Deserialize)]
struct FindUserQuery {
    q: String,
}

async fn find_user(
    State(s): State<AppState>,
    session: Session,
    Query(q): Query<FindUserQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let name = q.q.trim().to_lowercase();
    let rows = run_value(
        &s.repo.db,
        &format!(
            "SELECT {USER_PROJ} FROM users \
             WHERE string::lowercase(username) = $q OR string::lowercase(email) = $q \
             LIMIT 1"
        ),
        vec![("q", Value::String(name.clone()))],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| not_found(&name))
}

#[derive(Deserialize)]
struct CreateUserBody {
    email: String,
    name: Option<String>,
    is_admin: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateUserBody {
    email: Option<String>,
    name: Option<String>,
}

async fn create_user(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<CreateUserBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let email = body.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(bad("Email required."));
    }
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .unwrap_or(&email)
        .to_string();
    let slug: String = email
        .split('@')
        .next()
        .unwrap_or("u")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let avatar = avatar_initials(&name);
    let rows = run_value(
        &db,
        &format!(
            "CREATE type::record('users', $id) CONTENT {{
            username: $email, email: $email, name: $name, avatar: $avatar,
            is_admin: $is_admin, created_at: time::now(), updated_at: time::now()
        }} RETURN {USER_PROJ}"
        ),
        vec![
            ("id", Value::String(format!("u-{slug}"))),
            ("email", Value::String(email)),
            ("name", Value::String(name)),
            ("avatar", Value::String(avatar)),
            ("is_admin", Value::Bool(body.is_admin.unwrap_or(false))),
        ],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| bad("create failed"))
}

async fn update_user(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let email = body
        .email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| bad("Email required."))?;
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("Name required."))?
        .to_string();
    let avatar = avatar_initials(&name);

    let rows = run_value(
        &db,
        &format!(
            "UPDATE type::record('users', $id) SET
            username = $email,
            email = $email,
            name = $name,
            avatar = $avatar,
            updated_at = time::now()
            RETURN {USER_PROJ}"
        ),
        vec![
            ("id", Value::String(id.clone())),
            ("email", Value::String(email)),
            ("name", Value::String(name)),
            ("avatar", Value::String(avatar)),
        ],
    )
    .await?;

    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| not_found(&id))
}

async fn delete_user(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    run_value(
        &db,
        "DELETE user_access WHERE user_id = type::record('users', $id)",
        vec![("id", Value::String(id.clone()))],
    )
    .await?;
    run_value(&db, "DELETE type::record('users', $id)", vec![("id", Value::String(id))]).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SetAdminBody {
    #[serde(rename = "isAdmin")]
    is_admin: bool,
}

async fn set_admin(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SetAdminBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    run_value(
        &db,
        "UPDATE type::record('users', $id) SET is_admin = $v, updated_at = time::now()",
        vec![("id", Value::String(id)), ("v", Value::Bool(body.is_admin))],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn memberships_for(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_user_or_admin(&session, &headers, &s.repo.db, &user_id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let rows = run_value(
        &db,
        "SELECT
            meta::id(user_id)    AS userId,
            meta::id(product_id) AS productId,
            role,
            {
              id: meta::id(product_id),
              name: product_id.name,
              slug: product_id.slug,
              description: product_id.description,
              color: product_id.color
            } AS product
         FROM user_access WHERE user_id = type::record('users', $uid)",
        vec![("uid", Value::String(user_id))],
    )
    .await?;
    Ok(Json(Value::Array(rows)))
}

// --------------------------------------------------------------------
// products
// --------------------------------------------------------------------

#[derive(Deserialize)]
struct ListProductsQuery {
    scope: Option<String>,
    user: Option<String>,
}

async fn list_products(
    State(s): State<AppState>,
    session: Session,
    Query(q): Query<ListProductsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    match q.scope.as_deref() {
        Some("mine") => {
            let db = s.user_db(&session).await?;
            let Some(uid) = q.user.as_deref() else {
                return Ok(Json(Value::Array(vec![])));
            };
            let rows = run_value(
                &db,
                &format!(
                    "SELECT {PRODUCT_PROJ} FROM products
                    WHERE id IN (SELECT VALUE product_id FROM user_access
                        WHERE user_id = type::record('users', $uid))"
                ),
                vec![("uid", Value::String(uid.into()))],
            )
            .await?;
            Ok(Json(Value::Array(rows)))
        }
        Some("public") => {
            let rows = run_value(
                &s.repo.db,
                &format!("SELECT {PRODUCT_PROJ} FROM products WHERE public = true ORDER BY name"),
                vec![],
            )
            .await?;
            Ok(Json(Value::Array(rows)))
        }
        _ => {
            let db = s.user_db(&session).await?;
            let rows = run_value(
                &db,
                &format!("SELECT {PRODUCT_PROJ} FROM products ORDER BY name"),
                vec![],
            )
            .await?;
            Ok(Json(Value::Array(rows)))
        }
    }
}

async fn get_product(
    State(s): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&session).await?;
    let rows = run_value(
        &db,
        &format!("SELECT {PRODUCT_PROJ} FROM ONLY type::record('products', $id)"),
        vec![("id", Value::String(id.clone()))],
    )
    .await?;
    rows.into_iter()
        .find(|v| !v.is_null())
        .map(Json)
        .ok_or_else(|| not_found(&id))
}

#[derive(Deserialize)]
struct CreateProductBody {
    name: String,
    slug: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct UpdateProductBody {
    name: Option<String>,
    slug: Option<String>,
    description: Option<String>,
    color: Option<String>,
    public: Option<bool>,
}

async fn create_product(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<CreateProductBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let slug = body
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| {
            body.name
                .to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string()
        });
    let product_token = common::token::generate_product_token();
    let rows = run_value(
        &db,
        &format!(
            "CREATE type::record('products', $id) CONTENT {{
            name: $name, slug: $slug, description: $description,
            color: '#6b7280', public: false, accepting_crashes: true,
            product_token: $product_token, metadata: {{}}
        }} RETURN {PRODUCT_PROJ}"
        ),
        vec![
            ("id", Value::String(slug.clone())),
            ("name", Value::String(body.name)),
            ("slug", Value::String(slug)),
            ("description", Value::String(body.description.unwrap_or_default())),
            ("product_token", Value::String(product_token)),
        ],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| bad("create failed"))
}

async fn update_product(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateProductBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("Name required."))?
        .to_string();
    let slug = body
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("Slug required."))?
        .to_string();
    let description = body.description.unwrap_or_default();
    let color = body.color.unwrap_or_else(|| "#6b7280".into());

    let mut set_clauses = vec![
        "name = $name",
        "slug = $slug",
        "description = $description",
        "color = $color",
    ];
    if body.public.is_some() {
        set_clauses.push("public = $public");
    }
    let set_sql = set_clauses.join(", ");

    let mut args = vec![
        ("id", Value::String(id.clone())),
        ("name", Value::String(name)),
        ("slug", Value::String(slug)),
        ("description", Value::String(description)),
        ("color", Value::String(color)),
    ];
    if let Some(public) = body.public {
        args.push(("public", Value::Bool(public)));
    }

    let rows = run_value(
        &db,
        &format!("UPDATE type::record('products', $id) SET {set_sql} RETURN {PRODUCT_PROJ}"),
        args,
    )
    .await?;

    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| not_found(&id))
}

async fn delete_product(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let args = vec![("id", Value::String(id))];
    for sql in [
        "DELETE annotations WHERE product_id = type::record('products', $id)",
        "DELETE crashes     WHERE product_id = type::record('products', $id)",
        "DELETE crash_groups WHERE product_id = type::record('products', $id)",
        "DELETE symbols     WHERE product_id = type::record('products', $id)",
        "DELETE user_access WHERE product_id = type::record('products', $id)",
        "DELETE type::record('products', $id)",
    ] {
        run_value(&db, sql, args.clone()).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn get_product_email_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let settings = repos::product_settings::ProductSettingsRepo::get_or_create(&db, &id)
        .await
        .map_err(server_error)?;
    let email = settings.email;
    Ok(Json(json!({
        "invite_subject": email.invite_subject.unwrap_or_default(),
        "invite_html_template": email.invite_html_template.unwrap_or_default(),
        "invite_text_template": email.invite_text_template.unwrap_or_default(),
        "default_invite_html_template": crate::routes::invite::DEFAULT_INVITE_HTML,
        "default_invite_text_template": crate::routes::invite::DEFAULT_INVITE_TEXT,
    })))
}

#[derive(Deserialize)]
struct UpdateEmailSettingsBody {
    invite_subject: Option<String>,
    invite_html_template: Option<String>,
    invite_text_template: Option<String>,
}

async fn update_product_email_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateEmailSettingsBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let email = data::product_settings::EmailSettings {
        invite_subject: body.invite_subject.filter(|s| !s.is_empty()),
        invite_html_template: body.invite_html_template.filter(|s| !s.is_empty()),
        invite_text_template: body.invite_text_template.filter(|s| !s.is_empty()),
    };
    let saved = repos::product_settings::ProductSettingsRepo::upsert_email(&db, &id, email)
        .await
        .map_err(server_error)?;
    Ok(Json(json!({
        "invite_subject": saved.email.invite_subject.unwrap_or_default(),
        "invite_html_template": saved.email.invite_html_template.unwrap_or_default(),
        "invite_text_template": saved.email.invite_text_template.unwrap_or_default(),
    })))
}

// --------------------------------------------------------------------
// Product processor settings
// --------------------------------------------------------------------

async fn get_product_processor_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let settings = repos::product_settings::ProductSettingsRepo::get_or_create(&db, &id)
        .await
        .map_err(server_error)?;
    let p = settings.processor;
    let d = &s.settings.processor;
    Ok(Json(json!({
        "skip_patterns": p.skip_patterns,
        "end_patterns": p.end_patterns,
        "delimiter": p.delimiter,
        "maximum_frame_count": p.maximum_frame_count,
        "default_skip_patterns": d.skip_patterns,
        "default_end_patterns": d.end_patterns,
        "default_delimiter": d.delimiter.clone().unwrap_or_else(|| "|".to_string()),
        "default_maximum_frame_count": d.maximum_frame_count.unwrap_or(20),
    })))
}

#[derive(Deserialize)]
struct UpdateProcessorSettingsBody {
    skip_patterns: Option<Vec<String>>,
    end_patterns: Option<Vec<String>>,
    delimiter: Option<String>,
    maximum_frame_count: Option<u64>,
}

async fn update_product_processor_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateProcessorSettingsBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let processor = data::product_settings::ProcessorSettings {
        skip_patterns: body.skip_patterns,
        end_patterns: body.end_patterns,
        delimiter: body.delimiter.filter(|s| !s.is_empty()),
        maximum_frame_count: body.maximum_frame_count,
    };
    let saved = repos::product_settings::ProductSettingsRepo::upsert_processor(&db, &id, processor)
        .await
        .map_err(server_error)?;
    let p = saved.processor;
    Ok(Json(json!({
        "skip_patterns": p.skip_patterns,
        "end_patterns": p.end_patterns,
        "delimiter": p.delimiter,
        "maximum_frame_count": p.maximum_frame_count,
    })))
}

// --------------------------------------------------------------------
// Product minidump settings
// --------------------------------------------------------------------

async fn get_product_minidump_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let settings = repos::product_settings::ProductSettingsRepo::get_or_create(&db, &id)
        .await
        .map_err(server_error)?;
    Ok(Json(json!({
        "mandatory_annotations": settings.minidump.mandatory_annotations.unwrap_or_default(),
    })))
}

#[derive(Deserialize)]
struct UpdateMinidumpSettingsBody {
    mandatory_annotations: Vec<String>,
}

async fn update_product_minidump_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateMinidumpSettingsBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let annotations: Vec<String> = body
        .mandatory_annotations
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let minidump = data::product_settings::MinidumpSettings {
        mandatory_annotations: if annotations.is_empty() {
            None
        } else {
            Some(annotations)
        },
    };
    let saved = repos::product_settings::ProductSettingsRepo::upsert_minidump(&db, &id, minidump)
        .await
        .map_err(server_error)?;
    Ok(Json(json!({
        "mandatory_annotations": saved.minidump.mandatory_annotations.unwrap_or_default(),
    })))
}

// --------------------------------------------------------------------
// Validation scripts
// --------------------------------------------------------------------

async fn list_validation_scripts(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let scripts = repos::validation_scripts::ValidationScriptsRepo::list(&db, &id)
        .await
        .map_err(server_error)?;

    let result: Vec<Value> = scripts
        .into_iter()
        .map(|s| json!({ "id": s.id, "name": s.name, "created_at": s.created_at }))
        .collect();
    Ok(Json(Value::Array(result)))
}

#[derive(Deserialize)]
struct UploadValidationScriptBody {
    name: String,
    content: String,
}

async fn upload_validation_script(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UploadValidationScriptBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".to_string()));
    }
    if body.content.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "content is required".to_string()));
    }

    let script =
        repos::validation_scripts::ValidationScriptsRepo::create(&db, &id, &name, &body.content)
            .await
            .map_err(server_error)?;
    Ok(Json(
        json!({ "id": script.id, "name": script.name, "created_at": script.created_at }),
    ))
}

async fn get_validation_script(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((id, sid)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let script = repos::validation_scripts::ValidationScriptsRepo::get(&db, &sid, &id)
        .await
        .map_err(server_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Script not found".to_string()))?;

    Ok(Json(json!({
        "id": script.id,
        "name": script.name,
        "content": script.content,
        "created_at": script.created_at,
    })))
}

async fn delete_validation_script(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((id, sid)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    repos::validation_scripts::ValidationScriptsRepo::delete(&db, &sid, &id)
        .await
        .map_err(server_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct UpdateProductTokenBody {
    product_token: Option<String>,
}

async fn update_product_token(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateProductTokenBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let token = body
        .product_token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(common::token::generate_product_token);

    let rows = run_value(
        &db,
        &format!("UPDATE type::record('products', $id) SET product_token = $new_token RETURN {PRODUCT_PROJ}"),
        vec![("id", Value::String(id.clone())), ("new_token", Value::String(token))],
    )
    .await?;

    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| not_found(&id))
}

// --------------------------------------------------------------------
// Global app settings (admin-only)
// --------------------------------------------------------------------

async fn get_app_email_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;

    let settings = repos::app_settings::AppSettingsRepo::get_or_create(&s.repo.db)
        .await
        .map_err(server_error)?;
    let email = settings.email;
    Ok(Json(json!({
        "recovery_subject": email.recovery_subject.unwrap_or_default(),
        "recovery_html_template": email.recovery_html_template.unwrap_or_default(),
        "recovery_text_template": email.recovery_text_template.unwrap_or_default(),
        "default_recovery_html_template": crate::routes::auth::DEFAULT_RECOVERY_HTML,
        "default_recovery_text_template": crate::routes::auth::DEFAULT_RECOVERY_TEXT,
    })))
}

#[derive(Deserialize)]
struct UpdateAppEmailSettingsBody {
    recovery_subject: Option<String>,
    recovery_html_template: Option<String>,
    recovery_text_template: Option<String>,
}

async fn update_app_email_settings(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<UpdateAppEmailSettingsBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;

    let email = data::app_settings::AppEmailSettings {
        recovery_subject: body.recovery_subject.filter(|s| !s.is_empty()),
        recovery_html_template: body.recovery_html_template.filter(|s| !s.is_empty()),
        recovery_text_template: body.recovery_text_template.filter(|s| !s.is_empty()),
    };
    let saved = repos::app_settings::AppSettingsRepo::upsert_email(&s.repo.db, email)
        .await
        .map_err(server_error)?;
    Ok(Json(json!({
        "recovery_subject": saved.email.recovery_subject.unwrap_or_default(),
        "recovery_html_template": saved.email.recovery_html_template.unwrap_or_default(),
        "recovery_text_template": saved.email.recovery_text_template.unwrap_or_default(),
    })))
}

// --------------------------------------------------------------------
// memberships
// --------------------------------------------------------------------

async fn list_members(
    State(s): State<AppState>,
    session: Session,
    Path(pid): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&session).await?;
    let rows = run_value(
        &db,
        "SELECT
            meta::id(user_id)    AS userId,
            meta::id(product_id) AS productId,
            role,
            {
              id: meta::id(user_id),
              email: user_id.email,
              name: user_id.name,
              avatar: user_id.avatar,
              isAdmin: user_id.is_admin,
              joinedAt: user_id.created_at
            } AS user
         FROM user_access WHERE product_id = type::record('products', $pid)",
        vec![("pid", Value::String(pid))],
    )
    .await?;
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct GrantBody {
    role: String,
}

async fn grant_access(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((pid, uid)): Path<(String, String)>,
    Json(body): Json<GrantBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    run_value(
        &db,
        "DELETE user_access WHERE user_id = type::record('users', $uid)
                              AND product_id = type::record('products', $pid)",
        vec![
            ("uid", Value::String(uid.clone())),
            ("pid", Value::String(pid.clone())),
        ],
    )
    .await?;
    run_value(
        &db,
        "CREATE user_access CONTENT {
            user_id: type::record('users', $uid),
            product_id: type::record('products', $pid),
            role: $role,
            created_at: time::now(),
            updated_at: time::now()
        }",
        vec![
            ("uid", Value::String(uid)),
            ("pid", Value::String(pid)),
            ("role", Value::String(body.role)),
        ],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_access(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((pid, uid)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    run_value(
        &db,
        "DELETE user_access WHERE user_id = type::record('users', $uid)
                              AND product_id = type::record('products', $pid)",
        vec![("uid", Value::String(uid)), ("pid", Value::String(pid))],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------
// crashes
// --------------------------------------------------------------------

#[derive(Deserialize)]
struct ListGroupsQuery {
    #[serde(rename = "productId")]
    product_id: String,
    version: Option<String>,
    status: Option<String>,
    search: Option<String>,
    sort: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_groups(
    State(s): State<AppState>,
    session: Session,
    Query(q): Query<ListGroupsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&session).await?;
    let base_sql = format!(
        "{GROUP_BASE_SELECT}
        WHERE product_id = type::record('products', $pid)
        ORDER BY count DESC"
    );
    let reps_sql = "
        SELECT
            group_id,
            created_at,
            report.title              AS title,
            report.topFrame           AS topFrame,
            report.file               AS file,
            report.line               AS line,
            report.version            AS version,
            report.build              AS build,
            report.address            AS address,
            report.platform           AS platform,
            (report.exceptionType     ?? report.crash_info.type) AS exceptionType,
            report.exceptionTypeShort AS exceptionTypeShort,
            report.similarity         AS similarity
        FROM crashes
        WHERE product_id = type::record('products', $pid)
        ORDER BY created_at DESC";
    let (base_res, reps_res) = tokio::join!(
        run_value(&db, &base_sql, vec![("pid", Value::String(q.product_id.clone()))]),
        run_value(&db, reps_sql, vec![("pid", Value::String(q.product_id.clone()))]),
    );
    let base = base_res?;
    let rep_rows = reps_res?;

    let mut reps: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut trends: std::collections::HashMap<String, [u64; 14]> = std::collections::HashMap::new();
    let mut versions_set = std::collections::BTreeSet::new();
    let now = Utc::now();

    for r in rep_rows {
        if let Some(v) = r.get("version").and_then(|v| v.as_str())
            && !v.is_empty()
        {
            versions_set.insert(v.to_string());
        }
        let Some(gid_raw) = r.get("group_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let gid = extract_short_id(gid_raw);

        *counts.entry(gid.clone()).or_insert(0) += 1;

        // 30D trend: 14 two-day buckets; bucket 0 = oldest, 13 = most recent
        if let Some(created_str) = r.get("created_at").and_then(|v| v.as_str())
            && let Ok(ts) = created_str.parse::<chrono::DateTime<Utc>>()
        {
            let days_ago = (now - ts).num_days();
            if (0..28).contains(&days_ago) {
                let bucket = (13 - days_ago / 2) as usize;
                trends.entry(gid.clone()).or_insert([0u64; 14])[bucket] += 1;
            }
        }

        reps.entry(gid).or_insert(r);
    }
    let versions_list: Vec<String> = versions_set.into_iter().rev().collect();

    let mut groups: Vec<Value> = base
        .into_iter()
        .map(|g| {
            // meta::id() can return "⟨uuid⟩" with angle brackets; normalise so
            // the lookup into counts/trends/reps (keyed by plain UUID) always works.
            let gid = extract_short_id(g.get("id").and_then(|v| v.as_str()).unwrap_or_default());
            let mut merged = apply_rep(g, reps.get(&gid));
            if let Some(obj) = merged.as_object_mut() {
                if let Some(&actual) = counts.get(&gid) {
                    obj.insert("count".into(), json!(actual));
                }
                let trend = trends
                    .get(&gid)
                    .map(|t| t.iter().map(|&c| json!(c)).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![json!(0u64); 14]);
                obj.insert("trend".into(), json!(trend));
            }
            merged
        })
        .collect();

    if let Some(v) = q
        .version
        .as_deref()
        .filter(|x| *x != "all" && !x.is_empty())
    {
        groups.retain(|g| g.get("version").and_then(|v| v.as_str()) == Some(v));
    }
    if let Some(st) = q.status.as_deref().filter(|x| *x != "all" && !x.is_empty()) {
        groups.retain(|g| g.get("status").and_then(|v| v.as_str()) == Some(st));
    }
    if let Some(search) = q.search.as_deref().filter(|s| !s.trim().is_empty()) {
        let needle = search.to_lowercase();
        groups.retain(|g| {
            let t = g
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_lowercase();
            let f = g
                .get("topFrame")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_lowercase();
            t.contains(&needle) || f.contains(&needle)
        });
    }

    match q.sort.as_deref() {
        Some("recent") => groups.sort_by(|a, b| {
            b.get("lastSeen")
                .and_then(|v| v.as_str())
                .cmp(&a.get("lastSeen").and_then(|v| v.as_str()))
        }),
        Some("similarity") => groups.sort_by(|a, b| {
            b.get("similarity")
                .and_then(|v| v.as_f64())
                .partial_cmp(&a.get("similarity").and_then(|v| v.as_f64()))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        Some("version") => groups.sort_by(|a, b| {
            b.get("version")
                .and_then(|v| v.as_str())
                .cmp(&a.get("version").and_then(|v| v.as_str()))
        }),
        _ => {}
    }

    let total = groups.len();
    let off = q.offset.unwrap_or(0);
    let lim = q.limit.unwrap_or(groups.len());
    let slice: Vec<Value> = groups.into_iter().skip(off).take(lim).collect();
    let versions: Vec<Value> = versions_list.into_iter().map(Value::String).collect();

    Ok(Json(json!({
        "groups": slice,
        "total": total,
        "versions": versions,
    })))
}

async fn get_group(
    State(s): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&session).await?;
    let g = compose_group(&db, &id).await?;
    match g {
        Some(v) => Ok(Json(v)),
        None => Err(not_found(&id)),
    }
}

async fn get_crash(
    State(s): State<AppState>,
    session: Session,
    Path(crash_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&session).await?;
    let crashes = run_value(
        &db,
        "SELECT * FROM ONLY type::record('crashes', $cid)",
        vec![("cid", Value::String(crash_id.clone()))],
    )
    .await?;
    let Some(row) = crashes.into_iter().find(|v| !v.is_null()) else {
        return Err(not_found(&crash_id));
    };

    let (attachment_rows, annotation_rows) = tokio::join!(
        load_attachment_rows(&db, &crash_id),
        run_value(
            &db,
            "SELECT key, value, source
             FROM annotations
             WHERE crash_id = type::record('crashes', $cid)
             ORDER BY source, key",
            vec![("cid", Value::String(crash_id.clone()))],
        ),
    );

    let (attachments, user_text) = split_crash_attachments(attachment_rows?);
    let annotations = build_annotations_map(annotation_rows?);

    let mut crash_value = hydrate_crash(&row, attachments, user_text);
    if let Some(obj) = crash_value.as_object_mut() {
        obj.insert("annotations".into(), Value::Object(annotations));
    }

    let group_id = row
        .get("group_id")
        .and_then(|v| v.as_str())
        .map(extract_short_id)
        .unwrap_or_default();
    let Some(group) = compose_group(&db, &group_id).await? else {
        return Err(not_found(&crash_id));
    };
    Ok(Json(json!({ "crash": crash_value, "group": group })))
}

async fn delete_crash(
    State(s): State<AppState>,
    session: Session,
    Path(crash_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let (product_id, group_id) = crash_product_and_group(&s.repo.db, &crash_id).await?;
    crate::access::require_session_product_role(&session, &s.repo.db, &product_id, "readwrite")
        .await
        .map_err(access_err)?;

    let db = s.user_db(&session).await?;
    run_value(
        &db,
        "DELETE annotations WHERE crash_id = type::record('crashes', $cid)",
        vec![("cid", Value::String(crash_id.clone()))],
    )
    .await?;
    run_value(
        &db,
        "DELETE attachments WHERE crash_id = type::record('crashes', $cid)",
        vec![("cid", Value::String(crash_id.clone()))],
    )
    .await?;
    run_value(
        &db,
        "DELETE type::record('crashes', $cid)",
        vec![("cid", Value::String(crash_id))],
    )
    .await?;

    if let Some(group_id) = group_id {
        refresh_or_delete_group_after_crash_delete(&db, &group_id).await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_group(
    State(s): State<AppState>,
    session: Session,
    Path(group_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let product_id = product_id_for_crash_group(&s.repo.db, &group_id).await?;
    crate::access::require_session_product_role(&session, &s.repo.db, &product_id, "readwrite")
        .await
        .map_err(access_err)?;

    let db = s.user_db(&session).await?;
    delete_group_contents(&db, &group_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_group_contents(
    db: &Surreal<Any>,
    group_id: &str,
) -> Result<(), (StatusCode, String)> {
    let args = vec![("gid", Value::String(group_id.to_string()))];
    for sql in [
        "DELETE annotations WHERE group_id = type::record('crash_groups', $gid)",
        "DELETE annotations WHERE crash_id IN (
            SELECT VALUE id FROM crashes WHERE group_id = type::record('crash_groups', $gid)
        )",
        "DELETE attachments WHERE crash_id IN (
            SELECT VALUE id FROM crashes WHERE group_id = type::record('crash_groups', $gid)
        )",
        "DELETE crashes WHERE group_id = type::record('crash_groups', $gid)",
        "DELETE type::record('crash_groups', $gid)",
    ] {
        run_value(db, sql, args.clone()).await?;
    }
    Ok(())
}

async fn refresh_or_delete_group_after_crash_delete(
    db: &Surreal<Any>,
    group_id: &str,
) -> Result<(), (StatusCode, String)> {
    let rows = run_value(
        db,
        "SELECT created_at
         FROM crashes
         WHERE group_id = type::record('crash_groups', $gid)
         ORDER BY created_at ASC",
        vec![("gid", Value::String(group_id.to_string()))],
    )
    .await?;
    if rows.is_empty() {
        delete_group_contents(db, group_id).await?;
        return Ok(());
    }

    let first_seen = rows
        .first()
        .and_then(|r| r.get("created_at"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| server_error("remaining crash is missing created_at"))?
        .to_string();
    let last_seen = rows
        .last()
        .and_then(|r| r.get("created_at"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| server_error("remaining crash is missing created_at"))?
        .to_string();
    run_value(
        db,
        "UPDATE type::record('crash_groups', $gid) SET
            count = $count,
            first_seen = <datetime>$first_seen,
            last_seen = <datetime>$last_seen,
            updated_at = time::now()",
        vec![
            ("gid", Value::String(group_id.to_string())),
            ("count", Value::Number((rows.len() as i64).into())),
            ("first_seen", Value::String(first_seen)),
            ("last_seen", Value::String(last_seen)),
        ],
    )
    .await?;
    Ok(())
}

fn build_annotations_map(rows: Vec<Value>) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    for row in rows {
        if let (Some(key), Some(value)) = (
            row.get("key").and_then(|v| v.as_str()),
            row.get("value").and_then(|v| v.as_str()),
        ) {
            // script source wins over submission for the same key
            let source = row
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("submission");
            if source == "script" || !map.contains_key(key) {
                map.insert(key.to_string(), Value::String(value.to_string()));
            }
        }
    }
    map
}

fn hydrate_crash(row: &Value, attachments: Vec<Value>, user_text: Option<Value>) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
        out.insert("id".into(), Value::String(extract_short_id(id)));
    }
    if let Some(gid) = row.get("group_id").and_then(|v| v.as_str()) {
        out.insert("groupId".into(), Value::String(extract_short_id(gid)));
    }
    if let Some(pid) = row.get("product_id").and_then(|v| v.as_str()) {
        out.insert("productId".into(), Value::String(extract_short_id(pid)));
    }
    if let Some(report) = row.get("report").and_then(|v| v.as_object()) {
        for (k, v) in report {
            out.insert(k.clone(), v.clone());
        }
    }
    // Fall back to the DB record timestamp when submission_timestamp was not stored.
    if out
        .get("at")
        .and_then(|v| v.as_str())
        .map(|s| s.is_empty())
        .unwrap_or(true)
        && let Some(ts) = row.get("created_at")
    {
        out.insert("at".into(), ts.clone());
    }
    out.insert("attachments".into(), Value::Array(attachments));
    out.insert("userText".into(), user_text.unwrap_or(Value::Null));
    Value::Object(out)
}

async fn compose_group(db: &Surreal<Any>, id: &str) -> Result<Option<Value>, (StatusCode, String)> {
    let rows = run_value(
        db,
        &format!("{GROUP_BASE_SELECT} WHERE meta::id(id) = $id LIMIT 1"),
        vec![("id", Value::String(id.into()))],
    )
    .await?;
    let Some(base) = rows.into_iter().next() else {
        return Ok(None);
    };

    let rep_rows = run_value(
        db,
        "SELECT
            created_at,
            report.title              AS title,
            report.topFrame           AS topFrame,
            report.file               AS file,
            report.line               AS line,
            report.address            AS address,
            report.platform           AS platform,
            report.version            AS version,
            report.build              AS build,
            (report.exceptionType     ?? report.crash_info.type) AS exceptionType,
            report.exceptionTypeShort AS exceptionTypeShort,
            report.similarity         AS similarity
         FROM crashes
         WHERE group_id = type::record('crash_groups', $gid)
         ORDER BY created_at DESC LIMIT 1",
        vec![("gid", Value::String(id.into()))],
    )
    .await?;
    let rep = rep_rows.into_iter().next();
    let mut group = apply_rep(base, rep.as_ref());
    let group_obj = group.as_object_mut().unwrap();

    let crash_rows = run_value(
        db,
        "SELECT
            created_at,
            meta::id(id)             AS id,
            meta::id(group_id)       AS groupId,
            meta::id(product_id)     AS productId,
            report.version           AS version,
            report.os                AS os,
            report.at                AS at,
            report.user              AS user,
            report.similarity        AS similarity,
            report.commit            AS commit,
            report.signal            AS signal,
            report.title             AS title,
            report.topFrame          AS topFrame,
            report.file              AS file,
            report.line              AS line,
            report.address           AS address,
            report.platform          AS platform,
            report.build             AS build,
            report.exceptionType     AS exceptionType,
            report.exceptionTypeShort AS exceptionTypeShort
         FROM crashes
         WHERE group_id = type::record('crash_groups', $gid)
         ORDER BY created_at DESC",
        vec![("gid", Value::String(id.into()))],
    )
    .await?;
    let actual_count = crash_rows.len();
    group_obj.insert("crashes".into(), Value::Array(crash_rows));
    group_obj.insert("count".into(), json!(actual_count));

    let notes = run_value(
        db,
        "SELECT author, value AS body, created_at AS at
         FROM annotations
         WHERE source = 'user' AND group_id = type::record('crash_groups', $gid)
         ORDER BY created_at",
        vec![("gid", Value::String(id.into()))],
    )
    .await?;
    group_obj.insert("notes".into(), Value::Array(notes));

    let product_id = group_obj
        .get("productId")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let signal = group_obj
        .get("signal")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let related_base = run_value(
        db,
        "SELECT meta::id(id) AS id, count FROM crash_groups
         WHERE product_id = type::record('products', $pid)
           AND signal = $signal
           AND meta::id(id) != $gid
         ORDER BY count DESC LIMIT 3",
        vec![
            ("pid", Value::String(product_id.clone())),
            ("signal", Value::String(signal)),
            ("gid", Value::String(id.into())),
        ],
    )
    .await?;
    let mut related: Vec<Value> = Vec::with_capacity(related_base.len());
    for g in related_base {
        let gid = g
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let count = g.get("count").cloned().unwrap_or(Value::Null);
        let title_rows = run_value(
            db,
            "SELECT created_at, report.title AS title FROM crashes
             WHERE group_id = type::record('crash_groups', $gid)
             ORDER BY created_at DESC LIMIT 1",
            vec![("gid", Value::String(gid.clone()))],
        )
        .await?;
        let title = title_rows
            .into_iter()
            .next()
            .and_then(|r| r.get("title").cloned())
            .unwrap_or(Value::Null);
        related.push(json!({ "id": gid, "title": title, "count": count }));
    }
    group_obj.insert("related".into(), Value::Array(related));

    Ok(Some(group))
}

async fn download_attachment(
    State(s): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let db = s.user_db(&session).await?;
    let rows = run_value(
        &db,
        "SELECT
            meta::id(id) AS id,
            name,
            filename,
            mime_type AS mimeType,
            storage_path AS storagePath
         FROM ONLY type::record('attachments', $id)",
        vec![("id", Value::String(id.clone()))],
    )
    .await?;
    let row = rows.into_iter().next().filter(|v| v.is_object());
    let Some(row) = row else {
        return Err(not_found(&id));
    };

    let storage_path = row
        .get("storagePath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| bad("attachment missing storage path"))?;
    let object = s
        .storage
        .get(&ObjectPath::from(storage_path))
        .await
        .map_err(storage_error)?;
    let bytes = object.bytes().await.map_err(storage_error)?;

    let filename = attachment_filename(
        row.get("name").and_then(|v| v.as_str()),
        row.get("filename").and_then(|v| v.as_str()),
    );
    let mime_type = row
        .get("mimeType")
        .and_then(|v| v.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream");

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    if let Some(value) = quoted_header_value("attachment; filename=", &filename) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[derive(Deserialize)]
struct SetStatusBody {
    status: String,
}

async fn set_status(
    State(s): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(body): Json<SetStatusBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let product_id = product_id_for_crash_group(&s.repo.db, &id).await?;
    crate::access::require_session_product_role(&session, &s.repo.db, &product_id, "readwrite")
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    run_value(
        &db,
        "UPDATE type::record('crash_groups', $id) SET status = $st, updated_at = time::now()",
        vec![
            ("id", Value::String(id)),
            ("st", Value::String(body.status)),
        ],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AddNoteBody {
    body: String,
    author: String,
}

async fn add_note(
    State(s): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(payload): Json<AddNoteBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let product_id = product_id_for_crash_group(&s.repo.db, &id).await?;
    crate::access::require_session_product_role(&session, &s.repo.db, &product_id, "readwrite")
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let prod_rows = run_value(
        &db,
        "SELECT meta::id(product_id) AS pid FROM ONLY type::record('crash_groups', $id)",
        vec![("id", Value::String(id.clone()))],
    )
    .await?;
    let pid = prod_rows
        .into_iter()
        .next()
        .and_then(|r| r.get("pid").and_then(|v| v.as_str()).map(String::from))
        .ok_or_else(|| not_found(&id))?;

    let now = Utc::now().to_rfc3339();
    run_value(
        &db,
        "CREATE annotations CONTENT {
            source: 'user',
            value: $body,
            author: $author,
            group_id: type::record('crash_groups', $gid),
            product_id: type::record('products', $pid),
            created_at: <datetime>$at,
            updated_at: <datetime>$at
         }",
        vec![
            ("body", Value::String(payload.body.clone())),
            ("author", Value::String(payload.author.clone())),
            ("gid", Value::String(id)),
            ("pid", Value::String(pid)),
            ("at", Value::String(now.clone())),
        ],
    )
    .await?;

    Ok(Json(json!({
        "author": payload.author,
        "body": payload.body,
        "at": now,
    })))
}

#[derive(Deserialize)]
struct MergeBody {
    #[serde(rename = "mergedId")]
    merged_id: String,
}

async fn merge_groups(
    State(s): State<AppState>,
    session: Session,
    Path(primary_id): Path<String>,
    Json(body): Json<MergeBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let primary_product_id = product_id_for_crash_group(&s.repo.db, &primary_id).await?;
    let merged_product_id = product_id_for_crash_group(&s.repo.db, &body.merged_id).await?;
    if merged_product_id != primary_product_id {
        return Err(bad("Cannot merge crash groups from different products."));
    }
    crate::access::require_product_maintainer(
        &session,
        &HeaderMap::new(),
        &s.repo.db,
        &primary_product_id,
    )
    .await
    .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let pid = Value::String(primary_id.clone());
    let mid = Value::String(body.merged_id);
    run_value(
        &db,
        "UPDATE crashes SET group_id = type::record('crash_groups', $pid)
         WHERE group_id = type::record('crash_groups', $mid)",
        vec![("pid", pid.clone()), ("mid", mid.clone())],
    )
    .await?;
    // Fetch the merged group's count separately — SurrealDB loses $token context in
    // UPDATE subqueries, causing RLS to filter out the subquery result as NONE.
    let merged_count_rows = run_value(
        &db,
        "SELECT VALUE count FROM ONLY type::record('crash_groups', $mid)",
        vec![("mid", mid.clone())],
    )
    .await?;
    let merged_count = merged_count_rows
        .into_iter()
        .next()
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    run_value(
        &db,
        "UPDATE type::record('crash_groups', $pid) SET
           count = count + $c,
           updated_at = time::now()",
        vec![("pid", pid), ("c", Value::Number(merged_count.into()))],
    )
    .await?;
    run_value(&db, "DELETE type::record('crash_groups', $mid)", vec![("mid", mid)]).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------
// symbols
// --------------------------------------------------------------------

#[derive(Deserialize)]
struct SymbolsQuery {
    search: Option<String>,
    arch: Option<String>,
    format: Option<String>,
    sort: Option<String>,
}

async fn list_symbols(
    State(s): State<AppState>,
    session: Session,
    Path(pid): Path<String>,
    Query(q): Query<SymbolsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&session).await?;
    let mut rows = run_value(
        &db,
        &format!(
            "SELECT {SYMBOL_PROJ} FROM symbols
                  WHERE product_id = type::record('products', $pid)"
        ),
        vec![("pid", Value::String(pid))],
    )
    .await?;

    if let Some(search) = q.search.as_deref().filter(|s| !s.trim().is_empty()) {
        let needle = search.to_lowercase();
        rows.retain(|r| {
            let n = r
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_lowercase();
            let d = r
                .get("debugId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_lowercase();
            n.contains(&needle) || d.contains(&needle)
        });
    }
    if let Some(a) = q.arch.as_deref().filter(|v| *v != "all" && !v.is_empty()) {
        rows.retain(|r| r.get("arch").and_then(|v| v.as_str()) == Some(a));
    }
    if let Some(f) = q.format.as_deref().filter(|v| *v != "all" && !v.is_empty()) {
        rows.retain(|r| r.get("format").and_then(|v| v.as_str()) == Some(f));
    }
    match q.sort.as_deref() {
        Some("name") => rows.sort_by(|a, b| {
            let an = a.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            an.cmp(bn).then_with(|| {
                b.get("version")
                    .and_then(|v| v.as_str())
                    .cmp(&a.get("version").and_then(|v| v.as_str()))
            })
        }),
        Some("size") => rows.sort_by(|a, b| {
            let parse = |v: &Value| {
                v.get("size")
                    .and_then(|x| x.as_str())
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|n| n.parse::<f64>().ok())
                    .unwrap_or(0.0)
            };
            parse(b)
                .partial_cmp(&parse(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        _ => rows.sort_by(|a, b| {
            b.get("uploadedAt")
                .and_then(|v| v.as_str())
                .cmp(&a.get("uploadedAt").and_then(|v| v.as_str()))
        }),
    }
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct UploadSymbolBody {
    name: String,
    arch: Option<String>,
}

async fn upload_symbol(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(pid): Path<String>,
    Json(body): Json<UploadSymbolBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let id = uuid::Uuid::new_v4().to_string();
    let module_id = body.name;
    let build_id = uuid::Uuid::new_v4().to_string();
    let storage_path = format!("symbols/{module_id}-{build_id}");

    let rows = run_value(
        &db,
        &format!(
            "CREATE type::record('symbols', $id) CONTENT {{
            product_id: type::record('products', $pid),
            os: '',
            arch: $arch,
            build_id: $build_id,
            module_id: $module_id,
            storage_path: $storage_path,
            version: '',
            channel: '',
            commit: '',
            build_tag: '',
            created_at: time::now(),
            updated_at: time::now()
        }} RETURN {SYMBOL_PROJ}"
        ),
        vec![
            ("id", Value::String(id)),
            ("pid", Value::String(pid)),
            ("arch", Value::String(body.arch.unwrap_or_else(|| "x86_64".into()))),
            ("build_id", Value::String(build_id)),
            ("module_id", Value::String(module_id)),
            ("storage_path", Value::String(storage_path)),
        ],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| bad("create failed"))
}

async fn delete_symbol(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_session(&session)
        .await
        .map_err(access_err)?;
    let product_id = product_id_for_symbol(&s.repo.db, &id).await?;
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &product_id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    run_value(&db, "DELETE type::record('symbols', $id)", vec![("id", Value::String(id))]).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------
// API tokens
// --------------------------------------------------------------------

const API_TOKEN_PROJ: &str = "meta::id(id) AS id, description, entitlements, is_active AS isActive, last_used_at AS lastUsedAt, expires_at AS expiresAt, created_at AS createdAt";

async fn list_api_tokens(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(pid): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let rows = run_value(
        &db,
        &format!(
            "SELECT {API_TOKEN_PROJ} FROM api_tokens \
             WHERE product_id = type::record('products', $pid) \
             ORDER BY created_at DESC"
        ),
        vec![("pid", Value::String(pid))],
    )
    .await?;
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct CreateApiTokenBody {
    description: String,
    entitlements: Option<Vec<String>>,
}

async fn create_api_token(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(pid): Path<String>,
    Json(body): Json<CreateApiTokenBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let description = body.description.trim().to_string();
    if description.is_empty() {
        return Err(bad("description required"));
    }
    let entitlements = body
        .entitlements
        .unwrap_or_else(|| vec!["symbol-upload".into()]);

    let (token_id, token, token_hash) = common::token::generate_api_token()
        .map_err(|e| server_error(format!("token generation failed: {e}")))?;

    let id = uuid::Uuid::new_v4().to_string();
    run_value(
        &db,
        "CREATE type::record('api_tokens', $id) CONTENT {
            description: $description,
            token_id: type::uuid($token_id),
            token_hash: $token_hash,
            product_id: type::record('products', $pid),
            user_id: NONE,
            entitlements: $entitlements,
            expires_at: NONE,
            is_active: true,
            created_at: time::now(),
            updated_at: time::now()
        }",
        vec![
            ("id", Value::String(id.clone())),
            ("description", Value::String(description.clone())),
            ("token_id", Value::String(token_id.to_string())),
            ("token_hash", Value::String(token_hash)),
            ("pid", Value::String(pid)),
            (
                "entitlements",
                Value::Array(entitlements.into_iter().map(Value::String).collect()),
            ),
        ],
    )
    .await?;

    Ok(Json(json!({
        "id": id,
        "description": description,
        "token": token,
    })))
}

async fn delete_api_token(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((pid, id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let _ = pid;
    run_value(&db, "DELETE type::record('api_tokens', $id)", vec![("id", Value::String(id))])
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------
// Admin API tokens (product-optional)
// --------------------------------------------------------------------

async fn list_all_api_tokens(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    let rows = run_value(
        &db,
        &format!(
            "SELECT {API_TOKEN_PROJ}, \
             IF product_id IS NOT NONE THEN meta::id(product_id) ELSE NONE END AS productId, \
             product_id.name AS productName, \
             IF user_id IS NOT NONE THEN meta::id(user_id) ELSE NONE END AS userId, \
             user_id.name AS userName \
             FROM api_tokens \
             ORDER BY created_at DESC"
        ),
        vec![],
    )
    .await?;
    Ok(Json(Value::Array(rows)))
}

const ENTITLEMENT_DEFS: &[(&str, &str, &str)] = &[
    ("symbol-upload", "Upload debug symbols", "product"),
    ("invitation-create", "Create user invitations", "general"),
    ("token", "Generate JWT tokens as the bound user", "user"),
];

async fn list_entitlements_handler(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let defs: Vec<Value> = ENTITLEMENT_DEFS
        .iter()
        .map(|(name, description, scope)| json!({"name": name, "description": description, "scope": scope}))
        .collect();
    Ok(Json(Value::Array(defs)))
}

#[derive(Deserialize)]
struct CreateAdminApiTokenBody {
    description: String,
    entitlements: Option<Vec<String>>,
    #[serde(rename = "productId")]
    product_id: Option<String>,
    #[serde(rename = "userId")]
    user_id: Option<String>,
}

async fn create_admin_api_token(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<CreateAdminApiTokenBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let description = body.description.trim().to_string();
    if description.is_empty() {
        return Err(bad("description required"));
    }
    let entitlements = body
        .entitlements
        .unwrap_or_else(|| vec!["symbol-upload".into()]);

    let (token_id, token, token_hash) = common::token::generate_api_token()
        .map_err(|e| server_error(format!("token generation failed: {e}")))?;

    let id = uuid::Uuid::new_v4().to_string();

    run_value(
        &db,
        "CREATE type::record('api_tokens', $id) CONTENT {
            description: $description,
            token_id: type::uuid($token_id),
            token_hash: $token_hash,
            product_id: IF $pid != NONE THEN type::record('products', $pid) ELSE NONE END,
            user_id: IF $uid != NONE THEN type::record('users', $uid) ELSE NONE END,
            entitlements: $entitlements,
            expires_at: NONE,
            is_active: true,
            created_at: time::now(),
            updated_at: time::now()
        }",
        vec![
            ("id", Value::String(id.clone())),
            ("description", Value::String(description.clone())),
            ("token_id", Value::String(token_id.to_string())),
            ("token_hash", Value::String(token_hash)),
            ("pid", body.product_id.map(Value::String).unwrap_or(Value::Null)),
            ("uid", body.user_id.map(Value::String).unwrap_or(Value::Null)),
            (
                "entitlements",
                Value::Array(entitlements.into_iter().map(Value::String).collect()),
            ),
        ],
    )
    .await?;

    Ok(Json(json!({
        "id": id,
        "description": description,
        "token": token,
    })))
}

#[derive(Deserialize)]
struct UpdateAdminApiTokenBody {
    description: String,
    #[serde(rename = "isActive")]
    is_active: bool,
    entitlements: Vec<String>,
    #[serde(rename = "productId")]
    product_id: Option<String>,
    #[serde(rename = "userId")]
    user_id: Option<String>,
}

async fn update_admin_api_token(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateAdminApiTokenBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;

    let description = body.description.trim().to_string();
    if description.is_empty() {
        return Err(bad("description required"));
    }

    run_value(
        &db,
        "UPDATE type::record('api_tokens', $id) SET \
            description = $description, \
            is_active = $is_active, \
            entitlements = $entitlements, \
            product_id = IF $pid != NONE THEN type::record('products', $pid) ELSE NONE END, \
            user_id = IF $uid != NONE THEN type::record('users', $uid) ELSE NONE END, \
            updated_at = time::now()",
        vec![
            ("id", Value::String(id)),
            ("description", Value::String(description)),
            ("is_active", Value::Bool(body.is_active)),
            (
                "entitlements",
                Value::Array(body.entitlements.into_iter().map(Value::String).collect()),
            ),
            ("pid", body.product_id.map(Value::String).unwrap_or(Value::Null)),
            ("uid", body.user_id.map(Value::String).unwrap_or(Value::Null)),
        ],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_admin_api_token(
    State(s): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&session).await?;
    run_value(&db, "DELETE type::record('api_tokens', $id)", vec![("id", Value::String(id))])
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_error_helpers_return_expected_statuses() {
        assert_eq!(bad("bad"), (StatusCode::BAD_REQUEST, "bad".to_string()));
        assert_eq!(not_found("thing"), (StatusCode::NOT_FOUND, "not found: thing".to_string()));
        assert_eq!(
            server_error("boom"),
            (StatusCode::INTERNAL_SERVER_ERROR, "db error: boom".to_string())
        );
        assert_eq!(
            access_err(crate::error::AppError::Forbidden),
            (StatusCode::FORBIDDEN, "forbidden".to_string())
        );
        assert_eq!(
            access_err(crate::error::AppError::NotFound("gone".to_string())),
            (StatusCode::NOT_FOUND, "gone".to_string())
        );
        assert_eq!(
            access_err(crate::error::AppError::failure("bad")),
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
        );
    }

    #[test]
    fn avatar_initials_use_name_initials_or_default() {
        assert_eq!(avatar_initials("Ada Lovelace"), "AL");
        assert_eq!(avatar_initials("single"), "S");
        assert_eq!(avatar_initials("   "), "U");
    }

    #[test]
    fn split_crash_attachments_keeps_first_user_text_metadata_and_regular_files() {
        let rows = vec![
            json!({
                "id": "attachments:usertext1",
                "name": "user-text",
                "filename": "user1.txt",
                "storagePath": "user-text/one.txt",
                "createdAt": "2026-01-01T00:00:00Z",
            }),
            json!({
                "id": "attachments:usertext2",
                "name": "user-text",
                "filename": "user2.txt",
                "storagePath": "user-text/two.txt",
                "createdAt": "2026-01-01T00:00:01Z",
            }),
            json!({
                "id": "attachments:minidump",
                "name": "minidump",
                "filename": "crash.dmp",
                "mimeType": "application/octet-stream",
                "size": 10,
                "createdAt": "2026-01-01T00:00:02Z",
            }),
        ];

        let (attachments, user_text) = split_crash_attachments(rows);

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0]["id"], "minidump");
        let user_text = user_text.expect("first user-text attachment should be present");
        assert_eq!(user_text["attachmentId"], "usertext1");
        assert!(user_text.get("body").is_none(), "body must not be eagerly fetched");
    }
}
