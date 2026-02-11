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
                    .name("ix_reminder_chat_id")
                    .table(Reminder::Table)
                    .col(Reminder::ChatId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_reminder_message_rec_chat_delivery_id")
                    .table(ReminderMessage::Table)
                    .col(ReminderMessage::RecId)
                    .col(ReminderMessage::ChatId)
                    .col(ReminderMessage::IsDelivery)
                    .col(ReminderMessage::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop().name("ix_reminder_message_rec_id").to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("ix_reminder_message_rec_chat_delivery_id")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(Index::drop().name("ix_reminder_chat_id").to_owned())
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_reminder_message_rec_id")
                    .table(ReminderMessage::Table)
                    .col(ReminderMessage::RecId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Reminder {
    Table,
    ChatId,
}

#[derive(Iden)]
enum ReminderMessage {
    Table,
    Id,
    RecId,
    ChatId,
    IsDelivery,
}
