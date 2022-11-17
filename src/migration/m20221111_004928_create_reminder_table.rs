use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Reminder::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Reminder::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(Reminder::UserId).integer().not_null())
                    .col(ColumnDef::new(Reminder::Time).date_time().not_null())
                    .col(ColumnDef::new(Reminder::Desc).text().not_null())
                    .col(ColumnDef::new(Reminder::Sent).boolean().not_null())
                    .col(ColumnDef::new(Reminder::Edit).boolean().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Reminder::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    Id,
    UserId,
    Time,
    Desc,
    Sent,
    Edit,
}
