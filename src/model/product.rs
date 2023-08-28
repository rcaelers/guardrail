use sea_orm::*;
use serde::Serialize;

use super::base::{BaseRepoDef, HasId};
use super::error::DbError;
use crate::{entity, utils::make_api_key};

pub use entity::product::Model as Product;

pub struct ProductRepo;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct ProductCreateDto {
    pub name: String,
    pub report_api_key: Option<String>,
    pub symbol_api_key: Option<String>,
}

impl From<ProductCreateDto> for entity::product::ActiveModel {
    fn from(product: ProductCreateDto) -> Self {
        Self {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(product.name),
            report_api_key: Set(product.report_api_key.unwrap_or(make_api_key())),
            symbol_api_key: Set(product.symbol_api_key.unwrap_or(make_api_key())),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct ProductUpdateDto {
    pub id: uuid::Uuid,
    pub name: String,
    pub report_api_key: Option<String>,
    pub symbol_api_key: Option<String>,
}

impl From<ProductUpdateDto> for entity::product::ActiveModel {
    fn from(product: ProductUpdateDto) -> Self {
        let mut model = Self {
            id: Set(product.id),
            name: Set(product.name),
            ..Default::default()
        };
        if let Some(report_api_key) = product.report_api_key {
            model.report_api_key = Set(report_api_key);
        }
        if let Some(symbol_api_key) = product.symbol_api_key {
            model.symbol_api_key = Set(symbol_api_key);
        }
        model
    }
}

impl HasId for entity::product::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

impl BaseRepoDef for ProductRepo {
    type CreateDto = ProductCreateDto;
    type UpdateDto = ProductUpdateDto;
    type Entity = entity::product::Entity;
    type Repr = entity::product::Model;
    type ActiveModel = entity::product::ActiveModel;
    type PrimaryType = uuid::Uuid;
}

impl ProductRepo {
    pub async fn create(db: &DbConn, product: ProductCreateDto) -> Result<uuid::Uuid, DbError> {
        let model = entity::product::ActiveModel::from(product)
            .insert(db)
            .await?;
        Ok(model.id)
    }

    pub async fn update(db: &DbConn, product: ProductUpdateDto) -> Result<(), DbError> {
        entity::product::ActiveModel::from(product)
            .update(db)
            .await?;
        Ok(())
    }

    pub async fn get_all(db: &DbConn) -> Result<Vec<Product>, DbError> {
        let products = entity::product::Entity::find().all(db).await?;
        Ok(products)
    }

    pub async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Product, DbError> {
        let product = entity::product::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("product not found".to_owned()))?;
        Ok(product)
    }

    pub async fn get_by_name(db: &DbConn, name: &String) -> Result<Product, DbError> {
        let product = entity::product::Entity::find()
            .filter(entity::product::Column::Name.eq(name))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("product not found".to_owned()))?;

        Ok(product)
    }

    pub async fn delete(db: &DbConn, id: uuid::Uuid) -> Result<(), DbError> {
        entity::product::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::entity;
    use crate::model::error::DbError;
    use crate::model::product::{ProductCreateDto, ProductRepo, ProductUpdateDto};
    use serial_test::serial;

    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection, EntityTrait};

    #[serial]
    #[tokio::test]
    async fn test_create() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
            report_api_key: Some("test_report_api_key1".to_owned()),
            symbol_api_key: Some("test_symbol_api_key1".to_owned()),
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductCreateDto {
            name: "Scroom".to_owned(),
            report_api_key: Some("test_report_api_key2".to_owned()),
            symbol_api_key: Some("test_symbol_api_key2".to_owned()),
        };
        let id2 = ProductRepo::create(&db, product2.clone()).await.unwrap();

        let model1 = entity::product::Entity::find_by_id(id1)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model1.name, product1.name);
        assert_eq!(
            model1.report_api_key,
            product1.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model1.symbol_api_key,
            product1.symbol_api_key.unwrap_or("".to_owned())
        );

