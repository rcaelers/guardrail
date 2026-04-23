// REST mock that mirrors the TypeScript mock adapter. All data lives in a
// single JSON document loaded from `mock/seed.json`; routes read/mutate
// that document through an `RwLock`.
//
// Endpoint shapes match what `src/web/ui/src/lib/adapters/http.ts` expects.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use chrono::Utc;
use rand::{RngExt, distr::Alphanumeric};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock;

const VERSIONS: &[&str] = &["2.14.0", "2.13.4", "2.13.3", "2.13.2", "2.12.9", "2.12.7"];

#[derive(Clone)]
pub struct MockState {
    data: Arc<RwLock<Value>>,
}

impl MockState {
    pub fn new() -> Self {
        let seed: Value = serde_json::from_str(include_str!("../mock/seed.json"))
            .expect("mock/seed.json is invalid JSON");
        Self {
            data: Arc::new(RwLock::new(seed)),
        }
    }
}

pub fn router() -> Router<MockState> {
    Router::new()
        // auth / users
        .route("/auth/signin", post(signin))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user).delete(delete_user))
        .route("/users/{id}/admin", post(set_admin))
        .route("/users/{id}/memberships", get(memberships_for))
        // products
        .route("/products", get(list_products).post(create_product))
        .route("/products/{id}", get(get_product).delete(delete_product))
        .route("/products/{pid}/members", get(list_members))
        .route(
            "/products/{pid}/members/{uid}",
            post(grant_access).delete(revoke_access),
        )
        // crashes
        .route("/crashes", get(list_groups))
        .route("/crashes/{id}", get(get_group))
        .route("/crashes/by-crash/{crash_id}", get(get_crash))
        .route("/crashes/{id}/status", post(set_status))
        .route("/crashes/{id}/notes", post(add_note))
        .route("/crashes/{id}/merge", post(merge_groups))
        // symbols
        .route(
            "/products/{pid}/symbols",
            get(list_symbols).post(upload_symbol),
        )
        .route("/symbols/{id}", delete(delete_symbol))
}

// ---- helpers ----

fn not_found(what: &str) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("not found: {what}"))
}

fn bad(what: &str) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, what.to_string())
}

fn find_by_id<'a>(arr: &'a [Value], id: &str) -> Option<&'a Value> {
    arr.iter().find(|v| v.get("id").and_then(|x| x.as_str()) == Some(id))
}

fn find_by_id_mut<'a>(arr: &'a mut [Value], id: &str) -> Option<&'a mut Value> {
    arr.iter_mut()
        .find(|v| v.get("id").and_then(|x| x.as_str()) == Some(id))
}

fn summarize(group: &Value) -> Value {
    // CrashGroupSummary strips the deep fields from CrashGroup. Keep the
    // allow-list in sync with the TS `toSummary()`.
    const KEEP: &[&str] = &[
        "id", "productId", "signal", "exceptionType", "exceptionTypeShort",
        "title", "topFrame", "file", "line", "address", "platform", "version",
        "build", "count", "similarity", "status", "assignee", "firstSeen",
        "lastSeen",
    ];
    let mut out = serde_json::Map::new();
    if let Some(obj) = group.as_object() {
        for k in KEEP {
            if let Some(v) = obj.get(*k) {
                out.insert((*k).into(), v.clone());
            }
        }
    }
    Value::Object(out)
}

fn random_hex(n: usize) -> String {
    let mut rng = rand::rng();
    (0..n)
        .map(|_| {
            let v: u8 = rng.random_range(0..16);
            char::from_digit(v as u32, 16).unwrap()
        })
        .collect()
}

fn random_id_suffix() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

// ---- auth / users ----

#[derive(Deserialize)]
struct SignInBody {
    email: String,
}

