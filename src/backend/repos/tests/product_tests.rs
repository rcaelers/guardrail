#![cfg(test)]

use testware::setup::TestSetup;
use uuid::Uuid;

use common::QueryParams;
use data::product::*;
use repos::product::*;

use testware::create_test_product_with_details;

#[tokio::test]
async fn test_get_by_id() {
    let db = TestSetup::create_db().await;
    let name = "TestProduct";
    let description = "Test Description";
    let inserted_product = create_test_product_with_details(&db, name, description).await;

    let found_product = ProductRepo::get_by_id(&db, inserted_product.id.clone())
        .await
        .expect("Failed to get product by ID");

    assert!(found_product.is_some());
    let found_product = found_product.unwrap();
    assert_eq!(found_product.id, inserted_product.id);
    assert_eq!(found_product.name, name);
    assert_eq!(found_product.description, description);
}

#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4().to_string();
    let not_found = ProductRepo::get_by_id(&db, non_existent_id)
        .await
        .expect("Failed to query with non-existent ID");
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_get_by_name() {
    let db = TestSetup::create_db().await;
    let name = "UniqueProduct";
    let description = "Unique Description";
    create_test_product_with_details(&db, name, description).await;

    let found_product = ProductRepo::get_by_name(&db, name)
        .await
        .expect("Failed to get product by name");

    assert!(found_product.is_some());
    let found_product = found_product.unwrap();
    assert_eq!(found_product.name, name);
    assert_eq!(found_product.description, description);
}

#[tokio::test]
async fn test_get_by_name_not_found() {
    let db = TestSetup::create_db().await;
    let non_existent_name = "NonExistentProduct";
    let not_found = ProductRepo::get_by_name(&db, non_existent_name)
        .await
        .expect("Failed to query with non-existent name");
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_get_all_names() {
    let db = TestSetup::create_db().await;
    let product_names = vec!["Product1", "Product2", "Product3"];
    let description = "Test Description";

    for name in &product_names {
        create_test_product_with_details(&db, name, description).await;
    }

    let all_names = ProductRepo::get_all_names(&db)
        .await
        .expect("Failed to get all product names");

    for name in product_names {
        assert!(all_names.contains(name));
    }
}

#[tokio::test]
async fn test_get_all_names_empty() {
    let db = TestSetup::create_db().await;
    let all_names = ProductRepo::get_all_names(&db)
        .await
        .expect("Failed to get all product names");

    assert!(all_names.is_empty(), "Expected no product names to be returned");
}

#[tokio::test]
async fn test_get_all() {
    let db = TestSetup::create_db().await;
    let initial_count = ProductRepo::count(&db)
        .await
        .expect("Failed to get initial product count");
    assert_eq!(initial_count, 0);

    let test_products = vec![
        ("TestAppAlpha", "Alpha version of the test app"),
        ("TestAppBeta", "Beta version of the test app"),
        ("ProductionApp", "Production ready application"),
    ];

    for (name, description) in &test_products {
        create_test_product_with_details(&db, name, description).await;
    }

    let products = ProductRepo::get_all(&db, QueryParams::default())
        .await
        .expect("Failed to get all products");
    assert!(products.len() == test_products.len());
}

#[tokio::test]
async fn test_get_all_filter() {
    let db = TestSetup::create_db().await;
    let test_products = vec![
        ("TestAppAlpha1", "Alpha version of the test app"),
        ("TestAppAlpha2", "test app"),
        ("TestApp3", "Alpha version of the test app"),
        ("TestAppBeta", "Beta version of the test app"),
        ("ProductionApp", "Production ready application"),
    ];

    for (name, description) in &test_products {
        create_test_product_with_details(&db, name, description).await;
    }

    let params = QueryParams {
        filter: Some("Alpha".to_string()),
        ..QueryParams::default()
    };

    let filtered_products = ProductRepo::get_all(&db, params)
        .await
        .expect("Failed to get filtered products");

    assert!(filtered_products.len() == 3);
    for product in &filtered_products {
        assert!(product.name.contains("Alpha") || product.description.contains("Alpha"));
    }
}

#[tokio::test]
async fn test_create() {
    let db = TestSetup::create_db().await;
    let new_product = NewProduct {
        name: "NewProduct".to_string(),
        description: "New product description".to_string(),
        ..Default::default()
    };

    let product_id = ProductRepo::create(&db, new_product.clone())
        .await
        .expect("Failed to create product");

    let created_product = ProductRepo::get_by_id(&db, product_id)
        .await
        .expect("Failed to get created product")
        .expect("Created product not found");

    assert_eq!(created_product.name, new_product.name);
    assert_eq!(created_product.description, new_product.description);
}

#[tokio::test]
async fn test_create_duplicate_name() {
    let db = TestSetup::create_db().await;
    let product_name = "DuplicateNameProduct";
    let description = "Test description for duplicate product";

    let new_product = NewProduct {
        name: product_name.to_string(),
        description: description.to_string(),
        ..Default::default()
    };

    ProductRepo::create(&db, new_product)
        .await
        .expect("Failed to create first product");

    let duplicate_product = NewProduct {
        name: product_name.to_string(),
        description: "Different description".to_string(),
        ..Default::default()
    };

    let result = ProductRepo::create(&db, duplicate_product).await;
    assert!(result.is_err(), "Creating a product with duplicate name should fail");
}

#[tokio::test]
async fn test_update() {
    let db = TestSetup::create_db().await;
    let mut product =
        create_test_product_with_details(&db, "UpdateProduct", "Initial Description").await;

    product.name = "UpdatedProduct".to_string();
    product.description = "Updated Description".to_string();

    let updated_id = ProductRepo::update(&db, product.clone())
        .await
        .expect("Failed to update product")
        .expect("Product not found when updating");
    assert_eq!(updated_id, product.id);

    let updated_product = ProductRepo::get_by_id(&db, product.id.clone())
        .await
        .expect("Failed to get updated product")
        .expect("Updated product not found");
    assert_eq!(updated_product.name, "UpdatedProduct");
    assert_eq!(updated_product.description, "Updated Description");
}

#[tokio::test]
async fn test_update_non_existent() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4().to_string();

    let non_existent_product = Product {
        id: non_existent_id,
        name: "NonExistentProduct".to_string(),
        description: "This product does not exist".to_string(),
        accepting_crashes: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        ..Default::default()
    };

    let result = ProductRepo::update(&db, non_existent_product).await;
    assert!(result.is_ok(), "Update operation should not fail");
    assert!(result.unwrap().is_none(), "Update should return None for non-existent product");
}

