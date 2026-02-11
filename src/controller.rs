use std::sync::Arc;

#[cfg(not(test))]
use crate::db::Database;
#[cfg(test)]
use crate::db::MockDatabase as Database;
use crate::err::Error;
use crate::lang::get_user_language;
use crate::lang::Language;
use crate::parsers;
use crate::tg;
use crate::tz;

use crate::entity::reminder;
use crate::generic_reminder::GenericReminder;
use chrono_tz::Tz;
use sea_orm::IntoActiveModel;
use teloxide::prelude::*;
use teloxide::types::MessageId;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
};
use teloxide::{ApiError, RequestError};
use tg::TgResponse;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum EditMode {
    TimePattern,
    Description,
}

#[derive(Clone)]
pub(crate) struct TgMessageController {
    pub(crate) db: Arc<Database>,
    pub(crate) bot: Bot,
    pub(crate) chat_id: ChatId,
    pub(crate) user_id: UserId,
    pub(crate) msg_id: MessageId,
    pub(crate) reply_to_id: Option<MessageId>,
}

#[derive(Clone)]
pub(crate) struct TgCallbackController {
    pub(crate) msg_ctl: TgMessageController,
    pub(crate) cb_id: String,
}

pub(crate) enum ReminderUpdate {
    ReminderDescription(i64, String),
    ReminderTimePattern(i64, String),
}

fn is_ignorable_markup_clear_error(err: &RequestError) -> bool {
    matches!(
        err,
        RequestError::Api(ApiError::MessageNotModified)
            | RequestError::Api(ApiError::MessageCantBeEdited)
            | RequestError::Api(ApiError::MessageToEditNotFound)
            | RequestError::Api(ApiError::MessageIdInvalid)
    )
}

impl TgMessageController {
    pub(crate) fn new(
        db: Arc<Database>,
        bot: Bot,
        chat_id: ChatId,
        user_id: UserId,
        msg_id: MessageId,
        reply_to_id: Option<MessageId>,
    ) -> TgMessageController {
        Self {
            db,
            bot,
            chat_id,
            user_id,
            msg_id,
            reply_to_id,
        }
    }

    pub(crate) fn from_msg(
        db: Arc<Database>,
        bot: Bot,
        msg: Message,
    ) -> Option<TgMessageController> {
        Some(Self::new(
            db,
            bot,
            msg.chat.id,
            msg.clone().from?.id,
            msg.id,
            msg.reply_to_message().map(|msg| msg.id),
        ))
    }

    pub(crate) fn from_callback_query(
        db: Arc<Database>,
        bot: Bot,
        cb_query: &CallbackQuery,
    ) -> Option<TgMessageController> {
        let msg = cb_query.message.as_ref()?;
        Some(Self::new(
            db,
            bot,
            msg.chat().id,
            cb_query.from.id,
            msg.id(),
            None,
        ))
    }

    fn new_rec_id(&self) -> String {
        format!("{}:{}", self.chat_id.0, self.msg_id.0)
    }

    async fn user_lang(&self) -> crate::lang::Language {
        get_user_language(&self.db, self.user_id).await
    }

    pub(crate) async fn reply(
        &self,
        response: TgResponse,
    ) -> Result<Message, RequestError> {
        let lang = self.user_lang().await;
        tg::send_silent_message(
            &response.to_string_lang(lang.code()),
            &self.bot,
            self.chat_id,
        )
        .await
    }

