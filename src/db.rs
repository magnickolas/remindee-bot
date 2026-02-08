use std::path::Path;

use crate::cli::CLI;
use crate::entity::{
    reminder, reminder_message, reminder_occurrence, user_language,
    user_timezone,
};
use crate::generic_reminder;
use crate::migration::{DbErr, Migrator, MigratorTrait};
use crate::parsers::now_time;
use chrono::NaiveDateTime;
#[cfg(test)]
use mockall::automock;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Condition,
    ConnectOptions, Database as SeaOrmDatabase, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, Set,
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
                write!(f, "Database error: {err}")
            }
            Self::File(ref err) => write!(f, "File error: {err}"),
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

fn checked_add_seconds(
    dt: NaiveDateTime,
    seconds: i64,
) -> Result<NaiveDateTime, Error> {
    let delta = chrono::TimeDelta::try_seconds(seconds).ok_or_else(|| {
        Error::Database(DbErr::Custom(format!(
            "Invalid seconds value: {seconds}",
        )))
    })?;
    dt.checked_add_signed(delta).ok_or_else(|| {
        Error::Database(DbErr::Custom(format!(
            "Datetime overflow while adding {seconds} seconds",
        )))
    })
}

pub(crate) struct InsertReminderOccurrence {
    pub(crate) rec_id: String,
    pub(crate) chat_id: i64,
    pub(crate) user_id: Option<i64>,
    pub(crate) due_at: NaiveDateTime,
    pub(crate) nag_interval_sec: i64,
    pub(crate) stop_at: Option<NaiveDateTime>,
    pub(crate) desc_snapshot: String,
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

    async fn next_occurrence_nag_time(
        &self,
    ) -> Result<Option<NaiveDateTime>, Error> {
        Ok(reminder_occurrence::Entity::find()
            .filter(reminder_occurrence::Column::DoneAt.is_null())
            .order_by_asc(reminder_occurrence::Column::NextNagAt)
            .one(&self.pool)
            .await?
            .map(|occ| occ.next_nag_at))
    }