async fn signin(State(s): State<MockState>, Json(body): Json<SignInBody>) -> impl IntoResponse {
    let data = s.data.read().await;
    let users = data["users"].as_array().cloned().unwrap_or_default();
    let email = body.email.trim().to_lowercase();
    match users.iter().find(|u| {
        u.get("email")
            .and_then(|e| e.as_str())
            .map(|e| e.to_lowercase())
            == Some(email.clone())
    }) {
        Some(u) => Json(u.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn list_users(State(s): State<MockState>) -> Json<Value> {
    let data = s.data.read().await;
    Json(data["users"].clone())
}

async fn get_user(State(s): State<MockState>, Path(id): Path<String>) -> impl IntoResponse {
    let data = s.data.read().await;
    match find_by_id(data["users"].as_array().unwrap_or(&vec![]), &id) {
        Some(u) => Json(u.clone()).into_response(),
        None => not_found(&id).into_response(),
    }
}

#[derive(Deserialize)]
struct CreateUserBody {
    email: String,
    name: Option<String>,
}

async fn create_user(
    State(s): State<MockState>,
    Json(body): Json<CreateUserBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let clean = body.email.trim().to_lowercase();
    let mut data = s.data.write().await;
    let users = data["users"].as_array_mut().ok_or_else(|| bad("users missing"))?;
    if users.iter().any(|u| {
        u.get("email")
            .and_then(|e| e.as_str())
            .map(|e| e.to_lowercase())
            == Some(clean.clone())
    }) {
        return Err(bad(&format!("A user with email \"{clean}\" already exists")));
    }
    let final_name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&body.email)
        .to_string();
    let slug: String = clean
        .split('@')
        .next()
        .unwrap_or("u")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let avatar: String = final_name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    let user = json!({
        "id": format!("u-{slug}"),
        "email": body.email,
        "name": final_name,
        "avatar": if avatar.is_empty() { "U".into() } else { avatar },
        "isAdmin": false,
        "joinedAt": Utc::now().to_rfc3339(),
    });
    users.push(user.clone());
    Ok(Json(user))
}

async fn delete_user(State(s): State<MockState>, Path(id): Path<String>) -> StatusCode {
    let mut data = s.data.write().await;
    if let Some(arr) = data["users"].as_array_mut() {
        arr.retain(|u| u.get("id").and_then(|x| x.as_str()) != Some(&id));
    }
    if let Some(arr) = data["memberships"].as_array_mut() {
        arr.retain(|m| m.get("userId").and_then(|x| x.as_str()) != Some(&id));
    }
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct SetAdminBody {
    #[serde(rename = "isAdmin")]
    is_admin: bool,
}

async fn set_admin(
    State(s): State<MockState>,
    Path(id): Path<String>,
    Json(body): Json<SetAdminBody>,
) -> StatusCode {
    let mut data = s.data.write().await;
    if let Some(u) = data["users"]
        .as_array_mut()
        .and_then(|a| find_by_id_mut(a, &id))
    {
        if let Some(obj) = u.as_object_mut() {
            obj.insert("isAdmin".into(), Value::Bool(body.is_admin));
        }
    }
    StatusCode::NO_CONTENT
}

async fn memberships_for(
    State(s): State<MockState>,
    Path(user_id): Path<String>,
) -> Json<Value> {
    let data = s.data.read().await;
    let empty = vec![];
    let memberships = data["memberships"].as_array().unwrap_or(&empty);
    let products = data["products"].as_array().unwrap_or(&empty);
    let out: Vec<Value> = memberships
        .iter()
        .filter(|m| m.get("userId").and_then(|v| v.as_str()) == Some(&user_id))
        .filter_map(|m| {
            let pid = m.get("productId").and_then(|v| v.as_str())?;
            let product = find_by_id(products, pid)?.clone();
            let mut obj = m.as_object()?.clone();
            obj.insert("product".into(), product);
            Some(Value::Object(obj))
        })
        .collect();
    Json(Value::Array(out))
}

// ---- products ----

#[derive(Deserialize)]
struct ListProductsQuery {
    scope: Option<String>,
    user: Option<String>,
}

async fn list_products(
    State(s): State<MockState>,
    Query(q): Query<ListProductsQuery>,
) -> Json<Value> {
    let data = s.data.read().await;
    let empty = vec![];
    let products = data["products"].as_array().unwrap_or(&empty);
    if q.scope.as_deref() == Some("mine") {
        let Some(user_id) = q.user.as_deref() else {
            return Json(Value::Array(vec![]));
        };
        let memberships = data["memberships"].as_array().unwrap_or(&empty);
        let ids: std::collections::HashSet<&str> = memberships
            .iter()
            .filter(|m| m.get("userId").and_then(|v| v.as_str()) == Some(user_id))
            .filter_map(|m| m.get("productId").and_then(|v| v.as_str()))
            .collect();
        let filtered: Vec<Value> = products
            .iter()
            .filter(|p| {
                p.get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| ids.contains(id))
            })
            .cloned()
            .collect();
        Json(Value::Array(filtered))
    } else {
        Json(Value::Array(products.clone()))
    }
}

async fn get_product(State(s): State<MockState>, Path(id): Path<String>) -> impl IntoResponse {
    let data = s.data.read().await;
    match find_by_id(data["products"].as_array().unwrap_or(&vec![]), &id) {
        Some(p) => Json(p.clone()).into_response(),
        None => not_found(&id).into_response(),
    }
}

#[derive(Deserialize)]
struct CreateProductBody {
    name: String,
    slug: Option<String>,
    description: Option<String>,
}

async fn create_product(
    State(s): State<MockState>,
    Json(body): Json<CreateProductBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let slug = body
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            body.name
                .to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string()
        });
    let mut data = s.data.write().await;
    let products = data["products"]
        .as_array_mut()
        .ok_or_else(|| bad("products missing"))?;
    if products
        .iter()
        .any(|p| p.get("id").and_then(|v| v.as_str()) == Some(&slug))
    {
        return Err(bad(&format!("Product \"{slug}\" already exists")));
    }
    let product = json!({
        "id": slug.clone(),
        "name": body.name,
        "slug": slug,
        "description": body.description.unwrap_or_default(),
        "color": "#6b7280",
    });
    products.push(product.clone());
    Ok(Json(product))
}

