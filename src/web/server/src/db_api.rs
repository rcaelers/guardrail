// REST API backed by SurrealDB. Returns the same JSON shapes as
// `mock_api.rs` so the SvelteKit http adapter can point at either.
//
// Data layout assumptions (match database/schema/guardrail.surql +
// src/bin/import_mock.rs):
//   - users.id          = `users:⟨u-…⟩`       (string-based record id)
//   - products.id       = `products:⟨slug⟩`
//   - crash_groups.id   = `crash_groups:⟨GR-####⟩`
//   - crashes.id        = `crashes:⟨CR-####-###⟩`
//   - symbols.id        = `symbols:⟨SYM-####⟩`
//   - user_access       = { user_id, product_id, role } links
//   - annotations       = { source, value, author?, group_id?, crash_id?, product_id }
//
// Every query uses `meta::id(id) AS id` so the UI receives bare string ids
// (not `users:⟨u-you⟩`).

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Clone)]
pub struct DbState {
    pub db: Arc<Surreal<Any>>,
}

pub fn router() -> Router<DbState> {
    Router::new()
        .route("/auth/signin", post(signin))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user).delete(delete_user))
        .route("/users/{id}/admin", post(set_admin))
        .route("/users/{id}/memberships", get(memberships_for))
        .route("/products", get(list_products).post(create_product))
        .route("/products/{id}", get(get_product).delete(delete_product))
        .route("/products/{pid}/members", get(list_members))
        .route(
            "/products/{pid}/members/{uid}",
            post(grant_access).delete(revoke_access),
        )
        .route("/crashes", get(list_groups))
        .route("/crashes/{id}", get(get_group))
        .route("/crashes/by-crash/{crash_id}", get(get_crash))
        .route("/crashes/{id}/status", post(set_status))
        .route("/crashes/{id}/notes", post(add_note))
        .route("/crashes/{id}/merge", post(merge_groups))
        .route(
            "/products/{pid}/symbols",
            get(list_symbols).post(upload_symbol),
        )
        .route("/symbols/{id}", delete(delete_symbol))
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

// Runs a query and decodes the first result as JSON. Going through
// `Option<Vec<Value>>` matches how the rest of the codebase bridges
// SurrealDB's typed response into serde_json — the client only implements
// SurrealValue for a handful of types, but serde_json::Value is one of them.
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

// Projection strings shared across endpoints. Keeps the camelCase field
// names the UI expects stable in one place.
const USER_PROJ: &str = "meta::id(id) AS id, email, name, avatar, is_admin AS isAdmin, created_at AS joinedAt";
const PRODUCT_PROJ: &str = "meta::id(id) AS id, name, slug, description, color";
const SYMBOL_PROJ: &str = "external_id AS id, meta::id(product_id) AS productId, \
    name, version, arch, format, size, debug_id AS debugId, code_id AS codeId, \
    uploaded_at AS uploadedAt, meta::id(uploaded_by) AS uploadedBy, referenced_by AS referencedBy";

// Base columns from `crash_groups`. Display-only fields
// (title/topFrame/file/line/version/...) come from a representative crash's
// `report` — we fetch those separately and merge in Rust because
// correlated sub-SELECTs inside SurrealDB's projection with ORDER BY don't
// parse cleanly.
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

// SurrealDB returns record links as e.g. `crash_groups:⟨GR-0001⟩` or
// `crash_groups:GR-0001`. Strip the `<table>:` prefix and the decorative
// brackets so we get the plain short id the UI wants.
fn extract_short_id(s: &str) -> String {
    let after = s.split_once(':').map(|(_, r)| r).unwrap_or(s);
    after.trim_matches(|c: char| c == '⟨' || c == '⟩' || c == '`')
        .to_string()
}

