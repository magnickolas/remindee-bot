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
mod m20260130_130000_add_rec_id_and_message_links;
mod m20260131_120000_migrate_cron_reminders;
mod m20260204_000001_create_reminder_message_is_delivery_column;
mod m20260205_000001_add_nagging_occurrences;
mod m20260211_120000_optimize_lookup_indexes;
mod m20260402_120000_add_reminder_chat_paused_time_index;

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
            Box::new(m20260130_130000_add_rec_id_and_message_links::Migration),
            Box::new(m20260131_120000_migrate_cron_reminders::Migration),
            Box::new(
                m20260204_000001_create_reminder_message_is_delivery_column::Migration,
            ),
            Box::new(m20260205_000001_add_nagging_occurrences::Migration),
            Box::new(m20260211_120000_optimize_lookup_indexes::Migration),
            Box::new(
                m20260402_120000_add_reminder_chat_paused_time_index::Migration,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, EntityTrait};

    use super::{Migrator, MigratorTrait, SchemaManager};
    use crate::entity::{reminder, reminder_message};

    #[tokio::test]
    async fn modernized_migrations_preserve_legacy_data() {
        let db = Database::connect("sqlite::memory:").await.unwrap();

        // Stop after chat_id was split from user_id, immediately before the
        // modernized data migrations begin.
        Migrator::up(&db, Some(4)).await.unwrap();
        db.execute_unprepared(
            r#"
            INSERT INTO reminder
                (id, chat_id, time, desc, sent, edit, user_id)
            VALUES
                (7, 42, '2026-01-01 12:00:00', 'kept reminder', 0, 0, NULL),
                (8, 42, '2026-01-02 12:00:00', 'sent reminder', 1, 0, NULL);
            INSERT INTO cron_reminder
                (id, chat_id, cron_expr, time, desc, sent, edit, user_id)
            VALUES
                (5, 99, '0 9 * * *', '2026-01-03 09:00:00',
                 'kept cron reminder', 0, 0, NULL),
                (6, 99, '0 10 * * *', '2026-01-04 10:00:00',
                 'sent cron reminder', 1, 0, NULL)
            "#,
        )
        .await
        .unwrap();

        // Apply through the migration immediately before rec_id and message
        // links are introduced.
        Migrator::up(&db, Some(13)).await.unwrap();
        db.execute_unprepared(
            r#"
            UPDATE reminder SET msg_id = 101, reply_id = 102 WHERE id = 7;
            UPDATE cron_reminder SET msg_id = 201, reply_id = 202 WHERE id = 5
            "#,
        )
        .await
        .unwrap();

        Migrator::up(&db, None).await.unwrap();

        let mut reminders = reminder::Entity::find().all(&db).await.unwrap();
        reminders.sort_by_key(|model| model.rec_id.clone());
        assert_eq!(reminders.len(), 2);
        assert_eq!(reminders[0].rec_id, "7");
        assert_eq!(reminders[0].chat_id, 42);
        assert_eq!(reminders[0].user_id, Some(42));
        assert_eq!(reminders[0].desc, "kept reminder");
        assert_eq!(reminders[1].rec_id, "cron:5");
        assert_eq!(reminders[1].chat_id, 99);
        assert_eq!(reminders[1].user_id, Some(99));
        assert_eq!(reminders[1].desc, "kept cron reminder");

        let mut messages =
            reminder_message::Entity::find().all(&db).await.unwrap();
        messages.sort_by_key(|model| model.msg_id);
        assert_eq!(
            messages
                .iter()
                .map(|model| (
                    model.rec_id.as_str(),
                    model.chat_id,
                    model.msg_id,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("7", 42, 101),
                ("7", 42, 102),
                ("cron:5", 99, 201),
                ("cron:5", 99, 202),
            ]
        );

        let manager = SchemaManager::new(&db);
        assert!(!manager.has_table("cron_reminder").await.unwrap());
    }
}
