use crate::cli::CLI;
#[cfg(not(test))]
use crate::db::Database;
use crate::db::InsertReminderOccurrence;
#[cfg(test)]
use crate::db::MockDatabase as Database;
use crate::entity::{reminder, reminder_occurrence};
use crate::err::Error;
use crate::format;
use crate::handlers::{get_handler, Command, State};
use crate::lang::{get_user_language, Language};
use crate::markup::done_markup;
use crate::parsers::now_time;
use crate::serializers::Pattern;
use crate::tg::{clear_markup, send_message, send_message_with_markup};
use crate::tz::get_user_timezone;
use chrono::{NaiveDateTime, TimeDelta};
use chrono_tz::{Tz, UTC};
use sea_orm::{ActiveValue::NotSet, ActiveValue::Set, IntoActiveModel};
use serde_json::{from_str, to_string};
use std::cmp::max;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::dispatching::dialogue::serializer::Json;
use teloxide::dispatching::dialogue::{ErasedStorage, SqliteStorage, Storage};
use teloxide::types::MessageId;
use teloxide::{
    prelude::*, utils::command::BotCommands, ApiError, RequestError,
};
use tokio::time::Instant;

fn is_ignorable_markup_clear_error(err: &RequestError) -> bool {
    matches!(
        err,
        RequestError::Api(ApiError::MessageNotModified)
            | RequestError::Api(ApiError::MessageCantBeEdited)
            | RequestError::Api(ApiError::MessageToEditNotFound)
            | RequestError::Api(ApiError::MessageIdInvalid)
    )
}

async fn clear_previous_done_markup(
    bot: &Bot,
    chat_id: i64,
    prev_msg_id: Option<i32>,
    new_msg_id: i32,
) {
    if let Some(prev_msg_id) = prev_msg_id {
        if prev_msg_id == new_msg_id {
            return;
        }
        if let Err(err) =
            clear_markup(bot, MessageId(prev_msg_id), ChatId(chat_id)).await
        {
            if is_ignorable_markup_clear_error(&err) {
                log::debug!("{err}");
            } else {
                log::error!("{err}");
            }
        }
    }
}

async fn send_reminder(
    reminder: &reminder::Model,
    user_timezone: Tz,
    lang: Language,
    done_occurrence_id: Option<i64>,
    bot: &Bot,
) -> Result<Message, Error> {
    let text = format::format_reminder(
        &reminder.clone().into_active_model(),
        user_timezone,
    );
    if let Some(occ_id) = done_occurrence_id {
        send_message_with_markup(
            &text,
            done_markup(lang, occ_id),
            bot,
            ChatId(reminder.chat_id),
        )
        .await
        .map_err(From::from)
    } else {
        send_message(&text, bot, ChatId(reminder.chat_id))
            .await
            .map_err(From::from)
    }
}

async fn send_occurrence_reminder(
    occ: &reminder_occurrence::Model,
    user_timezone: Tz,
    lang: Language,
    bot: &Bot,
) -> Result<Message, Error> {
    let text = format::format_reminder(
        &reminder::ActiveModel {
            id: NotSet,
            rec_id: Set(occ.rec_id.clone()),
            chat_id: Set(occ.chat_id),
            time: Set(occ.due_at),
            desc: Set(occ.desc_snapshot.clone()),
            user_id: Set(occ.user_id),
            paused: Set(false),
            nag_interval_sec: Set(Some(occ.nag_interval_sec)),
            pattern: Set(None),
        },
        user_timezone,
    );
    send_message_with_markup(
        &text,
        done_markup(lang, occ.id),
        bot,
        ChatId(occ.chat_id),
    )
    .await
    .map_err(From::from)
}