    pub(crate) async fn get_next_reminder_time(
        &self,
    ) -> Result<Option<NaiveDateTime>, Error> {
        let rem_time = self.next_reminder_time().await?;
        let nag_time = self.next_occurrence_nag_time().await?;
        Ok(match (rem_time, nag_time) {
            (Some(rem), Some(nag)) => Some(rem.min(nag)),
            (Some(rem), None) => Some(rem),
            (None, Some(nag)) => Some(nag),
            (None, None) => None,
        })
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

    pub(crate) async fn get_user_language_name(
        &self,
        user_id: i64,
    ) -> Result<Option<String>, Error> {
        Ok(user_language::Entity::find_by_id(user_id)
            .one(&self.pool)
            .await?
            .map(|x| x.language))
    }

    async fn insert_user_language_name(
        &self,
        user_id: i64,
        language: &str,
    ) -> Result<(), Error> {
        defer!(self.notify.notify_one());
        user_language::Entity::insert(user_language::ActiveModel {
            user_id: Set(user_id),
            language: Set(language.to_string()),
        })
        .exec(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn insert_or_update_user_language(
        &self,
        user_id: i64,
        language: &str,
    ) -> Result<(), Error> {
        if let Some(mut lang_act) = user_language::Entity::find_by_id(user_id)
            .one(&self.pool)
            .await?
            .map(Into::<user_language::ActiveModel>::into)
        {
            lang_act.language = Set(language.to_string());
            lang_act.update(&self.pool).await?;
        } else {
            self.insert_user_language_name(user_id, language).await?;
        }
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

    pub(crate) async fn get_sorted_reminders(
        &self,
        chat_id: i64,
    ) -> Result<Vec<Box<dyn generic_reminder::GenericReminder>>, Error> {
        let reminders = self.get_pending_chat_reminders(chat_id).await?;

        let mut all_reminders: Vec<_> = reminders
            .into_iter()
            .map(|m| Box::new(reminder::ActiveModel::from(m)) as _)
            .collect();
        all_reminders.sort_unstable();
        Ok(all_reminders)
    }

    pub(crate) async fn get_reminder_by_message(
        &self,
        chat_id: i64,
        msg_id: i32,
    ) -> Result<Option<reminder::Model>, Error> {
        let rec_id = reminder_message::Entity::find()
            .filter(reminder_message::Column::ChatId.eq(chat_id))
            .filter(reminder_message::Column::MsgId.eq(msg_id))
            .one(&self.pool)
            .await?
            .map(|link| link.rec_id);

        match rec_id {
            Some(rec_id) => Ok(reminder::Entity::find()
                .filter(reminder::Column::RecId.eq(rec_id))
                .one(&self.pool)
                .await?),
            None => Ok(None),
        }
    }

    pub(crate) async fn get_latest_reminder_message_id(
        &self,
        chat_id: i64,
        rec_id: &str,
    ) -> Result<Option<i32>, Error> {
        Ok(reminder_message::Entity::find()
            .filter(reminder_message::Column::ChatId.eq(chat_id))
            .filter(reminder_message::Column::RecId.eq(rec_id))
            .filter(reminder_message::Column::IsDelivery.eq(true))
            .order_by_desc(reminder_message::Column::Id)
            .one(&self.pool)
            .await?
            .map(|link| link.msg_id))
    }

    pub(crate) async fn insert_reminder_message(
        &self,
        rec_id: &str,
        chat_id: i64,
        msg_id: i32,
        is_delivery: bool,
    ) -> Result<(), Error> {
        reminder_message::ActiveModel {
            id: NotSet,
            rec_id: Set(rec_id.to_owned()),
            chat_id: Set(chat_id),
            msg_id: Set(msg_id),
            is_delivery: Set(is_delivery),
        }
        .insert(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn insert_reminder_occurrence(
        &self,
        args: InsertReminderOccurrence,
    ) -> Result<reminder_occurrence::Model, Error> {
        let InsertReminderOccurrence {
            rec_id,
            chat_id,
            user_id,
            due_at,
            nag_interval_sec,
            stop_at,
            desc_snapshot,
        } = args;
        defer!(self.notify.notify_one());
        let first_nag_at = checked_add_seconds(now_time(), nag_interval_sec)?;
        Ok(reminder_occurrence::ActiveModel {
            id: NotSet,
            rec_id: Set(rec_id),
            chat_id: Set(chat_id),
            user_id: Set(user_id),
            due_at: Set(due_at),
            next_nag_at: Set(first_nag_at),
            nag_interval_sec: Set(nag_interval_sec),
            stop_at: Set(stop_at),
            done_at: Set(None),
            desc_snapshot: Set(desc_snapshot),
        }
        .insert(&self.pool)
        .await?)
    }

    pub(crate) async fn get_due_reminder_occurrences(
        &self,
    ) -> Result<Vec<reminder_occurrence::Model>, Error> {
        let now = now_time();
        Ok(reminder_occurrence::Entity::find()
            .filter(reminder_occurrence::Column::DoneAt.is_null())
            .filter(reminder_occurrence::Column::NextNagAt.lte(now))
            .filter(
                Condition::any()
                    .add(reminder_occurrence::Column::StopAt.is_null())
                    .add(reminder_occurrence::Column::StopAt.gt(now)),
            )
            .all(&self.pool)
            .await?)
    }

    pub(crate) async fn close_elapsed_occurrences(&self) -> Result<(), Error> {
        let now = now_time();
        reminder_occurrence::Entity::update_many()
            .set(reminder_occurrence::ActiveModel {
                done_at: Set(Some(now)),
                ..Default::default()
            })
            .filter(reminder_occurrence::Column::DoneAt.is_null())
            .filter(reminder_occurrence::Column::StopAt.lte(now))
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    pub(crate) async fn close_open_occurrences(
        &self,
        rec_id: &str,
    ) -> Result<u64, Error> {
        defer!(self.notify.notify_one());
        let now = now_time();
        let result = reminder_occurrence::Entity::update_many()
            .set(reminder_occurrence::ActiveModel {
                done_at: Set(Some(now)),
                ..Default::default()
            })
            .filter(reminder_occurrence::Column::RecId.eq(rec_id))
            .filter(reminder_occurrence::Column::DoneAt.is_null())
            .exec(&self.pool)
            .await?;
        Ok(result.rows_affected)
    }

    pub(crate) async fn is_occurrence_open(
        &self,
        occ_id: i64,
    ) -> Result<bool, Error> {
        Ok(reminder_occurrence::Entity::find_by_id(occ_id)
            .filter(reminder_occurrence::Column::DoneAt.is_null())
            .one(&self.pool)
            .await?
            .is_some())
    }

    pub(crate) async fn bump_occurrence_nag(
        &self,
        occ_id: i64,
    ) -> Result<(), Error> {
        defer!(self.notify.notify_one());
        let occ = reminder_occurrence::Entity::find_by_id(occ_id)
            .one(&self.pool)
            .await?
            .ok_or_else(|| {
                Error::Database(DbErr::RecordNotFound(occ_id.to_string()))
            })?;
        let mut occ_act: reminder_occurrence::ActiveModel = occ.into();
        let nag_interval_sec = occ_act.nag_interval_sec.clone().unwrap();
        let base = now_time().max(occ_act.next_nag_at.clone().unwrap());
        occ_act.next_nag_at = Set(checked_add_seconds(base, nag_interval_sec)?);
        occ_act.update(&self.pool).await?;
        Ok(())
    }

    pub(crate) async fn delete_occurrence(
        &self,
        occ_id: i64,
    ) -> Result<(), Error> {
        defer!(self.notify.notify_one());
        reminder_occurrence::ActiveModel {
            id: Set(occ_id),
            ..Default::default()
        }
        .delete(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn complete_occurrence(
        &self,
        occ_id: i64,
        chat_id: i64,
    ) -> Result<bool, Error> {
        defer!(self.notify.notify_one());
        let now = now_time();
        let result = reminder_occurrence::Entity::update_many()
            .set(reminder_occurrence::ActiveModel {
                done_at: Set(Some(now)),
                ..Default::default()
            })
            .filter(reminder_occurrence::Column::Id.eq(occ_id))
            .filter(reminder_occurrence::Column::ChatId.eq(chat_id))
            .filter(reminder_occurrence::Column::DoneAt.is_null())
            .exec(&self.pool)
            .await?
            .rows_affected;
        Ok(result > 0)
    }

    pub(crate) async fn delete_reminder_messages(
        &self,
        rec_id: &str,
    ) -> Result<(), Error> {
        reminder_message::Entity::delete_many()
            .filter(reminder_message::Column::RecId.eq(rec_id))
            .exec(&self.pool)
            .await?;
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::parsers::test::TEST_TIMESTAMP;
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use sea_orm::{ActiveValue::NotSet, IntoActiveModel};
    use serial_test::serial;

    async fn new_db_in_memory() -> Result<Database, Error> {
        let mut opts = ConnectOptions::new("sqlite::memory:");
        opts.max_connections(1);
        let pool = SeaOrmDatabase::connect(opts).await?;
        Ok(Database {
            pool,
            notify: Notify::new(),
        })
    }

    fn ts(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> NaiveDateTime {
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            NaiveTime::from_hms_opt(h, min, s).unwrap(),
        )
    }

    fn basic_mock_reminder() -> reminder::Model {
        reminder::Model {
            id: 1,
            chat_id: 1,
            time: NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2024, 2, 2).unwrap(),
                NaiveTime::from_hms_opt(1, 2, 3).unwrap(),
            ),
            desc: "".to_owned(),
            user_id: None,
            paused: false,
            nag_interval_sec: None,
            pattern: None,
            rec_id: "1:1".to_owned(),
        }
    }

    fn basic_mock_new_reminder_act() -> reminder::ActiveModel {
        let mut rem_act = basic_mock_reminder().into_active_model();
        rem_act.id = NotSet;
        rem_act
    }

    #[tokio::test]
    async fn test_get_sorted_reminders() {
        let db = new_db_in_memory().await.unwrap();
        db.apply_migrations().await.unwrap();
        let mut rem1_act = basic_mock_new_reminder_act();
        let mut rem2_act = basic_mock_new_reminder_act();
        let mut rem3_act = basic_mock_new_reminder_act();
        rem1_act.time = Set(ts(2024, 1, 1, 1, 0, 0));
        rem2_act.time = Set(ts(2024, 1, 1, 2, 0, 0));
        rem3_act.time = Set(ts(2024, 1, 1, 3, 0, 0));

        db.insert_reminder(rem1_act.clone()).await.unwrap();
        db.insert_reminder(rem3_act.clone()).await.unwrap();
        db.insert_reminder(rem2_act.clone()).await.unwrap();

        let result = db.get_sorted_reminders(1).await.unwrap();
        let times: Vec<_> = result.iter().map(|r| r.get_time()).collect();
        assert_eq!(
            times,
            vec![
                rem1_act.time.unwrap(),
                rem2_act.time.unwrap(),
                rem3_act.time.unwrap(),
            ]
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_occurrence_first_nag_is_from_now_time() {
        let db = new_db_in_memory().await.unwrap();
        db.apply_migrations().await.unwrap();

        let due_at = ts(2024, 1, 1, 10, 0, 0);
        let now = ts(2024, 1, 1, 10, 47, 0);
        *TEST_TIMESTAMP.write().unwrap() = now.and_utc().timestamp();

        let occ = db
            .insert_reminder_occurrence(InsertReminderOccurrence {
                rec_id: "1:1".to_owned(),
                chat_id: 1,
                user_id: Some(1),
                due_at,
                nag_interval_sec: 10 * 60,
                stop_at: None,
                desc_snapshot: "take a pill".to_owned(),
            })
            .await
            .unwrap();

        assert_eq!(occ.due_at, due_at);
        assert_eq!(occ.next_nag_at, ts(2024, 1, 1, 10, 57, 0));
    }

    #[tokio::test]
    async fn test_get_latest_reminder_message_id_uses_delivery_only() {
        let db = new_db_in_memory().await.unwrap();
        db.apply_migrations().await.unwrap();

        db.insert_reminder_message("1:1", 1, 100, false)
            .await
            .unwrap();
        db.insert_reminder_message("1:1", 1, 101, true)
            .await
            .unwrap();
        db.insert_reminder_message("1:1", 1, 102, false)
            .await
            .unwrap();

        assert_eq!(
            db.get_latest_reminder_message_id(1, "1:1").await.unwrap(),
            Some(101),
        );
    }

    #[tokio::test]
    async fn test_insert_occurrence_rejects_datetime_overflow() {
        let db = new_db_in_memory().await.unwrap();
        db.apply_migrations().await.unwrap();

        let result = db
            .insert_reminder_occurrence(InsertReminderOccurrence {
                rec_id: "1:1".to_owned(),
                chat_id: 1,
                user_id: Some(1),
                due_at: ts(2024, 1, 1, 10, 0, 0),
                nag_interval_sec: i64::MAX,
                stop_at: None,
                desc_snapshot: "take a pill".to_owned(),
            })
            .await;

        assert!(result.is_err());
    }
}
