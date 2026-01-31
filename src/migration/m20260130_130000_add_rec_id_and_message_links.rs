use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let db_backend = manager.get_database_backend();

        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(
                        ColumnDef::new(Reminder::RecId)
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        conn.execute(Statement::from_string(
            db_backend,
            "UPDATE reminder SET rec_id = id WHERE rec_id = '' OR rec_id IS NULL",
        ))
        .await?;

        manager
            .create_table(
                Table::create()
                    .table(ReminderMessage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReminderMessage::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(ReminderMessage::RecId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReminderMessage::ChatId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReminderMessage::MsgId)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        conn.execute(Statement::from_string(
            db_backend,
            concat!(
                "INSERT INTO reminder_message (rec_id, chat_id, msg_id) ",
                "SELECT rec_id, chat_id, msg_id FROM reminder ",
                "WHERE msg_id IS NOT NULL"
            ),
        ))
        .await?;
        conn.execute(Statement::from_string(
            db_backend,
            concat!(
                "INSERT INTO reminder_message (rec_id, chat_id, msg_id) ",
                "SELECT rec_id, chat_id, reply_id FROM reminder ",
                "WHERE reply_id IS NOT NULL"
            ),
        ))
        .await?;

        manager
            .drop_index(Index::drop().name("ix_reminder_msg_id").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("ix_reminder_reply_id").to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::MsgId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::ReplyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_reminder_message_chat_msg")
                    .table(ReminderMessage::Table)
                    .col(ReminderMessage::ChatId)
                    .col(ReminderMessage::MsgId)
                    .to_owned(),
            )
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
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_reminder_rec_id")
                    .table(Reminder::Table)
                    .col(Reminder::RecId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("ix_reminder_message_chat_msg")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop().name("ix_reminder_message_rec_id").to_owned(),
            )
            .await?;
        manager
            .drop_index(Index::drop().name("ix_reminder_rec_id").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ReminderMessage::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(ColumnDef::new(Reminder::MsgId).integer())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(ColumnDef::new(Reminder::ReplyId).integer())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::RecId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_reminder_msg_id")
                    .table(Reminder::Table)
                    .col(Reminder::MsgId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_reminder_reply_id")
                    .table(Reminder::Table)
                    .col(Reminder::ReplyId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    MsgId,
    ReplyId,
    RecId,
}

#[derive(Iden)]
pub enum ReminderMessage {
    Table,
    Id,
    RecId,
    ChatId,
    MsgId,
}
