// REST API backed by SurrealDB. Returns JSON shapes consumed by
// the SvelteKit http adapter.
//
// Every handler reads the `gr_uid` cookie from the request and generates a
// short-lived JWT for that user so SurrealDB row-level security (RLS) rules
// are enforced on every query.  When no cookie is present, an anonymous JWT
// is used, which grants access only to public data.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::{delete, get, post},
};
use chrono::Utc;
use common::settings::Settings;
use object_store::{ObjectStore, ObjectStoreExt, path::Path as ObjectPath};
use repos::Repo;
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tower_sessions::Session;

#[derive(Clone)]
pub struct DbState {
    pub repo: Repo,
    pub storage: Arc<dyn ObjectStore>,
    pub settings: Arc<Settings>,
}

pub fn router() -> Router<DbState> {
    Router::new()
        .route("/auth/signin", post(signin))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user).post(update_user).delete(delete_user))
        .route("/users/{id}/admin", post(set_admin))
        .route("/users/{id}/memberships", get(memberships_for))
        .route("/products", get(list_products).post(create_product))
        .route("/products/{id}", get(get_product).post(update_product).delete(delete_product))
        .route("/products/{pid}/members", get(list_members))
        .route("/products/{pid}/members/{uid}", post(grant_access).delete(revoke_access))
        .route("/crashes", get(list_groups))
        .route("/crashes/{id}", get(get_group))
        .route("/crashes/by-crash/{crash_id}", get(get_crash))
        .route("/attachments/{id}/download", get(download_attachment))
        .route("/crashes/{id}/status", post(set_status))
        .route("/crashes/{id}/notes", post(add_note))
        .route("/crashes/{id}/merge", post(merge_groups))
        .route("/products/{pid}/symbols", get(list_symbols).post(upload_symbol))
        .route("/symbols/{id}", delete(delete_symbol))
        .route("/products/{pid}/api-tokens", get(list_api_tokens).post(create_api_token))
        .route("/products/{pid}/api-tokens/{id}", delete(delete_api_token))
        .route("/api-tokens", get(list_all_api_tokens).post(create_admin_api_token))
        .route("/api-tokens/{id}", delete(delete_admin_api_token))
}

// --------------------------------------------------------------------
// per-request authenticated DB
// --------------------------------------------------------------------

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (k, v) = part.trim().split_once('=')?;
        if k.trim() == name {
            Some(v.trim().to_string())
        } else {
            None
        }
    })
}

impl DbState {
    /// Returns a SurrealDB handle authenticated as the user identified by the
    /// `gr_uid` request cookie.  Falls back to an anonymous JWT (public data
    /// only) when the cookie is absent or the user cannot be found.
    pub async fn user_db(&self, headers: &HeaderMap) -> Surreal<Any> {
        let Some(uid) = extract_cookie(headers, "gr_uid") else {
            return self.anon_db().await;
        };

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

        let Ok(jwt) = crate::jwt::make_jwt(username, user_id.as_deref(), is_admin, &self.settings)
        else {
            tracing::error!("JWT generation failed for user {uid}");
            return self.anon_db().await;
        };

        match self.repo.authenticated(&jwt).await {
            Ok(db) => db,
            Err(e) => {
                tracing::error!("DB authentication failed for user {uid}: {e}");
                self.anon_db().await
            }
        }
    }

