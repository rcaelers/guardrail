use super::base::HasId;
use crate::entity;
pub use entity::annotation::Model as Annotation;
pub use entity::attachment::Model as Attachment;

use chrono::NaiveDateTime;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type CrashCreateDto = entity::crash::CreateModel;
pub type CrashUpdateDto = entity::crash::UpdateModel;

impl HasId for Crash {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

impl HasId for entity::crash::Model {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
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

impl CrashRepo {
    async fn get_by_id(db: &DbConn, id: uuid::Uuid) -> Result<Crash, DbErr> {
        let model = entity::prelude::Crash::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("crash not found".to_owned()))?;

        let annotations: Vec<entity::annotation::Model> = model
            .find_related(entity::prelude::Annotation)
            .all(db)
            .await?;
        let attachments: Vec<entity::attachment::Model> = model
            .find_related(entity::prelude::Attachment)
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
    use crate::{entity::sea_orm_active_enums::AnnotationKind, model::crash::CrashRepo};
    use serial_test::serial;

    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection};

    use crate::model::base::Repo;

    #[serial]
    #[tokio::test]
    async fn test_create() {
        let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        let product = crate::entity::product::CreateModel {
            name: "Workrave".to_owned(),
        };
        let idp = Repo::create(&db, product).await.unwrap();

        let version = crate::entity::version::CreateModel {
            name: "1.0.0".to_owned(),
            hash: "test_hash1".to_owned(),
            tag: "test_tag1".to_owned(),
            product_id: idp,
        };
        let idv = Repo::create(&db, version).await.unwrap();

        let crash = crate::entity::crash::CreateModel {
            report: serde_json::json!("test_report1"),
            version_id: idv,
            product_id: idp,
        };
        let idc = Repo::create(&db, crash).await.unwrap();

        let attachment1 = crate::entity::attachment::CreateModel {
            name: "test_name1".to_owned(),
            mime_type: "test_mime_type1".to_owned(),
            size: 1,
            filename: "test_filename1".to_owned(),
            crash_id: idc,
        };
        let idat1 = Repo::create(&db, attachment1).await.unwrap();

        let attachment2 = crate::entity::attachment::CreateModel {
            name: "test_name2".to_owned(),
            mime_type: "test_mime_type2".to_owned(),
            size: 2,
            filename: "test_filename2".to_owned(),
            crash_id: idc,
        };
        let idat2 = Repo::create(&db, attachment2).await.unwrap();

        let annotation = crate::entity::annotation::CreateModel {
            key: "test_key1".to_owned(),
            kind: AnnotationKind::System,
            value: "test_value1".to_owned(),
            crash_id: idc,
        };
        let idan = Repo::create(&db, annotation).await.unwrap();

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
