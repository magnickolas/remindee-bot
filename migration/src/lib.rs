pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_user_timezone_table;
mod m20221111_004928_create_reminder_table;
mod m20221111_005303_create_cron_reminder_table;
mod m20221113_214952_create_user_id_columns;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_user_timezone_table::Migration),
            Box::new(m20221111_004928_create_reminder_table::Migration),
            Box::new(m20221111_005303_create_cron_reminder_table::Migration),
            Box::new(m20221113_214952_create_user_id_columns::Migration),
        ]
    }
}
