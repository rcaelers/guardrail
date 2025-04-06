#![cfg(all(test, feature = "ssr"))]

use sqlx::PgPool;
use uuid::Uuid;

use repos::QueryParams;
use repos::product::*;

async fn insert_test_product(pool: &PgPool, name: &str, description: &str) -> Product {
    sqlx::query_as!(
        Product,
        r#"
        INSERT INTO guardrail.products (name, description)
        VALUES ($1, $2)
        RETURNING id, name, description, created_at, updated_at
        "#,
        name,
        description
    )
    .fetch_one(pool)
    .await
    .expect("Failed to insert test product")
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let name = "TestProduct";
    let description = "Test Description";
    let inserted_product = insert_test_product(&pool, name, description).await;

    let found_product = ProductRepo::get_by_id(&pool, inserted_product.id)
        .await
        .expect("Failed to get product by ID");

    assert!(found_product.is_some());
    let found_product = found_product.unwrap();
    assert_eq!(found_product.id, inserted_product.id);
    assert_eq!(found_product.name, name);
    assert_eq!(found_product.description, description);

    let non_existent_id = Uuid::new_v4();
    let not_found = ProductRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name(pool: PgPool) {
    let name = "UniqueProduct";
    let description = "Unique Description";
    insert_test_product(&pool, name, description).await;

    let found_product = ProductRepo::get_by_name(&pool, name)
        .await
        .expect("Failed to get product by name");

    assert!(found_product.is_some());
    let found_product = found_product.unwrap();
    assert_eq!(found_product.name, name);
    assert_eq!(found_product.description, description);

    let non_existent_name = "NonExistentProduct";
    let not_found = ProductRepo::get_by_name(&pool, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");

    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names(pool: PgPool) {
    let product_names = vec!["Product1", "Product2", "Product3"];
    let description = "Test Description";

    for name in &product_names {
        insert_test_product(&pool, name, description).await;
    }

    let all_names = ProductRepo::get_all_names(&pool)
        .await
        .expect("Failed to get all product names");

    for name in product_names {
        assert!(all_names.contains(name));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let initial_count = ProductRepo::count(&pool)
        .await
        .expect("Failed to get initial product count");
    assert_eq!(initial_count, 0);

    // Create test products
    let test_products = vec![
        ("TestAppAlpha", "Alpha version of the test app"),
        ("TestAppBeta", "Beta version of the test app"),
        ("ProductionApp", "Production ready application"),
    ];

    for (name, description) in &test_products {
        insert_test_product(&pool, name, description).await;
    }

    // Test get all without filters
    let products = ProductRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to get all products");

    assert!(products.len() >= test_products.len());

    // Test with filtering - use a filter string that exists in our test data
    let params = QueryParams {
        filter: Some("Alpha".to_string()),
        ..QueryParams::default()
    };

    let filtered_products = ProductRepo::get_all(&pool, params)
        .await
        .expect("Failed to get filtered products");

    assert!(!filtered_products.is_empty());
    for product in &filtered_products {
        assert!(product.name.contains("Alpha") || product.description.contains("Alpha"));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_create(pool: PgPool) {
    let new_product = NewProduct {
        name: "NewProduct".to_string(),
        description: "New product description".to_string(),
    };

    let product_id = ProductRepo::create(&pool, new_product.clone())
        .await
        .expect("Failed to create product");

    let created_product = ProductRepo::get_by_id(&pool, product_id)
        .await
        .expect("Failed to get created product")
        .expect("Created product not found");

    assert_eq!(created_product.name, new_product.name);
    assert_eq!(created_product.description, new_product.description);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut product = insert_test_product(&pool, "UpdateProduct", "Initial Description").await;

    product.name = "UpdatedProduct".to_string();
    product.description = "Updated Description".to_string();

    let updated_id = ProductRepo::update(&pool, product.clone())
        .await
        .expect("Failed to update product")
        .expect("Product not found when updating");

    assert_eq!(updated_id, product.id);

    let updated_product = ProductRepo::get_by_id(&pool, product.id)
        .await
        .expect("Failed to get updated product")
        .expect("Updated product not found");

    assert_eq!(updated_product.name, "UpdatedProduct");
    assert_eq!(updated_product.description, "Updated Description");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let product = insert_test_product(&pool, "DeleteProduct", "Product to delete").await;

    ProductRepo::remove(&pool, product.id)
        .await
        .expect("Failed to remove product");

    let deleted_product = ProductRepo::get_by_id(&pool, product.id)
        .await
        .expect("Failed to query after deletion");

    assert!(deleted_product.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count(pool: PgPool) {
    let initial_count = ProductRepo::count(&pool)
        .await
        .expect("Failed to count initial products");

    let test_products = vec![
        ("CountProductA", "Description A"),
        ("CountProductB", "Description B"),
        ("CountProductC", "Description C"),
    ];

    for (name, description) in &test_products {
        insert_test_product(&pool, name, description).await;
    }

    let new_count = ProductRepo::count(&pool)
        .await
        .expect("Failed to count products after insertion");

    assert_eq!(new_count, initial_count + test_products.len() as i64);
}
