use sea_orm::*;
use serde::Serialize;
use uuid::Uuid;

use super::error::DbError;
use crate::entity;

pub use entity::crash::Model as Crash;

pub struct CrashRepo;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct CrashDto {
    pub report: String,
    pub version_id: Uuid,
    pub product_id: Uuid,
}

impl CrashRepo {
    pub async fn create(db: &DbConn, crash: CrashDto) -> Result<uuid::Uuid, DbError> {
        let model = entity::crash::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            report: Set(serde_json::json!(crash.report)),
            version_id: Set(crash.version_id),
            product_id: Set(crash.product_id),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(model.id)
    }

    pub async fn update(
        db: &DbConn,
        id: uuid::Uuid,
        crash: CrashDto,
    ) -> Result<uuid::Uuid, DbError> {
        entity::crash::ActiveModel {
            id: Set(id),
            report: Set(serde_json::json!(crash.report)),
            updated_at: Set(chrono::offset::Utc::now().naive_utc()),
            ..Default::default()
        }
        .update(db)
        .await
        .map(|_| id)
        .map_err(|e| DbError::RecordNotFound("crash not found".to_owned()))?;

        Ok(id)
    }

    pub async fn get_all(db: &DbConn) -> Result<Vec<Crash>, DbError> {
        let crashs = entity::crash::Entity::find().all(db).await?;
        Ok(crashs)
    }

    pub async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Crash, DbError> {
        let crash = entity::crash::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("crash not found".to_owned()))?;

        Ok(crash)
    }

    pub async fn delete(db: &DbConn, id: uuid::Uuid) -> Result<(), DbError> {
        entity::crash::Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
