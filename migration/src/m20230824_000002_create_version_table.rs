use sea_orm_migration::prelude::*;

use super::m20230824_000001_create_product_table::Product;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Version::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Version::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Version::CreatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Version::UpdatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Version::Name).string().not_null())
                    .col(ColumnDef::new(Version::Hash).string().not_null())
                    .col(ColumnDef::new(Version::Tag).string().not_null())
                    .col(ColumnDef::new(Version::ProductId).uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-version-project")
                            .from(Version::Table, Version::ProductId)
                            .to(Product::Table, Product::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-product-and-name")
                            .col(Version::Name)
                            .col(Version::ProductId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Version::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Version {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    Name,
    Hash,
    Tag,
    ProductId,
}
