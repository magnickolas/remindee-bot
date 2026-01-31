#[cfg(test)]
use std::fmt::Display;

use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::*;
use teloxide::types::ParseMode::MarkdownV2;
use teloxide::types::{
    ChatId, InlineKeyboardMarkup, LinkPreviewOptions, MessageId,
};
use teloxide::utils::markdown::escape;
use teloxide::RequestError;

use rust_i18n::t;

pub(crate) enum TgResponse {
    SuccessInsert(String),
    FailedInsert,
    IncorrectRequest,
    QueryingError,
    RemindersList(String),
    SelectTimezone,
    ChosenTimezone(String),
    FailedSetTimezone(String),
    ChooseDeleteReminder,
    SuccessDelete(String),
    FailedDelete,
    ChooseEditReminder,
    SuccessEdit(String, String),
    FailedEdit,
    EditReminderNotFound,
    CancelEdit,
    ChoosePauseReminder,
    SuccessPause(String),
    SuccessResume(String),
    FailedPause,
    Hello,
    HelloGroup,
    EnterNewTimePattern,
    EnterNewDescription,
    SettingsMenu,
    SelectLanguage,
    ChosenLanguage,
    FailedSetLanguage(String),
    Help,
}

impl TgResponse {
    pub(crate) fn to_unescaped_string_lang(&self, lang: &str) -> String {
        match self {
            Self::SuccessInsert(reminder_str) => {
                t!("SuccessInsert", locale = lang, reminder = reminder_str)
                    .to_string()
            }
            Self::FailedInsert => t!("FailedInsert", locale = lang).to_string(),
            Self::IncorrectRequest => {
                t!("IncorrectRequest", locale = lang).to_string()
            }
            Self::QueryingError => {
                t!("QueryingError", locale = lang).to_string()
            }
            Self::RemindersList(reminders_str) => {
                t!("RemindersList", locale = lang, reminders = reminders_str)
                    .to_string()
            }
            Self::SelectTimezone => {
                t!("SelectTimezone", locale = lang).to_string()
            }
            Self::ChosenTimezone(tz_name) => {
                t!("ChosenTimezone", locale = lang, tz = tz_name).to_string()
            }
            Self::FailedSetTimezone(tz_name) => {
                t!("FailedSetTimezone", locale = lang, tz = tz_name).to_string()
            }
            Self::ChooseDeleteReminder => {
                t!("ChooseDeleteReminder", locale = lang).to_string()
            }
            Self::SuccessDelete(reminder_str) => {
                t!("SuccessDelete", locale = lang, reminder = reminder_str)
                    .to_string()
            }
            Self::FailedDelete => t!("FailedDelete", locale = lang).to_string(),
            Self::ChooseEditReminder => {
                t!("ChooseEditReminder", locale = lang).to_string()
            }
            Self::SuccessEdit(old_reminder_str, reminder_str) => t!(
                "SuccessEdit",
                locale = lang,
                old = old_reminder_str,
                new = reminder_str
            )
            .to_string(),
            Self::FailedEdit => t!("FailedEdit", locale = lang).to_string(),
            Self::EditReminderNotFound => {
                t!("EditReminderNotFound", locale = lang).to_string()
            }
            Self::CancelEdit => t!("CancelEdit", locale = lang).to_string(),
            Self::ChoosePauseReminder => {
                t!("ChoosePauseReminder", locale = lang).to_string()
            }
            Self::SuccessPause(reminder_str) => {
                t!("SuccessPause", locale = lang, reminder = reminder_str)
                    .to_string()
            }
            Self::SuccessResume(reminder_str) => {
                t!("SuccessResume", locale = lang, reminder = reminder_str)
                    .to_string()
            }
            Self::FailedPause => t!("FailedPause", locale = lang).to_string(),
            Self::Hello => t!("Hello", locale = lang).to_string(),
            Self::HelloGroup => t!("HelloGroup", locale = lang).to_string(),
            Self::EnterNewTimePattern => {
                t!("EnterNewTimePattern", locale = lang).to_string()
            }
            Self::EnterNewDescription => {
                t!("EnterNewDescription", locale = lang).to_string()
            }
            Self::SettingsMenu => t!("SettingsMenu", locale = lang).to_string(),
            Self::SelectLanguage => {
                t!("SelectLanguage", locale = lang).to_string()
            }
            Self::ChosenLanguage => {
                t!("ChosenLanguage", locale = lang).to_string()
            }
            Self::FailedSetLanguage(lang_name) => {
                t!("FailedSetLanguage", locale = lang, lang = lang_name)
                    .to_string()
            }
            Self::Help => t!("Help", locale = lang).to_string(),
        }
    }

    pub(crate) fn to_string_lang(&self, lang: &str) -> String {
        escape(&self.to_unescaped_string_lang(lang))
    }
}

#[cfg(test)]
impl Display for TgResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_lang("en"))
    }
}

pub(crate) async fn _send_message(
    text: &str,
    bot: &Bot,
    chat_id: ChatId,
    silent: bool,
) -> Result<Message, RequestError> {
    bot.send_message(chat_id, text)
        .parse_mode(MarkdownV2)
        .link_preview_options(LinkPreviewOptions {
            is_disabled: true,
            url: Default::default(),
            prefer_small_media: Default::default(),
            prefer_large_media: Default::default(),
            show_above_text: Default::default(),
        })
        .disable_notification(silent)
        .send()
        .await
}

pub(crate) async fn send_message(
    text: &str,
    bot: &Bot,
    chat_id: ChatId,
) -> Result<Message, RequestError> {
    _send_message(text, bot, chat_id, false).await
}

pub(crate) async fn send_silent_message(
    text: &str,
    bot: &Bot,
    chat_id: ChatId,
) -> Result<Message, RequestError> {
    _send_message(text, bot, chat_id, true).await
}

pub(crate) async fn send_markup(
    text: &str,
    markup: InlineKeyboardMarkup,
    bot: &Bot,
    chat_id: ChatId,
) -> Result<(), RequestError> {
    bot.send_message(chat_id, text)
        .parse_mode(MarkdownV2)
        .link_preview_options(LinkPreviewOptions {
            is_disabled: true,
            url: Default::default(),
            prefer_small_media: Default::default(),
            prefer_large_media: Default::default(),
            show_above_text: Default::default(),
        })
        .disable_notification(true)
        .reply_markup(markup)
        .send()
        .await
        .map(|_| ())
}

pub(crate) async fn edit_markup(
    markup: InlineKeyboardMarkup,
    bot: &Bot,
    msg_id: MessageId,
    chat_id: ChatId,
) -> Result<(), RequestError> {
    bot.edit_message_reply_markup(chat_id, msg_id)
        .reply_markup(markup)
        .send()
        .await
        .map(|_| ())
}
