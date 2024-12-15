use std::sync::Arc;

use teloxide::{
    dispatching::{dialogue, UpdateHandler},
    prelude::*,
    utils::command::BotCommands,
    Bot,
};

#[cfg(not(test))]
use teloxide::dispatching::dialogue::ErasedStorage;
#[cfg(test)]
use teloxide::dispatching::dialogue::InMemStorage;

#[cfg(not(test))]
use crate::db::Database;
#[cfg(test)]
use crate::db::MockDatabase as Database;
use crate::{
    controller::{TgCallbackController, TgMessageController},
    entity::common::EditMode,
    err::Error,
    tz::get_timezone_name_of_location,
};

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum State {
    #[default]
    Default,
    Edit(ChatId),
    EditCron(ChatId),
}

#[cfg(not(test))]
type MyStorage = ErasedStorage<State>;
#[cfg(test)]
type MyStorage = InMemStorage<State>;

type MyDialogue = Dialogue<State, MyStorage>;

#[derive(BotCommands, Clone)]
#[command(description = "Commands:", rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "list the set reminders")]
    List,
    #[command(description = "choose reminders to delete")]
    Delete,
    #[command(description = "choose reminders to edit")]
    Edit,
    #[command(description = "cancel editing")]
    Cancel,
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

pub fn get_handler(
) -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    dialogue::enter::<Update, MyStorage, State, _>()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(
            Update::filter_edited_message().endpoint(edited_message_handler),
        )
        .branch(Update::filter_callback_query().endpoint(callback_handler))
}

pub async fn command_handler(
    msg: Message,
    bot: Bot,
    cmd: Command,
    db: Arc<Database>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ctl = TgMessageController::from_msg(db, &bot, &msg).await?;
    match cmd {
        Command::Help => ctl
            .reply(Command::descriptions())
            .await
            .map(|_| ())
            .map_err(From::from),
        Command::Start => ctl.start().await.map_err(From::from),
        Command::List => ctl.list().await.map_err(From::from),
        Command::SetTimezone => ctl.choose_timezone().await.map_err(From::from),
        Command::Timezone => ctl.get_timezone().await.map_err(From::from),
        Command::Delete => ctl.start_delete().await.map_err(From::from),
        Command::Edit => ctl.start_edit().await.map_err(From::from),
        Command::Cancel => ctl.cancel_edit().await.map_err(From::from),
        Command::Pause => ctl.start_pause().await.map_err(From::from),
        Command::Set(ref reminder_text) => {
            Ok(ctl.set_or_edit_reminder(reminder_text).await.map(|_| ())?)
        }
    }
}

pub async fn edited_message_handler(
    msg: Message,
    bot: Bot,
    db: Arc<Database>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ctl = TgMessageController::from_msg(db, &bot, &msg).await?;
    if !ctl.chat_id.is_user() {
        Ok(())
    } else if let Some(text) = msg.text() {
        Ok(ctl.edit_reminder_from_edited_message(text).await?)
    } else {
        ctl.incorrect_request().await.map_err(From::from)
    }
}

pub async fn message_handler(
    msg: Message,
    bot: Bot,
    db: Arc<Database>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ctl = TgMessageController::from_msg(db, &bot, &msg).await?;
    if !ctl.chat_id.is_user() {
        Ok(())
    } else if let Some(location) = msg.location() {
        ctl.set_timezone(get_timezone_name_of_location(
            location.longitude,
            location.latitude,
        ))
        .await
        .map_err(From::from)
    } else if let Some(text) = msg.text() {
        ctl.set_or_edit_reminder(text)
            .await
            .map(|_| ())
            .map_err(From::from)
    } else {
        ctl.incorrect_request().await.map_err(From::from)
    }
}

pub async fn callback_handler(
    cb_query: CallbackQuery,
    bot: Bot,
    dialogue: MyDialogue,
    db: Arc<Database>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(cb_data) = &cb_query.data {
        let ctl = TgCallbackController::new(db, &bot, &cb_query).await?;
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
            dialogue.update(State::Edit(ChatId(rem_id))).await?;
            ctl.choose_edit_mode_reminder(rem_id)
                .await
                .map_err(From::from)
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
        } else if let Some(rem_id) = cb_data
            .strip_prefix("pauserem::rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.pause_reminder(rem_id).await.map_err(From::from)
        } else if let Some(cron_rem_id) = cb_data
            .strip_prefix("pauserem::cron_rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.pause_cron_reminder(cron_rem_id)
                .await
                .map_err(From::from)
        } else if cb_data == "edit_rem_mode::rem_time_pattern" {
            ctl.set_edit_mode_reminder(EditMode::TimePattern)
                .await
                .map_err(From::from)
        } else if cb_data == "edit_rem_mode::rem_description" {
            ctl.set_edit_mode_reminder(EditMode::Description)
                .await
                .map_err(From::from)
        } else if cb_data == "edit_rem_mode::cron_rem_time_pattern" {
            ctl.set_edit_mode_cron_reminder(EditMode::TimePattern)
                .await
                .map_err(From::from)
        } else if cb_data == "edit_rem_mode::cron_rem_description" {
            ctl.set_edit_mode_cron_reminder(EditMode::Description)
                .await
                .map_err(From::from)
        } else {
            Err(Error::UnmatchedQuery(cb_query))?
        }
    } else {
        Err(Error::NoQueryData(cb_query))?
    }
}
