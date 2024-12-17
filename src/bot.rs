use crate::cli::CLI;
#[cfg(not(test))]
use crate::db::Database;
#[cfg(test)]
use crate::db::MockDatabase as Database;
use crate::entity::{cron_reminder, reminder};
use crate::err::Error;
use crate::format;
use crate::handlers::{get_handler, Command, State};
use crate::parsers::now_time;
use crate::serializers::Pattern;
use crate::tg::send_message;
use crate::tz::get_user_timezone;
use async_std::task;
use chrono::Utc;
use chrono_tz::Tz;
use cron_parser::parse as parse_cron;
use sea_orm::{ActiveValue::NotSet, IntoActiveModel};
use serde_json::{from_str, to_string};
use std::cmp::max;
use std::sync::Arc;
use std::time::Duration;
use teloxide::dispatching::dialogue::serializer::Json;
use teloxide::dispatching::dialogue::{ErasedStorage, SqliteStorage, Storage};
use teloxide::{prelude::*, utils::command::BotCommands};

async fn send_reminder(
    reminder: &reminder::Model,
    user_timezone: Tz,
    bot: &Bot,
) -> Result<(), Error> {
    let text = format::format_reminder(
        &reminder.clone().into_active_model(),
        user_timezone,
    );
    send_message(&text, bot, ChatId(reminder.chat_id))
        .await
        .map(|_| ())
        .map_err(From::from)
}

async fn send_cron_reminder(
    reminder: &cron_reminder::Model,
    next_reminder: Option<&cron_reminder::Model>,
    user_timezone: Tz,
    bot: &Bot,
) -> Result<(), Error> {
    let text =
        format::format_cron_reminder(reminder, next_reminder, user_timezone);
    send_message(&text, bot, ChatId(reminder.chat_id))
        .await
        .map(|_| ())
        .map_err(From::from)
}

/// Periodically (every second) check for new reminders.
/// Send and delete one-time reminders if time has come.
/// Send cron reminders if time has come and update next reminder time.
async fn poll_reminders(db: Arc<Database>, bot: Bot) {
    loop {
        let reminders = db
            .get_active_reminders()
            .await
            .expect("Failed to get reminders from database");
        for reminder in reminders {
            if let Some(user_id) = reminder.user_id.map(|x| UserId(x as u64)) {
                if let Ok(Some(user_timezone)) =
                    get_user_timezone(&db, user_id).await
                {
                    let mut next_reminder = None;
                    if let Some(ref serialized) = reminder.pattern {
                        let mut pattern: Pattern =
                            from_str(serialized).unwrap();
                        let lower_bound = max(reminder.time, now_time());
                        if let Some(next_time) = pattern.next(lower_bound) {
                            next_reminder = Some(reminder::Model {
                                time: next_time,
                                pattern: to_string(&pattern).ok(),
                                ..reminder.clone()
                            });
                        }
                    }
                    if send_reminder(&reminder, user_timezone, &bot)
                        .await
                        .is_ok()
                    {
                        db.delete_reminder(reminder.id).await.unwrap_or_else(
                            |err| {
                                log::error!("{}", err);
                            },
                        );
                        if let Some(next_reminder) = next_reminder {
                            let mut next_reminder: reminder::ActiveModel =
                                next_reminder.into();
                            next_reminder.id = NotSet;
                            db.insert_reminder(next_reminder)
                                .await
                                .map(|_| ())
                                .unwrap_or_else(|err| {
                                    log::error!("{}", err);
                                });
                        }
                    }
                }
            }
        }
        let cron_reminders = db
            .get_active_cron_reminders()
            .await
            .expect("Failed to get cron reminders from database");
        for cron_reminder in cron_reminders {
            if let Some(user_id) =
                cron_reminder.user_id.map(|x| UserId(x as u64))
            {
                if let Ok(Some(user_timezone)) =
                    get_user_timezone(&db, user_id).await
                {
                    let new_time = parse_cron(
                        &cron_reminder.cron_expr,
                        &Utc::now().with_timezone(&user_timezone),
                    )
                    .map(|user_time| user_time.with_timezone(&Utc));
                    let new_cron_reminder = match new_time {
                        Ok(new_time) => Some(cron_reminder::Model {
                            time: new_time.naive_utc(),
                            ..cron_reminder.clone()
                        }),
                        Err(err) => {
                            log::error!("{}", err);
                            None
                        }
                    };
                    match send_cron_reminder(
                        &cron_reminder,
                        new_cron_reminder.as_ref(),
                        user_timezone,
                        &bot,
                    )
                    .await
                    {
                        Ok(()) => {
                            db.delete_cron_reminder(cron_reminder.id)
                                .await
                                .unwrap_or_else(|err| {
                                    log::error!("{}", err);
                                });
                            if let Some(new_cron_reminder) = new_cron_reminder {
                                let mut new_cron_reminder: cron_reminder::ActiveModel = new_cron_reminder.into();
                                new_cron_reminder.id = NotSet;
                                db.insert_cron_reminder(new_cron_reminder)
                                    .await
                                    .map(|_| ())
                                    .unwrap_or_else(|err| {
                                        log::error!("{}", err);
                                    });
                            }
                        }
                        Err(err) => {
                            log::error!("{}", err);
                        }
                    }
                }
            }
        }
        task::sleep(Duration::from_secs(1)).await;
    }
}

