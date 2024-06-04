use super::base::HasId;
use crate::entity;
use sea_orm::*;

pub type Version = entity::version::Model;
pub type VersionCreateDto = entity::version::CreateModel;
pub type VersionUpdateDto = entity::version::UpdateModel;

impl HasId for entity::version::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

pub struct VersionRepo;
impl VersionRepo {
    pub async fn get_by_product_and_name(
        db: &DatabaseConnection,
        product_id: uuid::Uuid,
        name: String,
    ) -> Result<Option<entity::version::Model>, DbErr> {
        let version = entity::prelude::Version::find()
            .filter(
                Condition::all()
                    .add(entity::version::Column::Name.eq(name))
                    .add(entity::version::Column::ProductId.eq(product_id)),
            )
            .one(db)
            .await?
            .map(entity::version::Model::from);
        Ok(version)
    }
}
