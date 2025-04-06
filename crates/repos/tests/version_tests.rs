#![cfg(all(test, feature = "ssr"))]

use sqlx::PgPool;
use uuid::Uuid;

use repos::version::*;
use repos::{QueryParams, SortOrder};

async fn insert_test_product(pool: &PgPool) -> Uuid {
    sqlx::query_scalar!(
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
    .expect("Failed to insert test product")
}

async fn insert_test_version(
    pool: &PgPool,
    name: &str,
    hash: &str,
    tag: &str,
    product_id: Option<Uuid>,
) -> Version {
    let product_id = match product_id {
        Some(id) => id,
        None => insert_test_product(pool).await,
    };

    sqlx::query_as!(
        Version,
        r#"
        INSERT INTO guardrail.versions (name, hash, tag, product_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, hash, tag, product_id, created_at, updated_at
        "#,
        name,
        hash,
        tag,
        product_id
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test version")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let name = "1.0.0";
    let hash = "abcdef123456";
    let tag = "v1.0.0";

    let inserted_version = insert_test_version(&pool, name, hash, tag, None).await;

    let found_version = VersionRepo::get_by_id(&pool, inserted_version.id)
        .await
        .expect("Failed to get version by ID");

    assert!(found_version.is_some());
    let found_version = found_version.unwrap();
    assert_eq!(found_version.id, inserted_version.id);
    assert_eq!(found_version.name, name);
    assert_eq!(found_version.hash, hash);
    assert_eq!(found_version.tag, tag);

    let non_existent_id = Uuid::new_v4();
    let not_found = VersionRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_product_and_name(pool: PgPool) {
    let product_id = insert_test_product(&pool).await;
    let name = "2.0.0";
    let hash = "fedcba654321";
    let tag = "v2.0.0";

    insert_test_version(&pool, name, hash, tag, Some(product_id)).await;

    let found_version = VersionRepo::get_by_product_and_name(&pool, product_id, name)
        .await
        .expect("Failed to get version by product and name");

    assert!(found_version.is_some());
    let found_version = found_version.unwrap();
    assert_eq!(found_version.name, name);
    assert_eq!(found_version.product_id, product_id);

    let non_existent_name = "999.0.0";
    let not_found = VersionRepo::get_by_product_and_name(&pool, product_id, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names(pool: PgPool) {
    let product_id = insert_test_product(&pool).await;
    let version_names = vec!["1.1.0", "1.2.0", "1.3.0"];

    for (i, name) in version_names.iter().enumerate() {
        let hash = format!("hash{}", i);
        let tag = format!("v{}", name);
        insert_test_version(&pool, name, &hash, &tag, Some(product_id)).await;
    }

    let all_names = VersionRepo::get_all_names(&pool)
        .await
        .expect("Failed to get all version names");

    for name in version_names {
        assert!(all_names.contains(name));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let product_id = insert_test_product(&pool).await;

    let version_data = vec![
        ("3.0.0", "hash3", "v3.0.0"),
        ("3.1.0", "hash31", "v3.1.0"),
        ("3.2.0", "hash32", "v3.2.0"),
    ];

    for (name, hash, tag) in &version_data {
        insert_test_version(&pool, name, hash, tag, Some(product_id)).await;
    }

    // Test get_all with no params
    let query_params = QueryParams::default();
    let all_versions = VersionRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get all versions");

    assert!(all_versions.len() >= version_data.len());

    // Test get_all with sorting
    let mut query_params = QueryParams::default();
    query_params
        .sorting
        .push_back(("name".to_string(), SortOrder::Descending));

    let sorted_versions = VersionRepo::get_all(&pool, query_params)
        .await
        .expect("Failed to get sorted versions");

    // Verify descending order
    for i in 1..sorted_versions.len() {
        assert!(sorted_versions[i - 1].name >= sorted_versions[i].name);
    }

    // Test with filtering
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
async fn test_create(pool: PgPool) {
    let product_id = insert_test_product(&pool).await;

    let new_version = NewVersion {
        name: "4.0.0".to_string(),
        hash: "hash4".to_string(),
        tag: "v4.0.0".to_string(),
        product_id,
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
async fn test_update(pool: PgPool) {
    let mut version = insert_test_version(&pool, "5.0.0", "hash5", "v5.0.0", None).await;

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
async fn test_remove(pool: PgPool) {
    let version = insert_test_version(&pool, "6.0.0", "hash6", "v6.0.0", None).await;

    VersionRepo::remove(&pool, version.id)
        .await
        .expect("Failed to remove version");

    let deleted_version = VersionRepo::get_by_id(&pool, version.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_version.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = VersionRepo::count(&pool)
        .await
        .expect("Failed to count initial versions");

    let product_id = insert_test_product(&pool).await;

    let test_versions = vec![
        ("7.0.0", "hash7", "v7.0.0"),
        ("7.1.0", "hash71", "v7.1.0"),
        ("7.2.0", "hash72", "v7.2.0"),
    ];

    for (name, hash, tag) in &test_versions {
        insert_test_version(&pool, name, hash, tag, Some(product_id)).await;
    }

    let new_count = VersionRepo::count(&pool)
        .await
        .expect("Failed to count versions after insertion");

    assert_eq!(new_count, initial_count + test_versions.len() as i64);
}
