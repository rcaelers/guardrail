// Token-authenticated crash access, for integrations that analyse crashes
// rather than browse them.
//
// The web server is deliberately session-only, so these live here on the API
// server alongside symbol upload, which is where bearer tokens are accepted.
// Every route is scoped to the product the token is bound to; a token can never
// reach another product's crashes.
//
// Reports are redacted by default. `crash-read` yields the parts that describe
// the failure — stack frames, modules, threads, system summary and build
// metadata — and drops the parts that describe the reporter: the `guid`
// annotation, OS handles, and whatever free text they typed. Seeing those needs
// `crash-read-full`, so widening the exposure is a deliberate grant rather than
// a code change.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use data::api_token::{
    ENTITLEMENT_CRASH_ANNOTATE, ENTITLEMENT_CRASH_READ, ENTITLEMENT_CRASH_READ_FULL,
};
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product_by_id, validate_api_token_for_product};

/// Annotation carrying a per-installation identifier. Useful to the product,
/// but it is the one submitted field that can correlate crashes back to a
/// person, so it is withheld unless the caller may read the full report.
const IDENTIFYING_ANNOTATION: &str = "guid";

/// Report fields describing the reporter's machine state rather than the
/// failure. Withheld under `crash-read`.
const SENSITIVE_REPORT_FIELDS: &[&str] = &[
    "handles",
    "lsb_release",
    "mac_boot_args",
    "mac_crash_info",
    "proc_limits",
    "linux_memory_map_count",
    "unloaded_modules",
];

async fn run(
    db: &Surreal<Any>,
    sql: &str,
    binds: Vec<(&'static str, Value)>,
) -> Result<Vec<Value>, ApiError> {
    let mut q = db.query(sql);
    for (k, v) in binds {
        q = q.bind((k, v));
    }
    let mut resp = q.await.map_err(|e| {
        tracing::error!("crash api query failed: {e}");
        ApiError::InternalFailure()
    })?;
    resp.take(0).map_err(|e| {
        tracing::error!("crash api decode failed: {e}");
        ApiError::InternalFailure()
    })
}

/// Resolves the caller's token, checks the entitlement, and pins the request to
/// the product that token is bound to. Returns that product's id.
async fn authorize(
    headers: &HeaderMap,
    state: &AppState,
    entitlement: &str,
    product_id: &str,
) -> Result<String, ApiError> {
    let db = &state.repo.db;
    let token = crate::access::require_entitlement(headers, None, db, entitlement).await?;
    let product = get_product_by_id(db, product_id).await?;
    validate_api_token_for_product(&token, &product, &product.name)?;
    Ok(product.id)
}

/// True when the token may see the unredacted report.
async fn may_read_full(headers: &HeaderMap, state: &AppState) -> bool {
    crate::access::require_entitlement(
        headers,
        None,
        &state.repo.db,
        ENTITLEMENT_CRASH_READ_FULL,
    )
    .await
    .is_ok()
}

fn redact_report(report: &mut Value) {
    let Some(obj) = report.as_object_mut() else {
        return;
    };
    for field in SENSITIVE_REPORT_FIELDS {
        obj.remove(*field);
    }
}

fn redact_annotations(annotations: &mut Value) {
    if let Some(obj) = annotations.as_object_mut() {
        obj.remove(IDENTIFYING_ANNOTATION);
    }
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(rename = "productId")]
    product_id: String,
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

/// Crash groups for a product, newest activity first. Summary only — no report
/// is included, so this is cheap enough to poll.
pub async fn list_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let product_id = authorize(&headers, &state, ENTITLEMENT_CRASH_READ, &q.product_id).await?;
    let db = &state.repo.db;

    let limit = q.limit.unwrap_or(50).min(500);
    let offset = q.offset.unwrap_or(0);
    let mut sql = String::from(
        "SELECT meta::id(id) AS id, fingerprint, signal, count, status,
                fixed_in_version AS fixedInVersion,
                first_seen AS firstSeen, last_seen AS lastSeen
         FROM crash_groups
         WHERE product_id = type::record('products', $pid)",
    );
    if q.status.is_some() {
        sql.push_str(" AND status = $status");
    }
    sql.push_str(&format!(
        " ORDER BY last_seen DESC LIMIT {limit} START {offset}"
    ));

    let mut binds = vec![("pid", Value::String(product_id))];
    if let Some(status) = q.status.clone() {
        binds.push(("status", Value::String(status)));
    }
    let groups = run(db, &sql, binds).await?;
    Ok(Json(json!({ "groups": groups })))
}

