#![cfg(test)]

use sqlx::PgPool;
use uuid::Uuid;

use common::{QueryParams, SortOrder};
use data::version::*;
use repos::version::*;

use testware::{create_test_product, create_test_version};

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let name = "1.0.0";
    let hash = "abcdef123456";
    let tag = "v1.0.0";

    let _inserted_version = create_test_version(&pool, name, hash, tag, None).await;

    let name = "2.0.0";
    let hash = "abcdef98765";
    let tag = "v2.0.0";

    let inserted_version = create_test_version(&pool, name, hash, tag, None).await;

    let found_version = VersionRepo::get_by_id(&pool, inserted_version.id)
        .await
        .expect("Failed to get version by ID");

    assert!(found_version.is_some());
    let found_version = found_version.unwrap();
    assert_eq!(found_version.id, inserted_version.id);
    assert_eq!(found_version.name, name);
    assert_eq!(found_version.hash, hash);
    assert_eq!(found_version.tag, tag);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = VersionRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let name = "1.0.0-error";
    let hash = "abcde12345-error";
    let tag = "v1.0.0-error";

    let inserted_version = create_test_version(&pool, name, hash, tag, None).await;

    pool.close().await;

    let result = VersionRepo::get_by_id(&pool, inserted_version.id).await;
    assert!(result.is_err(), "Expected an error when getting version by ID with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_product_and_name(pool: PgPool) {
    let product = create_test_product(&pool).await;
    let name = "2.0.0";
    let hash = "fedcba654321";
    let tag = "v2.0.0";

    create_test_version(&pool, name, hash, tag, Some(product.id)).await;

    let product = create_test_product(&pool).await;
    let name = "3.0.0";
    let hash = "09fedcba654321";
    let tag = "v3.0.0";

    create_test_version(&pool, name, hash, tag, Some(product.id)).await;

    let found_version = VersionRepo::get_by_product_and_name(&pool, product.id, name)
        .await
        .expect("Failed to get version by product and name");

    assert!(found_version.is_some());
    let found_version = found_version.unwrap();
    assert_eq!(found_version.name, name);
    assert_eq!(found_version.product_id, product.id);

    let non_existent_name = "999.0.0";
    let not_found = VersionRepo::get_by_product_and_name(&pool, product.id, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_product_and_name_error(pool: PgPool) {
    let product = create_test_product(&pool).await;
    let name = "2.0.0-error";
    let hash = "hash-error";
    let tag = "v2.0.0-error";

    create_test_version(&pool, name, hash, tag, Some(product.id)).await;

    pool.close().await;

    let result = VersionRepo::get_by_product_and_name(&pool, product.id, name).await;
    assert!(
        result.is_err(),
        "Expected an error when getting version by product and name with closed pool"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names(pool: PgPool) {
    let product = create_test_product(&pool).await;
    let version_names = vec!["1.1.0", "1.2.0", "1.3.0", "2.0.0", "2.1.0"];

    for (i, name) in version_names.iter().enumerate() {
        let hash = format!("hash{i}");
        let tag = format!("v{name}");
        create_test_version(&pool, name, &hash, &tag, Some(product.id)).await;
    }

    let all_names = VersionRepo::get_all_names(&pool)
        .await
        .expect("Failed to get all version names");

    for name in version_names {
        assert!(all_names.contains(name));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names_error(pool: PgPool) {
    let product = create_test_product(&pool).await;
    create_test_version(&pool, "1.1.0-error", "hash1e", "v1.1.0-e", Some(product.id)).await;
    create_test_version(&pool, "1.2.0-error", "hash2e", "v1.2.0-e", Some(product.id)).await;

    pool.close().await;

    let result = VersionRepo::get_all_names(&pool).await;
    assert!(result.is_err(), "Expected an error when getting all version names with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let version_data = vec![
        ("3.0.0", "hash3", "v3.0.0"),
        ("3.1.0", "hash31", "v3.1.0"),
        ("3.2.0", "hash32", "v3.2.0"),
        ("2.0.0", "hash2", "v2.0.0"),
        ("2.1.0", "hash21", "v2.1.0"),
    ];

    for (name, hash, tag) in &version_data {
        create_test_version(&pool, name, hash, tag, Some(product.id)).await;
    }

    let query_params = QueryParams::default();
    let all_versions = VersionRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all versions");

    assert!(all_versions.len() >= version_data.len());

    let mut query_params = QueryParams::default();
    query_params
        .sorting
        .push_back(("name".to_string(), SortOrder::Descending));

    let sorted_versions = VersionRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get sorted versions");

    for i in 1..sorted_versions.len() {
        assert!(sorted_versions[i - 1].name >= sorted_versions[i].name);
    }

    let query_params = QueryParams {
        filter: Some("3.1".to_string()),
        ..QueryParams::default()
    };

    let filtered_versions = VersionRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get filtered versions");

    for version in filtered_versions {
        assert!(version.name.contains("3.1"));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_error(pool: PgPool) {
    let product = create_test_product(&pool).await;

    create_test_version(&pool, "3.0.0-error", "hash3e", "v3.0.0-e", Some(product.id)).await;
    create_test_version(&pool, "3.1.0-error", "hash31e", "v3.1.0-e", Some(product.id)).await;

    pool.close().await;

    let query_params = QueryParams::default();
    let result = VersionRepo::get_all(&pool, query_params).await;
    assert!(result.is_err(), "Expected an error when getting all versions with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let new_version = NewVersion {
        name: "4.0.0".to_string(),
        hash: "hash4".to_string(),
        tag: "v4.0.0".to_string(),
        product_id: product.id,
    };

    let version_id = VersionRepo::create(&pool, new_version.clone())
        .await
        .expect("Failed to create version");

    let created_version = VersionRepo::get_by_id(&pool, version_id)
        .await
        .expect("Failed to get created version")
        .expect("Created version not found");

    assert_eq!(created_version.name, new_version.name);
    assert_eq!(created_version.hash, new_version.hash);
    assert_eq!(created_version.tag, new_version.tag);
    assert_eq!(created_version.product_id, new_version.product_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_name_not_unique(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let new_version = NewVersion {
        name: "4.0.0".to_string(),
        hash: "hash4".to_string(),
        tag: "v4.0.0".to_string(),
        product_id: product.id,
    };

    VersionRepo::create(&pool, new_version.clone())
        .await
        .expect("Failed to create version");

    let duplicate_version = NewVersion {
        name: "4.0.0".to_string(),
        hash: "hash4-duplicate".to_string(),
        tag: "v4.0.0-duplicate".to_string(),
        product_id: product.id,
    };

    let duplicate_version_id = VersionRepo::create(&pool, duplicate_version.clone()).await;

    assert!(
        duplicate_version_id.is_err(),
        "Expected an error when creating a version with a non-unique name"
    );
    assert_eq!(duplicate_version_id.unwrap_err().to_string(), "database uniqueness violation")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_hash_not_unique(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let new_version = NewVersion {
        name: "4.0.0".to_string(),
        hash: "hash4".to_string(),
        tag: "v4.0.0".to_string(),
        product_id: product.id,
    };

    VersionRepo::create(&pool, new_version.clone())
        .await
        .expect("Failed to create version");

    let duplicate_version = NewVersion {
        name: "4.0.0-duplicate".to_string(),
        hash: "hash4".to_string(),
        tag: "v4.0.0-duplicate".to_string(),
        product_id: product.id,
    };

    let duplicate_version_id = VersionRepo::create(&pool, duplicate_version.clone()).await;

    assert!(
        duplicate_version_id.is_err(),
        "Expected an error when creating a version with a non-unique name"
    );
    assert_eq!(duplicate_version_id.unwrap_err().to_string(), "database uniqueness violation")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_tag_not_unique(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let new_version = NewVersion {
        name: "4.0.0".to_string(),
        hash: "hash4".to_string(),
        tag: "v4.0.0".to_string(),
        product_id: product.id,
    };

    VersionRepo::create(&pool, new_version.clone())
        .await
        .expect("Failed to create version");

    let duplicate_version = NewVersion {
        name: "4.0.0-duplicate".to_string(),
        hash: "hash4-duplicate".to_string(),
        tag: "v4.0.0".to_string(),
        product_id: product.id,
    };

    let duplicate_version_id = VersionRepo::create(&pool, duplicate_version.clone()).await;

    assert!(
        duplicate_version_id.is_err(),
        "Expected an error when creating a version with a non-unique name"
    );
    assert_eq!(duplicate_version_id.unwrap_err().to_string(), "database uniqueness violation")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create_error(pool: PgPool) {
    let product = create_test_product(&pool).await;

    let new_version = NewVersion {
        name: "4.0.0-error".to_string(),
        hash: "hash4e".to_string(),
        tag: "v4.0.0-e".to_string(),
        product_id: product.id,
    };

    pool.close().await;

    let result = VersionRepo::create(&pool, new_version).await;
    assert!(result.is_err(), "Expected an error when creating version with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut version = create_test_version(&pool, "5.0.0", "hash5", "v5.0.0", None).await;

    version.name = "5.1.0".to_string();
    version.hash = "hash51".to_string();
    version.tag = "v5.1.0".to_string();

    let updated_id = VersionRepo::update(&pool, version.clone())
        .await
        .expect("Failed to update version")
        .expect("Version not found when updating");

    assert_eq!(updated_id, version.id);

    let updated_version = VersionRepo::get_by_id(&pool, version.id)
        .await
        .expect("Failed to get updated version")
        .expect("Updated version not found");

    assert_eq!(updated_version.name, "5.1.0");
    assert_eq!(updated_version.hash, "hash51");
    assert_eq!(updated_version.tag, "v5.1.0");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_error(pool: PgPool) {
    let mut version = create_test_version(&pool, "5.0.0-error", "hash5e", "v5.0.0-e", None).await;

    version.name = "5.1.0-error".to_string();
    version.hash = "hash51e".to_string();
    version.tag = "v5.1.0-e".to_string();

    pool.close().await;

    let result = VersionRepo::update(&pool, version.clone()).await;
    assert!(result.is_err(), "Expected an error when updating version with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let version = create_test_version(&pool, "6.0.0", "hash6", "v6.0.0", None).await;

    VersionRepo::remove(&pool, version.id)
        .await
        .expect("Failed to remove version");

    let deleted_version = VersionRepo::get_by_id(&pool, version.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_version.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove_error(pool: PgPool) {
    let version = create_test_version(&pool, "6.0.0-error", "hash6e", "v6.0.0-e", None).await;

    pool.close().await;

    let result = VersionRepo::remove(&pool, version.id).await;
    assert!(result.is_err(), "Expected an error when removing version with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = VersionRepo::count(&pool)
        .await
        .expect("Failed to count initial versions");

    let product = create_test_product(&pool).await;

    let test_versions = vec![
        ("7.0.0", "hash7", "v7.0.0"),
        ("7.1.0", "hash71", "v7.1.0"),
        ("7.2.0", "hash72", "v7.2.0"),
    ];

    for (name, hash, tag) in &test_versions {
        create_test_version(&pool, name, hash, tag, Some(product.id)).await;
    }

    let new_count = VersionRepo::count(&pool)
        .await
        .expect("Failed to count versions after insertion");

    assert_eq!(new_count, initial_count + test_versions.len() as i64);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count_error(pool: PgPool) {
    let product = create_test_product(&pool).await;

    create_test_version(&pool, "7.0.0-error", "hash7e", "v7.0.0-e", Some(product.id)).await;
    create_test_version(&pool, "7.1.0-error", "hash71e", "v7.1.0-e", Some(product.id)).await;

    pool.close().await;

    let result = VersionRepo::count(&pool).await;
    assert!(result.is_err(), "Expected an error when counting versions with closed pool");
}
