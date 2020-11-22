use std::fmt;

#[derive(Debug)]
pub enum Error {
    Database(rusqlite::Error),
    Parse(String),
    CronParse(cron_parser::ParseError),
    CronFewFields,
    CronPanic,
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
            Self::CronPanic => write!(f, "Cron parse error"),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::Database(err)
    }
}

impl From<cron_parser::ParseError> for Error {
    fn from(err: cron_parser::ParseError) -> Self {
        Self::CronParse(err)
    }
}
