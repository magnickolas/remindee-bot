#[macro_use]
extern crate lazy_static;
extern crate pretty_env_logger;

mod bot;
mod cli;
mod controller;
mod date;
mod db;
mod entity;
mod err;
mod generic_reminder;
mod migration;
mod parsers;
mod tg;
mod tz;

#[tokio::main]
async fn main() {
    bot::run().await;
}
