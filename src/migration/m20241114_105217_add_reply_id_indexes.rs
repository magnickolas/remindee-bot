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
                    .name("ix_reminder_reply_id")
                    .table(Reminder::Table)
                    .col(Reminder::ReplyId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_cron_reminder_reply_id")
                    .table(CronReminder::Table)
                    .col(CronReminder::ReplyId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("ix_reminder_reply_id").to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop().name("ix_cron_reminder_reply_id").to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    ReplyId,
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    ReplyId,
}
