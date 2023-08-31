use async_trait::async_trait;
use sea_orm::*;
use serde::Serialize;

use super::base::{BaseRepo, BaseRepoWithSecondaryKey, HasId};
use crate::entity;

pub use entity::user::Model as User;

pub struct UserRepo;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct UserDto {
    pub name: String,
    pub password: String,
    pub admin: bool,
}

impl From<UserDto> for entity::user::ActiveModel {
    fn from(user: UserDto) -> Self {
        Self {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(user.name),
            password: Set(user.password),
            admin: Set(user.admin),
            ..Default::default()
        }
    }
}

impl From<(uuid::Uuid, UserDto)> for entity::user::ActiveModel {
    fn from((id, user): (uuid::Uuid, UserDto)) -> Self {
        Self {
            id: Set(id),
            ..From::from(user)
        }
    }
}

impl HasId for entity::user::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[async_trait]
impl BaseRepo for UserRepo {
    type CreateDto = UserDto;
    type UpdateDto = UserDto;
    type Entity = entity::user::Entity;
    type Repr = entity::user::Model;
    type ActiveModel = entity::user::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}

#[async_trait]
impl BaseRepoWithSecondaryKey for UserRepo {
    type Column = entity::user::Column;
    type SecondaryKeyType = String;

    fn secondary_column() -> Self::Column {
        entity::user::Column::Name
    }
}
