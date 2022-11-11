use std::fs::OpenOptions;

use chrono::Utc;
use directories::BaseDirs;
use entity::{cron_reminder, reminder, user_timezone};
use migration::sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use migration::sea_orm::{Database as SeaOrmDatabase, Set};
use migration::{Migrator, MigratorTrait};

#[derive(Debug)]
pub enum Error {
    Database(migration::DbErr),
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

impl From<migration::DbErr> for Error {
    fn from(err: migration::DbErr) -> Self {
        Self::Database(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::File(err)
    }
}

async fn get_db_pool() -> Result<DatabaseConnection, Error> {
    let base_dirs = BaseDirs::new();
    let db_name = "remindee_db.sqlite";
    let db_path = if std::env::consts::OS != "android" {
        base_dirs
            .map(|x| x.data_dir().join(db_name))
            .unwrap_or_else(|| db_name.into())
    } else {
        db_name.into()
    };
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&db_path)?;
    let db_str = format!("sqlite:{}", db_path.display());
    let pool = SeaOrmDatabase::connect(&db_str).await?;
    Ok(pool)
}

#[derive(Clone)]
pub struct Database {
    pool: DatabaseConnection,
}

impl Database {
    pub async fn new() -> Result<Self, Error> {
        get_db_pool().await.map(|pool| Self { pool })
    }

    pub async fn apply_migrations(&self) -> Result<(), Error> {
        Ok(Migrator::up(&self.pool, None).await?)
    }

    pub async fn insert_reminder(
        &self,
        rem: reminder::ActiveModel,
    ) -> Result<(), Error> {
        reminder::Entity::insert(rem).exec(&self.pool).await?;
        Ok(())
    }

    pub async fn mark_reminder_as_sent(&self, id: i64) -> Result<(), Error> {
        let rem: Option<reminder::Model> =
            reminder::Entity::find_by_id(id).one(&self.pool).await?;
        if let Some(rem) = rem {
            let mut rem: reminder::ActiveModel = rem.into();
            rem.sent = Set(true);
            rem.update(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn mark_reminder_as_edit(&self, id: i64) -> Result<(), Error> {
        let rem: Option<reminder::Model> =
            reminder::Entity::find_by_id(id).one(&self.pool).await?;
        if let Some(rem) = rem {
            let mut rem: reminder::ActiveModel = rem.into();
            rem.edit = Set(true);
            rem.update(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn reset_reminders_edit(
        &self,
        user_id: i64,
    ) -> Result<(), Error> {
        reminder::Entity::update_many()
            .filter(reminder::Column::UserId.eq(user_id))
            .set(reminder::ActiveModel {
                edit: Set(false),
                ..Default::default()
            })
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_edit_reminder(
        &self,
        user_id: i64,
    ) -> Result<Option<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::UserId.eq(user_id))
            .filter(reminder::Column::Edit.eq(true))
            .filter(reminder::Column::Sent.eq(false))
            .one(&self.pool)
            .await?)
    }

    pub async fn get_active_reminders(
        &self,
    ) -> Result<Vec<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::Sent.eq(false))
            .filter(reminder::Column::Time.lt(Utc::now().naive_utc()))
            .all(&self.pool)
            .await?)
    }

    pub async fn get_pending_user_reminders(
        &self,
        user_id: i64,
    ) -> Result<Vec<reminder::Model>, Error> {
        Ok(reminder::Entity::find()
            .filter(reminder::Column::UserId.eq(user_id))
            .filter(reminder::Column::Sent.eq(false))
            .all(&self.pool)
            .await?)
    }

    pub async fn get_user_timezone_name(
        &self,
        user_id: i64,
    ) -> Result<Option<String>, Error> {
        Ok(user_timezone::Entity::find_by_id(user_id)
            .one(&self.pool)
            .await?
            .map(|x| x.timezone))
    }

    async fn update_user_timezone_name(
        &self,
        user_id: i64,
        timezone: &str,
    ) -> Result<(), Error> {
        let model = user_timezone::ActiveModel {
            timezone: Set(timezone.to_string()),
            ..Default::default()
        };
        user_timezone::Entity::update(model)
            .filter(user_timezone::Column::UserId.eq(user_id))
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    async fn insert_user_timezone_name(
        &self,
        user_id: i64,
        timezone: &str,
    ) -> Result<(), Error> {
        let model = user_timezone::ActiveModel {
            user_id: Set(user_id),
            timezone: Set(timezone.to_string()),
        };
        user_timezone::Entity::insert(model)
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_user_timezone_name(
        &self,
        user_id: i64,
        timezone: &str,
    ) -> Result<(), Error> {
        match self.get_user_timezone_name(user_id).await? {
            None => self.insert_user_timezone_name(user_id, timezone).await?,
            _ => self.update_user_timezone_name(user_id, timezone).await?,
        }
        Ok(())
    }

    pub async fn insert_cron_reminder(
        &self,
        rem: cron_reminder::ActiveModel,
    ) -> Result<(), Error> {
        rem.save(&self.pool).await?;
        Ok(())
    }

    pub async fn mark_cron_reminder_as_sent(
        &self,
        id: i64,
    ) -> Result<(), Error> {
        let rem: Option<cron_reminder::Model> =
            cron_reminder::Entity::find_by_id(id)
                .one(&self.pool)
                .await?;
        if let Some(rem) = rem {
            let mut rem: cron_reminder::ActiveModel = rem.into();
            rem.sent = Set(true);
            rem.update(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn mark_cron_reminder_as_edit(
        &self,
        id: i64,
    ) -> Result<(), Error> {
        let rem: Option<cron_reminder::Model> =
            cron_reminder::Entity::find_by_id(id)
                .one(&self.pool)
                .await?;
        if let Some(rem) = rem {
            let mut rem: cron_reminder::ActiveModel = rem.into();
            rem.edit = Set(true);
            rem.update(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn reset_cron_reminders_edit(
        &self,
        user_id: i64,
    ) -> Result<(), Error> {
        cron_reminder::Entity::update_many()
            .filter(cron_reminder::Column::UserId.eq(user_id))
            .set(cron_reminder::ActiveModel {
                edit: Set(false),
                ..Default::default()
            })
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_edit_cron_reminder(
        &self,
        user_id: i64,
    ) -> Result<Option<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::UserId.eq(user_id))
            .filter(cron_reminder::Column::Edit.eq(true))
            .filter(cron_reminder::Column::Sent.eq(false))
            .one(&self.pool)
            .await?)
    }

    pub async fn get_active_cron_reminders(
        &self,
    ) -> Result<Vec<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::Sent.eq(false))
            .filter(cron_reminder::Column::Time.lt(Utc::now().naive_utc()))
            .all(&self.pool)
            .await?)
    }

    pub async fn get_pending_user_cron_reminders(
        &self,
        user_id: i64,
    ) -> Result<Vec<cron_reminder::Model>, Error> {
        Ok(cron_reminder::Entity::find()
            .filter(cron_reminder::Column::UserId.eq(user_id))
            .filter(cron_reminder::Column::Sent.eq(false))
            .all(&self.pool)
            .await?)
    }
}
