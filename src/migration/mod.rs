pub use sea_orm_migration::prelude::*;

pub mod m20220101_000001_create_user_timezone_table;
pub mod m20221111_004928_create_reminder_table;
pub mod m20221111_005303_create_cron_reminder_table;
pub mod m20221113_214952_create_user_id_columns;
pub mod m20221115_001608_set_user_id_to_chat_id;
mod m20221119_222755_create_paused_columns;
mod m20221130_233038_remove_sent_column;
mod m20230224_061834_create_reminder_paused_columns;
mod m20230301_070153_create_reminder_pattern_column;
mod m20230526_143912_add_active_reminders_indexes;
mod m20240503_161417_create_edit_mode_columns;
mod m20241113_194950_create_msg_id_columns;
mod m20241113_200129_add_msg_id_indexes;
mod m20241114_105214_create_reply_id_columns;
mod m20241114_105217_add_reply_id_indexes;
mod m20241217_154950_remove_edit_columns;
mod m20250618_171311_create_user_language_table;
mod m20260130_120000_drop_cron_reminder_table;
mod m20260130_130000_add_rec_id_and_message_links;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_user_timezone_table::Migration),
            Box::new(m20221111_004928_create_reminder_table::Migration),
            Box::new(m20221111_005303_create_cron_reminder_table::Migration),
            Box::new(m20221113_214952_create_user_id_columns::Migration),
            Box::new(m20221115_001608_set_user_id_to_chat_id::Migration),
            Box::new(m20221119_222755_create_paused_columns::Migration),
            Box::new(m20221130_233038_remove_sent_column::Migration),
            Box::new(
                m20230224_061834_create_reminder_paused_columns::Migration,
            ),
            Box::new(
                m20230301_070153_create_reminder_pattern_column::Migration,
            ),
            Box::new(m20230526_143912_add_active_reminders_indexes::Migration),
            Box::new(m20240503_161417_create_edit_mode_columns::Migration),
            Box::new(m20241113_194950_create_msg_id_columns::Migration),
            Box::new(m20241113_200129_add_msg_id_indexes::Migration),
            Box::new(m20241114_105214_create_reply_id_columns::Migration),
            Box::new(m20241114_105217_add_reply_id_indexes::Migration),
            Box::new(m20241217_154950_remove_edit_columns::Migration),
            Box::new(m20250618_171311_create_user_language_table::Migration),
            Box::new(m20260130_120000_drop_cron_reminder_table::Migration),
            Box::new(m20260130_130000_add_rec_id_and_message_links::Migration),
        ]
    }
}
