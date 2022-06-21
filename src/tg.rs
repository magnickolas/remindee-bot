use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::*;
use teloxide::types::ParseMode::MarkdownV2;
use teloxide::types::{ChatId, InlineKeyboardMarkup};
use teloxide::utils::markdown::escape;
use teloxide::RequestError;

pub enum TgResponse {
    SuccessInsert(String),
    SuccessPeriodicInsert(String),
    FailedInsert,
    IncorrectRequest,
    QueryingError,
    RemindersListHeader,
    SelectTimezone,
    ChosenTimezone(String),
    NoChosenTimezone,
    FailedSetTimezone(String),
    ChooseDeleteReminder,
    SuccessDelete,
    FailedDelete,
    ChooseEditReminder,
    EnterNewReminder,
    SuccessEdit,
    FailedEdit,
    Hello,
    CommandsHelp,
}

impl ToString for TgResponse {
    fn to_string(&self) -> String {
        let raw_text: String = match self {
            Self::SuccessInsert(reminder_str) => format!("Added a reminder:\n{}", reminder_str),
            Self::SuccessPeriodicInsert(reminder_str) => format!("Added a periodic reminder:\n{}", reminder_str),
            Self::FailedInsert => "Failed to create a reminder...".to_owned(),
            Self::IncorrectRequest => "Incorrect request!".to_owned(),
            Self::QueryingError => "Error occured while querying reminders...".to_owned(),
            Self::RemindersListHeader => "List of reminders:".to_owned(),
            Self::SelectTimezone => "Select your timezone:".to_owned(),
            Self::ChosenTimezone(tz_name) => format!(
                concat!(
                    "Selected timezone {}. Now you can set some reminders.\n\n",
                    "You can get the commands I understand with /help."
                ),
                tz_name
            ),
            Self::NoChosenTimezone => "You've not selected timezone yet, please do it with /tz command".to_owned(),
            Self::FailedSetTimezone(tz_name) => format!("Failed to set timezone {}", tz_name),
            Self::ChooseDeleteReminder => "Choose a reminder to delete:".to_owned(),
            Self::SuccessDelete => "Deleted!".to_owned(),
            Self::FailedDelete => "Failed to delete...".to_owned(),
            Self::ChooseEditReminder => "Choose a reminder to edit:".to_owned(),
            Self::EnterNewReminder => "Enter reminder to replace with:".to_owned(),
            Self::SuccessEdit => "Edited!".to_owned(),
            Self::FailedEdit => "Failed to edit...".to_owned(),
            Self::Hello => concat!(
                "Hello! I'm remindee bot. My purpose is to remind you of whatever you ask and ",
                "whenever you ask.\n\n",
                "Examples:\n17:30 go to restaurant => notify today at 5:30 PM\n",
                "01.01 00:00 Happy New Year => notify at 1st of January at 12 AM\n",
                "55 10 * * 1-5 meeting call => notify at 10:55 AM every weekday ",
                "(CRON expression format)\n\n",
                "Please, select your timezone with /tz command first."
            )
            .to_owned(),
            Self::CommandsHelp => concat!(
                "/list — list the set reminders\n",
                "/del — choose reminders to delete\n",
                "/edit — choose reminders to edit\n",
                "/tz — select timezone\n",
                "/mytz — print your timezone"
            )
            .to_owned(),
        };
        escape(&raw_text)
    }
}

pub async fn _send_message(
    text: &str,
    bot: &Bot,
    user_id: ChatId,
    silent: bool
) -> Result<(), RequestError> {
    bot.send_message(user_id, text)
        .parse_mode(MarkdownV2)
        .disable_web_page_preview(true)
        .disable_notification(silent)
        .send()
        .await
        .map(|_| ())
}

pub async fn send_message(
    text: &str,
    bot: &Bot,
    user_id: ChatId
) -> Result<(), RequestError> {
    _send_message(text, bot, user_id, false).await
}

pub async fn send_silent_message(
    text: &str,
    bot: &Bot,
    user_id: ChatId
) -> Result<(), RequestError> {
    _send_message(text, bot, user_id, true).await
}

pub async fn send_markup(
    text: &str,
    markup: InlineKeyboardMarkup,
    bot: &Bot,
    user_id: ChatId,
) -> Result<(), RequestError> {
    bot.send_message(user_id, text)
        .parse_mode(MarkdownV2)
        .disable_web_page_preview(true)
        .disable_notification(true)
        .reply_markup(markup)
        .send()
        .await
        .map(|_| ())
}

pub async fn edit_markup(
    markup: InlineKeyboardMarkup,
    bot: &Bot,
    msg_id: i32,
    user_id: ChatId,
) -> Result<(), RequestError> {
    bot.edit_message_reply_markup(user_id, msg_id)
        .reply_markup(markup)
        .send()
        .await
        .map(|_| ())
}