async fn process_due_reminders(db: &Database, bot: &Bot) {
    let reminders = db
        .get_active_reminders()
        .await
        .expect("Failed to get reminders from database");
    for reminder in reminders {
        if let Some(user_id) = reminder.user_id.map(|x| UserId(x as u64)) {
            if let Ok(Some(user_timezone)) =
                get_user_timezone(db, user_id).await
            {
                let lang = get_user_language(db, user_id).await;
                let mut next_reminder = None;
                let mut next_occurrence_time = None;
                if let Some(ref serialized) = reminder.pattern {
                    let mut pattern: Pattern = from_str(serialized).unwrap();
                    let lower_bound = max(reminder.time, now_time());
                    if let Some(next_time) = pattern.next(lower_bound) {
                        next_occurrence_time = Some(next_time);
                        next_reminder = Some(reminder::Model {
                            time: next_time,
                            pattern: to_string(&pattern).ok(),
                            ..reminder.clone()
                        });
                    }
                }
                let mut rollover_prev_msg_id = None;
                if reminder.nag_interval_sec.is_some() {
                    let closed_rows =
                        match db.close_open_occurrences(&reminder.rec_id).await
                        {
                            Ok(rows_affected) => rows_affected,
                            Err(err) => {
                                log::error!("{err}");
                                0
                            }
                        };
                    if closed_rows > 0 {
                        rollover_prev_msg_id = match db
                            .get_latest_reminder_message_id(
                                reminder.chat_id,
                                &reminder.rec_id,
                            )
                            .await
                        {
                            Ok(msg_id) => msg_id,
                            Err(err) => {
                                log::error!("{err}");
                                None
                            }
                        };
                    }
                }

                let mut created_occurrence = None;
                let send_result = if let Some(nag_interval_sec) =
                    reminder.nag_interval_sec
                {
                    match db
                        .insert_reminder_occurrence(InsertReminderOccurrence {
                            rec_id: reminder.rec_id.clone(),
                            chat_id: reminder.chat_id,
                            user_id: reminder.user_id,
                            due_at: reminder.time,
                            nag_interval_sec,
                            stop_at: next_occurrence_time,
                            desc_snapshot: reminder.desc.clone(),
                        })
                        .await
                    {
                        Ok(occ) => {
                            created_occurrence = Some(occ.id);
                            send_reminder(
                                &reminder,
                                user_timezone,
                                lang,
                                Some(occ.id),
                                bot,
                            )
                            .await
                        }
                        Err(err) => {
                            log::error!("{err}");
                            continue;
                        }
                    }
                } else {
                    send_reminder(&reminder, user_timezone, lang, None, bot)
                        .await
                };

                if let Ok(sent_msg) = send_result {
                    if let Err(err) = db
                        .insert_reminder_message(
                            &reminder.rec_id,
                            reminder.chat_id,
                            sent_msg.id.0,
                            true,
                        )
                        .await
                    {
                        log::error!("{err}");
                    }
                    clear_previous_done_markup(
                        bot,
                        reminder.chat_id,
                        rollover_prev_msg_id,
                        sent_msg.id.0,
                    )
                    .await;
                    db.delete_reminder(reminder.id).await.unwrap_or_else(
                        |err| {
                            log::error!("{err}");
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
                                log::error!("{err}");
                            });
                    }
                } else if let Some(occ_id) = created_occurrence {
                    if let Err(err) = db.delete_occurrence(occ_id).await {
                        log::error!("{err}");
                    }
                }
            }
        }
    }
}

async fn process_due_occurrences(db: &Database, bot: &Bot) {
    if let Err(err) = db.close_elapsed_occurrences().await {
        log::error!("{err}");
    }

    let occurrences = db
        .get_due_reminder_occurrences()
        .await
        .expect("Failed to get reminder occurrences from database");
    let mut user_context_cache: HashMap<i64, (Tz, Language)> = HashMap::new();
    for occ in occurrences {
        let (user_timezone, lang) = if let Some(user_id) = occ.user_id {
            if let Some(cached) = user_context_cache.get(&user_id) {
                *cached
            } else {
                let user_timezone =
                    get_user_timezone(db, UserId(user_id as u64))
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or(UTC);
                let lang = get_user_language(db, UserId(user_id as u64)).await;
                user_context_cache.insert(user_id, (user_timezone, lang));
                (user_timezone, lang)
            }
        } else {
            (UTC, Language::default())
        };

        let prev_msg_id = match db
            .get_latest_reminder_message_id(occ.chat_id, &occ.rec_id)
            .await
        {
            Ok(msg_id) => msg_id,
            Err(err) => {
                log::error!("{err}");
                None
            }
        };

        let is_open = match db.is_occurrence_open(occ.id).await {
            Ok(is_open) => is_open,
            Err(err) => {
                log::error!("{err}");
                false
            }
        };
        if !is_open {
            continue;
        }

        if let Ok(sent_msg) =
            send_occurrence_reminder(&occ, user_timezone, lang, bot).await
        {
            if let Err(err) = db
                .insert_reminder_message(
                    &occ.rec_id,
                    occ.chat_id,
                    sent_msg.id.0,
                    true,
                )
                .await
            {
                log::error!("{err}");
            }
            clear_previous_done_markup(
                bot,
                occ.chat_id,
                prev_msg_id,
                sent_msg.id.0,
            )
            .await;
            if let Err(err) = db.bump_occurrence_nag(occ.id).await {
                log::error!("{err}");
            }
        }
    }
}

