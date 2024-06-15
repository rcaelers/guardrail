use sea_orm_migration::prelude::*;

use super::m20230824_000001_create_product_table::Product;
use super::m20230824_000002_create_version_table::Version;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Crash::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Crash::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Crash::CreatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Crash::UpdatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Crash::Summary).string().not_null())
                    .col(ColumnDef::new(Crash::Report).json_binary().not_null())
                    .col(ColumnDef::new(Crash::VersionId).uuid().not_null())
                    .col(ColumnDef::new(Crash::ProductId).uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-crash-project")
                            .from(Crash::Table, Crash::ProductId)
                            .to(Product::Table, Product::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-crash-version")
                            .from(Crash::Table, Crash::VersionId)
                            .to(Version::Table, Version::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Crash::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Crash {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    Report,
    Summary,
    ProductId,
    VersionId,
}
