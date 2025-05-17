#![cfg(test)]

use sqlx::PgPool;
use uuid::Uuid;

use common::QueryParams;
use data::symbols::*;
use repos::symbols::*;

use testware::{create_test_product, create_test_symbols};

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let os = "linux";
    let arch = "x86_64";
    let build_id = "build123";
    let module_id = "module123";
    let storage_location = "/path/to/symbols";

    let inserted_symbols =
        create_test_symbols(&pool, os, arch, build_id, module_id, storage_location, None).await;

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
    assert_eq!(found_symbols.storage_location, storage_location);
}
#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = SymbolsRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let os = "linux";
    let arch = "x86_64";
    let build_id = "build123error";
    let module_id = "module123error";
    let storage_location = "/path/to/symbols_error";

    let inserted_symbols =
        create_test_symbols(&pool, os, arch, build_id, module_id, storage_location, None).await;

    pool.close().await;

    let result = SymbolsRepo::get_by_id(&pool, inserted_symbols.id).await;
    assert!(result.is_err(), "Expected an error when getting symbols by ID with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_error(pool: PgPool) {
    let product = create_test_product(&pool).await;

    create_test_symbols(
        &pool,
        "linux-err",
        "x86_64-err",
        "build-err",
        "module-err",
        "/path/to/err",
        Some(product.id),
    )
    .await;

    pool.close().await;

    let query_params = QueryParams::default();
    let result = SymbolsRepo::get_all(&pool, query_params).await;
    assert!(result.is_err(), "Expected an error when getting all symbols with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let test_symbol_data = vec![
        ("Linux", "x86_64", "build-linux-1", "module-1", "/path/to/linux/symbol"),
        ("Windows", "x86_64", "build-win-1", "module-2", "/path/to/windows/symbol"),
        ("macOS", "arm64", "build-mac-1", "module-3", "/path/to/macos/symbol"),
    ];

    for (os, arch, build_id, module_id, storage_location) in &test_symbol_data {
        create_test_symbols(
            &pool,
            os,
            arch,
            build_id,
            module_id,
            storage_location,
            Some(product.id),
        )
        .await;
    }

    let query_params = QueryParams::default();
    let all_symbols = SymbolsRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all symbols");

    assert!(all_symbols.len() >= test_symbol_data.len());

    let query_params = QueryParams {
        filter: Some("Windows".to_string()),
        ..QueryParams::default()
    };

    let filtered_symbols = SymbolsRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get filtered symbols");

    assert!(!filtered_symbols.is_empty());
    for symbol in &filtered_symbols {
        assert!(
            symbol.os.contains("Windows")
                || symbol.arch.contains("Windows")
                || symbol.build_id.contains("Windows")
                || symbol.module_id.contains("Windows")
                || symbol.storage_location.contains("Windows")
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let new_symbols = NewSymbols {
        os: "macos".to_string(),
        arch: "arm64".to_string(),
        build_id: "build_apple".to_string(),
        module_id: "module_apple".to_string(),
        storage_location: "/path/to/apple_symbols".to_string(),
        product_id: product.id,
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
    assert_eq!(created_symbols.storage_location, new_symbols.storage_location);
    assert_eq!(created_symbols.product_id, new_symbols.product_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_error(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let new_symbols = NewSymbols {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        build_id: "build123error".to_string(),
        module_id: "module123error".to_string(),
        storage_location: "/path/to/symbols_error".to_string(),
        product_id: product.id,
    };

    pool.close().await;

    let result = SymbolsRepo::create(&pool, new_symbols).await;
    assert!(result.is_err(), "Expected an error when creating symbols with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut symbols =
        create_test_symbols(&pool, "linux", "arm64", "build_old", "module_old", "/path/old", None)
            .await;

    symbols.os = "ios".to_string();
    symbols.arch = "arm64e".to_string();
    symbols.build_id = "build_new".to_string();
    symbols.module_id = "module_new".to_string();
    symbols.storage_location = "/path/new".to_string();

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
    assert_eq!(updated_symbols.storage_location, "/path/new");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_error(pool: PgPool) {
    let mut symbols = create_test_symbols(
        &pool,
        "linux-update-err",
        "arm64-err",
        "build_old_err",
        "module_old_err",
        "/path/old_err",
        None,
    )
    .await;

    symbols.os = "ios-err".to_string();
    symbols.arch = "arm64e-err".to_string();
    symbols.storage_location = "/path/new_err".to_string();

    pool.close().await;

    let result = SymbolsRepo::update(&pool, symbols.clone()).await;
    assert!(result.is_err(), "Expected an error when updating symbols with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let symbols = create_test_symbols(
        &pool,
        "android",
        "arm",
        "build_android",
        "module_android",
        "/path/android",
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
async fn test_remove_error(pool: PgPool) {
    let symbols = create_test_symbols(
        &pool,
        "android-err",
        "arm-err",
        "build_android_err",
        "module_android_err",
        "/path/android_err",
        None,
    )
    .await;

    pool.close().await;

    let result = SymbolsRepo::remove(&pool, symbols.id).await;
    assert!(result.is_err(), "Expected an error when removing symbols with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = SymbolsRepo::count(&pool)
        .await
        .expect("Failed to count initial symbols");

    let product = create_test_product(&pool).await;

    let test_symbols_data = vec![
        ("freebsd", "x86", "build_f1", "module_f1", "/path/f1"),
        ("openbsd", "x86", "build_o1", "module_o1", "/path/o1"),
        ("netbsd", "x86", "build_n1", "module_n1", "/path/n1"),
    ];

    for (os, arch, build_id, module_id, storage_location) in &test_symbols_data {
        create_test_symbols(
            &pool,
            os,
            arch,
            build_id,
            module_id,
            storage_location,
            Some(product.id),
        )
        .await;
    }

    let new_count = SymbolsRepo::count(&pool)
        .await
        .expect("Failed to count symbols after insertion");

    assert_eq!(new_count, initial_count + test_symbols_data.len() as i64);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count_error(pool: PgPool) {
    let product = create_test_product(&pool).await;

    create_test_symbols(
        &pool,
        "count-err",
        "x86-err",
        "build_count_err",
        "module_count_err",
        "/path/count_err",
        Some(product.id),
    )
    .await;

    pool.close().await;

    let result = SymbolsRepo::count(&pool).await;
    assert!(result.is_err(), "Expected an error when counting symbols with closed pool");
}
