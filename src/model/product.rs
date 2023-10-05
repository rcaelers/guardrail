use async_trait::async_trait;
use sea_orm::*;
use serde::{Deserialize, Serialize};

use super::base::{BaseRepo, BaseRepoWithSecondaryKey, HasId};
use crate::entity;

pub use entity::product::Model as Product;

pub struct ProductRepo;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductDto {
    pub name: String,
}

impl From<ProductDto> for entity::product::ActiveModel {
    fn from(product: ProductDto) -> Self {
        Self {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(product.name),
            ..Default::default()
        }
    }
}

impl From<(uuid::Uuid, ProductDto)> for entity::product::ActiveModel {
    fn from((id, product): (uuid::Uuid, ProductDto)) -> Self {
        Self {
            id: Set(id),
            name: Set(product.name),
            ..Default::default()
        }
    }
}

impl HasId for entity::product::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[async_trait]
impl BaseRepo for ProductRepo {
    type CreateDto = ProductDto;
    type UpdateDto = ProductDto;
    type Entity = entity::product::Entity;
    type Repr = entity::product::Model;
    type ActiveModel = entity::product::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}

#[async_trait]
impl BaseRepoWithSecondaryKey for ProductRepo {
    type Column = entity::product::Column;
    type SecondaryKeyType = String;

    fn secondary_column() -> Self::Column {
        entity::product::Column::Name
    }
}

#[cfg(test)]
mod tests {
    use crate::entity;
    use crate::model::base::{BaseRepo, BaseRepoWithSecondaryKey};
    use crate::model::product::{ProductDto, ProductRepo};
    use serial_test::serial;

    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection, EntityTrait};

    #[serial]
    #[tokio::test]
    async fn test_create() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductDto {
            name: "Workrave".to_owned(),
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductDto {
            name: "Scroom".to_owned(),
        };
        let id2 = ProductRepo::create(&db, product2.clone()).await.unwrap();

        let model1 = entity::product::Entity::find_by_id(id1)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model1.name, product1.name);

        let model2 = entity::product::Entity::find_by_id(id2)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model2.name, product2.name);
    }

    #[serial]
    #[tokio::test]
    async fn test_create_no_keys() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductDto {
            name: "Workrave".to_owned(),
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let model1 = entity::product::Entity::find_by_id(id1)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model1.name, product1.name);
    }

    #[serial]
    #[tokio::test]
    async fn test_update() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductDto {
            name: "Workrave".to_owned(),
        };
        let id = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let model = entity::product::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.name, product1.name);

        let product2 = ProductDto {
            name: "Scroom".to_owned(),
        };

        ProductRepo::update(&db, id, product2.clone())
            .await
            .unwrap();

        let model = entity::product::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.name, product2.name);

        let product3 = product2.clone();
        let err = ProductRepo::update(&db, uuid::Uuid::new_v4(), product3)
            .await
            .unwrap_err();
        // TODO assert!(matches!(err, DbError::RecordNotFound { .. }));
    }

    #[serial]
    #[tokio::test]
    async fn test_get_by_id() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = ProductDto {
            name: "Workrave".to_owned(),
        };
        let id = ProductRepo::create(&db, product.clone()).await.unwrap();

        let model = ProductRepo::get_by_id(&db, id).await.unwrap().unwrap();
        assert_eq!(model.name, product.name);

        let err = ProductRepo::get_by_id(&db, uuid::Uuid::new_v4())
            .await
            .unwrap();
        assert!(err.is_none())
    }

    #[serial]
    #[tokio::test]
    async fn test_get_by_name() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = ProductDto {
            name: "Workrave".to_owned(),
        };
        let id = ProductRepo::create(&db, product.clone()).await.unwrap();

        let model = ProductRepo::get_by_secondary_id(&db, "Workrave".to_string())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.id, id);
        assert_eq!(model.name, product.name);

        let err = ProductRepo::get_by_secondary_id(&db, "Foo".to_string())
            .await
            .unwrap();
        assert!(err.is_none())
    }

    #[serial]
    #[tokio::test]
    async fn test_get_all() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductDto {
            name: "Workrave".to_owned(),
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductDto {
            name: "Scroom".to_owned(),
        };
        let id2 = ProductRepo::create(&db, product2.clone()).await.unwrap();

        let model = ProductRepo::get_all(&db).await.unwrap();
        assert_eq!(model.len(), 2);
        assert_eq!(model[0].id, id1);
        assert_eq!(model[0].name, product1.name);
        assert_eq!(model[1].id, id2);
        assert_eq!(model[1].name, product2.name);
    }

    #[serial]
    #[tokio::test]
    async fn test_delete() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1: ProductDto = ProductDto {
            name: "Workrave".to_owned(),
        };
        let id1 = ProductRepo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductDto {
            name: "Scroom".to_owned(),
        };
        let id2 = ProductRepo::create(&db, product2.clone()).await.unwrap();

        ProductRepo::delete(&db, id2).await.unwrap();

        let model = ProductRepo::get_all(&db).await.unwrap();
        assert_eq!(model.len(), 1);
        assert_eq!(model[0].id, id1);
        assert_eq!(model[0].name, product1.name);

        //let err = ProductRepo::delete(&db, id2).await.unwrap_err();
        //assert_eq!(err.to_string(), "Record not found");
        //let err = ProductRepo::delete(&db, uuid::Uuid::new_v4()).await.unwrap_err();
        //assert_eq!(err.to_string(), "Record not found");
    }
}