    pub(crate) async fn start(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::Hello).await.map(|_| ())
    }

    pub(crate) async fn start_group(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::HelloGroup).await.map(|_| ())
    }

    pub(crate) async fn help(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::Help).await.map(|_| ())
    }

    /// Send a list of all notifications
    pub(crate) async fn list(&self, user_tz: Tz) -> Result<(), RequestError> {
        let lang = self.user_lang().await;

        let reminders_str =
            match self.db.get_sorted_reminders(self.chat_id.0).await {
                Ok(sorted_reminders) => sorted_reminders
                    .into_iter()
                    .map(|rem| {
                        rem.to_unescaped_string(user_tz)
                            .replace('@', "@\u{200B}")
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
                Err(err) => {
                    log::error!("{err}");
                    TgResponse::QueryingError.to_string_lang(lang.code())
                }
            };
        self.reply(TgResponse::RemindersList(reminders_str))
            .await
            .map(|_| ())
    }

    /// Send a markup with all timezones to select
    pub(crate) async fn choose_timezone(&self) -> Result<(), RequestError> {
        let lang = self.user_lang().await;
        tg::send_markup(
            &TgResponse::SelectTimezone.to_string_lang(lang.code()),
            self.get_markup_for_tz_page_idx(0),
            &self.bot,
            self.chat_id,
        )
        .await
    }

    pub(crate) async fn choose_language(&self) -> Result<(), RequestError> {
        let lang = self.user_lang().await;
        tg::send_markup(
            &TgResponse::SelectLanguage.to_string_lang(lang.code()),
            self.get_markup_for_languages(),
            &self.bot,
            self.chat_id,
        )
        .await
    }

    pub(crate) async fn choose_settings(&self) -> Result<(), RequestError> {
        let lang = self.user_lang().await;
        tg::send_markup(
            &TgResponse::SettingsMenu.to_string_lang(lang.code()),
            self.get_markup_for_settings().await,
            &self.bot,
            self.chat_id,
        )
        .await
    }

    /// Send user's timezone
    pub(crate) async fn get_timezone(
        &self,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        self.reply(TgResponse::ChosenTimezone(user_tz.to_string()))
            .await
            .map(|_| ())
    }

    /// General way to send a markup to select a reminder for some operation
    async fn start_alter(
        &self,
        response: TgResponse,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        let lang = self.user_lang().await;
        tg::send_markup(
            &response.to_string_lang(lang.code()),
            markup,
            &self.bot,
            self.chat_id,
        )
        .await
    }

    /// Send a markup to select a reminder for deleting
    pub(crate) async fn start_delete(&self, user_tz: Tz) -> Result<(), Error> {
        if let Some(reply_to_id) = self.reply_to_id {
            if let Ok(Some(reminder)) =
                self.get_reminder_by_message(reply_to_id).await
            {
                let response = match self.db.delete_reminder(reminder.id).await
                {
                    Ok(()) => {
                        if let Err(err) = self
                            .db
                            .close_open_occurrences(&reminder.rec_id)
                            .await
                        {
                            log::error!("{err}");
                        }
                        if let Err(err) = self
                            .db
                            .delete_reminder_messages(&reminder.rec_id)
                            .await
                        {
                            log::error!("{err}");
                        }
                        TgResponse::SuccessDelete(
                            reminder
                                .into_active_model()
                                .to_unescaped_string(user_tz),
                        )
                    }
                    Err(err) => {
                        log::error!("{err}");
                        TgResponse::FailedDelete
                    }
                };

                return self
                    .reply(response)
                    .await
                    .map(|_| ())
                    .map_err(From::from);
            }
        }

        let markup = self
            .get_markup_for_reminders_page_deletion(0, user_tz)
            .await;
        self.start_alter(TgResponse::ChooseDeleteReminder, markup)
            .await
            .map_err(From::from)
    }

    /// Send a markup to select a reminder for editing
    pub(crate) async fn start_edit(
        &self,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        if let Some(reply_to_id) = self.reply_to_id {
            if let Ok(Some(reminder)) =
                self.get_reminder_by_message(reply_to_id).await
            {
                self.send_edit_mode_markup(reminder.id).await?;
                return Ok(());
            }
        }

        let markup =
            self.get_markup_for_reminders_page_editing(0, user_tz).await;
        self.start_alter(TgResponse::ChooseEditReminder, markup)
            .await
    }

    /// Cancel ongoing reminder editing
    pub(crate) async fn cancel_edit(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::CancelEdit).await.map(|_| ())
    }

    /// Send a markup to select a reminder for pausing
    pub(crate) async fn start_pause(
        &self,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let markup =
            self.get_markup_for_reminders_page_pausing(0, user_tz).await;
        self.start_alter(TgResponse::ChoosePauseReminder, markup)
            .await
    }

    async fn send_edit_mode_markup(
        &self,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        let lang = self.user_lang().await;
        let markup = InlineKeyboardMarkup::default().append_row(vec![
            InlineKeyboardButton::new(
                t!("TimePattern", locale = lang.code()),
                InlineKeyboardButtonKind::CallbackData(format!(
                    "edit_rem_mode::rem_time_pattern::{rem_id}"
                )),
            ),
            InlineKeyboardButton::new(
                t!("Description", locale = lang.code()),
                InlineKeyboardButtonKind::CallbackData(format!(
                    "edit_rem_mode::rem_description::{rem_id}"
                )),
            ),
        ]);
        tg::send_markup(
            &t!("WhatToEdit", locale = lang.code()),
            markup,
            &self.bot,
            self.chat_id,
        )
        .await?;
        Ok(())
    }

    async fn parse_reminder(
        &self,
        text: &str,
        tz: Tz,
    ) -> Option<reminder::ActiveModel> {
        parsers::parse_reminder(
            text,
            self.chat_id.0,
            self.user_id.0,
            self.new_rec_id(),
            tz,
        )
        .await
    }

    async fn parse_reminder_with_rec_id(
        &self,
        text: &str,
        tz: Tz,
        rec_id: String,
    ) -> Option<reminder::ActiveModel> {
        parsers::parse_reminder(
            text,
            self.chat_id.0,
            self.user_id.0,
            rec_id,
            tz,
        )
        .await
    }

    /// Try to parse user's message into a one-time or recurring reminder and set it
    async fn _set_reminder(
        &self,
        text: &str,
        user_tz: Tz,
        rec_id: Option<String>,
    ) -> (Option<reminder::ActiveModel>, Option<TgResponse>) {
        let parsed_reminder = match rec_id {
            Some(rec_id) => {
                self.parse_reminder_with_rec_id(text, user_tz, rec_id).await
            }
            None => self.parse_reminder(text, user_tz).await,
        };

        match parsed_reminder {
            Some(reminder) => match self
                .db
                .insert_reminder(reminder.clone())
                .await
            {
                Ok(reminder) => {
                    let rem_str = reminder
                        .to_unescaped_string(user_tz)
                        .replace('@', "@\u{200B}");
                    (Some(reminder), Some(TgResponse::SuccessInsert(rem_str)))
                }
                Err(err) => {
                    log::error!("{err}");
                    (None, Some(TgResponse::FailedInsert))
                }
            },
            None => {
                if self.user_id.0 == self.chat_id.0 as u64 {
                    (None, Some(TgResponse::IncorrectRequest))
                } else {
                    (None, None)
                }
            }
        }
    }

    async fn link_reminder_message(
        &self,
        reminder: &reminder::ActiveModel,
        msg_id: MessageId,
    ) -> Result<(), Error> {
        let rec_id = reminder.rec_id.clone().unwrap();
        self.db
            .insert_reminder_message(&rec_id, self.chat_id.0, msg_id.0, false)
            .await
            .map_err(From::from)
    }

    async fn set_reminder(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> (Option<reminder::ActiveModel>, Option<TgResponse>) {
        self._set_reminder(text, user_tz, None).await
    }

    async fn set_reminder_silently(
        &self,
        text: &str,
        user_tz: Tz,
        rec_id: Option<String>,
    ) -> Option<reminder::ActiveModel> {
        self._set_reminder(text, user_tz, rec_id).await.0
    }

    pub(crate) async fn incorrect_request(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::IncorrectRequest).await.map(|_| ())
    }

    /// Switch the markup's page
    pub(crate) async fn select_timezone_set_page(
        &self,
        page_num: usize,
    ) -> Result<(), RequestError> {
        tg::edit_markup(
            self.get_markup_for_tz_page_idx(page_num),
            &self.bot,
            self.msg_id,
            self.chat_id,
        )
        .await
    }

    async fn alter_reminder_set_page(
        &self,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        tg::edit_markup(markup, &self.bot, self.msg_id, self.chat_id).await
    }

    pub(crate) async fn delete_reminder_set_page(
        &self,
        page_num: usize,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let markup = self
            .get_markup_for_reminders_page_deletion(page_num, user_tz)
            .await;
        self.alter_reminder_set_page(markup).await
    }

    pub(crate) async fn edit_reminder_set_page(
        &self,
        page_num: usize,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let markup = self
            .get_markup_for_reminders_page_editing(page_num, user_tz)
            .await;
        self.alter_reminder_set_page(markup).await
    }

    pub(crate) async fn pause_reminder_set_page(
        &self,
        page_num: usize,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let markup = self
            .get_markup_for_reminders_page_pausing(page_num, user_tz)
            .await;
        self.alter_reminder_set_page(markup).await
    }

    pub(crate) fn get_markup_for_tz_page_idx(
        &self,
        num: usize,
    ) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::default();
        let mut last_page: bool = false;
        if let Some(tz_names) = tz::get_tz_names_for_page_idx(num) {
            for chunk in tz_names.chunks(2) {
                markup = markup.append_row(
                    chunk
                        .iter()
                        .copied()
                        .map(|tz_name| {
                            InlineKeyboardButton::new(
                                tz_name,
                                InlineKeyboardButtonKind::CallbackData(
                                    "seltz::tz::".to_owned() + tz_name,
                                ),
                            )
                        })
                        .collect::<Vec<_>>(),
                );
            }
        } else {
            last_page = true;
        }
        let mut move_buttons = vec![];
        if num > 0 {
            move_buttons.push(InlineKeyboardButton::new(
                "⬅️",
                InlineKeyboardButtonKind::CallbackData(
                    "seltz::page::".to_owned() + &(num - 1).to_string(),
                ),
            ))
        }
        if !last_page {
            move_buttons.push(InlineKeyboardButton::new(
                "➡️",
                InlineKeyboardButtonKind::CallbackData(
                    "seltz::page::".to_owned() + &(num + 1).to_string(),
                ),
            ))
        }
        markup.append_row(move_buttons)
    }

    pub(crate) fn get_markup_for_languages(&self) -> InlineKeyboardMarkup {
        let row: Vec<InlineKeyboardButton> = crate::lang::LANGUAGES
            .iter()
            .map(|lang| {
                InlineKeyboardButton::new(
                    lang.name(),
                    InlineKeyboardButtonKind::CallbackData(format!(
                        "setlang::lang::{}",
                        lang.code()
                    )),
                )
            })
            .collect();
        InlineKeyboardMarkup::default().append_row(row)
    }

    pub(crate) async fn get_markup_for_settings(&self) -> InlineKeyboardMarkup {
        let lang = self.user_lang().await;
        InlineKeyboardMarkup::default().append_row(vec![
            InlineKeyboardButton::new(
                t!("ChangeLanguage", locale = lang.code()),
                InlineKeyboardButtonKind::CallbackData(
                    "settings::change_lang".into(),
                ),
            ),
        ])
    }

    async fn get_markup_for_reminders_page_alteration(
        &self,
        num: usize,
        cb_prefix: &str,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::default();
        let mut last_rem_page: bool = false;
        let sorted_reminders =
            self.db.get_sorted_reminders(self.chat_id.0).await;
        if let Some(reminders) = sorted_reminders
            .ok()
            .as_ref()
            .and_then(|rems| rems.chunks(45).nth(num))
        {
            for chunk in reminders.chunks(1) {
                let mut row = vec![];
                for rem in chunk {
                    let rem_str = rem.to_unescaped_string(user_timezone);
                    row.push(InlineKeyboardButton::new(
                        rem_str,
                        InlineKeyboardButtonKind::CallbackData(
                            cb_prefix.to_owned()
                                + &format!("::{}_alt::", rem.get_type())
                                + &rem.get_id().unwrap().to_string(),
                        ),
                    ))
                }
                markup = markup.append_row(row);
            }
        } else {
            last_rem_page = true;
        }
        let mut move_buttons = vec![];
        if num > 0 {
            move_buttons.push(InlineKeyboardButton::new(
                "⬅️",
                InlineKeyboardButtonKind::CallbackData(
                    cb_prefix.to_owned() + "::page::" + &(num - 1).to_string(),
                ),
            ))
        }
        if !last_rem_page {
            move_buttons.push(InlineKeyboardButton::new(
                "➡️",
                InlineKeyboardButtonKind::CallbackData(
                    cb_prefix.to_owned() + "::page::" + &(num + 1).to_string(),
                ),
            ))
        }
        markup.append_row(move_buttons)
    }

    pub(crate) async fn get_markup_for_reminders_page_deletion(
        &self,
        num: usize,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            "delrem",
            user_timezone,
        )
        .await
    }

    pub(crate) async fn get_markup_for_reminders_page_editing(
        &self,
        num: usize,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            "editrem",
            user_timezone,
        )
        .await
    }

    pub(crate) async fn get_markup_for_reminders_page_pausing(
        &self,
        num: usize,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            "pauserem",
            user_timezone,
        )
        .await
    }

    async fn replace_reminder(
        &self,
        text: &str,
        rem_id: i64,
        user_tz: Tz,
    ) -> (Option<reminder::ActiveModel>, TgResponse) {
        match self.db.get_reminder(rem_id).await {
            Ok(Some(old_reminder)) => {
                let rec_id = old_reminder.rec_id.clone();
                match self
                    .set_reminder_silently(text, user_tz, Some(rec_id))
                    .await
                {
                    Some(new_reminder) => {
                        match self.db.delete_reminder(rem_id).await {
                            Ok(()) => {
                                let new_reminder_str =
                                    new_reminder.to_unescaped_string(user_tz);
                                (
                                    Some(new_reminder),
                                    TgResponse::SuccessEdit(
                                        old_reminder
                                            .clone()
                                            .into_active_model()
                                            .to_unescaped_string(user_tz),
                                        new_reminder_str,
                                    ),
                                )
                            }
                            Err(err) => {
                                log::error!("{err}");
                                (None, TgResponse::FailedEdit)
                            }
                        }
                    }
                    None => (None, TgResponse::FailedEdit),
                }
            }
            Err(err) => {
                log::error!("{err}");
                (None, TgResponse::FailedEdit)
            }
            _ => (None, TgResponse::EditReminderNotFound),
        }
    }

    pub(crate) async fn edit_reminder(
        &self,
        update: ReminderUpdate,
        user_tz: Tz,
    ) -> Result<(), Error> {
        let (reminder, response) = match update {
            ReminderUpdate::ReminderDescription(rem_id, desc) => {
                match self.db.get_reminder(rem_id).await {
                    Ok(Some(old_reminder)) => {
                        let mut new_reminder = old_reminder.clone();
                        desc.clone_into(&mut new_reminder.desc);

                        let (reminder, response) = match self
                            .db
                            .update_reminder(new_reminder.clone())
                            .await
                        {
                            Ok(()) => (
                                Some(new_reminder.clone().into_active_model()),
                                TgResponse::SuccessEdit(
                                    old_reminder
                                        .clone()
                                        .into_active_model()
                                        .to_unescaped_string(user_tz),
                                    new_reminder
                                        .into_active_model()
                                        .to_unescaped_string(user_tz),
                                ),
                            ),
                            Err(_) => (None, TgResponse::FailedEdit),
                        };
                        (reminder, response)
                    }
                    Ok(None) => (None, TgResponse::EditReminderNotFound),
                    Err(err) => {
                        log::error!("{err}");
                        (None, TgResponse::FailedEdit)
                    }
                }
            }
            ReminderUpdate::ReminderTimePattern(rem_id, time_pattern) => {
                match self.db.get_reminder(rem_id).await {
                    Ok(Some(old_reminder)) => {
                        let (set_result, response) = self
                            .replace_reminder(
                                &(time_pattern + " " + &old_reminder.desc),
                                old_reminder.id,
                                user_tz,
                            )
                            .await;
                        (set_result, response)
                    }
                    Ok(None) => (None, TgResponse::EditReminderNotFound),
                    Err(err) => {
                        log::error!("{err}");
                        (None, TgResponse::FailedEdit)
                    }
                }
            }
        };

        let reply = self.reply(response).await?;

        if let Some(ref reminder) = reminder {
            if let Err(err) = self
                .db
                .close_open_occurrences(&reminder.rec_id.clone().unwrap())
                .await
            {
                log::error!("{err}");
            }
            self.link_reminder_message(reminder, reply.id).await?;
        }

        Ok(())
    }

    pub(crate) async fn set_new_reminder(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> Result<(), Error> {
        let (reminder, response) = self.set_reminder(text, user_tz).await;
        if let Some(response) = response {
            let reply = self.reply(response).await?;
            if let Some(ref reminder) = reminder {
                self.link_reminder_message(reminder, self.msg_id).await?;
                self.link_reminder_message(reminder, reply.id).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn set_timezone(
        &self,
        tz_name: &str,
    ) -> Result<(), RequestError> {
        let response = match self
            .db
            .insert_or_update_user_timezone(self.user_id.0 as i64, tz_name)
            .await
        {
            Ok(()) => TgResponse::ChosenTimezone(tz_name.to_owned()),
            Err(err) => {
                log::error!("{err}");
                TgResponse::FailedSetTimezone(tz_name.to_owned())
            }
        };
        self.reply(response).await.map(|_| ())
    }

    pub(crate) async fn set_language(
        &self,
        lang_code: &str,
    ) -> Result<(), RequestError> {
        let lang = Language::from_code(lang_code).unwrap_or_default();
        let response = match self
            .db
            .insert_or_update_user_language(self.user_id.0 as i64, lang_code)
            .await
        {
            Ok(()) => TgResponse::ChosenLanguage,
            Err(err) => {
                log::error!("{err}");
                TgResponse::FailedSetLanguage(lang.name().to_owned())
            }
        };
        self.reply(response).await.map(|_| ())
    }

    async fn get_reminder_by_message(
        &self,
        msg_id: MessageId,
    ) -> Result<Option<reminder::Model>, Error> {
        self.db
            .get_reminder_by_message(self.chat_id.0, msg_id.0)
            .await
            .map_err(From::from)
    }

    pub(crate) async fn edit_reminder_from_edited_message(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> Result<(), Error> {
        let (reminder, response, link_source_msg, close_open_occurrences) =
            match self
                .db
                .get_reminder_by_message(self.chat_id.0, self.msg_id.0)
                .await?
            {
                Some(old_rem) => {
                    let (reminder, response) =
                        self.replace_reminder(text, old_rem.id, user_tz).await;
                    (reminder, response, false, true)
                }
                None => {
                    let (reminder, response) =
                        self.set_reminder(text, user_tz).await;
                    (
                        reminder,
                        response.unwrap_or(TgResponse::IncorrectRequest),
                        true,
                        false,
                    )
                }
            };

        let reply = self.reply(response).await?;

        if let Some(ref reminder) = reminder {
            if close_open_occurrences {
                if let Err(err) = self
                    .db
                    .close_open_occurrences(&reminder.rec_id.clone().unwrap())
                    .await
                {
                    log::error!("{err}");
                }
            }
            if link_source_msg {
                self.link_reminder_message(reminder, self.msg_id).await?;
            }
            self.link_reminder_message(reminder, reply.id).await?;
        }
        Ok(())
    }
}

impl TgCallbackController {
    pub(crate) fn new(
        db: Arc<Database>,
        bot: Bot,
        cb_query: CallbackQuery,
    ) -> Option<TgCallbackController> {
        Some(Self {
            msg_ctl: TgMessageController::from_callback_query(
                db, bot, &cb_query,
            )?,
            cb_id: cb_query.id.to_string(),
        })
    }

    async fn answer_callback_query(
        &self,
        response: TgResponse,
    ) -> Result<(), RequestError> {
        self.msg_ctl.reply(response).await?;
        self.acknowledge_callback().await
    }

    pub(crate) async fn acknowledge_callback(
        &self,
    ) -> Result<(), RequestError> {
        self.msg_ctl
            .bot
            .answer_callback_query(teloxide::types::CallbackQueryId(
                self.cb_id.clone(),
            ))
            .send()
            .await
            .map(|_| ())
    }

    pub(crate) async fn set_timezone(
        &self,
        tz_name: &str,
    ) -> Result<(), RequestError> {
        self.msg_ctl.set_timezone(tz_name).await?;
        self.acknowledge_callback().await
    }

    pub(crate) async fn set_language(
        &self,
        lang_code: &str,
    ) -> Result<(), RequestError> {
        self.msg_ctl.set_language(lang_code).await?;
        self.acknowledge_callback().await
    }

    pub(crate) async fn delete_reminder(
        &self,
        rem_id: i64,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let response = match self.msg_ctl.db.get_reminder(rem_id).await {
            Ok(Some(reminder)) => {
                match self.msg_ctl.db.delete_reminder(rem_id).await {
                    Ok(()) => {
                        if let Err(err) = self
                            .msg_ctl
                            .db
                            .close_open_occurrences(&reminder.rec_id)
                            .await
                        {
                            log::error!("{err}");
                        }
                        if let Err(err) = self
                            .msg_ctl
                            .db
                            .delete_reminder_messages(&reminder.rec_id)
                            .await
                        {
                            log::error!("{err}");
                        }
                        TgResponse::SuccessDelete(
                            reminder
                                .into_active_model()
                                .to_unescaped_string(user_tz),
                        )
                    }
                    Err(err) => {
                        log::error!("{err}");
                        TgResponse::FailedDelete
                    }
                }
            }
            Err(err) => {
                log::error!("{err}");
                TgResponse::FailedDelete
            }
            _ => TgResponse::FailedDelete,
        };
        self.msg_ctl.delete_reminder_set_page(0, user_tz).await?;
        self.answer_callback_query(response).await
    }

    pub(crate) async fn choose_edit_mode_reminder(
        &self,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        self.msg_ctl.send_edit_mode_markup(rem_id).await?;
        self.acknowledge_callback().await
    }

    pub(crate) async fn pause_reminder(
        &self,
        rem_id: i64,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let response = match self.msg_ctl.db.get_reminder(rem_id).await {
            Ok(Some(reminder)) => {
                match self.msg_ctl.db.toggle_reminder_paused(rem_id).await {
                    Ok(true) => {
                        if let Err(err) = self
                            .msg_ctl
                            .db
                            .close_open_occurrences(&reminder.rec_id)
                            .await
                        {
                            log::error!("{err}");
                        }
                        TgResponse::SuccessPause(
                            reminder
                                .into_active_model()
                                .to_unescaped_string(user_tz),
                        )
                    }
                    Ok(false) => TgResponse::SuccessResume(
                        reminder
                            .into_active_model()
                            .to_unescaped_string(user_tz),
                    ),
                    Err(err) => {
                        log::error!("{err}");
                        TgResponse::FailedPause
                    }
                }
            }
            _ => TgResponse::FailedPause,
        };
        self.msg_ctl.pause_reminder_set_page(0, user_tz).await?;
        self.answer_callback_query(response).await
    }

    pub(crate) async fn set_edit_mode_reminder(
        &self,
        edit_mode: EditMode,
    ) -> Result<(), RequestError> {
        let response = match edit_mode {
            EditMode::TimePattern => TgResponse::EnterNewTimePattern,
            EditMode::Description => TgResponse::EnterNewDescription,
        };
        self.answer_callback_query(response).await
    }

    pub(crate) async fn done_occurrence(
        &self,
        occ_id: i64,
    ) -> Result<(), RequestError> {
        if let Err(err) = self
            .msg_ctl
            .db
            .complete_occurrence(occ_id, self.msg_ctl.chat_id.0)
            .await
        {
            log::error!("{err}");
        }

        self.acknowledge_callback().await?;

        if let Err(err) = tg::clear_markup(
            &self.msg_ctl.bot,
            self.msg_ctl.msg_id,
            self.msg_ctl.chat_id,
        )
        .await
        {
            if is_ignorable_markup_clear_error(&err) {
                log::debug!("{err}");
            } else {
                log::error!("{err}");
            }
        }

        Ok(())
    }
}
