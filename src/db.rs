use chrono::{DateTime, Utc};
use directories::BaseDirs;
use rusqlite::{params, Connection, Result, NO_PARAMS};
use teloxide::types::Message;

#[derive(Debug)]
pub struct Reminder {
    pub id: u32,
    pub user_id: i64,
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

pub fn mark_reminder_as_sent(rem: &Reminder) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute("update reminder set sent=true where id=?1", params![rem.id])?;
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

pub fn get_pending_user_reminders(msg: &Message) -> Result<Vec<Reminder>> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select id, user_id, time, desc, sent
            from reminder
            where user_id=?1 and datetime(time) >= datetime('now')",
    )?;
    let rows = stmt.query_map(params![msg.chat_id()], |row| {
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
