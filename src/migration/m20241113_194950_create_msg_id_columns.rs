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
                    .add_column(ColumnDef::new(CronReminder::MsgId).integer())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(ColumnDef::new(Reminder::MsgId).integer())
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
                    .drop_column(CronReminder::MsgId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::MsgId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    MsgId,
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    MsgId,
}
