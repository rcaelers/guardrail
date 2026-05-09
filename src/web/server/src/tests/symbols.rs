use super::common::*;

// ---------------------------------------------------------------------------
// Tests: symbol upload (product-maintainer)
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                          |
// | ------ | ------------------------------ |
// | POST   | /products/{product_id}/symbols |
// Cases:
// | Auth/product role                      | Expected |
// | -------------------------------------- | -------- |
// | no_session                             | 403      |
// | admin                                  | 200      |
// | imp_admin                              | 200      |
// | non_admin or imp_non_admin: no access  | 403      |
// | non_admin or imp_non_admin: read-only  | 403      |
// | non_admin or imp_non_admin: read-write | 403      |
// | non_admin or imp_non_admin: maintainer | 200      |
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

// API calls:
// | Method | Route                |
// | ------ | -------------------- |
// | DELETE | /symbols/{symbol_id} |
// Cases:
// | Case                                   | Expected |
// | -------------------------------------- | -------- |
// | no_session                             | 403      |
// | admin with nonexistent symbol          | 404      |
// | non_admin with nonexistent symbol      | 404      |
// | non_admin with read-write product role | 403      |
// | non_admin with maintainer product role | 204      |
#[tokio::test]
async fn test_delete_symbol_requires_session() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    assert_session_only_not_forbidden(&app, &f, "DELETE", "/symbols/nonexistent", None).await;

    let rw_symbol = testware::create_test_symbols(
        &app.db,
        "linux",
        "x86_64",
        "rw-build",
        "rw-module",
        "symbols/rw",
        Some(f.products[1].id.clone()),
    )
    .await;
    assert_eq!(
        app.call("DELETE", &format!("/symbols/{}", rw_symbol.id), None, Some(&f.non_admin))
            .await,
        StatusCode::FORBIDDEN,
        "read-write users cannot delete symbols"
    );

    let maint_symbol = testware::create_test_symbols(
        &app.db,
        "linux",
        "x86_64",
        "maint-build",
        "maint-module",
        "symbols/maint",
        Some(f.products[2].id.clone()),
    )
    .await;
    assert_eq!(
        app.call("DELETE", &format!("/symbols/{}", maint_symbol.id), None, Some(&f.non_admin))
            .await,
        StatusCode::NO_CONTENT,
        "maintainers can delete symbols"
    );
}

// ---------------------------------------------------------------------------
// Tests: symbol read endpoint with filters
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                         |
// | ------ | --------------------------------------------- |
// | GET    | /products/{product_id}/symbols                |
// | GET    | /products/{product_id}/symbols?search={query} |
// | GET    | /products/{product_id}/symbols?arch={arch}    |
// | GET    | /products/{product_id}/symbols?sort={sort}    |
// Cases:
// | Case                        | Expected |
// | --------------------------- | -------- |
// | plain list as admin         | 200      |
// | read-only member sees row   | 200      |
// | no-session private product  | empty    |
// | no-session public product   | sees row |
// | search filter as admin      | 200      |
// | arch=x86_64 filter as admin | 200      |
// | arch=all filter as admin    | 200      |
// | sort=name as admin          | 200      |
// | sort=size as admin          | 200      |
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
    let (status, readonly_symbols) = app.call_json("GET", &base, None, Some(&f.non_admin)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        readonly_symbols
            .as_array()
            .expect("symbols response should be an array")
            .len(),
        1,
        "read-only-or-better user should see private product symbols"
    );
    let (status, anonymous_private_symbols) = app.call_json("GET", &base, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        anonymous_private_symbols
            .as_array()
            .expect("symbols response should be an array")
            .is_empty(),
        "anonymous users must not see private product symbols"
    );

    app.db
        .query("UPDATE type::record('products', $pid) SET public = true")
        .bind(("pid", pid.to_string()))
        .await
        .expect("mark symbol product public failed");
    let (status, anonymous_public_symbols) = app.call_json("GET", &base, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        anonymous_public_symbols
            .as_array()
            .expect("symbols response should be an array")
            .len(),
        1,
        "anonymous users can see public product symbols"
    );
    // search filter
    assert_eq!(
        app.call("GET", &format!("{base}?search=app"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    // arch filter
    assert_eq!(
        app.call("GET", &format!("{base}?arch=x86_64"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}?arch=all"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    // sort variants
    assert_eq!(
        app.call("GET", &format!("{base}?sort=name"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}?sort=size"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
}

// ---------------------------------------------------------------------------
// Tests: db_api – list_symbols format / sort variants
// ---------------------------------------------------------------------------

// API calls:
// | Method | Route                                          |
// | ------ | ---------------------------------------------- |
// | GET    | /products/{product_id}/symbols?format={format} |
// | GET    | /products/{product_id}/symbols?sort={sort}     |
// | GET    | /products/{product_id}/symbols                 |
// Cases:
// | Case                     | Expected |
// | ------------------------ | -------- |
// | format=Breakpad as admin | 200      |
// | format=unknown as admin  | 200      |
// | sort=name as admin       | 200      |
// | sort=size as admin       | 200      |
// | default sort as admin    | 200      |
#[tokio::test]
async fn test_list_symbols_format_sort() {
    let app = TestApp::new().await;
    let f = Fixture::setup(&app).await;
    let pid = &f.products[2].id; // products[2]: non_admin has maintainer role

    // Create two symbols with the same name but different build IDs so the
    // then_with() comparator in the "name" sort fires (same name → compare version).
    testware::create_test_symbols(
        &app.db,
        "linux",
        "x86_64",
        "build1",
        "libfoo",
        "syms/foo1",
        Some(pid.clone()),
    )
    .await;
    testware::create_test_symbols(
        &app.db,
        "linux",
        "arm64",
        "build2",
        "libfoo",
        "syms/foo2",
        Some(pid.clone()),
    )
    .await;

    let base = format!("/products/{pid}/symbols");

    // format filter ("Breakpad" is the hardcoded format in SYMBOL_PROJ)
    assert_eq!(
        app.call("GET", &format!("{base}?format=Breakpad"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    assert_eq!(
        app.call("GET", &format!("{base}?format=unknown"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );

    // Sort by name (comparator needs 2+ elements)
    assert_eq!(
        app.call("GET", &format!("{base}?sort=name"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    // Sort by size
    assert_eq!(
        app.call("GET", &format!("{base}?sort=size"), None, Some(&f.admin))
            .await,
        StatusCode::OK
    );
    // Default sort (uploadedAt)
    assert_eq!(app.call("GET", &base, None, Some(&f.admin)).await, StatusCode::OK);
}
