use std::fmt::Display;

use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::*;
use teloxide::types::ParseMode::MarkdownV2;
use teloxide::types::{
    ChatId, InlineKeyboardMarkup, LinkPreviewOptions, MessageId,
};
use teloxide::utils::markdown::escape;
use teloxide::RequestError;

pub(crate) enum TgResponse {
    SuccessInsert(String),
    SuccessPeriodicInsert(String),
    FailedInsert,
    IncorrectRequest,
    QueryingError,
    RemindersListHeader,
    SelectTimezone,
    ChosenTimezone(String),
    FailedSetTimezone(String),
    ChooseDeleteReminder,
    SuccessDelete(String),
    FailedDelete,
    ChooseEditReminder,
    EnterNewReminder,
    SuccessEdit(String, String),
    FailedEdit,
    CancelEdit,
    ChoosePauseReminder,
    SuccessPause(String),
    SuccessResume(String),
    FailedPause,
    Hello,
    HelloGroup,
    EnterNewTimePattern,
    EnterNewDescription,
    SelectLanguage,
    ChosenLanguage(String),
    FailedSetLanguage(String),
}

impl TgResponse {
    pub(crate) fn to_unescaped_string_lang(
        &self,
        lang: crate::lang::Language,
    ) -> String {
        match lang {
            crate::lang::Language::English => self.to_unescaped_string_en(),
            crate::lang::Language::Dutch => self.to_unescaped_string_nl(),
        }
    }

    fn to_unescaped_string_en(&self) -> String {
        match self {
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
            Self::FailedSetTimezone(tz_name) => format!("Failed to set timezone {}", tz_name),
            Self::ChooseDeleteReminder => "Choose a reminder to delete:".to_owned(),
            Self::SuccessDelete(reminder_str) => format!("🗑 Deleted a reminder: {}", reminder_str),
            Self::FailedDelete => "Failed to delete...".to_owned(),
            Self::ChooseEditReminder => "Choose a reminder to edit:".to_owned(),
            Self::EnterNewReminder => "Enter reminder to replace with:".to_owned(),
            Self::SuccessEdit(old_reminder_str, reminder_str) => format!("📝 Replaced a reminder: {}\nwith ➡️ {}", old_reminder_str, reminder_str),
            Self::FailedEdit => "Failed to edit... You can try again or cancel editing with /cancel".to_owned(),
            Self::CancelEdit => "Canceled editing".to_owned(),
            Self::ChoosePauseReminder => "Choose a reminder to pause/resume:".to_owned(),
            Self::SuccessPause(reminder_str) => format!("⏸ Paused a reminder: {}", reminder_str),
            Self::SuccessResume(reminder_str) => format!("▶️ Resumed a reminder: {}", reminder_str),
            Self::FailedPause => "Failed to pause...".to_owned(),
            Self::Hello => concat!(
                "Hello! I'm remindee bot. My purpose is to remind you of whatever you ask and ",
                "whenever you ask.\n\n",
                "Examples:\n17:30 go to restaurant => notify today at 5:30 PM\n",
                "01.01 00:00 Happy New Year => notify at 1st of January at 12 AM\n",
                "55 10 * * 1-5 meeting call => notify at 10:55 AM every weekday ",
                "(CRON expression format)\n\n",
                "Before we start, please either send me your location 📍 or manually select the timezone using the /settimezone command first."
            )
            .to_owned(),
            Self::HelloGroup => concat!(
                "Hello! I'm remindee bot. My purpose is to remind you of whatever you ask and ",
                "whenever you ask.\n\n",
                "Examples:\n17:30 go to restaurant => notify today at 5:30 PM\n",
                "01.01 00:00 Happy New Year => notify at 1st of January at 12 AM\n",
                "55 10 * * 1-5 meeting call => notify at 10:55 AM every weekday ",
                "(CRON expression format)\n\n",
                "Before we start, please select the timezone using the /settimezone command first."
            )
            .to_owned(),
            Self::EnterNewTimePattern => "Enter a new time pattern for the reminder".to_owned(),
            Self::EnterNewDescription => "Enter a new description for the reminder".to_owned(),
            Self::SelectLanguage => "Select your language:".to_owned(),
            Self::ChosenLanguage(lang) => format!("Selected language {}.", lang),
            Self::FailedSetLanguage(lang) => format!("Failed to set language {}", lang),
        }
    }

