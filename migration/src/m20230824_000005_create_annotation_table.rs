use sea_orm::{DbBackend, EnumIter, Iterable};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_query::extension::postgres::Type;

use super::m20230824_000003_create_crash_table::Crash;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        if let DbBackend::Postgres = db.get_database_backend() {
            manager
                .create_type(
                    Type::create()
                        .as_enum(AnnotationKind::Table)
                        .values([AnnotationKind::System, AnnotationKind::User])
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(Annotation::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Annotation::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Annotation::CreatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Annotation::UpdatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Annotation::Key).string().not_null())
                    .col(
                        ColumnDef::new(Annotation::Kind)
                            .enumeration(AnnotationKind::Table, AnnotationKind::iter().skip(1))
                            .not_null(),
                    )
                    .col(ColumnDef::new(Annotation::Value).string().not_null())
                    .col(ColumnDef::new(Annotation::CrashId).uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-annotation-crash")
                            .from(Annotation::Table, Annotation::CrashId)
                            .to(Crash::Table, Crash::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Annotation::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Annotation {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    Kind,
    Key,
    Value,
    CrashId,
}

#[derive(Iden, EnumIter)]
pub enum AnnotationKind {
    Table,
    #[iden = "system"]
    System,
    #[iden = "user"]
    User,
}
