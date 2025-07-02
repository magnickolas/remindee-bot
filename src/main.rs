#[macro_use]
extern crate lazy_static;
extern crate pest;
extern crate pretty_env_logger;
#[macro_use]
extern crate pest_derive;
#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

mod bot;
mod cli;
mod controller;
mod date;
mod db;
mod entity;
mod err;
mod format;
mod generic_reminder;
mod grammar;
mod handlers;
mod lang;
mod migration;
mod parsers;
mod serializers;
mod tg;
mod tz;

#[tokio::main]
async fn main() {
    bot::run().await;
}
