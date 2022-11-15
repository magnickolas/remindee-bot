use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            UPDATE `reminder` SET `user_id` = `chat_id`;
            UPDATE `cron_reminder` SET `user_id` = `chat_id`
        "#;
        let stmt = Statement::from_string(
            manager.get_database_backend(),
            sql.to_owned(),
        );
        manager.get_connection().execute(stmt).await.map(|_| ())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