        let model2 = entity::product::Entity::find_by_id(id2)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model2.name, product2.name);
        assert_eq!(
            model2.report_api_key,
            product2.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model2.symbol_api_key,
            product2.symbol_api_key.unwrap_or("".to_owned())
        );
    }

    #[serial]
    #[tokio::test]
    async fn test_create_no_keys() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
            report_api_key: None,
            symbol_api_key: None,
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let model1 = entity::product::Entity::find_by_id(id1)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model1.name, product1.name);
        assert!(model1.report_api_key != *"",);
        assert!(model1.symbol_api_key != *"");
    }

    #[serial]
    #[tokio::test]
    async fn test_update() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
            report_api_key: Some("test_report_api_key1".to_owned()),
            symbol_api_key: Some("test_symbol_api_key1".to_owned()),
        };
        let id = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let model = entity::product::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.name, product1.name);
        assert_eq!(
            model.report_api_key,
            product1.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model.symbol_api_key,
            product1.symbol_api_key.as_deref().unwrap_or("")
        );

        let product2 = ProductUpdateDto {
            id,
            name: "Scroom".to_owned(),
            report_api_key: Some("test_report_api_key2".to_owned()),
            symbol_api_key: None,
        };

        ProductRepo::update(&db, product2.clone()).await.unwrap();

        let model = entity::product::Entity::find_by_id(product2.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.name, product2.name);
        assert_eq!(
            model.report_api_key,
            product2.report_api_key.as_deref().unwrap_or("")
        );
        assert_eq!(
            model.symbol_api_key,
            product1.symbol_api_key.as_deref().unwrap_or("")
        );

        let mut product3 = product2.clone();
        product3.id = uuid::Uuid::new_v4();
        let err = ProductRepo::update(&db, product3).await.unwrap_err();
        // TODO assert!(matches!(err, DbError::RecordNotFound { .. }));
    }

    #[serial]
    #[tokio::test]
    async fn test_get_by_id() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = ProductCreateDto {
            name: "Workrave".to_owned(),
            report_api_key: Some("test_report_api_key1".to_owned()),
            symbol_api_key: Some("test_symbol_api_key1".to_owned()),
        };
        let id = ProductRepo::create(&db, product.clone()).await.unwrap();

        let model = ProductRepo::get_by_id(&db, id).await.unwrap();
        assert_eq!(model.name, product.name);
        assert_eq!(
            model.report_api_key,
            product.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model.symbol_api_key,
            product.symbol_api_key.unwrap_or("".to_owned())
        );

        let err = ProductRepo::get_by_id(&db, uuid::Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::RecordNotFound { .. }));
    }

    #[serial]
    #[tokio::test]
    async fn test_get_by_name() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = ProductCreateDto {
            name: "Workrave".to_owned(),
            report_api_key: Some("test_report_api_key1".to_owned()),
            symbol_api_key: Some("test_symbol_api_key1".to_owned()),
        };
        let id = ProductRepo::create(&db, product.clone()).await.unwrap();

        let model = ProductRepo::get_by_name(&db, &"Workrave".to_string())
            .await
            .unwrap();
        assert_eq!(model.id, id);
        assert_eq!(model.name, product.name);
        assert_eq!(
            model.report_api_key,
            product.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model.symbol_api_key,
            product.symbol_api_key.unwrap_or("".to_owned())
        );

        let err = ProductRepo::get_by_name(&db, &"Foo".to_string())
            .await
            .unwrap_err();
        assert!(matches!(err, DbError::RecordNotFound { .. }));
    }

    #[serial]
    #[tokio::test]
    async fn test_get_all() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
            report_api_key: Some("test_report_api_key1".to_owned()),
            symbol_api_key: Some("test_symbol_api_key1".to_owned()),
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductCreateDto {
            name: "Scroom".to_owned(),
            report_api_key: Some("test_report_api_key2".to_owned()),
            symbol_api_key: Some("test_symbol_api_key2".to_owned()),
        };
        let id2 = ProductRepo::create(&db, product2.clone()).await.unwrap();

        let model = ProductRepo::get_all(&db).await.unwrap();
        assert_eq!(model.len(), 2);
        assert_eq!(model[0].id, id1);
        assert_eq!(model[0].name, product1.name);
        assert_eq!(
            model[0].report_api_key,
            product1.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model[0].symbol_api_key,
            product1.symbol_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(model[1].id, id2);
        assert_eq!(model[1].name, product2.name);
        assert_eq!(
            model[1].report_api_key,
            product2.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model[1].symbol_api_key,
            product2.symbol_api_key.unwrap_or("".to_owned())
        );
    }

    #[serial]
    #[tokio::test]
    async fn test_delete() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1: ProductCreateDto = ProductCreateDto {
            name: "Workrave".to_owned(),
            report_api_key: Some("test_report_api_key1".to_owned()),
            symbol_api_key: Some("test_symbol_api_key1".to_owned()),
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductCreateDto {
            name: "Scroom".to_owned(),
            report_api_key: Some("test_report_api_key2".to_owned()),
            symbol_api_key: Some("test_symbol_api_key2".to_owned()),
        };
        let id2 = ProductRepo::create(&db, product2.clone()).await.unwrap();

        ProductRepo::delete(&db, id2).await.unwrap();

        let model = ProductRepo::get_all(&db).await.unwrap();
        assert_eq!(model.len(), 1);
        assert_eq!(model[0].id, id1);
        assert_eq!(model[0].name, product1.name);
        assert_eq!(
            model[0].report_api_key,
            product1.report_api_key.unwrap_or("".to_owned())
        );
        assert_eq!(
            model[0].symbol_api_key,
            product1.symbol_api_key.unwrap_or("".to_owned())
        );

        //let err = ProductRepo::delete(&db, id2).await.unwrap_err();
        //assert_eq!(err.to_string(), "Record not found");
        //let err = ProductRepo::delete(&db, uuid::Uuid::new_v4()).await.unwrap_err();
        //assert_eq!(err.to_string(), "Record not found");
    }
}