// Merge report-derived display fields into a group summary row.
fn apply_rep(mut group: Value, rep: Option<&Value>) -> Value {
    if let (Some(obj), Some(rep_obj)) = (group.as_object_mut(), rep.and_then(|v| v.as_object())) {
        for key in [
            "title", "topFrame", "file", "line", "address", "platform",
            "version", "build", "exceptionType", "exceptionTypeShort",
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

// --------------------------------------------------------------------
// auth / users
// --------------------------------------------------------------------

#[derive(Deserialize)]
struct SignInBody { email: String }

async fn signin(
    State(s): State<DbState>,
    Json(body): Json<SignInBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let email = body.email.trim().to_lowercase();
    let rows = run_value(
        &s.db,
        &format!("SELECT {USER_PROJ} FROM users WHERE string::lowercase(email) = $email LIMIT 1"),
        vec![("email", Value::String(email))],
    )
    .await?;
    rows.into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "user not found".into()))
}

async fn list_users(State(s): State<DbState>) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = run_value(
        &s.db,
        &format!("SELECT {USER_PROJ} FROM users ORDER BY created_at"),
        vec![],
    )
    .await?;
    Ok(Json(Value::Array(rows)))
}

async fn get_user(
    State(s): State<DbState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = run_value(
        &s.db,
        &format!("SELECT {USER_PROJ} FROM ONLY type::record('users', $id)"),
        vec![("id", Value::String(id.clone()))],
    )
    .await?;
    rows.into_iter().next().map(Json).ok_or_else(|| not_found(&id))
}

#[derive(Deserialize)]
struct CreateUserBody { email: String, name: Option<String> }

async fn create_user(
    State(s): State<DbState>,
    Json(body): Json<CreateUserBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let email = body.email.trim().to_lowercase();
    let name = body.name.as_deref().map(str::trim).filter(|x| !x.is_empty())
        .unwrap_or(&email)
        .to_string();
    let slug: String = email.split('@').next().unwrap_or("u")
        .chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let avatar: String = name.split_whitespace()
        .filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
    let rows = run_value(
        &s.db,
        &format!("CREATE type::record('users', $id) CONTENT {{
            username: $email, email: $email, name: $name, avatar: $avatar,
            is_admin: false, created_at: time::now()
        }} RETURN {USER_PROJ}"),
        vec![
            ("id", Value::String(format!("u-{slug}"))),
            ("email", Value::String(email)),
            ("name", Value::String(name)),
            ("avatar", Value::String(if avatar.is_empty() { "U".into() } else { avatar })),
        ],
    ).await?;
    rows.into_iter().next().map(Json).ok_or_else(|| bad("create failed"))
}