#[tokio::test]
async fn test_remove() {
    let db = TestSetup::create_db().await;
    let product = create_test_product_with_details(&db, "DeleteProduct", "Product to delete").await;

    ProductRepo::remove(&db, product.id.clone())
        .await
        .expect("Failed to remove product");

    let deleted_product = ProductRepo::get_by_id(&db, product.id.clone())
        .await
        .expect("Failed to query after deletion");
    assert!(deleted_product.is_none());
}

#[tokio::test]
async fn test_remove_non_existent() {
    let db = TestSetup::create_db().await;
    let non_existent_id = Uuid::new_v4().to_string();
    let result = ProductRepo::remove(&db, non_existent_id).await;
    assert!(result.is_ok(), "Removal of non-existent product should not fail");
}

#[tokio::test]
async fn test_count() {
    let db = TestSetup::create_db().await;
    let initial_count = ProductRepo::count(&db)
        .await
        .expect("Failed to count initial products");

    let test_products = vec![
        ("CountProductA", "Description A"),
        ("CountProductB", "Description B"),
        ("CountProductC", "Description C"),
    ];

    for (name, description) in &test_products {
        create_test_product_with_details(&db, name, description).await;
    }

    let new_count = ProductRepo::count(&db)
        .await
        .expect("Failed to count products after insertion");

    assert_eq!(new_count, initial_count + test_products.len() as i64);
}

#[tokio::test]
async fn test_get_all_with_invalid_sort() {
    let db = TestSetup::create_db().await;
    create_test_product_with_details(&db, "SortTestA", "Description for sort test").await;
    create_test_product_with_details(&db, "SortTestB", "Another description").await;

    let params = QueryParams {
        sorting: std::collections::VecDeque::from([(
            "non_existent_column".to_string(),
            common::SortOrder::Ascending,
        )]),
        ..QueryParams::default()
    };

    let result = ProductRepo::get_all(&db, params).await;
    assert!(result.is_err(), "Sorting by non-existent column should fail");
}
