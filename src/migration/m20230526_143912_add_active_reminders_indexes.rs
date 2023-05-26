use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_reminder_paused_time")
                    .table(Reminder::Table)
                    .col(Reminder::Paused)
                    .col(Reminder::Time)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_cron_reminder_paused_time")
                    .table(CronReminder::Table)
                    .col(CronReminder::Paused)
                    .col(CronReminder::Time)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop().name("ix_reminder_paused_time").to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("ix_cron_reminder_paused_time")
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    Paused,
    Time,
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    Paused,
    Time,
}
