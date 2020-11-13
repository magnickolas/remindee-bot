use chrono::{DateTime, Utc};
use directories::BaseDirs;
use rusqlite::{params, Connection, Result, NO_PARAMS};

#[derive(Clone, Debug)]
pub struct Reminder {
    pub id: u32,
    pub user_id: i64,
    pub time: DateTime<Utc>,
    pub desc: String,
    pub sent: bool,
}

#[derive(Clone, Debug)]
pub struct CronReminder {
    pub id: u32,
    pub user_id: i64,
    pub cron_expr: String,
    pub time: DateTime<Utc>,
    pub desc: String,
    pub sent: bool,
}

pub fn get_db_connection() -> Result<Connection> {
    let base_dirs = BaseDirs::new().unwrap();
    Connection::open(base_dirs.data_dir().join("remindee_db.sqlite"))
}

pub fn create_reminder_table() -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "create table if not exists reminder (
             id         integer primary key,
             user_id    integer not null,
             time       timestamp not null,
             desc       text not null,
             sent       boolean not null
        )",
        params![],
    )?;
    Ok(())
}

pub fn insert_reminder(rem: &Reminder) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "insert into reminder (user_id, time, desc, sent) values (?1, ?2, ?3, ?4)",
        params![rem.user_id, rem.time, rem.desc, rem.sent],
    )?;
    Ok(())
}

pub fn mark_reminder_as_sent(id: u32) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute("update reminder set sent=true where id=?1", params![id])?;
    Ok(())
}

pub fn get_active_reminders() -> Result<Vec<Reminder>> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select id, user_id, time, desc, sent
        from reminder
        where sent=false and datetime(time) < datetime('now')",
    )?;
    let rows = stmt.query_map(NO_PARAMS, |row| {
        Ok(Reminder {
            id: row.get(0)?,
            user_id: row.get(1)?,
            time: row.get(2)?,
            desc: row.get(3)?,
            sent: row.get(4)?,
        })
    })?;
    let mut reminders = Vec::new();
    for reminder in rows {
        reminders.push(reminder?);
    }
    Ok(reminders)
}

pub fn get_pending_user_reminders(user_id: i64) -> Result<Vec<Reminder>> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select id, user_id, time, desc, sent
        from reminder
        where user_id=?1 and datetime(time) >= datetime('now') and sent=false",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(Reminder {
            id: row.get(0)?,
            user_id: row.get(1)?,
            time: row.get(2)?,
            desc: row.get(3)?,
            sent: row.get(4)?,
        })
    })?;
    let mut reminders = Vec::new();
    for reminder in rows {
        reminders.push(reminder?);
    }
    Ok(reminders)
}

pub fn create_user_timezone_table() -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "create table if not exists user_timezone (
             user_id    integer primary key,
             timezone   text not null
        )",
        params![],
    )?;
    Ok(())
}

pub fn get_user_timezone_name(user_id: i64) -> Result<String> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select timezone
        from user_timezone
        where user_id=?1",
    )?;
    let row = stmt.query_row(params![user_id], |row| Ok(row.get("timezone")?))?;
    Ok(row)
}

pub fn set_user_timezone_name(user_id: i64, timezone: &str) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "insert or replace into user_timezone (user_id, timezone)
        values (?1, ?2)",
        params![user_id, timezone],
    )?;
    Ok(())
}

pub fn create_cron_reminder_table() -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "create table if not exists cron_reminder (
             id         integer primary key,
             user_id    integer not null,
             cron_expr  text not null,
             time       timestamp not null,
             desc       text not null,
             sent       boolean not null
        )",
        params![],
    )?;
    Ok(())
}

pub fn insert_cron_reminder(rem: &CronReminder) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "insert into cron_reminder (user_id, cron_expr, time, desc, sent) values (?1, ?2, ?3, ?4, ?5)",
        params![rem.user_id, rem.cron_expr, rem.time, rem.desc, rem.sent],
    )?;
    Ok(())
}

pub fn mark_cron_reminder_as_sent(id: u32) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "update cron_reminder set sent=true where id=?1",
        params![id],
    )?;
    Ok(())
}

pub fn get_active_cron_reminders() -> Result<Vec<CronReminder>> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select id, user_id, cron_expr, time, desc, sent
        from cron_reminder
        where sent=false and datetime(time) < datetime('now')",
    )?;
    let rows = stmt.query_map(NO_PARAMS, |row| {
        Ok(CronReminder {
            id: row.get(0)?,
            user_id: row.get(1)?,
            cron_expr: row.get(2)?,
            time: row.get(3)?,
            desc: row.get(4)?,
            sent: row.get(5)?,
        })
    })?;
    let mut cron_reminders = Vec::new();
    for cron_reminder in rows {
        cron_reminders.push(cron_reminder?);
    }
    Ok(cron_reminders)
}

pub fn get_pending_user_cron_reminders(user_id: i64) -> Result<Vec<CronReminder>> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select id, user_id, cron_expr, time, desc, sent
        from cron_reminder
        where user_id=?1 and datetime(time) >= datetime('now') and sent=false",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(CronReminder {
            id: row.get(0)?,
            user_id: row.get(1)?,
            cron_expr: row.get(2)?,
            time: row.get(3)?,
            desc: row.get(4)?,
            sent: row.get(5)?,
        })
    })?;
    let mut cron_reminders = Vec::new();
    for cron_reminder in rows {
        cron_reminders.push(cron_reminder?);
    }
    Ok(cron_reminders)
}
