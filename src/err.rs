use crate::db;
use std::fmt;

#[derive(Debug)]
pub(crate) enum Error {
    Database(db::Error),
    Parse(chrono_tz::ParseError),
    CronParse(cron_parser::ParseError),
    TeloxideRequest(teloxide::RequestError),
    UnmatchedQuery(teloxide::types::CallbackQuery),
    ReminderNotFound(i64),
    CronReminderNotFound(i64),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Database(ref err) => write!(f, "Database error: {}", err),
            Self::Parse(ref err) => write!(f, "Parse error: {}", err),
            Self::CronParse(ref err) => write!(f, "Cron parse error: {}", err),
            Self::TeloxideRequest(ref err) => {
                write!(f, "Telegram request error: {}", err)
            }
            Self::UnmatchedQuery(ref cb_query) => {
                write!(f, "Could not match callback query: {:?}", cb_query)
            }
            Self::ReminderNotFound(rem_id) => {
                write!(f, "Reminder with id {} not found", rem_id)
            }
            Self::CronReminderNotFound(cron_rem_id) => {
                write!(f, "Cron reminder with id {} not found", cron_rem_id)
            }
        }
    }
}

impl From<db::Error> for Error {
    fn from(err: db::Error) -> Self {
        Self::Database(err)
    }
}

impl From<cron_parser::ParseError> for Error {
    fn from(err: cron_parser::ParseError) -> Self {
        Self::CronParse(err)
    }
}

impl From<teloxide::RequestError> for Error {
    fn from(err: teloxide::RequestError) -> Self {
        Self::TeloxideRequest(err)
    }
}

impl std::error::Error for Error {}
