#![cfg(test)]

use sqlx::PgPool;
use uuid::Uuid;

use common::QueryParams;
use data::product::*;
use repos::product::*;

use testware::create_test_product_with_details;

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id(pool: PgPool) {
    let name = "TestProduct";
    let description = "Test Description";
    let inserted_product = create_test_product_with_details(&pool, name, description).await;

    let found_product = ProductRepo::get_by_id(&pool, inserted_product.id)
        .await
        .expect("Failed to get product by ID");

    assert!(found_product.is_some());
    let found_product = found_product.unwrap();
    assert_eq!(found_product.id, inserted_product.id);
    assert_eq!(found_product.name, name);
    assert_eq!(found_product.description, description);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_not_found(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let not_found = ProductRepo::get_by_id(&pool, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");
    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_id_error(pool: PgPool) {
    let name = "TestProduct";
    let description = "Test Description";
    let inserted_product = create_test_product_with_details(&pool, name, description).await;

    pool.close().await;

    let found_product = ProductRepo::get_by_id(&pool, inserted_product.id).await;
    assert!(found_product.is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name(pool: PgPool) {
    let name = "UniqueProduct";
    let description = "Unique Description";
    create_test_product_with_details(&pool, name, description).await;

    let found_product = ProductRepo::get_by_name(&pool, name)
        .await
        .expect("Failed to get product by name");

    assert!(found_product.is_some());
    let found_product = found_product.unwrap();
    assert_eq!(found_product.name, name);
    assert_eq!(found_product.description, description);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name_not_found(pool: PgPool) {
    let non_existent_name = "NonExistentProduct";
    let not_found = ProductRepo::get_by_name(&pool, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");
    assert!(not_found.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_by_name_error(pool: PgPool) {
    let name = "UniqueProduct";
    let description = "Unique Description";
    create_test_product_with_details(&pool, name, description).await;

    pool.close().await;

    let found_product = ProductRepo::get_by_name(&pool, name).await;
    assert!(found_product.is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names(pool: PgPool) {
    let product_names = vec!["Product1", "Product2", "Product3"];
    let description = "Test Description";

    for name in &product_names {
        create_test_product_with_details(&pool, name, description).await;
    }

    let all_names = ProductRepo::get_all_names(&pool)
        .await
        .expect("Failed to get all product names");

    for name in product_names {
        assert!(all_names.contains(name));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names_empty(pool: PgPool) {
    let all_names = ProductRepo::get_all_names(&pool)
        .await
        .expect("Failed to get all product names");

    assert!(all_names.is_empty(), "Expected no product names to be returned");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_names_error(pool: PgPool) {
    pool.close().await;
    let all_names = ProductRepo::get_all_names(&pool).await;
    assert!(all_names.is_err(), "Expected an error when querying with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all(pool: PgPool) {
    let initial_count = ProductRepo::count(&pool)
        .await
        .expect("Failed to get initial product count");
    assert_eq!(initial_count, 0);

    let test_products = vec![
        ("TestAppAlpha", "Alpha version of the test app"),
        ("TestAppBeta", "Beta version of the test app"),
        ("ProductionApp", "Production ready application"),
    ];

    for (name, description) in &test_products {
        create_test_product_with_details(&pool, name, description).await;
    }

    let products = ProductRepo::get_all(&pool, QueryParams::default())
        .await
        .expect("Failed to get all products");
    assert!(products.len() == test_products.len());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_filter(pool: PgPool) {
    let test_products = vec![
        ("TestAppAlpha1", "Alpha version of the test app"),
        ("TestAppAlpha2", "test app"),
        ("TestApp3", "Alpha version of the test app"),
        ("TestAppBeta", "Beta version of the test app"),
        ("ProductionApp", "Production ready application"),
    ];

    for (name, description) in &test_products {
        create_test_product_with_details(&pool, name, description).await;
    }

    let params = QueryParams {
        filter: Some("Alpha".to_string()),
        ..QueryParams::default()
    };

    let filtered_products = ProductRepo::get_all(&pool, params)
        .await
        .expect("Failed to get filtered products");

    assert!(filtered_products.len() == 3);
    for product in &filtered_products {
        assert!(product.name.contains("Alpha") || product.description.contains("Alpha"));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_error(pool: PgPool) {
    let initial_count = ProductRepo::count(&pool)
        .await
        .expect("Failed to get initial product count");
    assert_eq!(initial_count, 0);

    let test_products = vec![
        ("TestAppAlpha", "Alpha version of the test app"),
        ("TestAppBeta", "Beta version of the test app"),
        ("ProductionApp", "Production ready application"),
    ];

    for (name, description) in &test_products {
        create_test_product_with_details(&pool, name, description).await;
    }

    pool.close().await;
    let products = ProductRepo::get_all(&pool, QueryParams::default()).await;
    assert!(products.is_err(), "Expected an error when querying with closed pool");
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
async fn test_create_duplicate_name(pool: PgPool) {
    let product_name = "DuplicateNameProduct";
    let description = "Test description for duplicate product";

    let new_product = NewProduct {
        name: product_name.to_string(),
        description: description.to_string(),
    };

    ProductRepo::create(&pool, new_product)
        .await
        .expect("Failed to create first product");

    let duplicate_product = NewProduct {
        name: product_name.to_string(),
        description: "Different description".to_string(),
    };

    let result = ProductRepo::create(&pool, duplicate_product).await;
    assert!(result.is_err(), "Creating a product with duplicate name should fail");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update(pool: PgPool) {
    let mut product =
        create_test_product_with_details(&pool, "UpdateProduct", "Initial Description").await;

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
async fn test_update_non_existent(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();

    let non_existent_product = Product {
        id: non_existent_id,
        name: "NonExistentProduct".to_string(),
        description: "This product does not exist".to_string(),
        accepting_crashes: true,
        created_at: chrono::Utc::now().naive_utc(),
        updated_at: chrono::Utc::now().naive_utc(),
    };

    let result = ProductRepo::update(&pool, non_existent_product).await;
    assert!(result.is_ok(), "Update operation should not fail");
    assert!(result.unwrap().is_none(), "Update should return None for non-existent product");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_update_error(pool: PgPool) {
    let mut product =
        create_test_product_with_details(&pool, "UpdateProduct", "Initial Description").await;

    product.name = "UpdatedProduct".to_string();
    product.description = "Updated Description".to_string();

    pool.close().await;

    let updated_id = ProductRepo::update(&pool, product.clone()).await;
    assert!(updated_id.is_err(), "Expected an error when querying with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove(pool: PgPool) {
    let product =
        create_test_product_with_details(&pool, "DeleteProduct", "Product to delete").await;

    ProductRepo::remove(&pool, product.id)
        .await
        .expect("Failed to remove product");

    let deleted_product = ProductRepo::get_by_id(&pool, product.id)
        .await
        .expect("Failed to query after deletion");
    assert!(deleted_product.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove_non_existent(pool: PgPool) {
    let non_existent_id = Uuid::new_v4();
    let result = ProductRepo::remove(&pool, non_existent_id).await;
    assert!(result.is_ok(), "Removal of non-existent product should not fail");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_remove_error(pool: PgPool) {
    let product =
        create_test_product_with_details(&pool, "DeleteProduct", "Product to delete").await;

    pool.close().await;
    let result = ProductRepo::remove(&pool, product.id).await;
    assert!(
        result.is_err(),
        "Expected an error when trying to remove a product with a closed pool"
    );
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
        create_test_product_with_details(&pool, name, description).await;
    }

    let new_count = ProductRepo::count(&pool)
        .await
        .expect("Failed to count products after insertion");

    assert_eq!(new_count, initial_count + test_products.len() as i64);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_count_errror(pool: PgPool) {
    let test_products = vec![
        ("CountProductA", "Description A"),
        ("CountProductB", "Description B"),
        ("CountProductC", "Description C"),
    ];

    for (name, description) in &test_products {
        create_test_product_with_details(&pool, name, description).await;
    }

    pool.close().await;

    let new_count = ProductRepo::count(&pool).await;
    assert!(new_count.is_err(), "Expected an error when counting products with closed pool");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_get_all_with_invalid_sort(pool: PgPool) {
    create_test_product_with_details(&pool, "SortTestA", "Description for sort test").await;
    create_test_product_with_details(&pool, "SortTestB", "Another description").await;

    let params = QueryParams {
        sorting: std::collections::VecDeque::from([(
            "non_existent_column".to_string(),
            common::SortOrder::Ascending,
        )]),
        ..QueryParams::default()
    };

    let result = ProductRepo::get_all(&pool, params).await;
    assert!(result.is_err(), "Sorting by non-existent column should fail");
}