async fn delete_product(State(s): State<MockState>, Path(id): Path<String>) -> StatusCode {
    let mut data = s.data.write().await;
    for key in ["products", "memberships", "crashes", "symbols"] {
        if let Some(arr) = data[key].as_array_mut() {
            let scope_key = match key {
                "memberships" | "crashes" | "symbols" => "productId",
                _ => "id",
            };
            arr.retain(|item| item.get(scope_key).and_then(|v| v.as_str()) != Some(&id));
        }
    }
    StatusCode::NO_CONTENT
}

// ---- memberships ----

async fn list_members(
    State(s): State<MockState>,
    Path(pid): Path<String>,
) -> Json<Value> {
    let data = s.data.read().await;
    let empty = vec![];
    let memberships = data["memberships"].as_array().unwrap_or(&empty);
    let users = data["users"].as_array().unwrap_or(&empty);
    let out: Vec<Value> = memberships
        .iter()
        .filter(|m| m.get("productId").and_then(|v| v.as_str()) == Some(&pid))
        .filter_map(|m| {
            let uid = m.get("userId").and_then(|v| v.as_str())?;
            let user = find_by_id(users, uid)?.clone();
            let mut obj = m.as_object()?.clone();
            obj.insert("user".into(), user);
            Some(Value::Object(obj))
        })
        .collect();
    Json(Value::Array(out))
}

#[derive(Deserialize)]
struct GrantBody {
    role: String,
}

