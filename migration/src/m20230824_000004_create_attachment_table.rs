use sea_orm_migration::prelude::*;

use super::m20230824_000003_create_crash_table::Crash;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Attachment::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Attachment::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Attachment::CreatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Attachment::UpdatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Attachment::Name).string().not_null())
                    .col(ColumnDef::new(Attachment::MimeType).string().not_null())
                    .col(ColumnDef::new(Attachment::Size).big_integer().not_null())
                    .col(ColumnDef::new(Attachment::Filename).string().not_null())
                    .col(ColumnDef::new(Attachment::CrashId).uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-attachment-crash")
                            .from(Attachment::Table, Attachment::CrashId)
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
            .drop_table(Table::drop().table(Attachment::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Attachment {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    Name,
    MimeType,
    Size,
    Filename,
    CrashId,
}
