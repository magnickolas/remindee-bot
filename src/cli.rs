use std::{ffi::OsString, path::PathBuf};

use clap::Parser;
use directories::BaseDirs;

lazy_static::lazy_static! {
    pub(crate) static ref CLI: Cli = parse_args();
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[arg(
        short,
        long,
        env = "REMINDEE_DB",
        value_name = "FILE",
        help = "Path to the SQLite database file (tries to create if not exists)",
        default_value = get_default_database_file()
    )]
    pub(crate) database: PathBuf,
    #[arg(short, long, value_name = "BOT TOKEN", env = "BOT_TOKEN")]
    pub(crate) token: String,
    #[arg(
        short,
        long,
        env = "SQLITE_MAX_CONNECTIONS",
        value_name = "NUMBER",
        help = "Maximum number of connections to the SQLite database",
        default_value = "1"
    )]
    pub(crate) sqlite_max_connections: u32,
}

pub(crate) fn parse_args() -> Cli {
    Cli::parse()
}

fn get_default_database_file() -> OsString {
    let db_name = "remindee_db.sqlite";
    if cfg!(target_os = "android") {
        db_name.into()
    } else {
        match BaseDirs::new() {
            Some(base_dirs) => base_dirs.data_dir().join(db_name).into(),
            None => db_name.into(),
        }
    }
}