async fn grant_access(
    State(s): State<MockState>,
    Path((pid, uid)): Path<(String, String)>,
    Json(body): Json<GrantBody>,
) -> StatusCode {
    let mut data = s.data.write().await;
    if let Some(arr) = data["memberships"].as_array_mut() {
        if let Some(existing) = arr.iter_mut().find(|m| {
            m.get("userId").and_then(|v| v.as_str()) == Some(&uid)
                && m.get("productId").and_then(|v| v.as_str()) == Some(&pid)
        }) {
            if let Some(obj) = existing.as_object_mut() {
                obj.insert("role".into(), Value::String(body.role));
            }
        } else {
            arr.push(json!({ "userId": uid, "productId": pid, "role": body.role }));
        }
    }
    StatusCode::NO_CONTENT
}

async fn revoke_access(
    State(s): State<MockState>,
    Path((pid, uid)): Path<(String, String)>,
) -> StatusCode {
    let mut data = s.data.write().await;
    if let Some(arr) = data["memberships"].as_array_mut() {
        arr.retain(|m| {
            !(m.get("userId").and_then(|v| v.as_str()) == Some(&uid)
                && m.get("productId").and_then(|v| v.as_str()) == Some(&pid))
        });
    }
    StatusCode::NO_CONTENT
}

