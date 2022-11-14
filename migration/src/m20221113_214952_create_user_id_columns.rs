use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rename user_id column to chat_id
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .rename_column(CronReminder::UserId, CronReminder::ChatId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .rename_column(Reminder::UserId, Reminder::ChatId)
                    .to_owned(),
            )
            .await?;

        // Add user_id column
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .add_column(ColumnDef::new(CronReminder::UserId).integer())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(ColumnDef::new(Reminder::UserId).integer())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove user_id column
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .drop_column(CronReminder::UserId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::UserId)
                    .to_owned(),
            )
            .await?;

        // Rename chat_id column to user_id
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .rename_column(CronReminder::ChatId, CronReminder::UserId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .rename_column(Reminder::ChatId, Reminder::UserId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    UserId,
    ChatId,
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    UserId,
    ChatId,
}
