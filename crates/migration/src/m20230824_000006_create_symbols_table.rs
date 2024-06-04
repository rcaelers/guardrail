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
                    .table(Symbols::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Symbols::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Symbols::CreatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Symbols::UpdatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Symbols::Os).string().not_null())
                    .col(ColumnDef::new(Symbols::Arch).string().not_null())
                    .col(ColumnDef::new(Symbols::BuildId).string().not_null())
                    .col(ColumnDef::new(Symbols::ModuleId).string().not_null())
                    .col(ColumnDef::new(Symbols::FileLocation).string().not_null())
                    .col(ColumnDef::new(Symbols::ProductId).uuid().not_null())
                    .col(ColumnDef::new(Symbols::VersionId).uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-symbols-product")
                            .from(Symbols::Table, Symbols::ProductId)
                            .to(Product::Table, Product::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-symbols-version")
                            .from(Symbols::Table, Symbols::VersionId)
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
            .drop_table(Table::drop().table(Symbols::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Symbols {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    VersionId,
    ProductId,
    Os,
    Arch,
    BuildId,
    ModuleId,
    FileLocation,
}