// ---- crashes ----

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
    State(s): State<MockState>,
    Query(q): Query<ListGroupsQuery>,
) -> Json<Value> {
    let data = s.data.read().await;
    let empty = vec![];
    let crashes = data["crashes"].as_array().unwrap_or(&empty);

    let mut rows: Vec<&Value> = crashes
        .iter()
        .filter(|g| g.get("productId").and_then(|v| v.as_str()) == Some(&q.product_id))
        .collect();

    if let Some(version) = q.version.as_deref().filter(|v| *v != "all" && !v.is_empty()) {
        rows.retain(|g| g.get("version").and_then(|v| v.as_str()) == Some(version));
    }
    if let Some(status) = q.status.as_deref().filter(|v| *v != "all" && !v.is_empty()) {
        rows.retain(|g| g.get("status").and_then(|v| v.as_str()) == Some(status));
    }
    if let Some(search) = q.search.as_deref().filter(|s| !s.trim().is_empty()) {
        let needle = search.to_lowercase();
        rows.retain(|g| {
            let title = g.get("title").and_then(|v| v.as_str()).unwrap_or_default();
            let top = g.get("topFrame").and_then(|v| v.as_str()).unwrap_or_default();
            title.to_lowercase().contains(&needle) || top.to_lowercase().contains(&needle)
        });
    }

    match q.sort.as_deref() {
        Some("recent") => rows.sort_by(|a, b| {
            b.get("lastSeen")
                .and_then(|v| v.as_str())
                .cmp(&a.get("lastSeen").and_then(|v| v.as_str()))
        }),
        Some("similarity") => rows.sort_by(|a, b| {
            b.get("similarity")
                .and_then(|v| v.as_f64())
                .partial_cmp(&a.get("similarity").and_then(|v| v.as_f64()))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        Some("version") => rows.sort_by(|a, b| {
            b.get("version")
                .and_then(|v| v.as_str())
                .cmp(&a.get("version").and_then(|v| v.as_str()))
        }),
        _ => rows.sort_by(|a, b| {
            b.get("count")
                .and_then(|v| v.as_u64())
                .cmp(&a.get("count").and_then(|v| v.as_u64()))
        }),
    }

    let total = rows.len();
    let offset = q.offset.unwrap_or(0);
    let limit = q.limit.unwrap_or(rows.len());
    let slice: Vec<Value> = rows
        .iter()
        .skip(offset)
        .take(limit)
        .map(|g| summarize(g))
        .collect();

    Json(json!({
        "groups": slice,
        "total": total,
        "versions": VERSIONS,
    }))
}

async fn get_group(State(s): State<MockState>, Path(id): Path<String>) -> impl IntoResponse {
    let data = s.data.read().await;
    match find_by_id(data["crashes"].as_array().unwrap_or(&vec![]), &id) {
        Some(g) => Json(g.clone()).into_response(),
        None => not_found(&id).into_response(),
    }
}

// Find a crash by its id by scanning every group's crashes array. Returns
// `{ crash, group }` so the caller doesn't need a second roundtrip.
async fn get_crash(
    State(s): State<MockState>,
    Path(crash_id): Path<String>,
) -> impl IntoResponse {
    let data = s.data.read().await;
    let groups = data["crashes"].as_array().cloned().unwrap_or_default();
    for group in groups {
        if let Some(crashes) = group.get("crashes").and_then(|v| v.as_array()) {
            if let Some(crash) = crashes
                .iter()
                .find(|c| c.get("id").and_then(|v| v.as_str()) == Some(&crash_id))
            {
                return Json(json!({ "crash": crash, "group": group })).into_response();
            }
        }
    }
    not_found(&crash_id).into_response()
}

#[derive(Deserialize)]
struct SetStatusBody {
    status: String,
}

async fn set_status(
    State(s): State<MockState>,
    Path(id): Path<String>,
    Json(body): Json<SetStatusBody>,
) -> StatusCode {
    let mut data = s.data.write().await;
    if let Some(g) = data["crashes"]
        .as_array_mut()
        .and_then(|a| find_by_id_mut(a, &id))
    {
        if let Some(obj) = g.as_object_mut() {
            obj.insert("status".into(), Value::String(body.status));
        }
    }
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct AddNoteBody {
    body: String,
    author: String,
}

async fn add_note(
    State(s): State<MockState>,
    Path(id): Path<String>,
    Json(payload): Json<AddNoteBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let note = json!({
        "author": payload.author,
        "body": payload.body,
        "at": Utc::now().to_rfc3339(),
    });
    let mut data = s.data.write().await;
    let g = data["crashes"]
        .as_array_mut()
        .and_then(|a| find_by_id_mut(a, &id))
        .ok_or_else(|| not_found(&id))?;
    let obj = g.as_object_mut().ok_or_else(|| bad("group corrupt"))?;
    let notes = obj
        .entry("notes".to_string())
        .or_insert_with(|| Value::Array(vec![]));
    if let Some(arr) = notes.as_array_mut() {
        arr.push(note.clone());
    }
    Ok(Json(note))
}

#[derive(Deserialize)]
struct MergeBody {
    #[serde(rename = "mergedId")]
    merged_id: String,
}

async fn merge_groups(
    State(s): State<MockState>,
    Path(primary_id): Path<String>,
    Json(body): Json<MergeBody>,
) -> StatusCode {
    let mut data = s.data.write().await;
    let Some(arr) = data["crashes"].as_array_mut() else {
        return StatusCode::NO_CONTENT;
    };
    let Some(merged_idx) = arr.iter().position(|g| {
        g.get("id").and_then(|v| v.as_str()) == Some(&body.merged_id)
    }) else {
        return StatusCode::NO_CONTENT;
    };
    let merged = arr.remove(merged_idx);
    if let Some(primary) = find_by_id_mut(arr, &primary_id) {
        let obj = primary.as_object_mut().unwrap();
        let primary_count = obj.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        let merged_count = merged.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        obj.insert("count".into(), Value::from(primary_count + merged_count));

        // Move all crashes from the merged group into primary, retagging
        // their `groupId` so the back-reference stays consistent.
        let mut merged_crashes = merged
            .get("crashes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for c in merged_crashes.iter_mut() {
            if let Some(o) = c.as_object_mut() {
                o.insert("groupId".into(), Value::String(primary_id.clone()));
            }
        }
        let primary_crashes = obj
            .entry("crashes".to_string())
            .or_insert_with(|| Value::Array(vec![]));
        if let Some(arr) = primary_crashes.as_array_mut() {
            arr.extend(merged_crashes);
            arr.sort_by(|a, b| {
                b.get("at")
                    .and_then(|v| v.as_str())
                    .cmp(&a.get("at").and_then(|v| v.as_str()))
            });
        }
    } else {
        // restore if primary doesn't exist
        arr.insert(merged_idx, merged);
    }
    StatusCode::NO_CONTENT
}

// ---- symbols ----

#[derive(Deserialize)]
struct SymbolsQuery {
    search: Option<String>,
    arch: Option<String>,
    format: Option<String>,
    sort: Option<String>,
}

async fn list_symbols(
    State(s): State<MockState>,
    Path(pid): Path<String>,
    Query(q): Query<SymbolsQuery>,
) -> Json<Value> {
    let data = s.data.read().await;
    let empty = vec![];
    let symbols = data["symbols"].as_array().unwrap_or(&empty);

    let mut rows: Vec<&Value> = symbols
        .iter()
        .filter(|r| r.get("productId").and_then(|v| v.as_str()) == Some(&pid))
        .collect();

    if let Some(search) = q.search.as_deref().filter(|s| !s.trim().is_empty()) {
        let needle = search.to_lowercase();
        rows.retain(|r| {
            let name = r.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let debug = r.get("debugId").and_then(|v| v.as_str()).unwrap_or_default();
            name.to_lowercase().contains(&needle) || debug.to_lowercase().contains(&needle)
        });
    }
    if let Some(arch) = q.arch.as_deref().filter(|v| *v != "all" && !v.is_empty()) {
        rows.retain(|r| r.get("arch").and_then(|v| v.as_str()) == Some(arch));
    }
    if let Some(format) = q.format.as_deref().filter(|v| *v != "all" && !v.is_empty()) {
        rows.retain(|r| r.get("format").and_then(|v| v.as_str()) == Some(format));
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
            parse(b).partial_cmp(&parse(a)).unwrap_or(std::cmp::Ordering::Equal)
        }),
        _ => rows.sort_by(|a, b| {
            b.get("uploadedAt")
                .and_then(|v| v.as_str())
                .cmp(&a.get("uploadedAt").and_then(|v| v.as_str()))
        }),
    }

    Json(Value::Array(rows.into_iter().cloned().collect()))
}

#[derive(Deserialize)]
struct UploadSymbolBody {
    name: String,
    version: Option<String>,
    arch: Option<String>,
    format: Option<String>,
    size: Option<String>,
    #[serde(rename = "uploadedBy")]
    uploaded_by: String,
}

async fn upload_symbol(
    State(s): State<MockState>,
    Path(pid): Path<String>,
    Json(body): Json<UploadSymbolBody>,
) -> Json<Value> {
    let mut data = s.data.write().await;
    let symbols = data["symbols"].as_array_mut().expect("symbols array");
    let id = format!("SYM-{}", random_id_suffix());
    let debug_id = format!(
        "{}{}{}{}1",
        random_hex(8),
        random_hex(8),
        random_hex(8),
        random_hex(8)
    )
    .to_uppercase();
    let row = json!({
        "id": id,
        "productId": pid,
        "name": body.name,
        "version": body.version.unwrap_or_else(|| "0.0.0".into()),
        "arch": body.arch.unwrap_or_else(|| "x86_64".into()),
        "format": body.format.unwrap_or_else(|| "PDB".into()),
        "size": body.size.unwrap_or_else(|| "1.0 MB".into()),
        "debugId": debug_id,
        "codeId": random_hex(14),
        "uploadedAt": Utc::now().to_rfc3339(),
        "uploadedBy": body.uploaded_by,
        "referencedBy": 0,
    });
    symbols.insert(0, row.clone());
    Json(row)
}

async fn delete_symbol(State(s): State<MockState>, Path(id): Path<String>) -> StatusCode {
    let mut data = s.data.write().await;
    if let Some(arr) = data["symbols"].as_array_mut() {
        arr.retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(&id));
    }
    StatusCode::NO_CONTENT
}
