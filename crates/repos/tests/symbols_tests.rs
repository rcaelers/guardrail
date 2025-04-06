#![cfg(all(test, feature = "ssr"))]

use sqlx::PgPool;
use uuid::Uuid;

use repos::QueryParams;
use repos::symbols::*;

async fn setup_test_dependencies(pool: &PgPool) -> (Uuid, Uuid) {
    // Create product first
    let product_id = sqlx::query_scalar!(
        r#"
        INSERT INTO guardrail.products (name, description)
        VALUES ($1, $2)
        RETURNING id
        "#,
        format!("TestProduct_{}", Uuid::new_v4()),
        "Test Product Description"
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test product");

    // Then create version
    let version_id = sqlx::query_scalar!(
        r#"
        INSERT INTO guardrail.versions (name, hash, tag, product_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        format!("Version_{}", Uuid::new_v4()),
        format!("Hash_{}", Uuid::new_v4()),
        format!("Tag_{}", Uuid::new_v4()),
        product_id
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test version");

    (product_id, version_id)
}

async fn insert_test_symbols(
    pool: &PgPool,
    os: &str,
    arch: &str,
    build_id: &str,
    module_id: &str,
    file_location: &str,
    product_id: Option<Uuid>,
    version_id: Option<Uuid>,
) -> Symbols {
    let (product_id, version_id) = match (product_id, version_id) {
        (Some(p), Some(v)) => (p, v),
        _ => setup_test_dependencies(pool).await,
    };

    sqlx::query_as!(
        Symbols,
        r#"
        INSERT INTO guardrail.symbols (
            os, arch, build_id, module_id, file_location,
            product_id, version_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, os, arch, build_id, module_id, file_location,
                 product_id, version_id, created_at, updated_at
        "#,
        os,
        arch,
        build_id,
        module_id,
        file_location,
        product_id,
        version_id
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test symbols")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let os = "linux";
    let arch = "x86_64";
    let build_id = "build123";
    let module_id = "module123";
    let file_location = "/path/to/symbols";

    let inserted_symbols =
        insert_test_symbols(&pool, os, arch, build_id, module_id, file_location, None, None).await;

    let found_symbols = SymbolsRepo::get_by_id(&pool, inserted_symbols.id)
        .await
        .expect("Failed to get symbols by ID");

    assert!(found_symbols.is_some());
    let found_symbols = found_symbols.unwrap();
    assert_eq!(found_symbols.id, inserted_symbols.id);
    assert_eq!(found_symbols.os, os);
    assert_eq!(found_symbols.arch, arch);
    assert_eq!(found_symbols.build_id, build_id);
    assert_eq!(found_symbols.module_id, module_id);
    assert_eq!(found_symbols.file_location, file_location);

    let non_existent_id = Uuid::new_v4();
    let not_found = SymbolsRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let test_symbol_data = vec![
        ("Linux", "x86_64", "build-linux-1", "module-1", "/path/to/linux/symbol"),
        ("Windows", "x86_64", "build-win-1", "module-2", "/path/to/windows/symbol"),
        ("macOS", "arm64", "build-mac-1", "module-3", "/path/to/macos/symbol"),
    ];

    for (os, arch, build_id, module_id, file_location) in &test_symbol_data {
        insert_test_symbols(
            &pool,
            os,
            arch,
            build_id,
            module_id,
            file_location,
            Some(product_id),
            Some(version_id),
        )
        .await;
    }

    // Test get_all with no params
    let query_params = QueryParams::default();
    let all_symbols = SymbolsRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all symbols");

    assert!(all_symbols.len() >= test_symbol_data.len());

    // Test with filtering - using a filter string that exists in the test data
    let query_params = QueryParams {
        filter: Some("Windows".to_string()),
        ..QueryParams::default()
    };

    let filtered_symbols = SymbolsRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get filtered symbols");

    // Verify at least one result with the Windows filter
    assert!(!filtered_symbols.is_empty());
    for symbol in &filtered_symbols {
        assert!(
            symbol.os.contains("Windows")
                || symbol.arch.contains("Windows")
                || symbol.build_id.contains("Windows")
                || symbol.module_id.contains("Windows")
                || symbol.file_location.contains("Windows")
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let new_symbols = NewSymbols {
        os: "macos".to_string(),
        arch: "arm64".to_string(),
        build_id: "build_apple".to_string(),
        module_id: "module_apple".to_string(),
        file_location: "/path/to/apple_symbols".to_string(),
        product_id,
        version_id,
    };

    let symbols_id = SymbolsRepo::create(&pool, new_symbols.clone())
        .await
        .expect("Failed to create symbols");

    let created_symbols = SymbolsRepo::get_by_id(&pool, symbols_id)
        .await
        .expect("Failed to get created symbols")
        .expect("Created symbols not found");

    assert_eq!(created_symbols.os, new_symbols.os);
    assert_eq!(created_symbols.arch, new_symbols.arch);
    assert_eq!(created_symbols.build_id, new_symbols.build_id);
    assert_eq!(created_symbols.module_id, new_symbols.module_id);
    assert_eq!(created_symbols.file_location, new_symbols.file_location);
    assert_eq!(created_symbols.product_id, new_symbols.product_id);
    assert_eq!(created_symbols.version_id, new_symbols.version_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut symbols = insert_test_symbols(
        &pool,
        "linux",
        "arm64",
        "build_old",
        "module_old",
        "/path/old",
        None,
        None,
    )
    .await;

    symbols.os = "ios".to_string();
    symbols.arch = "arm64e".to_string();
    symbols.build_id = "build_new".to_string();
    symbols.module_id = "module_new".to_string();
    symbols.file_location = "/path/new".to_string();

    let updated_id = SymbolsRepo::update(&pool, symbols.clone())
        .await
        .expect("Failed to update symbols")
        .expect("Symbols not found when updating");

    assert_eq!(updated_id, symbols.id);

    let updated_symbols = SymbolsRepo::get_by_id(&pool, symbols.id)
        .await
        .expect("Failed to get updated symbols")
        .expect("Updated symbols not found");

    assert_eq!(updated_symbols.os, "ios");
    assert_eq!(updated_symbols.arch, "arm64e");
    assert_eq!(updated_symbols.build_id, "build_new");
    assert_eq!(updated_symbols.module_id, "module_new");
    assert_eq!(updated_symbols.file_location, "/path/new");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let symbols = insert_test_symbols(
        &pool,
        "android",
        "arm",
        "build_android",
        "module_android",
        "/path/android",
        None,
        None,
    )
    .await;

    SymbolsRepo::remove(&pool, symbols.id)
        .await
        .expect("Failed to remove symbols");

    let deleted_symbols = SymbolsRepo::get_by_id(&pool, symbols.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_symbols.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = SymbolsRepo::count(&pool)
        .await
        .expect("Failed to count initial symbols");

    let (product_id, version_id) = setup_test_dependencies(&pool).await;

    let test_symbols_data = vec![
        ("freebsd", "x86", "build_f1", "module_f1", "/path/f1"),
        ("openbsd", "x86", "build_o1", "module_o1", "/path/o1"),
        ("netbsd", "x86", "build_n1", "module_n1", "/path/n1"),
    ];

    for (os, arch, build_id, module_id, file_location) in &test_symbols_data {
        insert_test_symbols(
            &pool,
            os,
            arch,
            build_id,
            module_id,
            file_location,
            Some(product_id),
            Some(version_id),
        )
        .await;
    }

    let new_count = SymbolsRepo::count(&pool)
        .await
        .expect("Failed to count symbols after insertion");

    assert_eq!(new_count, initial_count + test_symbols_data.len() as i64);
}