/// One crash group: its workflow state, its notes, and the ids of its member
/// crashes. Fetch a member through `get_crash` for the report itself.
pub async fn get_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
    Query(q): Query<ProductQuery>,
) -> Result<Json<Value>, ApiError> {
    let product_id = authorize(&headers, &state, ENTITLEMENT_CRASH_READ, &q.product_id).await?;
    let db = &state.repo.db;

    let groups = run(
        db,
        "SELECT meta::id(id) AS id, fingerprint, signal, count, status,
                fixed_in_version AS fixedInVersion,
                first_seen AS firstSeen, last_seen AS lastSeen
         FROM crash_groups
         WHERE meta::id(id) = $gid AND product_id = type::record('products', $pid)",
        vec![
            ("gid", Value::String(group_id.clone())),
            ("pid", Value::String(product_id)),
        ],
    )
    .await?;
    let Some(group) = groups.into_iter().next() else {
        return Err(ApiError::Failure(format!("crash group {group_id} not found")));
    };

    let crashes = run(
        db,
        "SELECT meta::id(id) AS id, created_at AS createdAt,
                report.{ version, os, platform, at } AS r
         FROM crashes WHERE group_id = type::record('crash_groups', $gid)
         ORDER BY created_at DESC",
        vec![("gid", Value::String(group_id.clone()))],
    )
    .await?;

    let notes = run(
        db,
        "SELECT author, value AS body, created_at AS at FROM annotations
         WHERE source = 'user' AND group_id = type::record('crash_groups', $gid)
         ORDER BY created_at",
        vec![("gid", Value::String(group_id))],
    )
    .await?;

    Ok(Json(json!({
        "group": group,
        "crashes": crashes,
        "notes": notes,
    })))
}

#[derive(Deserialize)]
pub struct ProductQuery {
    #[serde(rename = "productId")]
    product_id: String,
}

