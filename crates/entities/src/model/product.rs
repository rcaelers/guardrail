use super::base::HasId;
use crate::entity;

pub type Product = entity::product::Model;
pub type ProductCreateDto = entity::product::CreateModel;
pub type ProductUpdateDto = entity::product::UpdateModel;

impl HasId for entity::product::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        entity,
        model::{
            base::Repo,
            product::{ProductCreateDto, ProductUpdateDto},
        },
    };
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
        };
        let id1 = Repo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductCreateDto {
            name: "Scroom".to_owned(),
        };
        let id2 = Repo::create(&db, product2.clone()).await.unwrap();

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

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
        };
        let id1 = Repo::create(&db, product1.clone()).await.unwrap();

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

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
        };
        let id = Repo::create(&db, product1.clone()).await.unwrap();

        let model = entity::product::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.name, product1.name);

        let product2 = ProductUpdateDto {
            id,
            name: "Scroom".to_owned(),
        };

        Repo::update(&db, product2.clone()).await.unwrap();

        let model = entity::product::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.name, product2.name);
    }

    #[serial]
    #[tokio::test]
    async fn test_get_by_id() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = ProductCreateDto {
            name: "Workrave".to_owned(),
        };
        let id = Repo::create(&db, product.clone()).await.unwrap();

        let model = Repo::get_by_id::<entity::product::Entity>(&db, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.name, product.name);

        let err = Repo::get_by_id::<entity::product::Entity>(&db, uuid::Uuid::new_v4())
            .await
            .unwrap();
        assert!(err.is_none())
    }

    #[serial]
    #[tokio::test]
    async fn test_get_by_name() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = ProductCreateDto {
            name: "Workrave".to_owned(),
        };
        let id = Repo::create(&db, product.clone()).await.unwrap();

        let model = Repo::get_by_column::<entity::product::Entity, _, _>(
            &db,
            entity::product::Column::Name,
            "Workrave".to_string(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(model.id, id);
        assert_eq!(model.name, product.name);

        let err = Repo::get_by_column::<entity::product::Entity, _, _>(
            &db,
            entity::product::Column::Name,
            "Foo".to_string(),
        )
        .await
        .unwrap();
        assert!(err.is_none())
    }

    #[serial]
    #[tokio::test]
    async fn test_get_all() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
        };
        let id1 = Repo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductCreateDto {
            name: "Scroom".to_owned(),
        };
        let id2 = Repo::create(&db, product2.clone()).await.unwrap();

        let model = Repo::get_all::<entity::product::Entity>(&db).await.unwrap();
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

        let product1 = ProductCreateDto {
            name: "Workrave".to_owned(),
        };
        let id1 = Repo::create(&db, product1.clone()).await.unwrap();

        let product2 = ProductCreateDto {
            name: "Scroom".to_owned(),
        };
        let id2 = Repo::create(&db, product2.clone()).await.unwrap();

        Repo::delete_by_id::<entity::product::Entity>(&db, id2)
            .await
            .unwrap();

        let model = Repo::get_all::<entity::product::Entity>(&db).await.unwrap();
        assert_eq!(model.len(), 1);
        assert_eq!(model[0].id, id1);
        assert_eq!(model[0].name, product1.name);

        //let err = Repo::delete(&db, id2).await.unwrap_err();
        //assert_eq!(err.to_string(), "Record not found");
        //let err = Repo::delete(&db, uuid::Uuid::new_v4()).await.unwrap_err();
        //assert_eq!(err.to_string(), "Record not found");
    }
}
