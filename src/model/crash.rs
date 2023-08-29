use async_trait::async_trait;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::base::{BaseRepo, HasId};
use crate::entity;
pub use entity::crash::Model as Crash;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrashDto {
    pub report: String,
    pub version_id: Uuid,
    pub product_id: Uuid,
}

pub struct CrashRepo;

impl From<CrashDto> for entity::crash::ActiveModel {
    fn from(crash: CrashDto) -> Self {
        Self {
            id: Set(uuid::Uuid::new_v4()),
            report: Set(serde_json::json!(crash.report)),
            version_id: Set(crash.version_id),
            product_id: Set(crash.product_id),
            ..Default::default()
        }
    }
}

impl From<(uuid::Uuid, CrashDto)> for entity::crash::ActiveModel {
    fn from((id, crash): (uuid::Uuid, CrashDto)) -> Self {
        Self {
            id: Set(id),
            ..From::from(crash)
        }
    }
}

impl HasId for entity::crash::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[async_trait]
impl BaseRepo for CrashRepo {
    type CreateDto = CrashDto;
    type UpdateDto = CrashDto;
    type Entity = entity::crash::Entity;
    type Repr = entity::crash::Model;
    type ActiveModel = entity::crash::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}
