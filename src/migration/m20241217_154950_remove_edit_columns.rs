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
                    .drop_column(CronReminder::Edit)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::Edit)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .drop_column(CronReminder::EditMode)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::EditMode)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(
                        ColumnDef::new(Reminder::Edit).boolean().not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .add_column(
                        ColumnDef::new(CronReminder::Edit).boolean().not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(
                        ColumnDef::new(Reminder::EditMode)
                            .text()
                            .check(
                                Expr::col((
                                    Reminder::Table,
                                    Reminder::EditMode,
                                ))
                                .is_in([
                                    "time_pattern",
                                    "description",
                                    "none",
                                ]),
                            )
                            .not_null()
                            .default("none"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .add_column(
                        ColumnDef::new(CronReminder::EditMode)
                            .text()
                            .check(
                                Expr::col((
                                    CronReminder::Table,
                                    CronReminder::EditMode,
                                ))
                                .is_in([
                                    "time_pattern",
                                    "description",
                                    "none",
                                ]),
                            )
                            .not_null()
                            .default("none"),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    Edit,
    EditMode,
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    Edit,
    EditMode,
}
