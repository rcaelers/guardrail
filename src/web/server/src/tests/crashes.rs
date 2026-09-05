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
// | Method | Route                           |
// | ------ | ------------------------------- |
// | DELETE | /crashes/{group_id}             |
// | DELETE | /crashes/by-crash/{crash_id}    |
// Cases:
// | Auth context  | Expected |
// | ------------- | -------- |
// | no_session    | 403      |
// | admin         | not 403  |
// | non_admin     | not 403  |
// | imp_admin     | not 403  |
// | imp_non_admin | not 403  |
#[tokio::test]
async fn test_delete_crashes_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_session_only_not_forbidden(&app, &f, "DELETE", "/crashes/nonexistent", None).await;
    assert_session_only_not_forbidden(&app, &f, "DELETE", "/crashes/by-crash/nonexistent", None)
        .await;
}

// API calls:
// | Method | Route                      |
// | ------ | -------------------------- |
// | POST   | /crashes/{group_id}/status |
// | POST   | /crashes/{group_id}/notes  |
// | POST   | /crashes/{group_id}/merge  |
// | DELETE | /crashes/{group_id}        |
// | DELETE | /crashes/by-crash/{id}     |
// Cases:
// | Case                                   | Expected |
// | -------------------------------------- | -------- |
// | read-only user changes status          | 403      |
// | read-write user changes status         | 204      |
// | read-only user adds note               | 403      |
// | read-write user adds note              | 200      |
// | read-write user merges groups          | 403      |
// | maintainer user merges maintained group | 204      |
// | read-only user deletes crash           | 403      |
// | read-write user deletes crash          | 204      |
// | read-only user deletes group           | 403      |
// | read-write user deletes group          | 204      |
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

    let readonly_delete_group = create_test_crash_group(&app.db, &f.products[0].id).await;
    let readonly_delete_crash =
        create_test_crash_in_group(&app.db, &f.products[0].id, &readonly_delete_group).await;
    let readwrite_delete_crash_group = create_test_crash_group(&app.db, &f.products[1].id).await;
    let readwrite_delete_crash =
        create_test_crash_in_group(&app.db, &f.products[1].id, &readwrite_delete_crash_group).await;
    let readwrite_delete_group = create_test_crash_group(&app.db, &f.products[1].id).await;
    create_test_crash_in_group(&app.db, &f.products[1].id, &readwrite_delete_group).await;

    assert_eq!(
        app.call(
            "DELETE",
            &format!("/crashes/by-crash/{readonly_delete_crash}"),
            None,
            Some(&f.non_admin),
        )
        .await,
        StatusCode::FORBIDDEN,
        "read-only users cannot delete individual crashes"
    );
    assert_eq!(
        app.call(
            "DELETE",
            &format!("/crashes/by-crash/{readwrite_delete_crash}"),
            None,
            Some(&f.non_admin),
        )
        .await,
        StatusCode::NO_CONTENT,
        "read-write users can delete individual crashes"
    );
    assert_eq!(
        app.call(
            "GET",
            &format!("/crashes/by-crash/{readwrite_delete_crash}"),
            None,
            Some(&f.admin),
        )
        .await,
        StatusCode::NOT_FOUND,
        "deleted crash is gone"
    );

    assert_eq!(
        app.call(
            "DELETE",
            &format!("/crashes/{readonly_delete_group}"),
            None,
            Some(&f.non_admin),
        )
        .await,
        StatusCode::FORBIDDEN,
        "read-only users cannot delete crash groups"
    );
    assert_eq!(
        app.call(
            "DELETE",
            &format!("/crashes/{readwrite_delete_group}"),
            None,
            Some(&f.non_admin),
        )
        .await,
        StatusCode::NO_CONTENT,
        "read-write users can delete crash groups"
    );
    assert_eq!(
        app.call(
            "GET",
            &format!("/crashes/{readwrite_delete_group}"),
            None,
            Some(&f.admin),
        )
        .await,
        StatusCode::NOT_FOUND,
        "deleted crash group is gone"
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
        app.call(
            "POST",
            &format!("/crashes/{gid}/notes"),
            Some(note_body.clone()),
            Some(&f.admin)
        )
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
        (*app.storage)
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
// | Case                                       | Expected                              |
// | ------------------------------------------ | ------------------------------------- |
// | admin with missing user-text object in S3  | 200 with userText metadata (no body)  |
#[tokio::test]
async fn test_get_crash_user_text_not_in_store() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let gid = create_test_crash_group(&app.db, pid).await;
    let cid = create_test_crash_in_group(&app.db, pid, &gid).await;

    // "user-text" attachment in DB but NOT uploaded to object store.
    // get_crash no longer fetches S3 eagerly — userText metadata is always returned;
    // the body is loaded on-demand by the client via the attachment endpoint.
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
        !body["crash"]["userText"].is_null(),
        "userText metadata should be present even when S3 file is absent; body={body}",
    );
    assert_eq!(
        body["crash"]["userText"]["filename"].as_str(),
        Some("missing.txt"),
        "expected filename=missing.txt; body={body}",
    );
    assert!(
        body["crash"]["userText"]
            .get("body")
            .is_none_or(|b| b.is_null()),
        "body must not be eagerly fetched from S3; body={body}",
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

// ---------------------------------------------------------------------------
// Tests: db_api – list_group_crashes + inline group preview
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                       |
// | ------ | --------------------------- |
// | GET    | /crashes/{group_id}/crashes |
// Cases:
// | Case                                 | Expected                          |
// | ------------------------------------ | --------------------------------- |
// | admin, group with 7 crashes          | 200 with all 7 + total 7          |
// | admin, limit=2                       | 200 with 2 rows, total still 7    |
// | admin, limit=2&offset=6              | 200 with the 1 remaining row      |
// | admin, nonexistent group             | 200 with empty list, total 0      |
// | no_session on private product        | 200 with empty list (RLS)         |
#[tokio::test]
async fn test_list_group_crashes_handler() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let gid = create_test_crash_group(&app.db, pid).await;
    for _ in 0..7 {
        create_test_crash_in_group(&app.db, pid, &gid).await;
    }
    let uri = format!("/crashes/{gid}/crashes");

    let (status, body) = app.call_json("GET", &uri, None, Some(&f.admin)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"].as_u64(), Some(7));
    assert_eq!(body["crashes"].as_array().map(|a| a.len()), Some(7));
    assert!(body["crashes"][0]["id"].is_string());
    assert_eq!(body["crashes"][0]["groupId"].as_str(), Some(gid.as_str()));

    // limit trims the page but not the reported total
    let (status, body) = app
        .call_json("GET", &format!("{uri}?limit=2"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"].as_u64(), Some(7));
    assert_eq!(body["crashes"].as_array().map(|a| a.len()), Some(2));

    // offset walks past the first page
    let (status, body) = app
        .call_json("GET", &format!("{uri}?limit=2&offset=6"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["crashes"].as_array().map(|a| a.len()), Some(1));

    // Unknown group → empty, not an error
    let (status, body) = app
        .call_json("GET", "/crashes/nosuchgroup/crashes", None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"].as_u64(), Some(0));
    assert_eq!(body["crashes"].as_array().map(|a| a.len()), Some(0));

    // No session on a private product → RLS hides the rows
    let (status, body) = app.call_json("GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["crashes"].as_array().map(|a| a.len()), Some(0));
}

// API calls:
// | Method | Route                           |
// | ------ | ------------------------------- |
// | GET    | /crashes?productId={product_id} |
// Cases:
// | Case                              | Expected                              |
// | --------------------------------- | ------------------------------------- |
// | admin, group with 7 crashes       | 200, group carries 5-crash preview    |
#[tokio::test]
async fn test_list_groups_includes_crash_preview() {
    // The list view expands a group row from this inline preview, so it must
    // ship member crashes without a second request.
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let gid = create_test_crash_group(&app.db, pid).await;
    for _ in 0..7 {
        create_test_crash_in_group(&app.db, pid, &gid).await;
    }

    let (status, body) = app
        .call_json("GET", &format!("/crashes?productId={pid}"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    let group = body["groups"]
        .as_array()
        .expect("groups array")
        .iter()
        .find(|g| g["id"].as_str() == Some(gid.as_str()))
        .expect("group in list");
    assert_eq!(group["count"].as_u64(), Some(7));
    let preview = group["crashes"].as_array().expect("crash preview");
    assert_eq!(preview.len(), 5);
    assert!(preview[0]["id"].is_string());
    assert_eq!(preview[0]["version"].as_str(), Some("1.2.3"));
    assert!(preview[0].get("group_id").is_none());
}

// ---------------------------------------------------------------------------
// Tests: db_api – hasUserText filter and marker
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                        |
// | ------ | -------------------------------------------- |
// | GET    | /crashes?productId={id}&hasUserText=true      |
// | GET    | /crashes/{group_id}/crashes                   |
// Cases:
// | Case                                       | Expected                        |
// | ------------------------------------------ | ------------------------------- |
// | unfiltered list                            | both groups, marked per group   |
// | hasUserText=true                           | only the group with user text   |
// | hasUserText=false                          | both groups (no filtering)      |
// | inline preview                             | per-crash hasUserText flags     |
// | paged member crashes                       | same per-crash flags            |
#[tokio::test]
async fn test_list_groups_has_user_text_filter() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    // Group A: two crashes, only the second carries a user description.
    let group_a = create_test_crash_group(&app.db, pid).await;
    let plain_a = create_test_crash_in_group(&app.db, pid, &group_a).await;
    let described = create_test_crash_in_group(&app.db, pid, &group_a).await;
    create_test_attachment(
        &app.db,
        "user-text",
        "text/plain",
        15,
        "user-text.txt",
        Some(pid.to_string()),
        Some(described.clone()),
    )
    .await;

    // Group B: a crash with an ordinary attachment, which must not count.
    let group_b = create_test_crash_group(&app.db, pid).await;
    let plain_b = create_test_crash_in_group(&app.db, pid, &group_b).await;
    create_test_attachment(
        &app.db,
        "minidump",
        "application/octet-stream",
        10,
        "crash.dmp",
        Some(pid.to_string()),
        Some(plain_b.clone()),
    )
    .await;

    let group_ids = |body: &serde_json::Value| -> Vec<String> {
        body["groups"]
            .as_array()
            .expect("groups")
            .iter()
            .filter_map(|g| g["id"].as_str().map(str::to_string))
            .collect()
    };

    // Unfiltered: both groups, each flagged according to its contents.
    let (status, body) = app
        .call_json("GET", &format!("/crashes?productId={pid}"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    let ids = group_ids(&body);
    assert!(ids.contains(&group_a));
    assert!(ids.contains(&group_b));
    let flag_for = |body: &serde_json::Value, gid: &str| -> bool {
        body["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"].as_str() == Some(gid))
            .and_then(|g| g["hasUserText"].as_bool())
            .unwrap_or(false)
    };
    assert!(flag_for(&body, &group_a), "group A has a user description");
    assert!(!flag_for(&body, &group_b), "group B has none");

    // The inline preview marks the individual crashes.
    let preview_flag = |body: &serde_json::Value, gid: &str, cid: &str| -> bool {
        body["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"].as_str() == Some(gid))
            .and_then(|g| g["crashes"].as_array())
            .unwrap()
            .iter()
            .find(|c| c["id"].as_str() == Some(cid))
            .and_then(|c| c["hasUserText"].as_bool())
            .unwrap_or(false)
    };
    assert!(preview_flag(&body, &group_a, &described));
    assert!(!preview_flag(&body, &group_a, &plain_a));
    // Without the filter the preview still lists every member of the group.
    assert_eq!(
        body["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"].as_str() == Some(group_a.as_str()))
            .and_then(|g| g["crashes"].as_array())
            .map(Vec::len),
        Some(2)
    );

    // Filtered: only the group holding a user description, and within it only
    // the matching crash — group A also holds `plain_a`, which must not appear.
    let (status, body) = app
        .call_json(
            "GET",
            &format!("/crashes?productId={pid}&hasUserText=true"),
            None,
            Some(&f.admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(group_ids(&body), vec![group_a.clone()]);
    let filtered = &body["groups"][0];
    let listed: Vec<&str> = filtered["crashes"]
        .as_array()
        .expect("crashes")
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    assert_eq!(listed, vec![described.as_str()], "only the described crash is listed");
    // `count` stays the group's real size; `matchingCount` drives "+N more".
    assert_eq!(filtered["count"].as_u64(), Some(2));
    assert_eq!(filtered["matchingCount"].as_u64(), Some(1));
    // The sparkline is scaled by `count`, so the trend must keep counting every
    // crash in the group — narrowing the member list must not narrow it too.
    let trend_total: u64 = filtered["trend"]
        .as_array()
        .expect("trend")
        .iter()
        .filter_map(|v| v.as_u64())
        .sum();
    assert_eq!(trend_total, 2, "trend covers both crashes, not just the matching one");

    // "Load more" stays inside the same subset.
    let (status, body) = app
        .call_json(
            "GET",
            &format!("/crashes/{group_a}/crashes?hasUserText=true"),
            None,
            Some(&f.admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"].as_u64(), Some(1));
    let listed: Vec<&str> = body["crashes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    assert_eq!(listed, vec![described.as_str()]);

    // hasUserText=false must not filter anything out.
    let (status, body) = app
        .call_json(
            "GET",
            &format!("/crashes?productId={pid}&hasUserText=false"),
            None,
            Some(&f.admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(group_ids(&body).len() >= 2);

    // A description whose object was never stored is flagged unreadable, so the
    // list can mark it rather than promise text that cannot be opened.
    let listed_flag = |body: &serde_json::Value, gid: &str, cid: &str| -> Option<bool> {
        body["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"].as_str() == Some(gid))
            .and_then(|g| g["crashes"].as_array())
            .unwrap()
            .iter()
            .find(|c| c["id"].as_str() == Some(cid))
            .and_then(|c| c["userTextAvailable"].as_bool())
    };
    assert_eq!(
        listed_flag(&body, &group_a, &described),
        Some(false),
        "no object was stored for this description"
    );

    // Storing the object flips it, and crashes without a description are not
    // probed or flagged at all.
    {
        use object_store::ObjectStore as _;
        let path = app
            .db
            .query("SELECT VALUE storage_path FROM attachments WHERE name = 'user-text' LIMIT 1")
            .await
            .expect("query storage path")
            .take::<Vec<String>>(0)
            .expect("take storage path")
            .into_iter()
            .next()
            .expect("a user-text attachment exists");
        (*app.storage)
            .put_opts(
                &object_store::path::Path::from(path.as_str()),
                object_store::PutPayload::from_static(b"the description"),
                Default::default(),
            )
            .await
            .expect("put user-text failed");
    }
    let (_, body) = app
        .call_json(
            "GET",
            &format!("/crashes?productId={pid}&hasUserText=true"),
            None,
            Some(&f.admin),
        )
        .await;
    assert_eq!(listed_flag(&body, &group_a, &described), Some(true));

    let (_, body_all) = app
        .call_json("GET", &format!("/crashes?productId={pid}"), None, Some(&f.admin))
        .await;
    assert_eq!(
        listed_flag(&body_all, &group_a, &plain_a),
        None,
        "crashes without a description carry no availability flag"
    );

    // The paged member list carries the same per-crash flag.
    let (status, body) = app
        .call_json("GET", &format!("/crashes/{group_a}/crashes"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);
    let marked: Vec<&str> = body["crashes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["hasUserText"].as_bool() == Some(true))
        .filter_map(|c| c["id"].as_str())
        .collect();
    assert_eq!(marked, vec![described.as_str()]);

    // Anonymous callers never see the flag, even on a public product: the
    // attachments table grants select only to a product role, unlike crashes
    // and crash_groups which also allow product_id.public. Nothing leaks, but
    // the filter necessarily comes back empty for them.
    app.db
        .query("UPDATE type::record('products', $pid) SET public = true")
        .bind(("pid", pid.to_string()))
        .await
        .expect("publish product failed");
    let (status, body) = app
        .call_json("GET", &format!("/crashes?productId={pid}"), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body["groups"]
            .as_array()
            .expect("groups")
            .iter()
            .any(|g| g["hasUserText"].as_bool() == Some(true)),
        "attachments are invisible without a product role"
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – attachment availability
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                        |
// | ------ | ---------------------------- |
// | GET    | /crashes/by-crash/{crash_id} |
// Cases:
// | Case                                     | Expected            |
// | ---------------------------------------- | ------------------- |
// | attachment whose object exists           | available = true    |
// | attachment whose object was deleted      | available = false   |
// | user-text whose object was deleted       | available = false   |
#[tokio::test]
async fn test_get_crash_reports_lost_attachments() {
    use object_store::ObjectStore as _;

    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;
    let gid = create_test_crash_group(&app.db, pid).await;
    let cid = create_test_crash_in_group(&app.db, pid, &gid).await;

    // Present in storage.
    let kept = create_test_attachment(
        &app.db,
        "workrave.log",
        "text/plain",
        4,
        "workrave.log",
        Some(pid.to_string()),
        Some(cid.clone()),
    )
    .await;
    (*app.storage)
        .put_opts(
            &object_store::path::Path::from(kept.storage_path.as_str()),
            object_store::PutPayload::from_static(b"logs"),
            Default::default(),
        )
        .await
        .expect("put attachment failed");

    // Rows whose objects were never stored — what the orphan cleaner used to
    // leave behind. They must be reported as lost, not silently offered.
    create_test_attachment(
        &app.db,
        "workrave.1.log",
        "text/plain",
        4,
        "workrave.1.log",
        Some(pid.to_string()),
        Some(cid.clone()),
    )
    .await;
    create_test_attachment(
        &app.db,
        "user-text",
        "text/plain",
        9,
        "user-text.txt",
        Some(pid.to_string()),
        Some(cid.clone()),
    )
    .await;

    let (status, body) = app
        .call_json("GET", &format!("/crashes/by-crash/{cid}"), None, Some(&f.admin))
        .await;
    assert_eq!(status, StatusCode::OK);

    let available_for = |name: &str| -> Option<bool> {
        body["crash"]["attachments"]
            .as_array()
            .expect("attachments")
            .iter()
            .find(|a| a["name"].as_str() == Some(name))
            .and_then(|a| a["available"].as_bool())
    };
    assert_eq!(available_for("workrave.log"), Some(true));
    assert_eq!(available_for("workrave.1.log"), Some(false));

    // The user description is split out separately and carries the same flag.
    assert_eq!(body["crash"]["userText"]["available"].as_bool(), Some(false));
    assert!(
        body["crash"]["userText"]["attachmentId"].is_string(),
        "the row is kept so the crash still records that text was submitted"
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – version filtering across a group's crashes
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                     |
// | ------ | ----------------------------------------- |
// | GET    | /crashes?productId={id}&version={version} |
// | GET    | /crashes/{group_id}/crashes?version={v}   |
// Cases:
// | Case                                          | Expected                       |
// | --------------------------------------------- | ------------------------------ |
// | group holds 1.0 and 2.0, filter on the older   | group listed, only 1.0 members |
// | group holds only 2.0, filter on 1.0           | group not listed               |
// | "load more" with the same filter               | only 1.0 members, total 1      |
#[tokio::test]
async fn test_list_groups_version_filter_spans_group_members() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[0].id;

    let mixed = create_test_crash_group(&app.db, pid).await;
    let old_crash = create_test_crash_in_group(&app.db, pid, &mixed).await;
    let new_crash = create_test_crash_in_group(&app.db, pid, &mixed).await;
    // The newest crash carries the newer version, so matching on the group's
    // representative crash would hide this group when filtering on 1.0.0.
    for (cid, version) in [(&old_crash, "1.0.0"), (&new_crash, "2.0.0")] {
        app.db
            .query("UPDATE type::record('crashes', $cid) SET report.version = $v")
            .bind(("cid", cid.clone()))
            .bind(("v", version.to_string()))
            .await
            .expect("set version failed");
    }

    let newer_only = create_test_crash_group(&app.db, pid).await;
    let newer_crash = create_test_crash_in_group(&app.db, pid, &newer_only).await;
    app.db
        .query("UPDATE type::record('crashes', $cid) SET report.version = '2.0.0'")
        .bind(("cid", newer_crash.clone()))
        .await
        .expect("set version failed");

    let (status, body) = app
        .call_json(
            "GET",
            &format!("/crashes?productId={pid}&version=1.0.0"),
            None,
            Some(&f.admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let listed: Vec<&str> = body["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .filter_map(|g| g["id"].as_str())
        .collect();
    assert!(
        listed.contains(&mixed.as_str()),
        "a group holding a 1.0.0 crash must be listed even though its newest crash is 2.0.0"
    );
    assert!(
        !listed.contains(&newer_only.as_str()),
        "a group with no 1.0.0 crash must not be listed"
    );

    // Members are narrowed to the requested version, and `count` still reports
    // the group's real size.
    let group = body["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"].as_str() == Some(mixed.as_str()))
        .expect("mixed group listed");
    let members: Vec<&str> = group["crashes"]
        .as_array()
        .expect("crashes")
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    assert_eq!(members, vec![old_crash.as_str()]);
    assert_eq!(group["count"].as_u64(), Some(2));
    assert_eq!(group["matchingCount"].as_u64(), Some(1));

    // The version dropdown still offers every version in the product.
    let versions: Vec<&str> = body["versions"]
        .as_array()
        .expect("versions")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(versions.contains(&"1.0.0") && versions.contains(&"2.0.0"));

    // "Load more" stays inside the same subset.
    let (status, body) = app
        .call_json(
            "GET",
            &format!("/crashes/{mixed}/crashes?version=1.0.0"),
            None,
            Some(&f.admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"].as_u64(), Some(1));
    assert_eq!(body["crashes"][0]["id"].as_str(), Some(old_crash.as_str()));
}