async fn deadline_from_datetime(dt: NaiveDateTime) -> Instant {
    let now = now_time();

    let duration = (dt - now).max(TimeDelta::zero()).to_std().unwrap();
    Instant::now() + duration
}

/// Wait for the next reminder to send or some change in the database.
/// Send and update/delete reminders.
async fn poll_reminders(db: Arc<Database>, bot: Bot) {
    const DEFAULT_CHECK_INTERVAL: TimeDelta = TimeDelta::seconds(60);

    let next_deadline = tokio::time::sleep_until(Instant::now());
    tokio::pin!(next_deadline);

    let get_next_reminder_time = || async {
        deadline_from_datetime(
            db.get_next_reminder_time()
                .await
                .unwrap_or(None)
                .unwrap_or(now_time() + DEFAULT_CHECK_INTERVAL),
        )
        .await
    };

    loop {
        tokio::select! {
            _ = db.listen() => {
                next_deadline.as_mut().reset(get_next_reminder_time().await);
            }
            () = &mut next_deadline => {
                process_due_reminders(&db, &bot).await;
                process_due_occurrences(&db, &bot).await;

                next_deadline.as_mut().reset(get_next_reminder_time().await);
            }
        }
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
        callbacks, db::MockDatabase, entity::reminder,
        generic_reminder::GenericReminder, handlers::get_handler,
        parsers::test::TEST_TIMESTAMP, tg::TgResponse,
    };
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
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
    };
    use teloxide_tests::mock_bot::DistributionKey;
    use teloxide_tests::{
        IntoUpdate, MockBot, MockCallbackQuery, MockEditedMessage,
        MockMessageText, MockUser,
    };

    use super::State;

    fn basic_mock_reminder() -> reminder::Model {
        reminder::Model {
            id: 1,
            chat_id: 1,
            time: NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2024, 2, 2).unwrap(),
                NaiveTime::from_hms_opt(1, 2, 3).unwrap(),
            ),
            desc: "".to_owned(),
            user_id: None,
            paused: false,
            nag_interval_sec: None,
            pattern: None,
            rec_id: "1:1".to_owned(),
        }
    }

    fn mock_timezone_name() -> String {
        "Europe/Amsterdam".to_owned()
    }

    fn mock_timezone() -> Tz {
        mock_timezone_name().parse::<Tz>().unwrap()
    }

    fn mock_language_name() -> String {
        "en".to_owned()
    }

    fn mock_storage() -> Arc<InMemStorage<State>> {
        InMemStorage::<State>::new()
    }

    fn mock_bot<T>(
        db: MockDatabase,
        update: T,
    ) -> MockBot<Box<dyn std::error::Error + Send + Sync>, DistributionKey>
    where
        T: IntoUpdate,
    {
        let mut bot = MockBot::new(update, get_handler());
        bot.dependencies(deps![mock_storage(), Arc::new(db)]);
        bot
    }

    #[tokio::test]
    async fn test_help() {
        let message = MockMessageText::new().text("/help");
        let mut db = MockDatabase::new();
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&TgResponse::Help.to_string())
            .await;
    }

    #[tokio::test]
    async fn test_start() {
        let message = MockMessageText::new().text("/start");
        let mut db = MockDatabase::new();
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&TgResponse::Hello.to_string())
            .await;
    }

    #[tokio::test]
    async fn test_start_group() {
        let mut message = MockMessageText::new().text("/start");
        message.chat.id.0 = -1;
        let mut db = MockDatabase::new();
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&TgResponse::HelloGroup.to_string())
            .await;
    }

    #[tokio::test]
    async fn test_timezone() {
        let message = MockMessageText::new().text("/timezone");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::ChosenTimezone(mock_timezone_name()).to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_set_timezone() {
        let message = MockMessageText::new().text("/settimezone");
        let mut db = MockDatabase::new();
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
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
                effect_id: None,
                forward_origin: None,
                reply_to_message: None,
                external_reply: None,
                quote: None,
                reply_to_story: None,
                sender_boost_count: None,
                edit_date: None,
                media_kind: Text(MediaText {
                    text: val.media_text,
                    entities: vec![],
                    link_preview_options: None,
                }),
                reply_markup: Some(val.markup),
                is_automatic_forward: false,
                has_protected_content: false,
                is_from_offline: false,
                business_connection_id: None,
                paid_star_count: None,
            })
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_delete() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let message = MockMessageText::new().text("/delete");
        let mut db = MockDatabase::new();
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(vec![Box::new(rem_clone.clone().into_active_model())])
        });
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let rem_clone = rem.clone();
        db.expect_get_reminder()
            .with(eq(rem.id))
            .returning(move |_| Ok(Some(rem_clone.clone())));
        db.expect_delete_reminder()
            .with(eq(rem.id))
            .returning(move |_| Ok(()));
        db.expect_close_open_occurrences().returning(|_| Ok(0));
        db.expect_delete_reminder_messages().returning(|_| Ok(()));
        let mut bot = mock_bot(db, message);
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![MockMarkup {
                media_text: TgResponse::ChooseDeleteReminder.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![
                        vec![InlineKeyboardButton {
                            text: "02.02 02:02 <>".to_string(),
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
                            text: "02.02 02:02 <>".to_string(),
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
    #[serial]
    async fn test_delete_reply() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let mut db = MockDatabase::new();
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        db.expect_get_reminder_by_message()
            .with(eq(MockUser::ID as i64), eq(42))
            .returning(move |_, _| Ok(Some(rem_clone.clone())));
        db.expect_delete_reminder()
            .with(eq(rem.id))
            .returning(|_| Ok(()));
        db.expect_close_open_occurrences().returning(|_| Ok(0));
        db.expect_delete_reminder_messages().returning(|_| Ok(()));

        let reply_to_message = MockMessageText::new().id(42).build();
        let message = MockMessageText::new()
            .text("/delete")
            .reply_to_message(Box::new(reply_to_message));
        let mut bot = mock_bot(db, message);

        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessDelete(
                rem.into_active_model().to_unescaped_string(mock_timezone()),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_still_one_page() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
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
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        for rem in rems.iter() {
            let rem_clone = rem.clone();
            db.expect_get_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(Some(rem_clone.clone())));
            db.expect_delete_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(()));
            db.expect_close_open_occurrences().returning(|_| Ok(0));
            db.expect_delete_reminder_messages().returning(|_| Ok(()));
        }
        let mut bot = mock_bot(db, message);
        bot.dispatch().await;
        let mut page0_buttons = (1..=REMINDERS_COUNT)
            .map(|i| {
                vec![InlineKeyboardButton {
                    text: "02.02 02:02 <>".to_string().to_string(),
                    kind: CallbackData(
                        format!("delrem::rem_alt::{i}").to_string(),
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
    #[serial]
    async fn test_delete_two_pages() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
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
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        for rem in rems.iter() {
            let rem_clone = rem.clone();
            db.expect_get_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(Some(rem_clone.clone())));
            db.expect_delete_reminder()
                .with(eq(rem.id))
                .returning(move |_| Ok(()));
            db.expect_close_open_occurrences().returning(|_| Ok(0));
            db.expect_delete_reminder_messages().returning(|_| Ok(()));
        }
        let mut bot = mock_bot(db, message);
        bot.dispatch().await;
        let mut page0_buttons = (1..=PAGE_REMINDERS_COUNT)
            .map(|i| {
                vec![InlineKeyboardButton {
                    text: format!("02.02 02:02 <{i}>").to_string(),
                    kind: CallbackData(
                        format!("delrem::rem_alt::{i}").to_string(),
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
                    text: format!("02.02 02:02 <{i}>").to_string(),
                    kind: CallbackData(
                        format!("delrem::rem_alt::{i}").to_string(),
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
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let message = MockMessageText::new().text("/list");
        let mut bot = mock_bot(db, message);
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
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        db.expect_get_sorted_reminders().returning(|_| Ok(vec![]));
        let message = MockMessageText::new().text("/list");
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::RemindersList("".to_owned()).to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_list_one_reminder() {
        let mut db = MockDatabase::new();
        let tz = mock_timezone();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(vec![Box::new(rem_clone.clone().into_active_model())])
        });
        let message = MockMessageText::new().text("/list");
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::RemindersList(
                rem.into_active_model().to_unescaped_string(tz),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_edit_reply() {
        let mut db = MockDatabase::new();
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        db.expect_get_reminder_by_message()
            .with(eq(MockUser::ID as i64), eq(42))
            .returning(move |_, _| Ok(Some(rem_clone.clone())));

        let reply_to_message = MockMessageText::new().id(42).build();
        let message = MockMessageText::new()
            .text("/edit")
            .reply_to_message(Box::new(reply_to_message));
        let mut bot = mock_bot(db, message);

        bot.dispatch().await;

        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![MockMarkup {
                media_text: "What would you like to edit?".to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![
                        InlineKeyboardButton {
                            text: "Time pattern".to_string(),
                            kind: CallbackData(
                                "edit_rem_mode::rem_time_pattern::1"
                                    .to_string(),
                            ),
                        },
                        InlineKeyboardButton {
                            text: "Description".to_string(),
                            kind: CallbackData(
                                "edit_rem_mode::rem_description::1".to_string(),
                            ),
                        },
                    ]],
                },
            }
            .into()]
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_edit_reminder_not_found() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        let mut db = MockDatabase::new();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(vec![Box::new(rem_clone.clone().into_active_model())])
        });
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        db.expect_get_reminder()
            .with(eq(rem.id))
            .returning(|_| Ok(None));

        let message = MockMessageText::new().text("/edit");
        let mut bot = mock_bot(db, message);

        bot.dispatch().await;
        bot.update(
            MockCallbackQuery::new()
                .data("editrem::rem_alt::1")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch().await;
        bot.update(
            MockCallbackQuery::new()
                .data("edit_rem_mode::rem_description::1")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch().await;

        bot.update(MockMessageText::new().text("new description"));
        bot.dispatch_and_check_last_text(
            &TgResponse::EditReminderNotFound.to_string(),
        )
        .await;
    }

    #[tokio::test]
    #[serial]
    async fn test_pause() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let mut db = MockDatabase::new();
        let tz = mock_timezone();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let rem = basic_mock_reminder();
        let rem_clone = rem.clone();
        db.expect_get_reminder()
            .with(eq(rem.id))
            .returning(move |_| Ok(Some(rem_clone.clone())));
        db.expect_toggle_reminder_paused()
            .with(eq(rem.id))
            .times(1)
            .returning(move |_| Ok(true));
        db.expect_close_open_occurrences()
            .times(1)
            .returning(|_| Ok(0));
        db.expect_toggle_reminder_paused()
            .with(eq(rem.id))
            .times(1)
            .returning(move |_| Ok(false));
        let rem_clone = rem.clone();
        db.expect_get_sorted_reminders().returning(move |_| {
            Ok(vec![Box::new(rem_clone.clone().into_active_model())])
        });
        let message = MockMessageText::new().text("/pause");
        let mut bot = mock_bot(db, message);

        bot.dispatch().await;
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![
                MockMarkup {
                    media_text: TgResponse::ChoosePauseReminder.to_string(),
                    markup: InlineKeyboardMarkup {
                        inline_keyboard: vec![
                            vec![InlineKeyboardButton {
                                text: "02.02 02:02 <>".to_string(),
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
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        db.expect_insert_reminder()
            .returning(move |_| Ok(rem_clone.clone().into()));
        db.expect_insert_reminder_message()
            .times(2)
            .returning(|_, _, _, _| Ok(()));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessInsert(
                rem.into_active_model().to_unescaped_string(tz),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_time_interval_too_large_for_range_message() {
        let message = MockMessageText::new().text("14-15/2h gg");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::TimeIntervalTooLargeForRange.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_time_range_end_before_start_message() {
        let message = MockMessageText::new().text("18-10/1h ff");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::TimeRangeEndBeforeStart.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_date_interval_too_large_for_range_message() {
        let message = MockMessageText::new().text("11-12/2d 10:00 ff");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::DateIntervalTooLargeForRange.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_date_range_end_before_start_message() {
        let message =
            MockMessageText::new().text("12.02.2026-11.02.2026/1d 10:00 ff");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::DateRangeEndBeforeStart.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_date_range_end_before_start_message_with_mixed_year() {
        let message =
            MockMessageText::new().text("11.02-10.02.2026/1d 10:00 ff");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::DateRangeEndBeforeStart.to_string(),
        )
        .await;
    }

    #[tokio::test]
    #[serial]
    async fn test_date_in_past_message() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let message = MockMessageText::new().text("10.10.2000 ff");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(&TgResponse::DateInPast.to_string())
            .await;
    }

    #[tokio::test]
    async fn test_nag_interval_unsupported_unit_message() {
        let message = MockMessageText::new().text("12:40!1mo take pill");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::NagIntervalUnsupportedUnit.to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_invalid_cron_expression_message() {
        let message = MockMessageText::new().text("cron: */5 * * * invalid");
        let mut db = MockDatabase::new();
        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch_and_check_last_text(
            &TgResponse::CronExpressionInvalid.to_string(),
        )
        .await;
    }

    #[tokio::test]
    #[serial]
    async fn test_edited_message_sets_reminder_when_original_invalid() {
        *TEST_TIMESTAMP.write().unwrap() = mock_timezone()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp();

        let mut db = MockDatabase::new();
        let tz = mock_timezone();
        let rem = reminder::Model {
            id: 1,
            rec_id: format!("{}:{}", MockUser::ID, 42),
            chat_id: MockUser::ID as i64,
            time: NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            ),
            desc: "test".to_owned(),
            user_id: Some(MockUser::ID as i64),
            paused: false,
            nag_interval_sec: None,
            pattern: None,
        };
        let rem_clone = rem.clone();

        db.expect_get_user_timezone_name()
            .returning(|_| Ok(Some(mock_timezone_name())));
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        db.expect_get_reminder_by_message()
            .with(eq(MockUser::ID as i64), eq(42))
            .times(1)
            .returning(|_, _| Ok(None));
        db.expect_insert_reminder()
            .times(1)
            .returning(move |_| Ok(rem_clone.clone().into()));
        db.expect_insert_reminder_message()
            .times(2)
            .returning(|_, _, _, _| Ok(()));

        let original = MockMessageText::new().id(42).text("10:00test");
        let mut bot = mock_bot(db, original.clone());

        bot.dispatch_and_check_last_text(
            &TgResponse::IncorrectRequest.to_string(),
        )
        .await;

        bot.update(MockEditedMessage::new(
            original.text("10:00 test").edit_date(Utc::now()).build(),
        ));
        bot.dispatch_and_check_last_text(
            &TgResponse::SuccessInsert(
                rem.into_active_model().to_unescaped_string(tz),
            )
            .to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_settings_menu() {
        let message = MockMessageText::new().text("/settings");
        let mut db = MockDatabase::new();
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![MockMarkup {
                media_text: TgResponse::SettingsMenu.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![InlineKeyboardButton {
                        text: "Change language".to_string(),
                        kind: CallbackData("settings::change_lang".to_string()),
                    }]],
                },
            }
            .into()]
        );
    }

    #[tokio::test]
    async fn test_change_language_menu() {
        let message = MockMessageText::new().text("/settings");
        let mut db = MockDatabase::new();
        db.expect_get_user_language_name()
            .returning(|_| Ok(Some(mock_language_name())));
        let mut bot = mock_bot(db, message);
        bot.dispatch().await;
        bot.update(
            MockCallbackQuery::new()
                .data("settings::change_lang")
                .message(bot.get_responses().sent_messages[0].clone()),
        );
        bot.dispatch().await;
        assert_eq!(
            resp!(bot, sent_messages, kind),
            vec![MockMarkup {
                media_text: TgResponse::SelectLanguage.to_string(),
                markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![
                        InlineKeyboardButton {
                            text: "English".to_string(),
                            kind: CallbackData("setlang::lang::en".to_string()),
                        },
                        InlineKeyboardButton {
                            text: "Nederlands".to_string(),
                            kind: CallbackData("setlang::lang::nl".to_string()),
                        },
                        InlineKeyboardButton {
                            text: "Русский".to_string(),
                            kind: CallbackData("setlang::lang::ru".to_string()),
                        },
                    ]],
                },
            }
            .into()]
        );
    }

    #[tokio::test]
    async fn test_done_callback_without_timezone() {
        let cb_message = MockMessageText::new().id(42).build();
        let update = MockCallbackQuery::new()
            .data(callbacks::done_occurrence(1))
            .message(cb_message);
        let mut db = MockDatabase::new();
        db.expect_complete_occurrence()
            .with(eq(1), eq(MockUser::ID as i64))
            .returning(|_, _| Ok(true));
        let mut bot = mock_bot(db, update);
        bot.dispatch().await;
    }

    #[tokio::test]
    async fn test_done_callback_clears_stale_markup() {
        let cb_message = MockMessageText::new().id(42).build();
        let update = MockCallbackQuery::new()
            .data(callbacks::done_occurrence(1))
            .message(cb_message);
        let mut db = MockDatabase::new();
        db.expect_complete_occurrence()
            .with(eq(1), eq(MockUser::ID as i64))
            .returning(|_, _| Ok(false));
        let mut bot = mock_bot(db, update);
        bot.dispatch().await;
    }
}
