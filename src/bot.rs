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
        tokio::time::sleep(Duration::from_secs(1)).await;
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

pub(crate) async fn run() {
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
mod test {
    use std::sync::Arc;

    use crate::{
        bot::Command, db::MockDatabase, entity::reminder,
        generic_reminder::GenericReminder, handlers::get_handler,
        parsers::test::TEST_TIMESTAMP, tg::TgResponse,
    };
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
    use chrono_tz::Tz;
    use dptree::deps;
    use mockall::predicate::eq;
    use sea_orm::IntoActiveModel;
    use serial_test::serial;
    use teloxide::{
        dispatching::dialogue::InMemStorage,
        prelude::*,
        types::{
            InlineKeyboardButton, InlineKeyboardButtonKind::CallbackData,
            InlineKeyboardMarkup, MediaKind::Text, MediaText, MessageCommon,
            MessageKind,
        },
        utils::command::BotCommands,
    };
    use teloxide_tests::{
        IntoUpdate, MockBot, MockCallbackQuery, MockMessageText,
    };

    use super::State;

    fn basic_mock_reminder() -> reminder::Model {
        reminder::Model {
            id: 1,
            chat_id: 1,
            time: NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(0, 1, 2).unwrap(),
            ),
            desc: "".to_owned(),
            user_id: None,
            paused: false,
            pattern: None,
            msg_id: None,
            reply_id: None,
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
    async fn test_help() {
        let message = MockMessageText::new().text("/help");
        let db = MockDatabase::new();
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&Command::descriptions().to_string())
            .await;
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
    async fn test_start_group() {
        let mut message = MockMessageText::new().text("/start");
        message.chat.id.0 = -1;
        let db = MockDatabase::new();
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&TgResponse::HelloGroup.to_string())
            .await;
    }

    #[tokio::test]
    async fn test_timezone() {
        let message = MockMessageText::new().text("/timezone");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::ChosenTimezone(mock_timezone_name()).to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_set_timezone() {
        let message = MockMessageText::new().text("/settimezone");
        let db = MockDatabase::new();
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::SelectTimezone.to_string(),
        )
        .await;
    }

    macro_rules! resp {
        ($bot:expr, $field:ident, $($subfields:tt)+) => {
            $bot.get_responses().$field.iter().map(|m| (m.$($subfields)+).clone()).collect::<Vec<_>>()
        };
    }

    struct MockMarkup {
        media_text: String,
        markup: InlineKeyboardMarkup,
    }

    impl From<MockMarkup> for MessageKind {
        fn from(val: MockMarkup) -> Self {
            MessageKind::Common(MessageCommon {
                author_signature: None,
                forward_origin: None,
                reply_to_message: None,
                external_reply: None,
                quote: None,
                edit_date: None,
                media_kind: Text(MediaText {
                    text: val.media_text,
                    entities: vec![],
                    link_preview_options: None,
                }),
                reply_markup: Some(val.markup),
                is_automatic_forward: false,
                has_protected_content: false,
            })
        }
    }

    #[tokio::test]
    async fn test_delete() {
        let message = MockMessageText::new().text("/delete");
        let mut db = MockDatabase::new();
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(vec![Box::new(rem_clone.clone().into_active_model())])
        });
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        let rem_clone = rem.clone();
        db.expect_get_reminder()
            .with(eq(rem.id))
            .returning(move |_| Ok(Some(rem_clone.clone())));
        db.expect_delete_reminder()
            .with(eq(rem.id))
            .returning(move |_| Ok(()));
        let bot = mock_bot(db, message);
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![
                        vec![InlineKeyboardButton {
                            text: "01.01 01:01 <>".to_string(),
                            kind: CallbackData(
                                "delrem::rem_alt::1".to_string(),
                            ),
                        },],
                        vec![InlineKeyboardButton {
                            text: "➡️".to_string(),
                            kind: CallbackData("delrem::page::1".to_string(),),
                        },],
                    ],
                },
            }
            .into()]
        );

        bot.update(
            MockCallbackQuery::new()
                .data("delrem::page::1")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, edited_messages_reply_markup, message.kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![InlineKeyboardButton {
                        text: "⬅️".to_string(),
                        kind: CallbackData("delrem::page::0".to_string(),),
                    },],],
                },
            }
            .into()]
        );

        bot.update(
            MockCallbackQuery::new().data("delrem::page::0").message(
                bot.get_responses().edited_messages_reply_markup[0]
                    .message
                    .clone(),
            ),
        );
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, edited_messages_reply_markup, message.kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![
                        vec![InlineKeyboardButton {
                            text: "01.01 01:01 <>".to_string(),
                            kind: CallbackData(
                                "delrem::rem_alt::1".to_string(),
                            ),
                        },],
                        vec![InlineKeyboardButton {
                            text: "➡️".to_string(),
                            kind: CallbackData("delrem::page::1".to_string(),),
                        },],
                    ],
                },
            }
            .into()]
        );

        bot.update(
            MockCallbackQuery::new().data("delrem::rem_alt::1").message(
                bot.get_responses().edited_messages_reply_markup[0]
                    .message
                    .clone(),
            ),
        );
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessDelete(
                rem.into_active_model().to_unescaped_string(mock_timezone()),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_delete_still_one_page() {
        const REMINDERS_COUNT: i64 = 45;
        let message = MockMessageText::new().text("/delete");
        let mut db = MockDatabase::new();
        let mut rems = vec![];
        for i in 1..=REMINDERS_COUNT {
            let mut rem = basic_mock_reminder();
            rem.id = i;
            rems.push(rem);
        }
        let rems_clone = rems.clone();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(rems_clone
                .iter()
                .map(|rem| -> Box<dyn GenericReminder> {
                    Box::new(rem.clone().into_active_model())
                })
                .collect())
        });
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        for rem in rems.iter() {
            let rem_clone = rem.clone();
            db.expect_get_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(Some(rem_clone.clone())));
            db.expect_delete_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(()));
        }
        let bot = mock_bot(db, message);
        bot.dispatch().await;
        let mut page0_buttons = (1..=REMINDERS_COUNT)
            .map(|i| {
                vec![InlineKeyboardButton {
                    text: "01.01 01:01 <>".to_string().to_string(),
                    kind: CallbackData(
                        format!("delrem::rem_alt::{}", i).to_string(),
                    ),
                }]
            })
            .collect::<Vec<_>>();
        page0_buttons.push(vec![InlineKeyboardButton {
            text: "➡️".to_string(),
            kind: CallbackData("delrem::page::1".to_string()),
        }]);
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: page0_buttons.clone(),
                },
            }
            .into()]
        );

        bot.update(
            MockCallbackQuery::new()
                .data("delrem::page::1")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, edited_messages_reply_markup, message.kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![InlineKeyboardButton {
                        text: "⬅️".to_string(),
                        kind: CallbackData("delrem::page::0".to_string(),),
                    },],],
                },
            }
            .into()]
        );

        bot.update(
            MockCallbackQuery::new().data("delrem::page::0").message(
                bot.get_responses().edited_messages_reply_markup[0]
                    .message
                    .clone(),
            ),
        );
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, edited_messages_reply_markup, message.kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: page0_buttons
                },
            }
            .into()]
        );

        let rem = rems[0].clone();
        bot.update(
            MockCallbackQuery::new().data("delrem::rem_alt::1").message(
                bot.get_responses().edited_messages_reply_markup[0]
                    .message
                    .clone(),
            ),
        );
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessDelete(
                rem.into_active_model().to_unescaped_string(mock_timezone()),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_delete_two_pages() {
        const REMINDERS_COUNT: i64 = 46;
        const PAGE_REMINDERS_COUNT: i64 = 45;
        let message = MockMessageText::new().text("/delete");
        let mut db = MockDatabase::new();
        let mut rems = vec![];
        for i in 1..=REMINDERS_COUNT {
            let mut rem = basic_mock_reminder();
            rem.id = i;
            rem.desc = i.to_string();
            rems.push(rem);
        }
        let rems_clone = rems.clone();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(rems_clone
                .iter()
                .map(|rem| -> Box<dyn GenericReminder> {
                    Box::new(rem.clone().into_active_model())
                })
                .collect())
        });
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        for rem in rems.iter() {
            let rem_clone = rem.clone();
            db.expect_get_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(Some(rem_clone.clone())));
            db.expect_delete_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(()));
        }
        let bot = mock_bot(db, message);
        bot.dispatch().await;
        let mut page0_buttons = (1..=PAGE_REMINDERS_COUNT)
            .map(|i| {
                vec![InlineKeyboardButton {
                    text: format!("01.01 01:01 <{}>", i).to_string(),
                    kind: CallbackData(
                        format!("delrem::rem_alt::{}", i).to_string(),
                    ),
                }]
            })
            .collect::<Vec<_>>();
        page0_buttons.push(vec![InlineKeyboardButton {
            text: "➡️".to_string(),
            kind: CallbackData("delrem::page::1".to_string()),
        }]);
        let mut page1_buttons = (PAGE_REMINDERS_COUNT + 1..=REMINDERS_COUNT)
            .map(|i| {
                vec![InlineKeyboardButton {
                    text: format!("01.01 01:01 <{}>", i).to_string(),
                    kind: CallbackData(
                        format!("delrem::rem_alt::{}", i).to_string(),
                    ),
                }]
            })
            .collect::<Vec<_>>();
        page1_buttons.push(vec![
            InlineKeyboardButton {
                text: "⬅️".to_string(),
                kind: CallbackData("delrem::page::0".to_string()),
            },
            InlineKeyboardButton {
                text: "➡️".to_string(),
                kind: CallbackData("delrem::page::2".to_string()),
            },
        ]);
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: page0_buttons.clone(),
                },
            }
            .into()]
        );

        bot.update(
            MockCallbackQuery::new()
                .data("delrem::page::1")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, edited_messages_reply_markup, message.kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: page1_buttons,
                },
            }
            .into()]
        );

        bot.update(
            MockCallbackQuery::new().data("delrem::page::0").message(
                bot.get_responses().edited_messages_reply_markup[0]
                    .message
                    .clone(),
            ),
        );
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, edited_messages_reply_markup, message.kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: page0_buttons
                },
            }
            .into()]
        );

        let rem = rems[0].clone();
        bot.update(
            MockCallbackQuery::new().data("delrem::rem_alt::1").message(
                bot.get_responses().edited_messages_reply_markup[0]
                    .message
                    .clone(),
            ),
        );
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessDelete(
                rem.into_active_model().to_unescaped_string(mock_timezone()),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_list_no_timezone() {
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name().returning(|_| Ok(None));
        let message = MockMessageText::new().text("/list");
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::SelectTimezone.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_list_no_reminders() {
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_sorted_reminders().returning(|_| Ok(vec![]));
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
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(vec![Box::new(rem_clone.clone().into_active_model())])
        });
        let message = MockMessageText::new().text("/list");
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&format!(
            "{}\n{}",
            TgResponse::RemindersListHeader,
            rem.into_active_model().to_string(tz)
        ))
        .await;
    }

    #[tokio::test]
    async fn test_pause() {
        let mut db = MockDatabase::new();
        let tz = mock_timezone();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_reminder()
            .with(eq(rem.id))
            .returning(move |_| Ok(Some(rem_clone.clone())));
        db.expect_toggle_reminder_paused()
            .with(eq(rem.id))
            .times(1)
            .returning(move |_| Ok(true));
        db.expect_toggle_reminder_paused()
            .with(eq(rem.id))
            .times(1)
            .returning(move |_| Ok(false));
        let rem_clone = rem.clone();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(vec![Box::new(rem_clone.clone().into_active_model())])
        });
        let message = MockMessageText::new().text("/pause");
        let bot = mock_bot(db, message);

        bot.dispatch().await;
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![
                MockMarkup {
                    media_text: TgResponse::ChoosePauseReminder.to_string(),
                    markup: InlineKeyboardMarkup {
                        inline_keyboard: vec![
                            vec![InlineKeyboardButton {
                                text: "01.01 01:01 <>".to_string(),
                                kind: CallbackData(
                                    "pauserem::rem_alt::1".to_string(),
                                ),
                            },],
                            vec![InlineKeyboardButton {
                                text: "➡️".to_string(),
                                kind: CallbackData(
                                    "pauserem::page::1".to_string(),
                                ),
                            },],
                        ],
                    },
                }
                .into()
            ]
        );

        bot.update(
            MockCallbackQuery::new()
                .data("pauserem::rem_alt::1")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessPause(
                rem.clone().into_active_model().to_unescaped_string(tz),
            )
            .to_string(),
        )
        .await;

        bot.update(
            MockCallbackQuery::new()
                .data("pauserem::rem_alt::1")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessResume(
                rem.into_active_model().to_unescaped_string(tz),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    #[serial]
    async fn test_new_reminder() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let message = MockMessageText::new().text("10:00 test");
        let mut db = MockDatabase::new();
        let tz = mock_timezone();
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_insert_reminder()
            .returning(move |_| Ok(rem_clone.clone().into()));
        db.expect_set_reminder_reply_id().returning(|_, _| Ok(()));
        let bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessInsert(
                rem.into_active_model().to_unescaped_string(tz),
            )
            .to_string(),
        )
        .await;
    }
}
