use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CronReminder::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CronReminder::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CronReminder::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CronReminder::CronExpr)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CronReminder::Time)
                            .date_time()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CronReminder::Desc).text().not_null())
                    .col(
                        ColumnDef::new(CronReminder::Sent).boolean().not_null(),
                    )
                    .col(
                        ColumnDef::new(CronReminder::Edit).boolean().not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CronReminder::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    Id,
    UserId,
    CronExpr,
    Time,
    Desc,
    Sent,
    Edit,
}
