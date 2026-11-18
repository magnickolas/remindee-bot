use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
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

        let populate_rec_ids = Query::update()
            .table(Reminder::Table)
            .value(Reminder::RecId, Expr::col(Reminder::Id))
            .cond_where(
                Cond::any()
                    .add(Expr::col(Reminder::RecId).eq(""))
                    .add(Expr::col(Reminder::RecId).is_null()),
            )
            .to_owned();
        conn.execute(&populate_rec_ids).await?;

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

        let delivery_messages = Query::select()
            .columns([Reminder::RecId, Reminder::ChatId, Reminder::MsgId])
            .from(Reminder::Table)
            .and_where(Expr::col(Reminder::MsgId).is_not_null())
            .to_owned();
        let mut insert_delivery_messages = Query::insert();
        insert_delivery_messages
            .into_table(ReminderMessage::Table)
            .columns([
                ReminderMessage::RecId,
                ReminderMessage::ChatId,
                ReminderMessage::MsgId,
            ])
            .select_from(delivery_messages)
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        conn.execute(&insert_delivery_messages).await?;

        let reply_messages = Query::select()
            .columns([Reminder::RecId, Reminder::ChatId, Reminder::ReplyId])
            .from(Reminder::Table)
            .and_where(Expr::col(Reminder::ReplyId).is_not_null())
            .to_owned();
        let mut insert_reply_messages = Query::insert();
        insert_reply_messages
            .into_table(ReminderMessage::Table)
            .columns([
                ReminderMessage::RecId,
                ReminderMessage::ChatId,
                ReminderMessage::MsgId,
            ])
            .select_from(reply_messages)
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        conn.execute(&insert_reply_messages).await?;

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
    Id,
    ChatId,
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
