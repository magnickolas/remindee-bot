use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(ColumnDef::new(Reminder::Pattern).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::Pattern)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    Pattern,
}