    fn to_unescaped_string_nl(&self) -> String {
        match self {
            Self::SuccessInsert(reminder_str) => format!("Nieuwe herinnering toegevoegd:\n{}", reminder_str),
            Self::SuccessPeriodicInsert(reminder_str) => format!("Periodieke herinnering toegevoegd:\n{}", reminder_str),
            Self::FailedInsert => "Kon geen herinnering maken...".to_owned(),
            Self::IncorrectRequest => "Ongeldige aanvraag!".to_owned(),
            Self::QueryingError => "Fout bij het opvragen van herinneringen...".to_owned(),
            Self::RemindersListHeader => "Lijst van herinneringen:".to_owned(),
            Self::SelectTimezone => "Selecteer je tijdzone:".to_owned(),
            Self::ChosenTimezone(tz_name) => format!(
                "Gekozen tijdzone {}. Nu kun je herinneringen instellen.\n\nJe kunt de commando's die ik begrijp opvragen met /help.",
                tz_name
            ),
            Self::FailedSetTimezone(tz_name) => format!("Kon tijdzone {} niet instellen", tz_name),
            Self::ChooseDeleteReminder => "Kies een herinnering om te verwijderen:".to_owned(),
            Self::SuccessDelete(reminder_str) => format!("🗑 Herinnering verwijderd: {}", reminder_str),
            Self::FailedDelete => "Verwijderen mislukt...".to_owned(),
            Self::ChooseEditReminder => "Kies een herinnering om te bewerken:".to_owned(),
            Self::EnterNewReminder => "Voer de nieuwe herinnering in:".to_owned(),
            Self::SuccessEdit(old_reminder_str, reminder_str) => format!("📝 Herinnering vervangen: {}\nmet ➡️ {}", old_reminder_str, reminder_str),
            Self::FailedEdit => "Bewerken mislukt... Je kunt het opnieuw proberen of annuleren met /cancel".to_owned(),
            Self::CancelEdit => "Bewerken geannuleerd".to_owned(),
            Self::ChoosePauseReminder => "Kies een herinnering om te pauzeren/hervatten:".to_owned(),
            Self::SuccessPause(reminder_str) => format!("⏸ Herinnering gepauzeerd: {}", reminder_str),
            Self::SuccessResume(reminder_str) => format!("▶️ Herinnering hervat: {}", reminder_str),
            Self::FailedPause => "Pauzeren mislukt...".to_owned(),
            Self::Hello => concat!(
                "Hallo! Ik ben remindee bot. Mijn doel is je te herinneren aan wat je maar wilt en wanneer je maar wilt.\n\n",
                "Voorbeelden:\n17:30 ga naar restaurant => vandaag om 17:30\n",
                "01.01 00:00 Gelukkig nieuwjaar => 1 januari om 00:00\n",
                "55 10 * * 1-5 meeting call => om 10:55 elke werkdag (CRON-formaat)\n\n",
                "Voordat we beginnen, stuur me je locatie 📍 of selecteer handmatig de tijdzone via het commando /settimezone."
            ).to_owned(),
            Self::HelloGroup => concat!(
                "Hallo! Ik ben remindee bot. Mijn doel is jullie te herinneren aan wat je maar wilt en wanneer je maar wilt.\n\n",
                "Voorbeelden:\n17:30 ga naar restaurant => vandaag om 17:30\n",
                "01.01 00:00 Gelukkig nieuwjaar => 1 januari om 00:00\n",
                "55 10 * * 1-5 meeting call => om 10:55 elke werkdag (CRON-formaat)\n\n",
                "Voordat we beginnen, selecteer de tijdzone met het commando /settimezone."
            ).to_owned(),
            Self::EnterNewTimePattern => "Voer een nieuw tijdpatroon in voor de herinnering".to_owned(),
            Self::EnterNewDescription => "Voer een nieuwe omschrijving in voor de herinnering".to_owned(),
            Self::SelectLanguage => "Selecteer je taal:".to_owned(),
            Self::ChosenLanguage(lang) => format!("Gekozen taal {}.", lang),
            Self::FailedSetLanguage(lang) => format!("Kon taal {} niet instellen", lang),
        }
    }
}

impl Display for TgResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", escape(&self.to_unescaped_string_en()))
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

pub(crate) async fn delete_message(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: MessageId,
) -> Result<(), RequestError> {
    bot.delete_message(chat_id, msg_id).await.map(|_| ())
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
