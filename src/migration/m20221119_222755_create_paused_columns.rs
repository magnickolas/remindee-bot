use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .add_column(
                        ColumnDef::new(CronReminder::Paused)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .drop_column(CronReminder::Paused)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    Paused,
}