/// One crash, with its report. Redacted unless the token also holds
/// `crash-read-full`; the response says which it got, so a caller never has to
/// guess whether a field is absent or withheld.
pub async fn get_crash(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(crash_id): Path<String>,
    Query(q): Query<ProductQuery>,
) -> Result<Json<Value>, ApiError> {
    let product_id = authorize(&headers, &state, ENTITLEMENT_CRASH_READ, &q.product_id).await?;
    let db = &state.repo.db;

    let rows = run(
        db,
        "SELECT meta::id(id) AS id, meta::id(group_id) AS groupId, created_at AS createdAt, report
         FROM crashes
         WHERE meta::id(id) = $cid AND product_id = type::record('products', $pid)",
        vec![
            ("cid", Value::String(crash_id.clone())),
            ("pid", Value::String(product_id)),
        ],
    )
    .await?;
    let Some(mut crash) = rows.into_iter().next() else {
        return Err(ApiError::Failure(format!("crash {crash_id} not found")));
    };

    let mut annotations = json!({});
    for row in run(
        db,
        "SELECT key, value FROM annotations
         WHERE crash_id = type::record('crashes', $cid) AND key != NONE",
        vec![("cid", Value::String(crash_id))],
    )
    .await?
    {
        if let (Some(k), Some(v)) = (
            row.get("key").and_then(|v| v.as_str()),
            row.get("value").cloned(),
        ) && let Some(obj) = annotations.as_object_mut()
        {
            obj.insert(k.to_string(), v);
        }
    }

    let full = may_read_full(&headers, &state).await;
    if !full {
        if let Some(report) = crash.get_mut("report") {
            redact_report(report);
        }
        redact_annotations(&mut annotations);
    }
    if let Some(obj) = crash.as_object_mut() {
        obj.insert("annotations".into(), annotations);
        obj.insert("redacted".into(), json!(!full));
    }

    Ok(Json(crash))
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct NoteBody {
    #[serde(rename = "productId")]
    product_id: String,
    body: String,
    /// Shown as the note's author. Defaults to the token's description, so a
    /// note always says which integration wrote it.
    author: Option<String>,
}

/// Adds a note to a crash group. Notes are the same annotations the UI shows,
/// so an integration's findings land where a person will actually see them.
pub async fn add_group_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
    Json(body): Json<NoteBody>,
) -> Result<Json<Value>, ApiError> {
    if body.body.trim().is_empty() {
        return Err(ApiError::Failure("note body is empty".into()));
    }
    let db = &state.repo.db;
    let token =
        crate::access::require_entitlement(&headers, None, db, ENTITLEMENT_CRASH_ANNOTATE).await?;
    let product = get_product_by_id(db, &body.product_id).await?;
    validate_api_token_for_product(&token, &product, &product.name)?;

    let author = body
        .author
        .clone()
        .unwrap_or_else(|| token.description.clone());

    // Confirm the group belongs to the token's product before writing, so a
    // valid token cannot annotate another product's group by guessing an id.
    let found = run(
        db,
        "SELECT VALUE meta::id(id) FROM crash_groups
         WHERE meta::id(id) = $gid AND product_id = type::record('products', $pid)",
        vec![
            ("gid", Value::String(group_id.clone())),
            ("pid", Value::String(product.id.clone())),
        ],
    )
    .await?;
    if found.is_empty() {
        return Err(ApiError::Failure(format!("crash group {group_id} not found")));
    }

    run(
        db,
        "CREATE annotations CONTENT {
             source: 'user', key: NONE, value: $body, author: $author,
             group_id: type::record('crash_groups', $gid),
             product_id: type::record('products', $pid),
             created_at: time::now(), updated_at: time::now()
         }",
        vec![
            ("gid", Value::String(group_id)),
            ("pid", Value::String(product.id)),
            ("body", Value::String(body.body.trim().to_string())),
            ("author", Value::String(author)),
        ],
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct StatusBody {
    #[serde(rename = "productId")]
    product_id: String,
    status: String,
    /// The release the fix goes into. Recorded when resolving so a later crash
    /// from a build that carries it reopens the group.
    #[serde(rename = "fixedInVersion")]
    fixed_in_version: Option<String>,
}

/// Sets a crash group's triage status. Status lives on the group, not on
/// individual crashes, so this is the only state a caller can change.
pub async fn set_group_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Result<Json<Value>, ApiError> {
    if !matches!(
        body.status.as_str(),
        "new" | "triaged" | "resolved" | "wontfix" | "regressed" | "obsolete"
    ) {
        return Err(ApiError::Failure(format!(
            "invalid status '{}': expected new, triaged, resolved, wontfix, regressed or obsolete",
            body.status
        )));
    }
    let fixed_in_version = body.fixed_in_version.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(version) = fixed_in_version
        && semver::Version::parse(version).is_err()
    {
        return Err(ApiError::Failure(format!(
            "invalid fixedInVersion '{version}': expected a semantic version such as 1.11.2"
        )));
    }
    let db = &state.repo.db;
    let token =
        crate::access::require_entitlement(&headers, None, db, ENTITLEMENT_CRASH_ANNOTATE).await?;
    let product = get_product_by_id(db, &body.product_id).await?;
    validate_api_token_for_product(&token, &product, &product.name)?;

    // An explicit value is always recorded; leaving it out when reopening
    // clears it, so a stale version cannot reopen the group again.
    let fixed_value = match fixed_in_version {
        Some(version) => Value::String(version.to_string()),
        None => Value::Null,
    };
    let rows = run(
        db,
        "UPDATE crash_groups
         SET status = $status,
             fixed_in_version = IF $fixed = NULL THEN NONE ELSE $fixed END,
             updated_at = time::now()
         WHERE meta::id(id) = $gid AND product_id = type::record('products', $pid)
         RETURN meta::id(id) AS id",
        vec![
            ("gid", Value::String(group_id.clone())),
            ("pid", Value::String(product.id)),
            ("status", Value::String(body.status.clone())),
            ("fixed", fixed_value),
        ],
    )
    .await?;
    if rows.is_empty() {
        return Err(ApiError::Failure(format!("crash group {group_id} not found")));
    }
    Ok(Json(json!({
        "ok": true,
        "status": body.status,
        "fixedInVersion": fixed_in_version
    })))
}