async fn init_database() -> Database {
    Database::new_with_path(&CLI.database)
        .await
        .unwrap_or_else(|err| {
            panic!("Failed to connect to database {:?}: {}", CLI.database, err)
        })
}

async fn init_dialogue_storage() -> Arc<ErasedStorage<State>> {
    SqliteStorage::open(CLI.database.to_str().unwrap(), Json)
        .await
        .unwrap_or_else(|err| {
            panic!("Failed to connect to database {:?}: {}", CLI.database, err)
        })
        .erase()
}

pub async fn run() {
    pretty_env_logger::init();
    log::info!("Starting remindee-bot!");

    let db = Arc::new(init_database().await);

    db.apply_migrations()
        .await
        .expect("Failed to apply migrations");

    let bot = Bot::new(&CLI.token);

    bot.set_my_commands(Command::bot_commands())
        .await
        .expect("Failed to set bot commands");

    let db_clone = db.clone();

    tokio::spawn(poll_reminders(db_clone, bot.clone()));

    let storage = init_dialogue_storage().await;

    let handler = get_handler();

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![storage, db])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use crate::{
        db::MockDatabase, entity::reminder, generic_reminder::GenericReminder,
        handlers::get_handler, tg::TgResponse,
    };
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use chrono_tz::Tz;
    use dptree::deps;
    use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};
    use teloxide_tests::{IntoUpdate, MockBot, MockMessageText};

    use super::State;

    fn basic_mock_reminder() -> reminder::ActiveModel {
        reminder::ActiveModel {
            id: sea_orm::ActiveValue::Set(1),
            chat_id: sea_orm::ActiveValue::Set(1),
            time: sea_orm::ActiveValue::Set(NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(0, 1, 2).unwrap(),
            )),
            desc: sea_orm::ActiveValue::Set("".to_owned()),
            edit: sea_orm::ActiveValue::Set(false),
            edit_mode: sea_orm::ActiveValue::Set(
                crate::entity::common::EditMode::None,
            ),
            user_id: sea_orm::ActiveValue::Set(None),
            paused: sea_orm::ActiveValue::Set(false),
            pattern: sea_orm::ActiveValue::Set(None),
            msg_id: sea_orm::ActiveValue::Set(None),
            reply_id: sea_orm::ActiveValue::Set(None),
        }
    }

    fn mock_timezone_name() -> String {
        "Europe/Amsterdam".to_owned()
    }

    fn mock_timezone() -> Tz {
        mock_timezone_name().parse::<Tz>().unwrap()
    }

    fn mock_storage() -> Arc<InMemStorage<State>> {
        InMemStorage::<State>::new()
    }

    fn mock_bot<T>(db: MockDatabase, update: T) -> MockBot
    where
        T: IntoUpdate,
    {
        let bot = MockBot::new(update, get_handler());
        bot.dependencies(deps![mock_storage(), Arc::new(db)]);
        bot
    }

    #[tokio::test]
    async fn test_start() {
        let message = MockMessageText::new().text("/start");
        let db = MockDatabase::new();
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&TgResponse::Hello.to_string())
            .await;
    }

    #[tokio::test]
    async fn test_list_no_timezone() {
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name().returning(|_| Ok(None));
        let message = MockMessageText::new().text("/list");
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::NoChosenTimezone.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_list_no_reminders() {
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_sorted_all_reminders()
            .returning(|_| Ok(vec![]));
        let message = MockMessageText::new().text("/list");
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::RemindersListHeader.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_list_one_reminder() {
        let mut db = MockDatabase::new();
        let tz = mock_timezone();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_sorted_all_reminders()
            .returning(move |_| Ok(vec![Box::new(rem_clone.clone())]));
        let message = MockMessageText::new().text("/list");
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&format!(
            "{}\n{}",
            TgResponse::RemindersListHeader,
            rem.to_string(tz)
        ))
        .await;
    }
}
