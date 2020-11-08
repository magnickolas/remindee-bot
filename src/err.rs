use std::fmt;

#[derive(Debug)]
pub enum Error {
    Database(rusqlite::Error),
    Parse(String),
    CronParse(cron_parser::ParseError),
    CronPanic,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Database(ref err) => write!(f, "Database error: {}", err),
            Self::Parse(ref err) => write!(f, "Parse error: {}", err),
            Self::CronParse(ref err) => write!(f, "Cron parse error: {}", err),
            Self::CronPanic => write!(f, "Cron parse error"),
        }
    }
}
