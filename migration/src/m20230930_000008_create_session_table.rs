use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Session::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Session::Id).text().not_null().primary_key())
                    .col(ColumnDef::new(Session::ExpiresAt).date_time().not_null())
                    .col(ColumnDef::new(Session::Data).binary().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Session::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Session {
    Table,
    Id,
    ExpiresAt,
    Data,
}
