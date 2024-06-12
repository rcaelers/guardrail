use sea_orm_migration::prelude::*;

use super::m20231210_000009_create_user_table::User;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Role::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Role::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Role::CreatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Role::UpdatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Role::Name).string().not_null())
                    .col(ColumnDef::new(Role::UserId).uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-role-user")
                            .from(Role::Table, Role::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Role::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Role {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    Name,
    UserId,
}
