use async_trait::async_trait;
use sea_orm::*;
use serde::{Serialize, de::DeserializeOwned};

use super::error::DbError;
pub trait HasId {
    fn id(&self) -> uuid::Uuid;
}

#[async_trait]
pub trait BaseRepo
where
    <<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
        IntoActiveModel<Self::ActiveModel> + HasId,
{
    type Repr: Serialize + From<<Self::Entity as sea_orm::EntityTrait>::Model> + Send;
    type Entity: EntityTrait;

    type ActiveModel: ActiveModelTrait<Entity = Self::Entity>
        + From<Self::CreateDto>
        + From<Self::UpdateDto>
        + From<(Self::PrimaryKeyType, Self::UpdateDto)>
        + ActiveModelBehavior
        + Send;
    type CreateDto: Send + DeserializeOwned;
    type UpdateDto: Send + DeserializeOwned;
    type PrimaryKeyType: Into<<<Self::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType>
        + Clone
        + Sync
        + Send;

    async fn create(db: &DbConn, dto: Self::CreateDto) -> Result<uuid::Uuid, DbError> {
        let model = Self::ActiveModel::from(dto).insert(db).await?;
        Ok(model.id())
    }

    async fn update(
        db: &DbConn,
        id: Self::PrimaryKeyType,
        dto: Self::UpdateDto,
    ) -> Result<(), DbError> {
        Self::ActiveModel::from((id, dto)).update(db).await?;
        Ok(())
    }

    async fn get_all(db: &DbConn) -> Result<Vec<Self::Repr>, DbError> {
        let r = <Self::Entity as EntityTrait>::find().all(db).await?;
        Ok(r.into_iter().map(Self::Repr::from).collect())
    }

    async fn get_by_id(db: &DbConn, id: Self::PrimaryKeyType) -> Result<Self::Repr, DbError> {
        let r = Self::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("product not found".to_owned()))?;
        Ok(Self::Repr::from(r))
    }

    async fn delete(db: &DbConn, id: Self::PrimaryKeyType) -> Result<(), DbError> {
        Self::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}

#[async_trait]
pub trait BaseRepoWithSecondaryKey: BaseRepo
where
    <<Self::ActiveModel as ActiveModelTrait>::Entity as EntityTrait>::Model:
        IntoActiveModel<Self::ActiveModel> + HasId,
{
    type Column: ColumnTrait + Clone + Sync + Send;
    type SecondaryKeyType: Clone + Sync + Send;

    fn secondary_column() -> Self::Column;

    async fn get_by_secondary_id(
        db: &DbConn,
        key: Self::SecondaryKeyType,
    ) -> Result<Self::Repr, DbError>
    where
        <Self as BaseRepoWithSecondaryKey>::SecondaryKeyType: 'async_trait,
        sea_orm::Value: From<<Self as BaseRepoWithSecondaryKey>::SecondaryKeyType>,
    {
        let r = Self::Entity::find()
            .filter(Self::secondary_column().eq(key))
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("not found".to_owned()))?;
        Ok(Self::Repr::from(r))
    }
}
