use std::path::Path;

use crate::cli::CLI;
use crate::entity::{cron_reminder, reminder, user_timezone};
use crate::generic_reminder;
use crate::migration::{DbErr, Migrator, MigratorTrait};
use crate::parsers::now_time;
use chrono::NaiveDateTime;
#[cfg(test)]
use mockall::automock;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database as SeaOrmDatabase,
    DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use tokio::sync::futures::Notified;
use tokio::sync::Notify;

#[derive(Debug)]
pub(crate) enum Error {
    Database(DbErr),
    File(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Database(ref err) => {
                write!(f, "Database error: {}", err)
            }
            Self::File(ref err) => write!(f, "File error: {}", err),
        }
    }
}

impl From<DbErr> for Error {
    fn from(err: DbErr) -> Self {
        Self::Database(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::File(err)
    }
}

async fn get_db_pool(db_path: &Path) -> Result<DatabaseConnection, Error> {
    let db_str = format!("sqlite:{}?mode=rwc", db_path.display());
    let mut opts = ConnectOptions::new(&db_str);
    opts.max_connections(CLI.sqlite_max_connections);
    let pool = SeaOrmDatabase::connect(opts).await?;
    Ok(pool)
}

struct ScopeCall<F: FnMut()> {
    c: F,
}
impl<F: FnMut()> Drop for ScopeCall<F> {
    fn drop(&mut self) {
        (self.c)();
    }
}

macro_rules! defer {
    ($e:expr) => {
        let _scope_call = ScopeCall {
            c: || -> () {
                $e;
            },
        };
    };
}

pub(crate) struct Database {
    pool: DatabaseConnection,
    notify: Notify,
}

#[cfg_attr(test, automock, allow(dead_code))]
impl Database {
    pub(crate) async fn new_with_path(db_path: &Path) -> Result<Self, Error> {
        get_db_pool(db_path).await.map(|pool| Self {
            pool,
            notify: Notify::new(),
        })
    }

    pub(crate) async fn apply_migrations(&self) -> Result<(), Error> {
        Ok(Migrator::up(&self.pool, None).await?)
    }

