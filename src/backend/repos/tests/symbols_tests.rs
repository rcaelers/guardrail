#![cfg(test)]

use testware::setup::TestSetup;
use uuid::Uuid;

use common::QueryParams;
use data::symbols::*;
use repos::symbols::*;

use testware::{create_test_product, create_test_symbols};

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let os = "linux";
    let arch = "x86_64";
    let build_id = "build123";
    let module_id = "module123";
    let storage_path = "/path/to/symbols";

    let inserted_symbols =
        create_test_symbols(&db, os, arch, build_id, module_id, storage_path, None).await;

    let found_symbols = SymbolsRepo::get_by_id(&db, inserted_symbols.id)
        .await
        .expect("Failed to get symbols by ID");

    assert!(found_symbols.is_some());
    let found_symbols = found_symbols.unwrap();
    assert_eq!(found_symbols.id, inserted_symbols.id);
    assert_eq!(found_symbols.os, os);
    assert_eq!(found_symbols.arch, arch);
    assert_eq!(found_symbols.build_id, build_id);
    assert_eq!(found_symbols.module_id, module_id);
    assert_eq!(found_symbols.storage_path, storage_path);
}
#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4();
    let not_found = SymbolsRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_get_all() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;

    let test_symbol_data = vec![
        ("Linux", "x86_64", "build-linux-1", "module-1", "/path/to/linux/symbol"),
        ("Windows", "x86_64", "build-win-1", "module-2", "/path/to/windows/symbol"),
        ("macOS", "arm64", "build-mac-1", "module-3", "/path/to/macos/symbol"),
    ];

    for (os, arch, build_id, module_id, storage_path) in &test_symbol_data {
        create_test_symbols(&db, os, arch, build_id, module_id, storage_path, Some(product.id))
            .await;
    }

    let query_params = QueryParams::default();
    let all_symbols = SymbolsRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get all symbols");

    assert!(all_symbols.len() >= test_symbol_data.len());

    let query_params = QueryParams {
        filter: Some("Windows".to_string()),
        ..QueryParams::default()
    };

    let filtered_symbols = SymbolsRepo::get_all(&db, query_params)
        .await
        .expect("Failed to get filtered symbols");

    assert!(!filtered_symbols.is_empty());
    for symbol in &filtered_symbols {
        assert!(
            symbol.os.contains("Windows")
                || symbol.arch.contains("Windows")
                || symbol.build_id.contains("Windows")
                || symbol.module_id.contains("Windows")
                || symbol.storage_path.contains("Windows")
        );
    }
}

#[tokio::test]
async fn test_create() {
    let db = TestSetup::create_db().await;
    let product = create_test_product(&db).await;

    let new_symbols = NewSymbols {
        os: "macos".to_string(),
        arch: "arm64".to_string(),
        build_id: "build_apple".to_string(),
        module_id: "module_apple".to_string(),
        storage_path: "/path/to/apple_symbols".to_string(),
        product_id: product.id.to_string(),
    };

    let symbols_id = SymbolsRepo::create(&db, new_symbols.clone())
        .await
        .expect("Failed to create symbols");

    let created_symbols = SymbolsRepo::get_by_id(&db, symbols_id)
        .await
        .expect("Failed to get created symbols")
        .expect("Created symbols not found");

    assert_eq!(created_symbols.os, new_symbols.os);
    assert_eq!(created_symbols.arch, new_symbols.arch);
    assert_eq!(created_symbols.build_id, new_symbols.build_id);
    assert_eq!(created_symbols.module_id, new_symbols.module_id);
    assert_eq!(created_symbols.storage_path, new_symbols.storage_path);
    assert_eq!(created_symbols.product_id, new_symbols.product_id);
}

#[tokio::test]
async fn test_update() {
    let db = TestSetup::create_db().await;
    let mut symbols =
        create_test_symbols(&db, "linux", "arm64", "build_old", "module_old", "/path/old", None)
            .await;

    symbols.os = "ios".to_string();
    symbols.arch = "arm64e".to_string();
    symbols.build_id = "build_new".to_string();
    symbols.module_id = "module_new".to_string();
    symbols.storage_path = "/path/new".to_string();

    let updated_id = SymbolsRepo::update(&db, symbols.clone())
        .await
        .expect("Failed to update symbols")
        .expect("Symbols not found when updating");

    assert_eq!(updated_id, symbols.id);

    let updated_symbols = SymbolsRepo::get_by_id(&db, symbols.id)
        .await
        .expect("Failed to get updated symbols")
        .expect("Updated symbols not found");

    assert_eq!(updated_symbols.os, "ios");
    assert_eq!(updated_symbols.arch, "arm64e");
    assert_eq!(updated_symbols.build_id, "build_new");
    assert_eq!(updated_symbols.module_id, "module_new");
    assert_eq!(updated_symbols.storage_path, "/path/new");
}

#[tokio::test]
async fn test_remove() {
    let db = TestSetup::create_db().await;
    let symbols = create_test_symbols(
        &db,
        "android",
        "arm",
        "build_android",
        "module_android",
        "/path/android",
        None,
    )
    .await;

    SymbolsRepo::remove(&db, symbols.id)
        .await
        .expect("Failed to remove symbols");

    let deleted_symbols = SymbolsRepo::get_by_id(&db, symbols.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_symbols.is_none());
}

#[tokio::test]
async fn test_count() {
    let db = TestSetup::create_db().await;
    let initial_count = SymbolsRepo::count(&db)
        .await
        .expect("Failed to count initial symbols");

    let product = create_test_product(&db).await;

    let test_symbols_data = vec![
        ("freebsd", "x86", "build_f1", "module_f1", "/path/f1"),
        ("openbsd", "x86", "build_o1", "module_o1", "/path/o1"),
        ("netbsd", "x86", "build_n1", "module_n1", "/path/n1"),
    ];

    for (os, arch, build_id, module_id, storage_path) in &test_symbols_data {
        create_test_symbols(&db, os, arch, build_id, module_id, storage_path, Some(product.id))
            .await;
    }

    let new_count = SymbolsRepo::count(&db)
        .await
        .expect("Failed to count symbols after insertion");

    assert_eq!(new_count, initial_count + test_symbols_data.len() as i64);
}
