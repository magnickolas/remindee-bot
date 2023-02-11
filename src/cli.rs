use std::{ffi::OsString, path::PathBuf};

use clap::Parser;
use directories::BaseDirs;

lazy_static::lazy_static! {
    pub static ref CLI: Cli = parse_args();
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(
        short,
        long,
        env = "REMINDEE_DB",
        value_name = "FILE",
        help = "Path to the SQLite database file (tries to create if not exists)",
        default_value = get_default_database_file()
    )]
    pub database: PathBuf,
    #[arg(short, long, value_name = "BOT TOKEN", env = "BOT_TOKEN")]
    pub token: String,
}

pub fn parse_args() -> Cli {
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
