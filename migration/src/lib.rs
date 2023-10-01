pub use sea_orm_migration::prelude::*;

mod m20230824_000001_create_product_table;
mod m20230824_000002_create_version_table;
mod m20230824_000003_create_crash_table;
mod m20230824_000004_create_attachment_table;
mod m20230824_000005_create_annotation_table;
mod m20230824_000006_create_symbols_table;
mod m20230824_000007_create_user_table;
mod m20230930_000008_create_session_table;
pub struct Migrator;
pub use m20230930_000008_create_session_table::Session as SessionColumns;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230824_000001_create_product_table::Migration),
            Box::new(m20230824_000002_create_version_table::Migration),
            Box::new(m20230824_000003_create_crash_table::Migration),
            Box::new(m20230824_000004_create_attachment_table::Migration),
            Box::new(m20230824_000005_create_annotation_table::Migration),
            Box::new(m20230824_000006_create_symbols_table::Migration),
            Box::new(m20230824_000007_create_user_table::Migration),
            Box::new(m20230930_000008_create_session_table::Migration),
        ]
    }
}