    async fn anon_db(&self) -> Surreal<Any> {
        let Ok(jwt) = crate::jwt::make_anon_jwt(&self.settings) else {
            return self.repo.db.clone();
        };
        match self.repo.authenticated(&jwt).await {
            Ok(db) => db,
            Err(_) => self.repo.db.clone(),
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
    "meta::id(id) AS id, email, name, avatar, is_admin AS isAdmin, created_at AS joinedAt";
const PRODUCT_PROJ: &str = "meta::id(id) AS id, name, slug, description, color, public";
const SYMBOL_PROJ: &str = "meta::id(id) AS id, meta::id(product_id) AS productId, \
    module_id AS name, '' AS version, arch, 'Breakpad' AS format, '' AS size, \
    build_id AS debugId, '' AS codeId, created_at AS uploadedAt, '' AS uploadedBy, 0 AS referencedBy";

const GROUP_BASE_SELECT: &str = "
    SELECT
        meta::id(id)         AS id,
        meta::id(product_id) AS productId,
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
            if !obj.contains_key(key) {
                if let Some(val) = rep_obj.get(key) {
                    obj.insert(key.into(), val.clone());
                }
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

async fn load_user_text(
    storage: &dyn ObjectStore,
    attachment: &Value,
) -> Result<Option<Value>, (StatusCode, String)> {
    let Some(storage_path) = attachment.get("storagePath").and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let object = storage
        .get(&ObjectPath::from(storage_path))
        .await
        .map_err(storage_error)?;
    let bytes = object.bytes().await.map_err(storage_error)?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    Ok(Some(json!({
        "attachmentId": attachment.get("id").and_then(|v| v.as_str()).map(extract_short_id).unwrap_or_default(),
        "filename": attachment_filename(
            attachment.get("name").and_then(|v| v.as_str()),
            attachment.get("filename").and_then(|v| v.as_str())
        ),
        "createdAt": attachment.get("createdAt").cloned().unwrap_or(Value::Null),
        "body": body,
    })))
}

async fn split_crash_attachments(
    storage: &dyn ObjectStore,
    rows: Vec<Value>,
) -> Result<(Vec<Value>, Option<Value>), (StatusCode, String)> {
    let mut attachments = Vec::new();
    let mut user_text = None;

    for row in rows {
        let name = row.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        if name == "user-text" {
            if user_text.is_none() {
                user_text = load_user_text(storage, &row).await?;
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

    Ok((attachments, user_text))
}

// --------------------------------------------------------------------
// auth / users
// --------------------------------------------------------------------

#[derive(Deserialize)]
struct SignInBody {
    email: String,
}

async fn signin(
    State(s): State<DbState>,
    headers: HeaderMap,
    Json(body): Json<SignInBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
    let email = body.email.trim().to_lowercase();
    let rows = run_value(
        &db,
        &format!("SELECT {USER_PROJ} FROM users WHERE string::lowercase(email) = $email LIMIT 1"),
        vec![("email", Value::String(email))],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "user not found".into()))
}

async fn list_users(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
    let rows =
        run_value(&db, &format!("SELECT {USER_PROJ} FROM users ORDER BY created_at"), vec![])
            .await?;
    Ok(Json(Value::Array(rows)))
}

async fn get_user(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
struct CreateUserBody {
    email: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct UpdateUserBody {
    email: Option<String>,
    name: Option<String>,
}

async fn create_user(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<CreateUserBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
    let email = body.email.trim().to_lowercase();
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
    let avatar: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    let rows = run_value(
        &db,
        &format!(
            "CREATE type::record('users', $id) CONTENT {{
            username: $email, email: $email, name: $name, avatar: $avatar,
            is_admin: false, created_at: time::now(), updated_at: time::now()
        }} RETURN {USER_PROJ}"
        ),
        vec![
            ("id", Value::String(format!("u-{slug}"))),
            ("email", Value::String(email)),
            ("name", Value::String(name)),
            (
                "avatar",
                Value::String(if avatar.is_empty() {
                    "U".into()
                } else {
                    avatar
                }),
            ),
        ],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| bad("create failed"))
}

async fn update_user(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    let avatar: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();

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
            (
                "avatar",
                Value::String(if avatar.is_empty() {
                    "U".into()
                } else {
                    avatar
                }),
            ),
        ],
    )
    .await?;

    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| not_found(&id))
}

async fn delete_user(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SetAdminBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
    run_value(
        &db,
        "UPDATE type::record('users', $id) SET is_admin = $v, updated_at = time::now()",
        vec![("id", Value::String(id)), ("v", Value::Bool(body.is_admin))],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn memberships_for(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_session(&session).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    headers: HeaderMap,
    Query(q): Query<ListProductsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
    match q.scope.as_deref() {
        Some("mine") => {
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
                &db,
                &format!("SELECT {PRODUCT_PROJ} FROM products WHERE public = true ORDER BY name"),
                vec![],
            )
            .await?;
            Ok(Json(Value::Array(rows)))
        }
        _ => {
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
    State(s): State<DbState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
    let rows = run_value(
        &db,
        &format!("SELECT {PRODUCT_PROJ} FROM ONLY type::record('products', $id)"),
        vec![("id", Value::String(id.clone()))],
    )
    .await?;
    rows.into_iter()
        .next()
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<CreateProductBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    let rows = run_value(
        &db,
        &format!(
            "CREATE type::record('products', $id) CONTENT {{
            name: $name, slug: $slug, description: $description,
            color: '#6b7280', public: false, accepting_crashes: true, metadata: {{}}
        }} RETURN {PRODUCT_PROJ}"
        ),
        vec![
            ("id", Value::String(slug.clone())),
            ("name", Value::String(body.name)),
            ("slug", Value::String(slug)),
            ("description", Value::String(body.description.unwrap_or_default())),
        ],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| bad("create failed"))
}

async fn update_product(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateProductBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &id)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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

// --------------------------------------------------------------------
// memberships
// --------------------------------------------------------------------

async fn list_members(
    State(s): State<DbState>,
    headers: HeaderMap,
    Path(pid): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path((pid, uid)): Path<(String, String)>,
    Json(body): Json<GrantBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path((pid, uid)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    headers: HeaderMap,
    Query(q): Query<ListGroupsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
    let base_sql = format!(
        "{GROUP_BASE_SELECT}
        WHERE product_id = type::record('products', $pid)
        ORDER BY count DESC"
    );
    let reps_sql = "
        SELECT
            group_id,
            created_at,
            report.title             AS title,
            report.topFrame          AS topFrame,
            report.file              AS file,
            report.line              AS line,
            report.version           AS version,
            report.build             AS build,
            report.address           AS address,
            report.platform          AS platform,
            report.exceptionType     AS exceptionType,
            report.exceptionTypeShort AS exceptionTypeShort,
            report.similarity        AS similarity
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
    let mut versions_set = std::collections::BTreeSet::new();
    for r in rep_rows {
        if let Some(v) = r.get("version").and_then(|v| v.as_str()) {
            if !v.is_empty() {
                versions_set.insert(v.to_string());
            }
        }
        let Some(gid_raw) = r.get("group_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let gid = extract_short_id(gid_raw);
        if reps.contains_key(&gid) {
            continue;
        }
        reps.insert(gid, r);
    }
    let versions_list: Vec<String> = versions_set.into_iter().rev().collect();

    let mut groups: Vec<Value> = base
        .into_iter()
        .map(|g| {
            let gid = g
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            apply_rep(g, reps.get(&gid))
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
    State(s): State<DbState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
    let g = compose_group(&db, &id).await?;
    match g {
        Some(v) => Ok(Json(v)),
        None => Err(not_found(&id)),
    }
}

async fn get_crash(
    State(s): State<DbState>,
    headers: HeaderMap,
    Path(crash_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
    let crashes = run_value(
        &db,
        "SELECT * FROM ONLY type::record('crashes', $cid)",
        vec![("cid", Value::String(crash_id.clone()))],
    )
    .await?;
    let Some(row) = crashes.into_iter().next() else {
        return Err(not_found(&crash_id));
    };
    let attachment_rows = load_attachment_rows(&db, &crash_id).await?;
    let (attachments, user_text) =
        split_crash_attachments(s.storage.as_ref(), attachment_rows).await?;
    let crash_value = hydrate_crash(&row, attachments, user_text);
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
            report.title             AS title,
            report.topFrame          AS topFrame,
            report.file              AS file,
            report.line              AS line,
            report.address           AS address,
            report.platform          AS platform,
            report.version           AS version,
            report.build             AS build,
            report.exceptionType     AS exceptionType,
            report.exceptionTypeShort AS exceptionTypeShort,
            report.similarity        AS similarity
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
    group_obj.insert("crashes".into(), Value::Array(crash_rows));

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
    State(s): State<DbState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
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
    let Some(row) = rows.into_iter().next() else {
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SetStatusBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_session(&session).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<AddNoteBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_session(&session).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(primary_id): Path<String>,
    Json(body): Json<MergeBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_session(&session).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
    let pid = Value::String(primary_id.clone());
    let mid = Value::String(body.merged_id);
    run_value(
        &db,
        "UPDATE crashes SET group_id = type::record('crash_groups', $pid)
         WHERE group_id = type::record('crash_groups', $mid)",
        vec![("pid", pid.clone()), ("mid", mid.clone())],
    )
    .await?;
    run_value(
        &db,
        "UPDATE type::record('crash_groups', $pid) SET
           count = count + (SELECT VALUE count FROM ONLY type::record('crash_groups', $mid)),
           updated_at = time::now()",
        vec![("pid", pid), ("mid", mid.clone())],
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
    State(s): State<DbState>,
    headers: HeaderMap,
    Path(pid): Path<String>,
    Query(q): Query<SymbolsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(pid): Path<String>,
    Json(body): Json<UploadSymbolBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    // No pid in path — require a session; RLS enforces product-level access.
    crate::access::require_session(&session).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
    run_value(&db, "DELETE type::record('symbols', $id)", vec![("id", Value::String(id))]).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------
// API tokens
// --------------------------------------------------------------------

const API_TOKEN_PROJ: &str = "meta::id(id) AS id, description, entitlements, is_active, last_used_at, expires_at, created_at";

async fn list_api_tokens(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(pid): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&headers).await;
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(pid): Path<String>,
    Json(body): Json<CreateApiTokenBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&headers).await;

    let description = body.description.trim().to_string();
    if description.is_empty() {
        return Err(bad("description required"));
    }
    let entitlements = body
        .entitlements
        .unwrap_or_else(|| vec!["symbol-upload".into(), "minidump-upload".into()]);

    let (token_id, token, token_hash) = common::token::generate_api_token()
        .map_err(|e| server_error(format!("token generation failed: {e}")))?;

    let id = uuid::Uuid::new_v4().to_string();
    run_value(
        &db,
        "CREATE type::record('api_tokens', $id) CONTENT {
            description: $description,
            token_id: $token_id,
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
            ("entitlements", Value::Array(entitlements.into_iter().map(Value::String).collect())),
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
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path((pid, id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_product_maintainer(&session, &headers, &s.repo.db, &pid)
        .await
        .map_err(access_err)?;
    let db = s.user_db(&headers).await;
    let _ = pid;
    run_value(&db, "DELETE type::record('api_tokens', $id)", vec![("id", Value::String(id))])
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------
// Admin API tokens (product-optional)
// --------------------------------------------------------------------

async fn list_all_api_tokens(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
    let rows = run_value(
        &db,
        &format!(
            "SELECT {API_TOKEN_PROJ}, \
             meta::id(product_id) AS productId, \
             product_id.name AS productName \
             FROM api_tokens \
             ORDER BY created_at DESC"
        ),
        vec![],
    )
    .await?;
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct CreateAdminApiTokenBody {
    description: String,
    entitlements: Option<Vec<String>>,
    #[serde(rename = "productId")]
    product_id: Option<String>,
}

async fn create_admin_api_token(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<CreateAdminApiTokenBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;

    let description = body.description.trim().to_string();
    if description.is_empty() {
        return Err(bad("description required"));
    }
    let entitlements = body
        .entitlements
        .unwrap_or_else(|| vec!["symbol-upload".into(), "minidump-upload".into()]);

    let (token_id, token, token_hash) = common::token::generate_api_token()
        .map_err(|e| server_error(format!("token generation failed: {e}")))?;

    let id = uuid::Uuid::new_v4().to_string();

    run_value(
        &db,
        "CREATE type::record('api_tokens', $id) CONTENT {
            description: $description,
            token_id: $token_id,
            token_hash: $token_hash,
            product_id: IF $pid IS NOT NONE THEN type::record('products', $pid) ELSE NONE END,
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
            ("pid", body.product_id.map(Value::String).unwrap_or(Value::Null)),
            ("entitlements", Value::Array(entitlements.into_iter().map(Value::String).collect())),
        ],
    )
    .await?;

    Ok(Json(json!({
        "id": id,
        "description": description,
        "token": token,
    })))
}

async fn delete_admin_api_token(
    State(s): State<DbState>,
    session: Session,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::access::require_admin(&session, &headers, &s.repo.db).await.map_err(access_err)?;
    let db = s.user_db(&headers).await;
    run_value(&db, "DELETE type::record('api_tokens', $id)", vec![("id", Value::String(id))])
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
