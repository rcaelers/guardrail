pub use sea_orm_migration::prelude::*;

mod m20230824_000000_create_functions;
mod m20230824_000001_create_product_table;
mod m20230824_000002_create_version_table;
mod m20230824_000003_create_crash_table;
mod m20230824_000004_create_attachment_table;
mod m20230824_000005_create_annotation_table;
mod m20230824_000006_create_symbols_table;
mod m20230930_000008_create_session_table;
mod m20231210_000009_create_user_table;
mod m20231210_000010_create_credential_table;
mod m20240608_000011_create_role_table;

pub struct Migrator;
pub use m20230930_000008_create_session_table::Session as SessionColumns;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230824_000000_create_functions::Migration),
            Box::new(m20230824_000001_create_product_table::Migration),
            Box::new(m20230824_000002_create_version_table::Migration),
            Box::new(m20230824_000003_create_crash_table::Migration),
            Box::new(m20230824_000004_create_attachment_table::Migration),
            Box::new(m20230824_000005_create_annotation_table::Migration),
            Box::new(m20230824_000006_create_symbols_table::Migration),
            Box::new(m20230930_000008_create_session_table::Migration),
            Box::new(m20231210_000009_create_user_table::Migration),
            Box::new(m20231210_000010_create_credential_table::Migration),
            Box::new(m20240608_000011_create_role_table::Migration),
        ]
    }
}
