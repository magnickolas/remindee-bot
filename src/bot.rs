use crate::cli::CLI;
use crate::controller::{TgCallbackController, TgMessageController};
use crate::db::Database;
use crate::entity::{cron_reminder, reminder};
use crate::err::Error;
use crate::generic_reminder::GenericReminder;
use crate::parsers::now_time;
use crate::serializers::Pattern;
use crate::tg::send_message;
use crate::tz::get_user_timezone;
use async_once::AsyncOnce;
use async_std::task;
use chrono::Utc;
use chrono_tz::Tz;
use cron_parser::parse as parse_cron;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, IntoActiveModel};
use serde_json::{from_str, to_string};
use std::cmp::max;
use std::time::Duration;
use teloxide::{prelude::*, types::MessageId, utils::command::BotCommands};

#[derive(BotCommands, Clone)]
#[command(description = "Commands:", rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "list the set reminders")]
    List,
    #[command(description = "choose reminders to delete")]
    Delete,
    #[command(description = "choose reminders to edit")]
    Edit,
    #[command(description = "choose reminders to pause")]
    Pause,
    #[command(description = "set a new reminder")]
    Set(String),
    #[command(description = "select a timezone")]
    SetTimezone,
    #[command(description = "show your timezone")]
    Timezone,
    #[command(description = "show this text")]
    Help,
    #[command(description = "start")]
    Start,
}

async fn format_reminder<T: ActiveModelTrait + GenericReminder>(
    reminder: &T,
    user_timezone: Tz,
) -> Result<String, Error> {
    Ok(match reminder.user_id() {
        Some(user_id) if reminder.chat_id().is_group() => {
            reminder.to_string_with_mention(user_timezone, user_id.0 as i64)
        }
        _ => reminder.to_string(user_timezone),
    })
}

async fn format_cron_reminder(
    reminder: &cron_reminder::Model,
    next_reminder: &Option<cron_reminder::Model>,
    user_timezone: Tz,
) -> Result<String, Error> {
    let formatted_reminder =
        format_reminder(&reminder.clone().into_active_model(), user_timezone)
            .await?;
    Ok(match next_reminder {
        Some(next_reminder) => format!(
            "{}\n\nNext time → {}",
            formatted_reminder,
            next_reminder
                .clone()
                .into_active_model()
                .serialize_time(user_timezone)
        ),
        None => formatted_reminder,
    })
}

async fn send_reminder(
    reminder: &reminder::Model,
    user_timezone: Tz,
    bot: &Bot,
) -> Result<(), Error> {
    let text =
        format_reminder(&reminder.clone().into_active_model(), user_timezone)
            .await?;
    send_message(&text, bot, ChatId(reminder.chat_id))
        .await
        .map_err(From::from)
}

async fn send_cron_reminder(
    reminder: &cron_reminder::Model,
    next_reminder: &Option<cron_reminder::Model>,
    user_timezone: Tz,
    bot: &Bot,
) -> Result<(), Error> {
    let text =
        format_cron_reminder(reminder, next_reminder, user_timezone).await?;
    send_message(&text, bot, ChatId(reminder.chat_id))
        .await
        .map_err(From::from)
}

/// Periodically (every second) check for new reminders.
/// Send and delete one-time reminders if time has come.
/// Send cron reminders if time has come and update next reminder time.
async fn poll_reminders(db: &Database, bot: Bot) {
    loop {
        let reminders = db
            .get_active_reminders()
            .await
            .expect("Failed to get reminders from database");
        for reminder in reminders {
            if let Some(user_id) = reminder.user_id.map(|x| UserId(x as u64)) {
                if let Ok(Some(user_timezone)) =
                    get_user_timezone(db, user_id).await
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
                                dbg!(err);
                            },
                        );
                        if let Some(next_reminder) = next_reminder {
                            let mut next_reminder: reminder::ActiveModel =
                                next_reminder.into();
                            next_reminder.id = NotSet;
                            db.insert_reminder(next_reminder)
                                .await
                                .unwrap_or_else(|err| {
                                    dbg!(err);
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
                    get_user_timezone(db, user_id).await
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
                            dbg!(err);
                            None
                        }
                    };
                    match send_cron_reminder(
                        &cron_reminder,
                        &new_cron_reminder,
                        user_timezone,
                        &bot,
                    )
                    .await
                    {
                        Ok(()) => {
                            db.delete_cron_reminder(cron_reminder.id)
                                .await
                                .unwrap_or_else(|err| {
                                    dbg!(err);
                                });
                            if let Some(new_cron_reminder) = new_cron_reminder {
                                let mut new_cron_reminder: cron_reminder::ActiveModel = new_cron_reminder.into();
                                new_cron_reminder.id = NotSet;
                                db.insert_cron_reminder(new_cron_reminder)
                                    .await
                                    .unwrap_or_else(|err| {
                                        dbg!(err);
                                    });
                            }
                        }
                        Err(err) => {
                            dbg!(err);
                        }
                    }
                }
            }
        }
        task::sleep(Duration::from_secs(1)).await;
    }
}

lazy_static! {
    /// A singleton database with a pool connection
    /// that can be shared between threads
    static ref DATABASE: AsyncOnce<Database> = AsyncOnce::new(async {
        Database::new(&CLI.database)
            .await
            .unwrap_or_else(|err| panic!("Failed to connect to database {:?}: {}", CLI.database, err))
    });
}

