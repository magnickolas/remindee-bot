use crate::db;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Database(db::Error),
    Parse(String),
    CronParse(cron_parser::ParseError),
    CronFewFields,
    TeloxideRequest(teloxide::RequestError),
    UnmatchedQuery(teloxide::types::CallbackQuery),
    NoQueryData(teloxide::types::CallbackQuery),
    NoQueryMessage(teloxide::types::CallbackQuery),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Database(ref err) => write!(f, "Database error: {}", err),
            Self::Parse(ref err) => write!(f, "Parse error: {}", err),
            Self::CronParse(ref err) => write!(f, "Cron parse error: {}", err),
            Self::CronFewFields => {
                write!(f, "Can't parse cron since no enough fields")
            }
            Self::TeloxideRequest(ref err) => {
                write!(f, "Telegram request error: {}", err)
            }
            Self::UnmatchedQuery(ref cb_query) => {
                write!(f, "Could not match callback query: {:?}", cb_query)
            }
            Self::NoQueryData(ref cb_query) => {
                write!(f, "Could not get query data: {:?}", cb_query)
            }
            Self::NoQueryMessage(ref cb_query) => {
                write!(f, "Could not get query message: {:?}", cb_query)
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
