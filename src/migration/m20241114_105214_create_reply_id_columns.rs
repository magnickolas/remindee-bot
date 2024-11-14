use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create msg_id column
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .add_column(ColumnDef::new(CronReminder::ReplyId).integer())
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
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove user_id column
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .drop_column(CronReminder::ReplyId)
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
            .await
    }
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    ReplyId,
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    ReplyId,
}