async fn delete_user(
    State(s): State<DbState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_value(&s.db, "DELETE user_access WHERE user_id = type::record('users', $id)",
        vec![("id", Value::String(id.clone()))]).await?;
    run_value(&s.db, "DELETE type::record('users', $id)",
        vec![("id", Value::String(id))]).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SetAdminBody { #[serde(rename = "isAdmin")] is_admin: bool }

async fn set_admin(
    State(s): State<DbState>,
    Path(id): Path<String>,
    Json(body): Json<SetAdminBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_value(&s.db,
        "UPDATE type::record('users', $id) SET is_admin = $v, updated_at = time::now()",
        vec![
            ("id", Value::String(id)),
            ("v", Value::Bool(body.is_admin)),
        ]).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn memberships_for(
    State(s): State<DbState>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Build the joined product object via field access on the record link
    // (`product_id.name` etc.). SurrealDB lets us dereference record links
    // inline this way — a sub-SELECT like `FROM ONLY product_id` instead
    // parses `product_id` as a table name and fails.
    let rows = run_value(
        &s.db,
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
    ).await?;
    Ok(Json(Value::Array(rows)))
}

// --------------------------------------------------------------------
// products
// --------------------------------------------------------------------

#[derive(Deserialize)]
struct ListProductsQuery { scope: Option<String>, user: Option<String> }

async fn list_products(
    State(s): State<DbState>,
    Query(q): Query<ListProductsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if q.scope.as_deref() == Some("mine") {
        let Some(uid) = q.user.as_deref() else {
            return Ok(Json(Value::Array(vec![])));
        };
        let rows = run_value(
            &s.db,
            &format!("SELECT {PRODUCT_PROJ} FROM products
                WHERE id IN (SELECT VALUE product_id FROM user_access
                    WHERE user_id = type::record('users', $uid))"),
            vec![("uid", Value::String(uid.into()))],
        ).await?;
        return Ok(Json(Value::Array(rows)));
    }
    let rows = run_value(&s.db,
        &format!("SELECT {PRODUCT_PROJ} FROM products ORDER BY name"),
        vec![]).await?;
    Ok(Json(Value::Array(rows)))
}

async fn get_product(
    State(s): State<DbState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = run_value(&s.db,
        &format!("SELECT {PRODUCT_PROJ} FROM ONLY type::record('products', $id)"),
        vec![("id", Value::String(id.clone()))]).await?;
    rows.into_iter().next().map(Json).ok_or_else(|| not_found(&id))
}

#[derive(Deserialize)]
struct CreateProductBody {
    name: String,
    slug: Option<String>,
    description: Option<String>,
}

async fn create_product(
    State(s): State<DbState>,
    Json(body): Json<CreateProductBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let slug = body.slug.as_deref().map(str::trim).filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| body.name.to_lowercase()
            .chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>().trim_matches('-').to_string());
    let rows = run_value(&s.db,
        &format!("CREATE type::record('products', $id) CONTENT {{
            name: $name, slug: $slug, description: $description,
            color: '#6b7280', accepting_crashes: true, metadata: {{}}
        }} RETURN {PRODUCT_PROJ}"),
        vec![
            ("id", Value::String(slug.clone())),
            ("name", Value::String(body.name)),
            ("slug", Value::String(slug)),
            ("description", Value::String(body.description.unwrap_or_default())),
        ]).await?;
    rows.into_iter().next().map(Json).ok_or_else(|| bad("create failed"))
}

async fn delete_product(
    State(s): State<DbState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let args = vec![("id", Value::String(id))];
    // FK-safe cascade
    for sql in [
        "DELETE annotations WHERE product_id = type::record('products', $id)",
        "DELETE crashes     WHERE product_id = type::record('products', $id)",
        "DELETE crash_groups WHERE product_id = type::record('products', $id)",
        "DELETE symbols     WHERE product_id = type::record('products', $id)",
        "DELETE user_access WHERE product_id = type::record('products', $id)",
        "DELETE type::record('products', $id)",
    ] {
        run_value(&s.db, sql, args.clone()).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------
// memberships
// --------------------------------------------------------------------

async fn list_members(
    State(s): State<DbState>,
    Path(pid): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = run_value(&s.db,
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
        vec![("pid", Value::String(pid))]).await?;
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct GrantBody { role: String }

async fn grant_access(
    State(s): State<DbState>,
    Path((pid, uid)): Path<(String, String)>,
    Json(body): Json<GrantBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Upsert (role change vs grant): delete existing row first, then create.
    run_value(&s.db,
        "DELETE user_access WHERE user_id = type::record('users', $uid)
                              AND product_id = type::record('products', $pid)",
        vec![
            ("uid", Value::String(uid.clone())),
            ("pid", Value::String(pid.clone())),
        ]).await?;
    run_value(&s.db,
        "CREATE user_access CONTENT {
            user_id: type::record('users', $uid),
            product_id: type::record('products', $pid),
            role: $role
        }",
        vec![
            ("uid", Value::String(uid)),
            ("pid", Value::String(pid)),
            ("role", Value::String(body.role)),
        ]).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_access(
    State(s): State<DbState>,
    Path((pid, uid)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_value(&s.db,
        "DELETE user_access WHERE user_id = type::record('users', $uid)
                              AND product_id = type::record('products', $pid)",
        vec![
            ("uid", Value::String(uid)),
            ("pid", Value::String(pid)),
        ]).await?;
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
    Query(q): Query<ListGroupsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Two parallel scans: one for the group rows, one for every crash's
    // lean display fields. We take the first crash per group_id out of
    // the scan to build the per-group representative, and the distinct
    // `report.version` set powers the filter dropdown — same scan.
    //
    // (An alternative correlated sub-SELECT per group was ~3× slower on
    // this dataset because SurrealDB re-scans `crashes` per group.)
    let base_sql = format!("{GROUP_BASE_SELECT}
        WHERE product_id = type::record('products', $pid)
        ORDER BY count DESC");
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
        run_value(&s.db, &base_sql,
            vec![("pid", Value::String(q.product_id.clone()))]),
        run_value(&s.db, reps_sql,
            vec![("pid", Value::String(q.product_id.clone()))]),
    );
    let base = base_res?;
    let rep_rows = reps_res?;

    let mut reps: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let mut versions_set = std::collections::BTreeSet::new();
    for r in rep_rows {
        if let Some(v) = r.get("version").and_then(|v| v.as_str()) {
            if !v.is_empty() { versions_set.insert(v.to_string()); }
        }
        let Some(gid_raw) = r.get("group_id").and_then(|v| v.as_str()) else { continue; };
        let gid = extract_short_id(gid_raw);
        if reps.contains_key(&gid) { continue; } // first (most recent) wins
        reps.insert(gid, r);
    }
    let versions_list: Vec<String> = versions_set.into_iter().rev().collect();

    let mut groups: Vec<Value> = base.into_iter().map(|g| {
        let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        apply_rep(g, reps.get(&gid))
    }).collect();

    // Post-filter (keeps the SurrealQL simple; filter sets are small).
    if let Some(v) = q.version.as_deref().filter(|x| *x != "all" && !x.is_empty()) {
        groups.retain(|g| g.get("version").and_then(|v| v.as_str()) == Some(v));
    }
    if let Some(st) = q.status.as_deref().filter(|x| *x != "all" && !x.is_empty()) {
        groups.retain(|g| g.get("status").and_then(|v| v.as_str()) == Some(st));
    }
    if let Some(search) = q.search.as_deref().filter(|s| !s.trim().is_empty()) {
        let needle = search.to_lowercase();
        groups.retain(|g| {
            let t = g.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_lowercase();
            let f = g.get("topFrame").and_then(|v| v.as_str()).unwrap_or_default().to_lowercase();
            t.contains(&needle) || f.contains(&needle)
        });
    }

    match q.sort.as_deref() {
        Some("recent") => groups.sort_by(|a, b| {
            b.get("lastSeen").and_then(|v| v.as_str())
                .cmp(&a.get("lastSeen").and_then(|v| v.as_str()))
        }),
        Some("similarity") => groups.sort_by(|a, b| {
            b.get("similarity").and_then(|v| v.as_f64())
                .partial_cmp(&a.get("similarity").and_then(|v| v.as_f64()))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        Some("version") => groups.sort_by(|a, b| {
            b.get("version").and_then(|v| v.as_str())
                .cmp(&a.get("version").and_then(|v| v.as_str()))
        }),
        _ => {} // default "count" already sorted by the SurrealQL
    }

    let total = groups.len();
    let off = q.offset.unwrap_or(0);
    let lim = q.limit.unwrap_or(groups.len());
    let slice: Vec<Value> = groups.into_iter().skip(off).take(lim).collect();

    // Versions dropdown: reuse the same scan that built `reps`.
    let versions: Vec<Value> = versions_list.into_iter().map(Value::String).collect();

    Ok(Json(json!({
        "groups": slice,
        "total": total,
        "versions": versions,
    })))
}

async fn get_group(
    State(s): State<DbState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let g = compose_group(&s.db, &id).await?;
    match g {
        Some(v) => Ok(Json(v)),
        None => Err(not_found(&id)),
    }
}

async fn get_crash(
    State(s): State<DbState>,
    Path(crash_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Fetch the crash row, then its group.
    let crashes = run_value(&s.db,
        "SELECT * FROM ONLY type::record('crashes', $cid)",
        vec![("cid", Value::String(crash_id.clone()))]).await?;
    let Some(row) = crashes.into_iter().next() else { return Err(not_found(&crash_id)); };
    let crash_value = hydrate_crash(&row);
    let group_id = row.get("group_id").and_then(|v| v.as_str())
        .map(extract_short_id).unwrap_or_default();
    let Some(group) = compose_group(&s.db, &group_id).await? else {
        return Err(not_found(&crash_id));
    };
    Ok(Json(json!({ "crash": crash_value, "group": group })))
}

// Materialize a Crash (in the UI shape) from a DB row: the per-crash
// metadata lives as top-level columns, but all the detail blobs (stack,
// threads, modules, env, breadcrumbs, logs, userDescription, dump, derived,
// plus title/topFrame/...) are nested inside `report`. SurrealDB returns
// `id`, `group_id`, and `product_id` as record links; we strip the prefix
// so the UI sees plain short ids.
fn hydrate_crash(row: &Value) -> Value {
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
        for (k, v) in report { out.insert(k.clone(), v.clone()); }
    }
    Value::Object(out)
}

// Return the full group including crashes[], notes, related.
async fn compose_group(
    db: &Surreal<Any>,
    id: &str,
) -> Result<Option<Value>, (StatusCode, String)> {
    let rows = run_value(db,
        &format!("{GROUP_BASE_SELECT} WHERE meta::id(id) = $id LIMIT 1"),
        vec![("id", Value::String(id.into()))]).await?;
    let Some(base) = rows.into_iter().next() else { return Ok(None); };

    // Fetch only the display-relevant fields from the most-recent crash
    // (not the whole `report` blob — that's KB per row).
    let rep_rows = run_value(db,
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
        vec![("gid", Value::String(id.into()))]).await?;
    let rep = rep_rows.into_iter().next();
    let mut group = apply_rep(base, rep.as_ref());
    let group_obj = group.as_object_mut().unwrap();

    // Lightweight list of crashes in this group. We return only the
    // fields the expanded GroupRow + detail header read (id/os/version/
    // at/user/similarity/commit + title/topFrame/file/line/signal/etc.).
    // The full per-crash blob (stack/threads/modules/dump/…) is loaded on
    // demand by GET /crashes/by-crash/:id when the user selects a crash.
    let crash_rows = run_value(db,
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
        vec![("gid", Value::String(id.into()))]).await?;
    group_obj.insert("crashes".into(), Value::Array(crash_rows));

    // notes = annotations with source=user on this group
    let notes = run_value(db,
        "SELECT author, value AS body, created_at AS at
         FROM annotations
         WHERE source = 'user' AND group_id = type::record('crash_groups', $gid)
         ORDER BY created_at",
        vec![("gid", Value::String(id.into()))]).await?;
    group_obj.insert("notes".into(), Value::Array(notes));

    // related = other groups in this product with the same signal.
    // We only need {id, title, count} — do a single narrow query that
    // joins to one representative crash per group, instead of scanning
    // every crash in the product.
    let product_id = group_obj.get("productId").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let signal = group_obj.get("signal").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let related_base = run_value(db,
        "SELECT meta::id(id) AS id, count FROM crash_groups
         WHERE product_id = type::record('products', $pid)
           AND signal = $signal
           AND meta::id(id) != $gid
         ORDER BY count DESC LIMIT 3",
        vec![
            ("pid", Value::String(product_id.clone())),
            ("signal", Value::String(signal)),
            ("gid", Value::String(id.into())),
        ]).await?;
    let mut related: Vec<Value> = Vec::with_capacity(related_base.len());
    for g in related_base {
        let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let count = g.get("count").cloned().unwrap_or(Value::Null);
        // One tiny query per related group for the title.
        let title_rows = run_value(db,
            "SELECT created_at, report.title AS title FROM crashes
             WHERE group_id = type::record('crash_groups', $gid)
             ORDER BY created_at DESC LIMIT 1",
            vec![("gid", Value::String(gid.clone()))]).await?;
        let title = title_rows.into_iter().next()
            .and_then(|r| r.get("title").cloned())
            .unwrap_or(Value::Null);
        related.push(json!({ "id": gid, "title": title, "count": count }));
    }
    group_obj.insert("related".into(), Value::Array(related));

    Ok(Some(group))
}

#[derive(Deserialize)]
struct SetStatusBody { status: String }

async fn set_status(
    State(s): State<DbState>,
    Path(id): Path<String>,
    Json(body): Json<SetStatusBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_value(&s.db,
        "UPDATE type::record('crash_groups', $id) SET status = $st, updated_at = time::now()",
        vec![
            ("id", Value::String(id)),
            ("st", Value::String(body.status)),
        ]).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AddNoteBody { body: String, author: String }

async fn add_note(
    State(s): State<DbState>,
    Path(id): Path<String>,
    Json(payload): Json<AddNoteBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // look up product_id for FK on the annotation
    let prod_rows = run_value(&s.db,
        "SELECT meta::id(product_id) AS pid FROM ONLY type::record('crash_groups', $id)",
        vec![("id", Value::String(id.clone()))]).await?;
    let pid = prod_rows.into_iter().next()
        .and_then(|r| r.get("pid").and_then(|v| v.as_str()).map(String::from))
        .ok_or_else(|| not_found(&id))?;

    let now = Utc::now().to_rfc3339();
    run_value(&s.db,
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
        ]).await?;

    Ok(Json(json!({
        "author": payload.author,
        "body": payload.body,
        "at": now,
    })))
}

#[derive(Deserialize)]
struct MergeBody { #[serde(rename = "mergedId")] merged_id: String }

async fn merge_groups(
    State(s): State<DbState>,
    Path(primary_id): Path<String>,
    Json(body): Json<MergeBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Move crashes, add counts, delete merged group.
    let pid = Value::String(primary_id.clone());
    let mid = Value::String(body.merged_id);
    run_value(&s.db,
        "UPDATE crashes SET group_id = type::record('crash_groups', $pid)
         WHERE group_id = type::record('crash_groups', $mid)",
        vec![("pid", pid.clone()), ("mid", mid.clone())]).await?;
    run_value(&s.db,
        "UPDATE type::record('crash_groups', $pid) SET
           count = count + (SELECT VALUE count FROM ONLY type::record('crash_groups', $mid)),
           updated_at = time::now()",
        vec![("pid", pid), ("mid", mid.clone())]).await?;
    run_value(&s.db, "DELETE type::record('crash_groups', $mid)",
        vec![("mid", mid)]).await?;
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
    Path(pid): Path<String>,
    Query(q): Query<SymbolsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let mut rows = run_value(&s.db,
        &format!("SELECT {SYMBOL_PROJ} FROM symbols
                  WHERE product_id = type::record('products', $pid)"),
        vec![("pid", Value::String(pid))]).await?;

    if let Some(search) = q.search.as_deref().filter(|s| !s.trim().is_empty()) {
        let needle = search.to_lowercase();
        rows.retain(|r| {
            let n = r.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_lowercase();
            let d = r.get("debugId").and_then(|v| v.as_str()).unwrap_or_default().to_lowercase();
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
                b.get("version").and_then(|v| v.as_str())
                    .cmp(&a.get("version").and_then(|v| v.as_str()))
            })
        }),
        Some("size") => rows.sort_by(|a, b| {
            let parse = |v: &Value| v.get("size").and_then(|x| x.as_str())
                .and_then(|s| s.split_whitespace().next())
                .and_then(|n| n.parse::<f64>().ok()).unwrap_or(0.0);
            parse(b).partial_cmp(&parse(a)).unwrap_or(std::cmp::Ordering::Equal)
        }),
        _ => rows.sort_by(|a, b| {
            b.get("uploadedAt").and_then(|v| v.as_str())
                .cmp(&a.get("uploadedAt").and_then(|v| v.as_str()))
        }),
    }
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct UploadSymbolBody {
    name: String,
    version: Option<String>,
    arch: Option<String>,
    format: Option<String>,
    size: Option<String>,
    #[serde(rename = "uploadedBy")] uploaded_by: String,
}

async fn upload_symbol(
    State(s): State<DbState>,
    Path(pid): Path<String>,
    Json(body): Json<UploadSymbolBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Pick a short id like SYM-0123. Uses a count for uniqueness.
    let count_rows = run_value(&s.db, "SELECT VALUE count() FROM symbols GROUP ALL", vec![]).await?;
    let n = count_rows.into_iter().next().and_then(|v| v.as_u64()).unwrap_or(0) + 1;
    let id = format!("SYM-{:04}", n);

    let rows = run_value(&s.db,
        &format!("CREATE type::record('symbols', $id) CONTENT {{
            external_id: $id,
            product_id: type::record('products', $pid),
            name: $name, version: $version, arch: $arch, format: $format, size: $size,
            debug_id: '', code_id: '',
            uploaded_by: type::record('users', $uploaded_by),
            uploaded_at: time::now(), referenced_by: 0
        }} RETURN {SYMBOL_PROJ}"),
        vec![
            ("id", Value::String(id)),
            ("pid", Value::String(pid)),
            ("name", Value::String(body.name)),
            ("version", Value::String(body.version.unwrap_or_else(|| "0.0.0".into()))),
            ("arch", Value::String(body.arch.unwrap_or_else(|| "x86_64".into()))),
            ("format", Value::String(body.format.unwrap_or_else(|| "PDB".into()))),
            ("size", Value::String(body.size.unwrap_or_else(|| "1.0 MB".into()))),
            ("uploaded_by", Value::String(body.uploaded_by)),
        ]).await?;
    rows.into_iter().next().map(Json).ok_or_else(|| bad("create failed"))
}

async fn delete_symbol(
    State(s): State<DbState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_value(&s.db, "DELETE type::record('symbols', $id)",
        vec![("id", Value::String(id))]).await?;
    Ok(StatusCode::NO_CONTENT)
}
