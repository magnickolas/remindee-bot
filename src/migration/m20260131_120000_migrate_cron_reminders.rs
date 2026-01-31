use chrono::NaiveDateTime;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ConnectionTrait, EntityTrait,
    QueryResult, Set, Statement,
};
use sea_orm_migration::prelude::*;

use crate::entity::{reminder, reminder_message, user_timezone};
use crate::serializers::Pattern;
use chrono_tz::Tz;
use remindee_parser as grammar;
use serde_json::to_string;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table("cron_reminder").await? {
            return Ok(());
        }

        let conn = manager.get_connection();
        let db_backend = manager.get_database_backend();

        let rows = conn
            .query_all(Statement::from_string(
                db_backend,
                concat!(
                    "SELECT id, chat_id, cron_expr, time, desc, user_id, ",
                    "paused, msg_id, reply_id ",
                    "FROM cron_reminder"
                ),
            ))
            .await?;

        for row in rows {
            let id = get_required_i64(&row, "id")?;
            let chat_id = get_required_i64(&row, "chat_id")?;
            let cron_expr = get_required_string(&row, "cron_expr")?;
            let time = get_required_time(&row, "time")?;
            let desc = get_required_string(&row, "desc")?;
            let user_id = get_optional_i64(&row, "user_id")?;
            let paused = get_required_bool(&row, "paused")?;
            let msg_id = get_optional_i32(&row, "msg_id")?;
            let reply_id = get_optional_i32(&row, "reply_id")?;

            let tz_name = match user_id {
                Some(user_id) => user_timezone::Entity::find_by_id(user_id)
                    .one(conn)
                    .await?
                    .map(|tz| tz.timezone),
                None => None,
            };

            let tz = tz_name
                .as_deref()
                .unwrap_or("UTC")
                .parse::<Tz>()
                .unwrap_or(chrono_tz::UTC);

            let pattern = match Pattern::from_with_tz(
                grammar::ReminderPattern::Cron(grammar::Cron {
                    expr: cron_expr.clone(),
                }),
                tz,
            ) {
                Ok(pattern) => pattern,
                Err(_) => continue,
            };

            let pattern_json = to_string(&pattern)
                .map_err(|err| DbErr::Custom(err.to_string()))?;

            let rec_id = format!("cron:{id}");
            reminder::ActiveModel {
                id: NotSet,
                rec_id: Set(rec_id.clone()),
                chat_id: Set(chat_id),
                time: Set(time),
                desc: Set(desc),
                user_id: Set(user_id),
                paused: Set(paused),
                pattern: Set(Some(pattern_json)),
            }
            .insert(conn)
            .await?;

            if let Some(msg_id) = msg_id {
                reminder_message::ActiveModel {
                    id: NotSet,
                    rec_id: Set(rec_id.clone()),
                    chat_id: Set(chat_id),
                    msg_id: Set(msg_id),
                }
                .insert(conn)
                .await?;
            }

            if let Some(reply_id) = reply_id {
                reminder_message::ActiveModel {
                    id: NotSet,
                    rec_id: Set(rec_id.clone()),
                    chat_id: Set(chat_id),
                    msg_id: Set(reply_id),
                }
                .insert(conn)
                .await?;
            }
        }

        conn.execute(Statement::from_string(
            db_backend,
            "DROP INDEX IF EXISTS ix_cron_reminder_reply_id",
        ))
        .await?;
        conn.execute(Statement::from_string(
            db_backend,
            "DROP INDEX IF EXISTS ix_cron_reminder_msg_id",
        ))
        .await?;
        conn.execute(Statement::from_string(
            db_backend,
            "DROP INDEX IF EXISTS ix_cron_reminder_paused_time",
        ))
        .await?;

        manager
            .drop_table(Table::drop().table(CronReminder::Table).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_table("cron_reminder").await? {
            return Ok(());
        }

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
                        ColumnDef::new(CronReminder::ChatId)
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
                    .col(ColumnDef::new(CronReminder::UserId).integer())
                    .col(
                        ColumnDef::new(CronReminder::Paused)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(CronReminder::MsgId).integer())
                    .col(ColumnDef::new(CronReminder::ReplyId).integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_cron_reminder_paused_time")
                    .table(CronReminder::Table)
                    .col(CronReminder::Paused)
                    .col(CronReminder::Time)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_cron_reminder_msg_id")
                    .table(CronReminder::Table)
                    .col(CronReminder::MsgId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_cron_reminder_reply_id")
                    .table(CronReminder::Table)
                    .col(CronReminder::ReplyId)
                    .to_owned(),
            )
            .await
    }
}

fn get_required_i64(row: &QueryResult, name: &str) -> Result<i64, DbErr> {
    row.try_get::<i64>("", name)
        .map_err(|err| DbErr::Custom(err.to_string()))
}

fn get_optional_i64(
    row: &QueryResult,
    name: &str,
) -> Result<Option<i64>, DbErr> {
    row.try_get::<Option<i64>>("", name)
        .map_err(|err| DbErr::Custom(err.to_string()))
}

fn get_optional_i32(
    row: &QueryResult,
    name: &str,
) -> Result<Option<i32>, DbErr> {
    row.try_get::<Option<i32>>("", name)
        .map_err(|err| DbErr::Custom(err.to_string()))
}

fn get_required_string(row: &QueryResult, name: &str) -> Result<String, DbErr> {
    row.try_get::<String>("", name)
        .map_err(|err| DbErr::Custom(err.to_string()))
}

fn get_required_time(
    row: &QueryResult,
    name: &str,
) -> Result<NaiveDateTime, DbErr> {
    row.try_get::<NaiveDateTime>("", name)
        .map_err(|err| DbErr::Custom(err.to_string()))
}

fn get_required_bool(row: &QueryResult, name: &str) -> Result<bool, DbErr> {
    row.try_get::<bool>("", name)
        .map_err(|err| DbErr::Custom(err.to_string()))
}

#[derive(Iden)]
pub enum CronReminder {
    Table,
    Id,
    ChatId,
    CronExpr,
    Time,
    Desc,
    UserId,
    Paused,
    MsgId,
    ReplyId,
}
