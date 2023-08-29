use async_trait::async_trait;
use chrono::NaiveDateTime;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::base::{BaseRepo, HasId};
use super::error::DbError;
use crate::entity;
pub use entity::annotation::Model as Annotation;
pub use entity::attachment::Model as Attachment;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrashDto {
    pub report: String,
    pub version_id: Uuid,
    pub product_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Crash {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub report: serde_json::Value,
    pub version_id: Uuid,
    pub product_id: Uuid,
    pub annotations: Vec<Annotation>,
    pub attachments: Vec<Attachment>,
}

impl From<entity::crash::Model> for Crash {
    fn from(crash: entity::crash::Model) -> Self {
        Self {
            id: crash.id,
            created_at: crash.created_at,
            updated_at: crash.updated_at,
            report: crash.report,
            version_id: crash.version_id,
            product_id: crash.product_id,
            annotations: vec![],
            attachments: vec![],
        }
    }
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
    type Repr = Crash;
    type ActiveModel = entity::crash::ActiveModel;
    type PrimaryKeyType = uuid::Uuid;
}

impl CrashRepo {
    async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Crash, DbError> {
        let model = entity::crash::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbError::RecordNotFound("product not found".to_owned()))?;

        let annotations: Vec<entity::annotation::Model> = model
            .find_related(entity::annotation::Entity)
            .all(db)
            .await?;
        let attachments: Vec<entity::attachment::Model> = model
            .find_related(entity::attachment::Entity)
            .all(db)
            .await?;

        let mut crash = Crash::from(model);
        crash.annotations = annotations.into_iter().map(Annotation::from).collect();
        crash.attachments = attachments.into_iter().map(Attachment::from).collect();
        Ok(crash)
    }
}
#[cfg(test)]
mod tests {
    use crate::entity::sea_orm_active_enums::AnnotationKind;
    use crate::model::annotation::{AnnotationDto, AnnotationRepo};
    use crate::model::attachment::{AttachmentDto, AttachmentRepo};
    use crate::model::product::{ProductDto, ProductRepo};
    use crate::model::version::{VersionDto, VersionRepo};
    use serial_test::serial;

    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection};

    use super::{CrashDto, CrashRepo};
    use crate::model::base::BaseRepo;

    #[serial]
    #[tokio::test]
    async fn test_create() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = ProductDto {
            name: "Wprkrave".to_owned(),
            report_api_key: Some("test_report_api_key1".to_owned()),
            symbol_api_key: Some("test_symbol_api_key1".to_owned()),
        };
        let idp = ProductRepo::create(&db, product).await.unwrap();

        let version = VersionDto {
            name: "1.0.0".to_owned(),
            hash: "test_hash1".to_owned(),
            tag: "test_tag1".to_owned(),
            product_id: idp,
        };
        let idv = VersionRepo::create(&db, version).await.unwrap();

        let crash = CrashDto {
            report: "test_report1".to_owned(),
            version_id: idv,
            product_id: idp,
        };
        let idc = CrashRepo::create(&db, crash).await.unwrap();

        let attachment1 = AttachmentDto {
            name: "test_name1".to_owned(),
            mime_type: "test_mime_type1".to_owned(),
            size: 1,
            filename: "test_filename1".to_owned(),
            crash_id: idc,
        };
        let idat1 = AttachmentRepo::create(&db, attachment1).await.unwrap();

        let attachment2 = AttachmentDto {
            name: "test_name2".to_owned(),
            mime_type: "test_mime_type2".to_owned(),
            size: 2,
            filename: "test_filename2".to_owned(),
            crash_id: idc,
        };
        let idat2 = AttachmentRepo::create(&db, attachment2).await.unwrap();

        let annotation = AnnotationDto {
            key: "test_key1".to_owned(),
            kind: AnnotationKind::System,
            value: "test_value1".to_owned(),
            crash_id: idc,
        };
        let idan = AnnotationRepo::create(&db, annotation).await.unwrap();

        let c = CrashRepo::get_by_id(&db, idc).await.unwrap();

        assert_eq!(c.id, idc);
        assert_eq!(c.report, serde_json::json!("test_report1"));
        assert_eq!(c.version_id, idv);
        assert_eq!(c.product_id, idp);
        assert_eq!(c.annotations.len(), 1);
        assert_eq!(c.attachments.len(), 2);

        assert_eq!(c.annotations[0].id, idan);
        assert_eq!(c.annotations[0].crash_id, idc);
        assert_eq!(c.annotations[0].crash_id, idc);
        assert_eq!(c.annotations[0].key, "test_key1");
        assert_eq!(c.annotations[0].kind, AnnotationKind::System);
        assert_eq!(c.annotations[0].value, "test_value1");

        assert_eq!(c.attachments[0].id, idat1);
        assert_eq!(c.attachments[0].crash_id, idc);
        assert_eq!(c.attachments[0].name, "test_name1");
        assert_eq!(c.attachments[0].mime_type, "test_mime_type1");
        assert_eq!(c.attachments[0].size, 1);
        assert_eq!(c.attachments[0].filename, "test_filename1");
        assert_eq!(c.attachments[0].crash_id, idc);

        assert_eq!(c.attachments[1].id, idat2);
        assert_eq!(c.attachments[1].crash_id, idc);
        assert_eq!(c.attachments[1].name, "test_name2");
        assert_eq!(c.attachments[1].mime_type, "test_mime_type2");
        assert_eq!(c.attachments[1].size, 2);
        assert_eq!(c.attachments[1].filename, "test_filename2");
        assert_eq!(c.attachments[1].crash_id, idc);
    }
}