pub async fn run() {
    pretty_env_logger::init();
    log::info!("Starting remindee-bot!");

    DATABASE
        .get()
        .await
        .apply_migrations()
        .await
        .expect("Failed to apply migrations");

    let bot = Bot::new(&CLI.token);

    bot.set_my_commands(Command::bot_commands())
        .await
        .expect("Failed to set bot commands");

    tokio::spawn(poll_reminders(DATABASE.get().await, bot.clone()));

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

impl<'a> TgMessageController<'a> {
    pub async fn new(
        bot: &'a Bot,
        chat_id: ChatId,
        user_id: UserId,
        msg_id: MessageId,
    ) -> Result<TgMessageController<'a>, Error> {
        Ok(Self {
            db: DATABASE.get().await,
            bot,
            chat_id,
            user_id,
            msg_id,
        })
    }

    pub async fn from_msg(
        bot: &'a Bot,
        msg: &Message,
    ) -> Result<TgMessageController<'a>, Error> {
        Self::new(
            bot,
            msg.chat.id,
            msg.from()
                .ok_or_else(|| Error::UserNotFound(msg.clone()))?
                .id,
            msg.id,
        )
        .await
    }

    pub async fn from_callback_query(
        bot: &'a Bot,
        cb_query: &CallbackQuery,
    ) -> Result<TgMessageController<'a>, Error> {
        let msg = cb_query
            .message
            .as_ref()
            .ok_or_else(|| Error::NoQueryMessage(cb_query.clone()))?;
        Self::new(bot, msg.chat.id, cb_query.from.id, msg.id).await
    }
}

impl<'a> TgCallbackController<'a> {
    pub async fn new(
        bot: &'a Bot,
        cb_query: &'a CallbackQuery,
    ) -> Result<TgCallbackController<'a>, Error> {
        Ok(Self {
            msg_ctl: TgMessageController::from_callback_query(bot, cb_query)
                .await?,
            cb_id: &cb_query.id,
        })
    }
}

async fn command_handler(
    msg: Message,
    bot: Bot,
    cmd: Command,
) -> Result<(), Error> {
    let ctl = TgMessageController::from_msg(&bot, &msg).await?;
    match cmd {
        Command::Help => ctl.reply(Command::descriptions()).await,
        Command::Start => ctl.start().await,
        Command::List => ctl.list().await,
        Command::SetTimezone => ctl.choose_timezone().await,
        Command::Timezone => ctl.get_timezone().await,
        Command::Delete => ctl.start_delete().await,
        Command::Edit => ctl.start_edit().await,
        Command::Pause => ctl.start_pause().await,
        Command::Set(ref reminder_text) => {
            ctl.set_or_edit_reminder(reminder_text).await
        }
    }
    .map_err(From::from)
}

async fn message_handler(msg: Message, bot: Bot) -> Result<(), Error> {
    let ctl = TgMessageController::from_msg(&bot, &msg).await?;
    if !ctl.chat_id.is_user() {
        Ok(())
    } else if let Some(text) = msg.text() {
        ctl.set_or_edit_reminder(text).await
    } else {
        ctl.incorrect_request().await
    }
    .map_err(From::from)
}

async fn callback_handler(
    cb_query: CallbackQuery,
    bot: Bot,
) -> Result<(), Error> {
    if let Some(cb_data) = &cb_query.data {
        let ctl = TgCallbackController::new(&bot, &cb_query).await?;
        let msg_ctl = &ctl.msg_ctl;
        if let Some(page_num) = cb_data
            .strip_prefix("seltz::page::")
            .and_then(|x| x.parse::<usize>().ok())
        {
            msg_ctl
                .select_timezone_set_page(page_num)
                .await
                .map_err(From::from)
        } else if let Some(tz_name) = cb_data.strip_prefix("seltz::tz::") {
            ctl.set_timezone(tz_name).await.map_err(From::from)
        } else if let Some(page_num) = cb_data
            .strip_prefix("delrem::page::")
            .and_then(|x| x.parse::<usize>().ok())
        {
            msg_ctl
                .delete_reminder_set_page(page_num)
                .await
                .map_err(From::from)
        } else if let Some(rem_id) = cb_data
            .strip_prefix("delrem::rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.delete_reminder(rem_id).await.map_err(From::from)
        } else if let Some(cron_rem_id) = cb_data
            .strip_prefix("delrem::cron_rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.delete_cron_reminder(cron_rem_id)
                .await
                .map_err(From::from)
        } else if let Some(page_num) = cb_data
            .strip_prefix("editrem::page::")
            .and_then(|x| x.parse::<usize>().ok())
        {
            msg_ctl
                .edit_reminder_set_page(page_num)
                .await
                .map_err(From::from)
        } else if let Some(rem_id) = cb_data
            .strip_prefix("editrem::rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.edit_reminder(rem_id).await.map_err(From::from)
        } else if let Some(cron_rem_id) = cb_data
            .strip_prefix("editrem::cron_rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.edit_cron_reminder(cron_rem_id)
                .await
                .map_err(From::from)
        } else if let Some(page_num) = cb_data
            .strip_prefix("pauserem::page::")
            .and_then(|x| x.parse::<usize>().ok())
        {
            msg_ctl
                .pause_reminder_set_page(page_num)
                .await
                .map_err(From::from)
        } else if let Some(cron_rem_id) = cb_data
            .strip_prefix("pauserem::cron_rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.pause_cron_reminder(cron_rem_id)
                .await
                .map_err(From::from)
        } else {
            Err(Error::UnmatchedQuery(cb_query))
        }
    } else {
        Err(Error::NoQueryData(cb_query))
    }
}
