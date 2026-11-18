use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let update_reminders = Query::update()
            .table(Reminder::Table)
            .value(Reminder::UserId, Expr::col(Reminder::ChatId))
            .to_owned();
        conn.execute(&update_reminders).await?;

        let update_cron_reminders = Query::update()
            .table(CronReminder::Table)
            .value(CronReminder::UserId, Expr::col(CronReminder::ChatId))
            .to_owned();
        conn.execute(&update_cron_reminders).await.map(|_| ())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(Iden)]
enum Reminder {
    Table,
    UserId,
    ChatId,
}

#[derive(Iden)]
enum CronReminder {
    Table,
    UserId,
    ChatId,
}
