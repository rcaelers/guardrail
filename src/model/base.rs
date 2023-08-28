use sea_orm::{ActiveModelBehavior, ActiveModelTrait, DbConn, EntityTrait, IntoActiveModel};
use sea_orm::{IdenStatic, Iterable, PrimaryKeyToColumn, PrimaryKeyTrait};
use serde::Serialize;

use super::error::DbError;

pub trait BaseRepoDef {
    // type Repr: Serialize + From<<Self::Entity as EntityTrait>::Model>;
    type Repr: Serialize;
    type Entity: EntityTrait + 'static;
    type ActiveModel: ActiveModelTrait<Entity = Self::Entity>
        + From<Self::CreateDto>
        + From<Self::UpdateDto>
        + ActiveModelBehavior
        + Send;
    type CreateDto;
    type UpdateDto;
    type PrimaryType: Into<<<Self::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType>
        + Clone
        + Sync
        + Send;
}

pub trait HasId {
    fn id(&self) -> uuid::Uuid;
}

pub struct BaseRepo;

impl BaseRepo {
    pub async fn create<'a, 'async_trait, B>(
        db: &DbConn,
        dto: B::CreateDto,
    ) -> Result<uuid::Uuid, DbError>
    where
        B: BaseRepoDef,
        B::ActiveModel: 'a + 'async_trait + 'static,
        <<B::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<B::ActiveModel> + HasId,
    {
        let model = B::ActiveModel::from(dto).insert(db).await?;
        Ok(model.id())
    }

    pub async fn update<'a, 'async_trait, B>(db: &DbConn, dto: B::UpdateDto) -> Result<(), DbError>
    where
        B: BaseRepoDef,
        B::ActiveModel: 'a + 'async_trait + 'static,
        <<B::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<B::ActiveModel> + HasId,
    {
        B::ActiveModel::from(dto).update(db).await?;
        Ok(())
    }
    pub async fn get_by_id<'a, 'async_trait, B>(
        db: &DbConn,
        id: B::PrimaryType,
    ) -> Result<<B::Entity as EntityTrait>::Model, DbError>
    where
        B: BaseRepoDef,
        B::ActiveModel: 'a + 'async_trait + 'static,
        <<B::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<B::ActiveModel> + HasId,
    {
        let r = B::Entity::find()
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("product not found".to_owned()))?;
        Ok(r)
    }

    pub async fn get_all<'a, 'async_trait, B>(
        db: &DbConn,
        id: B::PrimaryType,
    ) -> Result<Vec<<B::Entity as EntityTrait>::Model>, DbError>
    where
        B: BaseRepoDef,
        B::Entity: EntityTrait,
        B::ActiveModel: 'a + 'async_trait + 'static,
        <<B::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<B::ActiveModel> + HasId,
    {
        let r = B::Entity::find_by_id(id).all(db).await?;
        Ok(r)
    }
}
