use super::common::*;

// ---------------------------------------------------------------------------
// Tests: crash / symbol session-only endpoints
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /crashes/{group_id}/status |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | not 403  |
// | non_admin     | not 403  |
// | imp_admin     | not 403  |
// | imp_non_admin | not 403  |
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

// API calls:
// | Method | Route                     |
// | ------ | ------------------------- |
// | POST   | /crashes/{group_id}/notes |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | not 403  |
// | non_admin     | not 403  |
// | imp_admin     | not 403  |
// | imp_non_admin | not 403  |
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

// API calls:
// | Method | Route                     |
// | ------ | ------------------------- |
// | POST   | /crashes/{group_id}/merge |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | not 403  |
// | non_admin     | not 403  |
// | imp_admin     | not 403  |
// | imp_non_admin | not 403  |
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

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /crashes/{group_id}/status |
// | POST   | /crashes/{group_id}/notes  |
// | POST   | /crashes/{group_id}/merge  |
// Cases:
// | Case                                   | Expected |
// | -------------------------------------- | -------- |
// | read-only user changes status          | 403      |
// | read-write user changes status         | 204      |
// | read-only user adds note               | 403      |
// | read-write user adds note              | 200      |
// | read-write user merges groups          | 403      |
// | maintainer user merges maintained group | 204      |
#[tokio::test]
async fn test_crash_mutations_by_product_role() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;

    let readonly_group = create_test_crash_group(&app.db, &f.products[0].id).await;
    let readwrite_group = create_test_crash_group(&app.db, &f.products[1].id).await;
    let maintainer_primary = create_test_crash_group(&app.db, &f.products[2].id).await;
    let maintainer_merged = create_test_crash_group(&app.db, &f.products[2].id).await;
    create_test_crash_in_group(&app.db, &f.products[2].id, &maintainer_merged).await;

    let status_body = json!({"status": "resolved"});
    assert_eq!(
        app.call(
            "POST",
            &format!("/crashes/{readonly_group}/status"),
            Some(status_body.clone()),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::FORBIDDEN,
        "read-only users cannot change crash status"
    );
    assert_eq!(
        app.call(
            "POST",
            &format!("/crashes/{readwrite_group}/status"),
            Some(status_body),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::NO_CONTENT,
        "read-write users can change crash status"
    );

    let note_body = json!({"body": "A role-checked note", "author": "tester"});
    assert_eq!(
        app.call(
            "POST",
            &format!("/crashes/{readonly_group}/notes"),
            Some(note_body.clone()),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::FORBIDDEN,
        "read-only users cannot add notes"
    );
    assert_eq!(
        app.call(
            "POST",
            &format!("/crashes/{readwrite_group}/notes"),
            Some(note_body),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::OK,
        "read-write users can add notes"
    );

    let readwrite_merged = create_test_crash_group(&app.db, &f.products[1].id).await;
    assert_eq!(
        app.call(
            "POST",
            &format!("/crashes/{readwrite_group}/merge"),
            Some(json!({"mergedId": readwrite_merged})),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::FORBIDDEN,
        "read-write users cannot merge groups"
    );
    assert_eq!(
        app.call(
            "POST",
            &format!("/crashes/{maintainer_primary}/merge"),
            Some(json!({"mergedId": maintainer_merged})),
            Some(&f.non_admin),
        )
        .await,
        StatusCode::NO_CONTENT,
        "maintainers can merge groups for maintained products"
    );
}

// ---------------------------------------------------------------------------
// Tests: crash group endpoints
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                                         |
// | ------ | ------------------------------------------------------------- |
// | GET    | /crashes?productId={product_id}                               |
// | GET    | /crashes?productId={product_id}&status={status}               |
// | GET    | /crashes?productId={product_id}&version={version}             |
// | GET    | /crashes?productId={product_id}&search={query}                |
// | GET    | /crashes?productId={product_id}&sort={sort}                   |
// | GET    | /crashes?productId={product_id}&limit={limit}&offset={offset} |
// Cases:
// | Case                               | Expected |
// | ---------------------------------- | -------- |
// | empty product list without session | 200      |
// | empty product list as admin        | 200      |
// | seeded product list as admin       | 200      |
// | status filters as admin            | 200      |
// | version/search filters as admin    | 200      |
// | sort variants as admin             | 200      |
// | pagination as admin                | 200      |
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
    assert_eq!(
        app.call("GET", &format!("{base}&status=unresolved"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}&status=all"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}&version=1.0"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}&search=test"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    // sort variants
    assert_eq!(
        app.call("GET", &format!("{base}&sort=recent"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}&sort=similarity"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}&sort=version"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    // pagination
    assert_eq!(
        app.call("GET", &format!("{base}&limit=5&offset=0"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
}

// API calls:
// | Method | Route               |
// | ------ | ------------------- |
// | GET    | /crashes/{group_id} |
// Cases:
// | Case                                  | Expected |
// | ------------------------------------- | -------- |
// | admin with nonexistent group          | 404      |
// | admin with real group                 | 200      |
// | non_admin with read-only product role | 200      |
// | no_session on private product         | 404      |
#[tokio::test]
async fn test_get_group() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Nonexistent group → 404
    assert_eq!(
        app.call("GET", "/crashes/nonexistent-group", None, Some(&f.admin))
            .await,
        StatusCode::NOT_FOUND
    );

    // Real group → 200 for admin and non_admin (products[0] grants readonly to non_admin)
    // No session → 404 because products[0] is non-public
    let gid = create_test_crash_group(&app.db, pid).await;
    let uri = format!("/crashes/{gid}");
    assert_eq!(app.call("GET", &uri, None, Some(&f.admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &uri, None, Some(&f.non_admin)).await, StatusCode::OK);
    assert_eq!(app.call("GET", &uri, None, None).await, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tests: db_api – list_groups with crash data (trend / count / sort paths)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                             |
// | ------ | ------------------------------------------------- |
// | GET    | /crashes?productId={product_id}                   |
// | GET    | /crashes?productId={product_id}&sort={sort}       |
// | GET    | /crashes?productId={product_id}&search={query}    |
// | GET    | /crashes?productId={product_id}&version={version} |
// Cases:
// | Case                                | Expected |
// | ----------------------------------- | -------- |
// | basic list with crash data as admin | 200      |
// | sort=recent as admin                | 200      |
// | sort=similarity as admin            | 200      |
// | sort=version as admin               | 200      |
// | search by title/topFrame as admin   | 200      |
// | version filter as admin             | 200      |
#[tokio::test]
async fn test_list_groups_with_crash_data() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Create two crash groups so that the sort comparators run (need >= 2 elements)
    let gid1 = create_test_crash_group(&app.db, pid).await;
    let gid2 = create_test_crash_group(&app.db, pid).await;
    // Create crashes linked to the groups so rep_rows are non-empty → covers the
    // version / trend / count / reps accumulation paths inside list_groups
    create_test_crash_in_group(&app.db, pid, &gid1).await;
    create_test_crash_in_group(&app.db, pid, &gid2).await;

    let base = format!("/crashes?productId={pid}");
    // Basic list with crash data
    assert_eq!(app.call("GET", &base, None, Some(&f.admin)).await, StatusCode::OK);

    // Sort variants (need 2+ groups for the comparators to execute)
    assert_eq!(
        app.call("GET", &format!("{base}&sort=recent"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}&sort=similarity"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}&sort=version"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );

    // Search filter (title / topFrame)
    assert_eq!(
        app.call("GET", &format!("{base}&search=Test"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    // Version filter
    assert_eq!(
        app.call("GET", &format!("{base}&version=1.2.3"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – get_crash (by crash ID, not group ID)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | GET    | /crashes/by-crash/{crash_id} |
// Cases:
// | Case                                  | Expected |
// | ------------------------------------- | -------- |
// | admin with nonexistent crash          | 404      |
// | admin with real crash                 | 200      |
// | non_admin with read-only product role | 200      |
// | no_session on private product         | 404      |
// | admin with crash whose group is gone  | 404      |
#[tokio::test]
async fn test_get_crash_handler() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Nonexistent crash → 404
    assert_eq!(
        app.call("GET", "/crashes/by-crash/nosuchcrash", None, Some(&f.admin))
            .await,
        StatusCode::NOT_FOUND,
    );

    // Create group + crash linked to it
    let gid = create_test_crash_group(&app.db, pid).await;
    let cid = create_test_crash_in_group(&app.db, pid, &gid).await;
    let uri = format!("/crashes/by-crash/{cid}");

    // Admin can access
    assert_eq!(app.call("GET", &uri, None, Some(&f.admin)).await, StatusCode::OK);
    // non_admin with readonly role on products[0]
    assert_eq!(app.call("GET", &uri, None, Some(&f.non_admin)).await, StatusCode::OK);
    // No session → private product → 404
    assert_eq!(app.call("GET", &uri, None, None).await, StatusCode::NOT_FOUND);

    let missing_group = create_test_crash_group(&app.db, pid).await;
    let orphan_cid = create_test_crash_in_group(&app.db, pid, &missing_group).await;
    app.db
        .query("DELETE type::record('crash_groups', $gid)")
        .bind(("gid", missing_group))
        .await
        .expect("delete crash group failed");
    let uri = format!("/crashes/by-crash/{orphan_cid}");
    assert_eq!(app.call("GET", &uri, None, Some(&f.admin)).await, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tests: db_api – compose_group related groups
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route               |
// | ------ | ------------------- |
// | GET    | /crashes/{group_id} |
// Cases:
// | Case                                         | Expected                         |
// | -------------------------------------------- | -------------------------------- |
// | admin with real group and related crash data | 200 with non-empty related array |
#[tokio::test]
async fn test_get_group_with_related() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Two groups with the same signal → compose_group's related query finds them
    let gid1 = create_test_crash_group(&app.db, pid).await;
    let gid2 = create_test_crash_group(&app.db, pid).await;
    // Link a crash to gid2 so it appears in the related query (needs count > 0)
    create_test_crash_in_group(&app.db, pid, &gid2).await;

    let uri = format!("/crashes/{gid1}");
    let (status, body) = app.call_json("GET", &uri, None, Some(&f.admin)).await;
    assert_eq!(status, StatusCode::OK);
    // "related" key should be present and contain gid2
    let related = body
        .get("related")
        .and_then(|v| v.as_array())
        .expect("related array missing");
    assert!(!related.is_empty(), "related should contain gid2; body={body}");
}

// ---------------------------------------------------------------------------
// Tests: db_api – add_note on an existing group
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                     |
// | ------ | ------------------------- |
// | POST   | /crashes/{group_id}/notes |
// Cases:
// | Case                                   | Expected |
// | -------------------------------------- | -------- |
// | no_session                             | 403      |
// | admin on read-only product             | 200      |
// | non_admin with read-write product role | 200      |
#[tokio::test]
async fn test_add_note_on_group() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let gid = create_test_crash_group(&app.db, pid).await;
    let note_body = json!({"body": "A test note", "author": "tester"});

    // No session → 403
    assert_eq!(
        app.call("POST", &format!("/crashes/{gid}/notes"), Some(note_body.clone()), None)
            .await,
        StatusCode::FORBIDDEN,
    );

    // Admin with session → 200
    assert_eq!(
        app.call("POST", &format!("/crashes/{gid}/notes"), Some(note_body.clone()), Some(&f.admin))
            .await,
        StatusCode::OK,
    );
    // Non-admin with readwrite/maintainer role also succeeds
    let gid2 = create_test_crash_group(&app.db, &f.products[1].id).await;
    assert_eq!(
        app.call("POST", &format!("/crashes/{gid2}/notes"), Some(note_body), Some(&f.non_admin))
            .await,
        StatusCode::OK,
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – merge_groups (functional path)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                     |
// | ------ | ------------------------- |
// | POST   | /crashes/{group_id}/merge |
// Cases:
// | Case                     | Expected |
// | ------------------------ | -------- |
// | no_session               | 403      |
// | admin on private product | 204      |
#[tokio::test]
async fn test_merge_groups_success() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let primary = create_test_crash_group(&app.db, pid).await;
    let merged = create_test_crash_group(&app.db, pid).await;
    let body = json!({"mergedId": merged});

    // No session → 403
    assert_eq!(
        app.call("POST", &format!("/crashes/{primary}/merge"), Some(body.clone()), None)
            .await,
        StatusCode::FORBIDDEN,
    );

    // Admin → 204 No Content
    assert_eq!(
        app.call("POST", &format!("/crashes/{primary}/merge"), Some(body), Some(&f.admin))
            .await,
        StatusCode::NO_CONTENT,
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – get_crash with user-text attachment and annotations
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | GET    | /crashes/by-crash/{crash_id} |
// Cases:
// | Case                                                  | Expected                             |
// | ----------------------------------------------------- | ------------------------------------ |
// | admin with user-text attachment and keyed annotations | 200 with expected annotation payload |
#[tokio::test]
async fn test_get_crash_with_annotations_and_user_text() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Use no-hyphen UUIDs for crash so SurrealDB IDs are consistent
    let gid = create_test_crash_group(&app.db, pid).await;
    let cid = create_test_crash_in_group(&app.db, pid, &gid).await;

    // Create a "user-text" attachment WITH content in the store → covers load_user_text happy path
    let user_text_att = create_test_attachment(
        &app.db,
        "user-text",
        "text/plain",
        5,
        "user_text.txt",
        Some(pid.clone()),
        Some(cid.clone()),
    )
    .await;
    {
        use object_store::ObjectStore as _;
        (&*app.storage)
            .put_opts(
                &object_store::path::Path::from(user_text_att.storage_path.as_str()),
                object_store::PutPayload::from_static(b"hello from user"),
                Default::default(),
            )
            .await
            .expect("put user-text failed");
    }

    // Create a regular attachment → covers non-user-text branch of split_crash_attachments
    create_test_attachment(
        &app.db,
        "minidump",
        "application/octet-stream",
        10,
        "crash.dmp",
        Some(pid.clone()),
        Some(cid.clone()),
    )
    .await;

    // Create a keyed annotation (source=script) → covers build_annotations_map if-let body
    app.db
        .query(
            "CREATE annotations CONTENT {
                source: 'script',
                key: 'os',
                value: 'Linux',
                crash_id: type::record('crashes', $cid),
                product_id: type::record('products', $pid),
                created_at: time::now(),
                updated_at: time::now()
            }",
        )
        .bind(("cid", cid.clone()))
        .bind(("pid", pid.to_string()))
        .await
        .unwrap();

    // Create a user annotation (no key) → covers build_annotations_map else branch (line 1223)
    app.db
        .query(
            "CREATE annotations CONTENT {
                source: 'user',
                key: NONE,
                value: 'a note',
                crash_id: type::record('crashes', $cid),
                product_id: type::record('products', $pid),
                created_at: time::now(),
                updated_at: time::now()
            }",
        )
        .bind(("cid", cid.clone()))
        .bind(("pid", pid.to_string()))
        .await
        .unwrap();

    let uri = format!("/crashes/by-crash/{cid}");
    let (status, body) = app.call_json("GET", &uri, None, Some(&f.admin)).await;
    assert_eq!(status, StatusCode::OK, "get_crash failed; body={body}");
    // crash.annotations map should have "os" key from the script annotation
    assert_eq!(
        body["crash"]["annotations"]["os"].as_str(),
        Some("Linux"),
        "expected 'Linux' annotation; body={body}",
    );
}

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | GET    | /crashes/by-crash/{crash_id} |
// Cases:
// | Case                                | Expected               |
// | ----------------------------------- | ---------------------- |
// | admin with missing user-text object | 200 with null userText |
#[tokio::test]
async fn test_get_crash_user_text_not_in_store() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let gid = create_test_crash_group(&app.db, pid).await;
    let cid = create_test_crash_in_group(&app.db, pid, &gid).await;

    // "user-text" attachment in DB but NOT uploaded to object store →
    // load_user_text gets NotFound → returns Ok(None) → userText absent from response
    create_test_attachment(
        &app.db,
        "user-text",
        "text/plain",
        0,
        "missing.txt",
        Some(pid.clone()),
        Some(cid.clone()),
    )
    .await;

    let uri = format!("/crashes/by-crash/{cid}");
    let (status, body) = app.call_json("GET", &uri, None, Some(&f.admin)).await;
    assert_eq!(status, StatusCode::OK, "expected OK; body={body}");
    assert!(
        body["crash"]["userText"].is_null(),
        "userText should be absent when file is missing from store; body={body}",
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – load_user_text without storagePath (line 302)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | GET    | /crashes/by-crash/{crash_id} |
// Cases:
// | Case                                                | Expected               |
// | --------------------------------------------------- | ---------------------- |
// | admin with user-text attachment missing storagePath | 200 with null userText |
#[tokio::test]
async fn test_load_user_text_no_storage_path() {
    // Covers line 302: user-text attachment exists in DB but has no storagePath field.
    // load_user_text returns Ok(None) immediately (before touching the object store).
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let gid = create_test_crash_group(&app.db, pid).await;
    let cid = create_test_crash_in_group(&app.db, pid, &gid).await;

    // Insert a "user-text" attachment WITHOUT storagePath.
    app.db
        .query(
            "CREATE attachments CONTENT {
                name: 'user-text',
                mimeType: 'text/plain',
                size: 0,
                filename: 'note.txt',
                crash_id: type::record('crashes', $cid),
                product_id: type::record('products', $pid),
                created_at: time::now(),
                updated_at: time::now()
            }",
        )
        .bind(("cid", cid.clone()))
        .bind(("pid", pid.to_string()))
        .await
        .unwrap();

    let (status, body) = app
        .call_json("GET", &format!("/crashes/by-crash/{cid}"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK, "expected OK; body={body}");
    assert!(
        body["crash"]["userText"].is_null(),
        "userText should be absent when storagePath missing; body={body}",
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – list_groups edge cases (lines 1046, 1061)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                           |
// | ------ | ------------------------------- |
// | GET    | /crashes?productId={product_id} |
// Cases:
// | Case                                   | Expected            |
// | -------------------------------------- | ------------------- |
// | admin list with crash missing group_id | 200 and row skipped |
#[tokio::test]
async fn test_list_groups_crash_without_group_id() {
    // Covers line 1046: crash row without group_id → continue (skipped in rep_rows loop).
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Create a crash directly with NONE group_id so rep_rows contains a row
    // where r.get("group_id") returns None → line 1046 continue.
    app.db
        .query(
            "CREATE crashes CONTENT {
                product_id: type::record('products', $pid),
                group_id: NONE,
                fingerprint: 'no-group-fp',
                report: { title: 'Orphan crash', version: '0.1.0' },
                created_at: time::now(),
                updated_at: time::now()
            }",
        )
        .bind(("pid", pid.to_string()))
        .await
        .unwrap();

    let (status, _) = app
        .call_json("GET", &format!("/crashes?productId={pid}"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
}

// API calls:
// | Method | Route                           |
// | ------ | ------------------------------- |
// | GET    | /crashes?productId={product_id} |
// Cases:
// | Case                                       | Expected |
// | ------------------------------------------ | -------- |
// | admin list with crash outside trend window | 200      |
#[tokio::test]
async fn test_list_groups_old_crash() {
    // Covers line 1061: the inner if `(0..28).contains(&days_ago)` false branch.
    // A crash older than 28 days is outside the trend window.
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let gid = create_test_crash_group(&app.db, pid).await;

    // Insert a crash with created_at = 30 days ago so days_ago = 30 ∉ [0, 28).
    // Use a fixed RFC 3339 timestamp far in the past so the parse always succeeds
    // and days_ago is reliably >= 28.
    app.db
        .query(
            "CREATE crashes CONTENT {
                product_id: type::record('products', $pid),
                group_id:   type::record('crash_groups', $gid),
                fingerprint: 'old-fp',
                report: { title: 'Old crash', version: '1.0.0' },
                created_at: <datetime>'2020-01-01T00:00:00Z',
                updated_at: time::now()
            }",
        )
        .bind(("pid", pid.to_string()))
        .bind(("gid", gid.clone()))
        .await
        .unwrap();

    let (status, _) = app
        .call_json("GET", &format!("/crashes?productId={pid}"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
}
