use async_trait::async_trait;
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, DbConn, EntityTrait, IntoActiveModel, PrimaryKeyTrait,
};
use serde::Serialize;

use super::error::DbError;
pub trait HasId {
    fn id(&self) -> uuid::Uuid;
}

#[async_trait]
pub trait BaseRepo {
    type Repr: Serialize;
    type Entity: EntityTrait + 'static;
    type ActiveModel: ActiveModelTrait<Entity = Self::Entity>
        + From<Self::CreateDto>
        + From<Self::UpdateDto>
        + From<(Self::PrimaryKeyType, Self::UpdateDto)>
        + ActiveModelBehavior
        + Send;
    type CreateDto: Send;
    type UpdateDto: Send;
    type PrimaryKeyType: Into<<<Self::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType>
        + Clone
        + Sync
        + Send;

    async fn create(db: &DbConn, dto: Self::CreateDto) -> Result<uuid::Uuid, DbError>
    where
        Self::ActiveModel: 'static,
        <<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<Self::ActiveModel> + HasId,
    {
        let model = Self::ActiveModel::from(dto).insert(db).await?;
        Ok(model.id())
    }

    async fn update(
        db: &DbConn,
        id: Self::PrimaryKeyType,
        dto: Self::UpdateDto,
    ) -> Result<(), DbError>
    where
        Self::ActiveModel: 'static,
        <<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<Self::ActiveModel> + HasId,
    {
        Self::ActiveModel::from((id, dto)).update(db).await?;
        Ok(())
    }

    async fn get_all(db: &DbConn) -> Result<Vec<<Self::Entity as EntityTrait>::Model>, DbError>
    where
        Self::ActiveModel: 'static,
        <<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<Self::ActiveModel> + HasId,
    {
        let r = <Self::Entity as EntityTrait>::find().all(db).await?;
        Ok(r)
    }

    async fn get_by_id(
        db: &DbConn,
        id: Self::PrimaryKeyType,
    ) -> Result<<Self::Entity as EntityTrait>::Model, DbError>
    where
        Self::Entity: EntityTrait,
        Self::ActiveModel: 'static,
        <<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<Self::ActiveModel> + HasId,
    {
        let r = Self::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("product not found".to_owned()))?;
        Ok(r)
    }

    async fn delete(db: &DbConn, id: Self::PrimaryKeyType) -> Result<(), DbError>
    where
        Self::Entity: EntityTrait,
        Self::ActiveModel: 'static,
        <<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
            IntoActiveModel<Self::ActiveModel> + HasId,
    {
        Self::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
