use sea_orm_migration::prelude::*;

use crate::m20231210_000009_create_user_table::User;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Credential::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Credential::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Credential::UserId).uuid().not_null())
                    .col(ColumnDef::new(Credential::Name).string().not_null())
                    .col(ColumnDef::new(Credential::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(Credential::LastUsed).timestamp().not_null())
                    .col(ColumnDef::new(Credential::Credential).json().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-credential-user")
                            .from(Credential::Table, Credential::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Credential::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Credential {
    Table,
    Id,
    UserId,
    Name,
    CreatedAt,
    LastUsed,
    Credential,
}
