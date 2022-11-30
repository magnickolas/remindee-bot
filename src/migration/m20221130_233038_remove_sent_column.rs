use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let db_backend = manager.get_database_backend();
        // Delete sent reminders
        let sql = Query::delete()
            .from_table(CronReminder::Table)
            .and_where(Expr::col(CronReminder::Sent).eq(true))
            .to_string(SqliteQueryBuilder);
        let stmt = Statement::from_string(db_backend, sql);
        conn.execute(stmt).await?;
        let sql = Query::delete()
            .from_table(Reminder::Table)
            .and_where(Expr::col(Reminder::Sent).eq(true))
            .to_string(SqliteQueryBuilder);
        let stmt = Statement::from_string(db_backend, sql);
        conn.execute(stmt).await?;

        // Remove sent columns
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .drop_column(CronReminder::Sent)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::Sent)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create sent columns with false value
        manager
            .alter_table(
                Table::alter()
                    .table(CronReminder::Table)
                    .add_column(
                        ColumnDef::new(CronReminder::Sent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(
                        ColumnDef::new(Reminder::Sent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Reminder {
    Table,
    Sent,
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    Sent,
}