    pub(crate) async fn get_reminder(
        &self,
        id: i64,
    ) -> Result<Option<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::Id.eq(id))
            .one(&self.pool)
            .await?)
    }

    pub(crate) async fn insert_reminder(
        &self,
        rem: reminder::ActiveModel,
    ) -> Result<reminder::ActiveModel, Error> {
        defer!(self.notify.notify_one());
        Ok(rem.save(&self.pool).await?)
    }

    pub(crate) async fn delete_reminder(&self, id: i64) -> Result<(), Error> {
        reminder::ActiveModel {
            id: Set(id),
            ..Default::default()
        }
        .delete(&self.pool)
        .await?;
        Ok(())
    }

    async fn next_reminder_time(&self) -> Result<Option<NaiveDateTime>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::Paused.eq(false))
            .order_by_asc(reminder::Column::Time)
            .one(&self.pool)
            .await?
            .map(|r| r.time))
    }

    async fn next_cron_reminder_time(
        &self,
    ) -> Result<Option<NaiveDateTime>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::Paused.eq(false))
            .order_by_asc(cron_reminder::Column::Time)
            .one(&self.pool)
            .await?
            .map(|r| r.time))
    }

    pub(crate) async fn get_next_reminder_time(
        &self,
    ) -> Result<Option<NaiveDateTime>, Error> {
        let next_reminder_time = self.next_reminder_time().await?;
        let next_cron_reminder_time = self.next_cron_reminder_time().await?;
        match (next_reminder_time, next_cron_reminder_time) {
            (Some(rem), Some(cron_rem)) => Ok(Some(rem.min(cron_rem))),
            (Some(rem), None) => Ok(Some(rem)),
            (None, Some(cron_rem)) => Ok(Some(cron_rem)),
            (None, None) => Ok(None),
        }
    }

    pub(crate) async fn get_active_reminders(
        &self,
    ) -> Result<Vec<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::Paused.eq(false))
            .filter(reminder::Column::Time.lt(now_time()))
            .all(&self.pool)
            .await?)
    }

    pub(crate) async fn get_pending_chat_reminders(
        &self,
        chat_id: i64,
    ) -> Result<Vec<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::ChatId.eq(chat_id))
            .all(&self.pool)
            .await?)
    }

    pub(crate) async fn get_user_timezone_name(
        &self,
        user_id: i64,
    ) -> Result<Option<String>, Error> {
        Ok(user_timezone::Entity::find_by_id(user_id)
            .one(&self.pool)
            .await?
            .map(|x| x.timezone))
    }

    async fn insert_user_timezone_name(
        &self,
        user_id: i64,
        timezone: &str,
    ) -> Result<(), Error> {
        defer!(self.notify.notify_one());
        user_timezone::Entity::insert(user_timezone::ActiveModel {
            user_id: Set(user_id),
            timezone: Set(timezone.to_string()),
        })
        .exec(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn insert_or_update_user_timezone(
        &self,
        user_id: i64,
        timezone: &str,
    ) -> Result<(), Error> {
        if let Some(mut tz_act) = user_timezone::Entity::find_by_id(user_id)
            .one(&self.pool)
            .await?
            .map(Into::<user_timezone::ActiveModel>::into)
        {
            tz_act.timezone = Set(timezone.to_string());
            tz_act.update(&self.pool).await?;
        } else {
            self.insert_user_timezone_name(user_id, timezone).await?;
        }
        Ok(())
    }

    pub(crate) async fn get_cron_reminder(
        &self,
        id: i64,
    ) -> Result<Option<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::Id.eq(id))
            .one(&self.pool)
            .await?)
    }

    pub(crate) async fn insert_cron_reminder(
        &self,
        rem: cron_reminder::ActiveModel,
    ) -> Result<cron_reminder::ActiveModel, Error> {
        defer!(self.notify.notify_one());
        Ok(rem.save(&self.pool).await?)
    }

    pub(crate) async fn delete_cron_reminder(
        &self,
        id: i64,
    ) -> Result<(), Error> {
        cron_reminder::ActiveModel {
            id: Set(id),
            ..Default::default()
        }
        .delete(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn toggle_reminder_paused(
        &self,
        id: i64,
    ) -> Result<bool, Error> {
        defer!(self.notify.notify_one());
        let rem: Option<reminder::Model> =
            reminder::Entity::find_by_id(id).one(&self.pool).await?;
        if let Some(rem) = rem {
            let paused_value = !rem.paused;
            let mut rem_act: reminder::ActiveModel = rem.into();
            rem_act.paused = Set(paused_value);
            rem_act.update(&self.pool).await?;
            Ok(paused_value)
        } else {
            Err(Error::Database(DbErr::RecordNotFound(id.to_string())))
        }
    }

    pub(crate) async fn toggle_cron_reminder_paused(
        &self,
        id: i64,
    ) -> Result<bool, Error> {
        defer!(self.notify.notify_one());
        let cron_rem: Option<cron_reminder::Model> =
            cron_reminder::Entity::find_by_id(id)
                .one(&self.pool)
                .await?;
        if let Some(cron_rem) = cron_rem {
            let paused_value = !cron_rem.paused;
            let mut cron_rem_act: cron_reminder::ActiveModel = cron_rem.into();
            cron_rem_act.paused = Set(paused_value);
            cron_rem_act.update(&self.pool).await?;
            Ok(paused_value)
        } else {
            Err(Error::Database(DbErr::RecordNotFound(id.to_string())))
        }
    }

    pub(crate) async fn get_active_cron_reminders(
        &self,
    ) -> Result<Vec<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::Paused.eq(false))
            .filter(cron_reminder::Column::Time.lt(now_time()))
            .all(&self.pool)
            .await?)
    }

    pub(crate) async fn get_pending_chat_cron_reminders(
        &self,
        chat_id: i64,
    ) -> Result<Vec<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::ChatId.eq(chat_id))
            .all(&self.pool)
            .await?)
    }

    pub(crate) async fn get_sorted_reminders(
        &self,
        chat_id: i64,
    ) -> Result<Vec<Box<dyn generic_reminder::GenericReminder>>, Error> {
        let reminders = self
            .get_pending_chat_reminders(chat_id)
            .await?
            .into_iter()
            .map(|x| -> Box<dyn generic_reminder::GenericReminder> {
                Box::<reminder::ActiveModel>::new(x.into())
            });
        let cron_reminders = self
            .get_pending_chat_cron_reminders(chat_id)
            .await?
            .into_iter()
            .map(|x| -> Box<dyn generic_reminder::GenericReminder> {
                Box::<cron_reminder::ActiveModel>::new(x.into())
            });

        let mut all_reminders = vec![];
        all_reminders.extend(reminders);
        all_reminders.extend(cron_reminders);
        all_reminders.sort_unstable();
        Ok(all_reminders)
    }

    pub(crate) async fn get_reminder_by_msg_id(
        &self,
        msg_id: i32,
    ) -> Result<Option<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::MsgId.eq(msg_id))
            .one(&self.pool)
            .await?)
    }

    pub(crate) async fn get_cron_reminder_by_msg_id(
        &self,
        msg_id: i32,
    ) -> Result<Option<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::MsgId.eq(msg_id))
            .one(&self.pool)
            .await?)
    }

    pub(crate) async fn get_reminder_by_reply_id(
        &self,
        reply_id: i32,
    ) -> Result<Option<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::ReplyId.eq(reply_id))
            .one(&self.pool)
            .await?)
    }

    pub(crate) async fn get_cron_reminder_by_reply_id(
        &self,
        reply_id: i32,
    ) -> Result<Option<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::ReplyId.eq(reply_id))
            .one(&self.pool)
            .await?)
    }

    pub(crate) async fn set_reminder_reply_id(
        &self,
        mut rem: reminder::ActiveModel,
        reply_id: i32,
    ) -> Result<(), Error> {
        rem.reply_id = Set(Some(reply_id));
        rem.update(&self.pool).await?;
        Ok(())
    }

    pub(crate) async fn set_cron_reminder_reply_id(
        &self,
        mut cron_rem: cron_reminder::ActiveModel,
        reply_id: i32,
    ) -> Result<(), Error> {
        cron_rem.reply_id = Set(Some(reply_id));
        cron_rem.update(&self.pool).await?;
        Ok(())
    }

    pub(crate) async fn update_reminder(
        &self,
        rem: reminder::Model,
    ) -> Result<(), Error> {
        defer!(self.notify.notify_one());
        let desc = rem.desc.clone();
        let mut rem_act = Into::<reminder::ActiveModel>::into(rem);
        rem_act.desc = Set(desc);
        rem_act.update(&self.pool).await?;
        Ok(())
    }

    pub(crate) fn listen(&self) -> Notified<'_> {
        self.notify.notified()
    }
}
