use std::future::Future;
use std::sync::Arc;

use crate::db;
#[cfg(not(test))]
use crate::db::Database;
#[cfg(test)]
use crate::db::MockDatabase as Database;
use crate::err::Error;
use crate::parsers;
use crate::tg;
use crate::tz;

use crate::entity::{cron_reminder, reminder};
use crate::generic_reminder::GenericReminder;
use chrono_tz::Tz;
use sea_orm::IntoActiveModel;
use teloxide::prelude::*;
use teloxide::types::MessageId;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
};
use teloxide::RequestError;
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
    CronReminder(i64, String),
}

pub(crate) enum Reminder {
    Reminder(reminder::Model),
    CronReminder(cron_reminder::Model),
}

pub(crate) enum ActiveReminder {
    Reminder(reminder::ActiveModel),
    CronReminder(cron_reminder::ActiveModel),
}

trait ReminderModel {
    type R: GenericReminder;
    fn into_active(self) -> Self::R;
}

impl ReminderModel for reminder::Model {
    type R = reminder::ActiveModel;

    fn into_active(self) -> Self::R {
        self.into_active_model()
    }
}

impl ReminderModel for cron_reminder::Model {
    type R = cron_reminder::ActiveModel;

    fn into_active(self) -> Self::R {
        self.into_active_model()
    }
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

    pub(crate) async fn reply<R: ToString>(
        &self,
        response: R,
    ) -> Result<Message, RequestError> {
        tg::send_silent_message(&response.to_string(), &self.bot, self.chat_id)
            .await
    }

    pub(crate) async fn start(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::Hello).await.map(|_| ())
    }

    pub(crate) async fn start_group(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::HelloGroup).await.map(|_| ())
    }

    /// Send a list of all notifications
    pub(crate) async fn list(&self, user_tz: Tz) -> Result<(), RequestError> {
        // Format reminders
        let text = match self.db.get_sorted_reminders(self.chat_id.0).await {
            Ok(sorted_reminders) => {
                std::iter::once(TgResponse::RemindersListHeader.to_string())
                    .chain(sorted_reminders.into_iter().map(|rem| {
                        rem.to_string(user_tz).replace('@', "@\u{200B}")
                    }))
                    .collect::<Vec<String>>()
                    .join("\n")
            }
            Err(err) => {
                log::error!("{}", err);
                TgResponse::QueryingError.to_string()
            }
        };
        self.reply(&text).await.map(|_| ())
    }

    /// Send a markup with all timezones to select
    pub(crate) async fn choose_timezone(&self) -> Result<(), RequestError> {
        tg::send_markup(
            &TgResponse::SelectTimezone.to_string(),
            self.get_markup_for_tz_page_idx(0),
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
        tg::send_markup(&response.to_string(), markup, &self.bot, self.chat_id)
            .await
    }

    /// Send a markup to select a reminder for deleting
    pub(crate) async fn start_delete(&self, user_tz: Tz) -> Result<(), Error> {
        if let Some(reply_to_id) = self.reply_to_id {
            if let Ok(Some(generic_reminder)) =
                self.get_reminder_by_msg_or_reply_id(reply_to_id).await
            {
                let response = match generic_reminder {
                    Reminder::Reminder(reminder) => {
                        match self.db.delete_reminder(reminder.id).await {
                            Ok(()) => TgResponse::SuccessDelete(
                                reminder
                                    .into_active_model()
                                    .to_unescaped_string(user_tz),
                            ),
                            Err(err) => {
                                log::error!("{}", err);
                                TgResponse::FailedDelete
                            }
                        }
                    }
                    Reminder::CronReminder(cron_reminder) => match self
                        .db
                        .delete_cron_reminder(cron_reminder.id)
                        .await
                    {
                        Ok(()) => TgResponse::SuccessDelete(
                            cron_reminder
                                .into_active_model()
                                .to_unescaped_string(user_tz),
                        ),
                        Err(err) => {
                            log::error!("{}", err);
                            TgResponse::FailedDelete
                        }
                    },
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

    async fn parse_reminder(
        &self,
        text: &str,
        tz: Tz,
    ) -> Option<ActiveReminder> {
        parsers::parse_cron_reminder(
            text,
            self.chat_id.0,
            self.user_id.0,
            self.msg_id.0,
            tz,
        )
        .await
        .map(ActiveReminder::CronReminder)
        .or(parsers::parse_reminder(
            text,
            self.chat_id.0,
            self.user_id.0,
            self.msg_id.0,
            tz,
        )
        .await
        .map(ActiveReminder::Reminder))
    }

    /// Try to parse user's message into a one-time or periodic reminder and set it
    async fn _set_reminder(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> (Option<ActiveReminder>, Option<TgResponse>) {
        match self.parse_reminder(text, user_tz).await {
            Some(ActiveReminder::Reminder(reminder)) => {
                match self.db.insert_reminder(reminder.clone()).await {
                    Ok(reminder) => {
                        let rem_str = reminder
                            .to_unescaped_string(user_tz)
                            .replace('@', "@\u{200B}");
                        (
                            Some(ActiveReminder::Reminder(reminder)),
                            Some(TgResponse::SuccessInsert(rem_str)),
                        )
                    }
                    Err(err) => {
                        log::error!("{}", err);
                        (None, Some(TgResponse::FailedInsert))
                    }
                }
            }
            Some(ActiveReminder::CronReminder(cron_reminder)) => {
                match self.db.insert_cron_reminder(cron_reminder.clone()).await
                {
                    Ok(cron_reminder) => {
                        let rem_str =
                            cron_reminder.to_unescaped_string(user_tz);
                        (
                            Some(ActiveReminder::CronReminder(cron_reminder)),
                            Some(TgResponse::SuccessPeriodicInsert(rem_str)),
                        )
                    }
                    Err(err) => {
                        log::error!("{}", err);
                        (None, Some(TgResponse::FailedInsert))
                    }
                }
            }
            None => {
                if self.user_id.0 == self.chat_id.0 as u64 {
                    (None, Some(TgResponse::IncorrectRequest))
                } else {
                    (None, None)
                }
            }
        }
    }

    async fn link_reminder_with_reply_msg(
        &self,
        reminder: reminder::ActiveModel,
        reply: &Message,
    ) -> Result<(), Error> {
        self.db
            .set_reminder_reply_id(reminder, reply.id.0)
            .await
            .map_err(From::from)
    }

    async fn link_cron_reminder_with_reply_msg(
        &self,
        reminder: cron_reminder::ActiveModel,
        reply: &Message,
    ) -> Result<(), Error> {
        self.db
            .set_cron_reminder_reply_id(reminder, reply.id.0)
            .await
            .map_err(From::from)
    }

    async fn set_reminder(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> Result<(Option<ActiveReminder>, Option<Message>), RequestError> {
        let (reminder, response) = self._set_reminder(text, user_tz).await;
        match response {
            Some(response) => {
                self.reply(response).await.map(|msg| (reminder, Some(msg)))
            }
            None => Ok((reminder, None)),
        }
    }

    async fn set_reminder_silently(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> Option<ActiveReminder> {
        self._set_reminder(text, user_tz).await.0
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

    async fn _replace_reminder<GetFut, DelFut, R>(
        &self,
        text: &str,
        rem_id: i64,
        user_tz: Tz,
        get_reminder: impl FnOnce(i64) -> GetFut,
        delete_reminder: impl FnOnce(i64) -> DelFut,
    ) -> Result<(Option<ActiveReminder>, Message), RequestError>
    where
        GetFut: Future<Output = Result<Option<R>, db::Error>>,
        DelFut: Future<Output = Result<(), db::Error>>,
        R: ReminderModel,
    {
        let (reminder, response) = match get_reminder(rem_id).await {
            Ok(Some(old_reminder)) => {
                match self.set_reminder_silently(text, user_tz).await {
                    Some(ActiveReminder::Reminder(new_reminder)) => {
                        match delete_reminder(rem_id).await {
                            Ok(()) => {
                                let new_reminder_str =
                                    new_reminder.to_unescaped_string(user_tz);
                                (
                                    Some(ActiveReminder::Reminder(
                                        new_reminder,
                                    )),
                                    TgResponse::SuccessEdit(
                                        old_reminder
                                            .into_active()
                                            .to_unescaped_string(user_tz),
                                        new_reminder_str,
                                    ),
                                )
                            }
                            Err(err) => {
                                log::error!("{}", err);
                                (None, TgResponse::FailedEdit)
                            }
                        }
                    }
                    Some(ActiveReminder::CronReminder(new_cron_reminder)) => {
                        match delete_reminder(rem_id).await {
                            Ok(()) => {
                                let new_cron_reminder_str = new_cron_reminder
                                    .to_unescaped_string(user_tz);
                                (
                                    Some(ActiveReminder::CronReminder(
                                        new_cron_reminder,
                                    )),
                                    TgResponse::SuccessEdit(
                                        old_reminder
                                            .into_active()
                                            .to_unescaped_string(user_tz),
                                        new_cron_reminder_str,
                                    ),
                                )
                            }
                            Err(err) => {
                                log::error!("{}", err);
                                (None, TgResponse::FailedEdit)
                            }
                        }
                    }
                    _ => (None, TgResponse::FailedEdit),
                }
            }
            Err(err) => {
                log::error!("{}", err);
                (None, TgResponse::FailedEdit)
            }
            _ => {
                log::error!("missing reminder with id: {}", rem_id);
                (None, TgResponse::FailedEdit)
            }
        };

        self.reply(response).await.map(|msg| (reminder, msg))
    }

    async fn replace_reminder(
        &self,
        text: &str,
        rem_id: i64,
        user_tz: Tz,
    ) -> Result<(Option<ActiveReminder>, Message), RequestError> {
        self._replace_reminder(
            text,
            rem_id,
            user_tz,
            |id: i64| self.db.get_reminder(id),
            |id: i64| self.db.delete_reminder(id),
        )
        .await
    }

    async fn replace_cron_reminder(
        &self,
        text: &str,
        cron_rem_id: i64,
        user_tz: Tz,
    ) -> Result<(Option<ActiveReminder>, Message), RequestError> {
        self._replace_reminder(
            text,
            cron_rem_id,
            user_tz,
            |id: i64| self.db.get_cron_reminder(id),
            |id: i64| self.db.delete_cron_reminder(id),
        )
        .await
    }

    pub(crate) async fn edit_reminder(
        &self,
        update: ReminderUpdate,
        user_tz: Tz,
    ) -> Result<(), Error> {
        let (reminder, old_reply_id, reply) = match update {
            ReminderUpdate::ReminderDescription(rem_id, desc) => {
                let old_reminder = self
                    .db
                    .get_reminder(rem_id)
                    .await?
                    .ok_or(Error::ReminderNotFound(rem_id))?;
                let mut new_reminder = old_reminder.clone();
                desc.clone_into(&mut new_reminder.desc);

                let (reminder, old_reply, response) =
                    match self.db.update_reminder(new_reminder.clone()).await {
                        Ok(()) => (
                            Some(ActiveReminder::Reminder(
                                new_reminder.clone().into_active_model(),
                            )),
                            old_reminder.reply_id,
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
                        Err(_) => (None, None, TgResponse::FailedEdit),
                    };
                self.reply(response)
                    .await
                    .map(|msg| (reminder, old_reply, Some(msg)))
            }
            ReminderUpdate::ReminderTimePattern(rem_id, time_pattern) => {
                let old_reminder = self
                    .db
                    .get_reminder(rem_id)
                    .await?
                    .ok_or(Error::ReminderNotFound(rem_id))?;
                self.replace_reminder(
                    &(time_pattern + " " + &old_reminder.desc),
                    old_reminder.id,
                    user_tz,
                )
                .await
                .map(|(set_result, msg)| {
                    (set_result, old_reminder.reply_id, Some(msg))
                })
            }
            ReminderUpdate::CronReminder(cron_rem_id, text) => {
                let old_cron_reminder = self
                    .db
                    .get_cron_reminder(cron_rem_id)
                    .await?
                    .ok_or(Error::CronReminderNotFound(cron_rem_id))?;
                self.replace_cron_reminder(&text, old_cron_reminder.id, user_tz)
                    .await
                    .map(|(set_result, msg)| {
                        (set_result, old_cron_reminder.reply_id, Some(msg))
                    })
            }
        }?;

        if let Some(ref reminder) = reminder {
            if let Some(ref reply) = reply {
                self.update_reply_link(
                    reminder,
                    reply,
                    old_reply_id.map(MessageId),
                )
                .await?;
            }
        }

        Ok(())
    }

    pub(crate) async fn set_new_reminder(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> Result<(), Error> {
        let (reminder, reply) = self.set_reminder(text, user_tz).await?;

        if let Some(ref reminder) = reminder {
            if let Some(ref reply) = reply {
                self.update_reply_link(reminder, reply, None).await?;
            }
        }

        Ok(())
    }

    pub(crate) async fn update_reply_link(
        &self,
        reminder: &ActiveReminder,
        reply: &Message,
        old_reply_id: Option<MessageId>,
    ) -> Result<(), Error> {
        if let Some(old_reply_id) = old_reply_id {
            tg::delete_message(&self.bot, self.chat_id, old_reply_id).await?;
        }
        match reminder {
            ActiveReminder::Reminder(ref reminder) => {
                self.link_reminder_with_reply_msg(reminder.clone(), reply)
                    .await
            }
            ActiveReminder::CronReminder(ref cron_reminder) => {
                self.link_cron_reminder_with_reply_msg(
                    cron_reminder.clone(),
                    reply,
                )
                .await
            }
        }
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
                log::error!("{}", err);
                TgResponse::FailedSetTimezone(tz_name.to_owned())
            }
        };
        self.reply(response).await.map(|_| ())
    }

    async fn get_reminder_by_msg_id(
        &self,
        msg_id: MessageId,
    ) -> Result<Option<Reminder>, Error> {
        if let reminder @ Some(_) = self
            .db
            .get_reminder_by_msg_id(msg_id.0)
            .await
            .map(|rem| rem.map(Reminder::Reminder))?
        {
            Ok(reminder)
        } else {
            self.db
                .get_cron_reminder_by_msg_id(msg_id.0)
                .await
                .map(|cron_rem| cron_rem.map(Reminder::CronReminder))
                .map_err(From::from)
        }
    }

    async fn get_reminder_by_reply_id(
        &self,
        reply_id: MessageId,
    ) -> Result<Option<Reminder>, Error> {
        if let reminder @ Some(_) = self
            .db
            .get_reminder_by_reply_id(reply_id.0)
            .await
            .map(|rem| rem.map(Reminder::Reminder))?
        {
            Ok(reminder)
        } else {
            self.db
                .get_cron_reminder_by_reply_id(reply_id.0)
                .await
                .map(|cron_rem| cron_rem.map(Reminder::CronReminder))
                .map_err(From::from)
        }
    }

    async fn get_reminder_by_msg_or_reply_id(
        &self,
        id: MessageId,
    ) -> Result<Option<Reminder>, Error> {
        if let reminder @ Some(_) = self.get_reminder_by_msg_id(id).await? {
            Ok(reminder)
        } else {
            self.get_reminder_by_reply_id(id).await
        }
    }

    pub(crate) async fn edit_reminder_from_edited_message(
        &self,
        text: &str,
        user_tz: Tz,
    ) -> Result<(), Error> {
        let (reminder, old_reply_id, reply) = match self
            .db
            .get_reminder_by_msg_id(self.msg_id.0)
            .await?
        {
            Some(old_rem) => self
                .replace_reminder(text, old_rem.id, user_tz)
                .await
                .map(|(rem, msg)| (rem, old_rem.reply_id, Some(msg))),
            None => {
                match self.db.get_cron_reminder_by_msg_id(self.msg_id.0).await?
                {
                    Some(old_cron_rem) => self
                        .replace_cron_reminder(text, old_cron_rem.id, user_tz)
                        .await
                        .map(|(rem, msg)| {
                            (rem, old_cron_rem.reply_id, Some(msg))
                        }),
                    None => Ok((None, None, None)),
                }
            }
        }?;

        if let Some(ref reminder) = reminder {
            if let Some(ref reply) = reply {
                self.update_reply_link(
                    reminder,
                    reply,
                    old_reply_id.map(MessageId),
                )
                .await?;
            }
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
            cb_id: cb_query.id,
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
            .answer_callback_query(&self.cb_id)
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

    pub(crate) async fn delete_reminder(
        &self,
        rem_id: i64,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let response = match self.msg_ctl.db.get_reminder(rem_id).await {
            Ok(Some(reminder)) => {
                match self.msg_ctl.db.delete_reminder(rem_id).await {
                    Ok(()) => TgResponse::SuccessDelete(
                        reminder
                            .into_active_model()
                            .to_unescaped_string(user_tz),
                    ),
                    Err(err) => {
                        log::error!("{}", err);
                        TgResponse::FailedDelete
                    }
                }
            }
            Err(err) => {
                log::error!("{}", err);
                TgResponse::FailedDelete
            }
            _ => {
                log::error!("missing reminder with id: {}", rem_id);
                TgResponse::FailedDelete
            }
        };
        self.msg_ctl.delete_reminder_set_page(0, user_tz).await?;
        self.answer_callback_query(response).await
    }

    pub(crate) async fn delete_cron_reminder(
        &self,
        cron_rem_id: i64,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let response = match self
            .msg_ctl
            .db
            .get_cron_reminder(cron_rem_id)
            .await
        {
            Ok(Some(cron_reminder)) => {
                match self.msg_ctl.db.delete_cron_reminder(cron_rem_id).await {
                    Ok(()) => TgResponse::SuccessDelete(
                        cron_reminder
                            .into_active_model()
                            .to_unescaped_string(user_tz),
                    ),
                    Err(err) => {
                        log::error!("{}", err);
                        TgResponse::FailedDelete
                    }
                }
            }
            Err(err) => {
                log::error!("{}", err);
                TgResponse::FailedDelete
            }
            _ => {
                log::error!("missing cron reminder with id: {}", cron_rem_id);
                TgResponse::FailedDelete
            }
        };
        self.msg_ctl.delete_reminder_set_page(0, user_tz).await?;
        self.answer_callback_query(response).await
    }

    pub(crate) async fn choose_edit_mode_reminder(
        &self,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        let markup = InlineKeyboardMarkup::default().append_row(vec![
            InlineKeyboardButton::new(
                "Time pattern",
                InlineKeyboardButtonKind::CallbackData(format!(
                    "edit_rem_mode::rem_time_pattern::{}",
                    rem_id
                )),
            ),
            InlineKeyboardButton::new(
                "Description",
                InlineKeyboardButtonKind::CallbackData(format!(
                    "edit_rem_mode::rem_description::{}",
                    rem_id
                )),
            ),
        ]);
        tg::send_markup(
            "What would you like to edit?",
            markup,
            &self.msg_ctl.bot,
            self.msg_ctl.chat_id,
        )
        .await?;
        self.acknowledge_callback().await
    }

    pub(crate) async fn edit_cron_reminder(&self) -> Result<(), RequestError> {
        let response = TgResponse::EnterNewReminder;
        self.answer_callback_query(response).await
    }

    pub(crate) async fn pause_reminder(
        &self,
        rem_id: i64,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let response = match self.msg_ctl.db.get_reminder(rem_id).await {
            Ok(Some(reminder)) => {
                match self.msg_ctl.db.toggle_reminder_paused(rem_id).await {
                    Ok(true) => TgResponse::SuccessPause(
                        reminder
                            .into_active_model()
                            .to_unescaped_string(user_tz),
                    ),
                    Ok(false) => TgResponse::SuccessResume(
                        reminder
                            .into_active_model()
                            .to_unescaped_string(user_tz),
                    ),
                    Err(err) => {
                        log::error!("{}", err);
                        TgResponse::FailedPause
                    }
                }
            }
            _ => {
                log::error!("missing reminder with id: {}", rem_id);
                TgResponse::FailedPause
            }
        };
        self.msg_ctl.pause_reminder_set_page(0, user_tz).await?;
        self.answer_callback_query(response).await
    }

    pub(crate) async fn pause_cron_reminder(
        &self,
        cron_rem_id: i64,
        user_tz: Tz,
    ) -> Result<(), RequestError> {
        let response =
            match self.msg_ctl.db.get_cron_reminder(cron_rem_id).await {
                Ok(Some(cron_reminder)) => {
                    match self
                        .msg_ctl
                        .db
                        .toggle_cron_reminder_paused(cron_rem_id)
                        .await
                    {
                        Ok(true) => TgResponse::SuccessPause(
                            cron_reminder
                                .into_active_model()
                                .to_unescaped_string(user_tz),
                        ),
                        Ok(false) => TgResponse::SuccessResume(
                            cron_reminder
                                .into_active_model()
                                .to_unescaped_string(user_tz),
                        ),
                        Err(err) => {
                            log::error!("{}", err);
                            TgResponse::FailedPause
                        }
                    }
                }
                _ => {
                    log::error!(
                        "missing cron reminder with id: {}",
                        cron_rem_id
                    );
                    TgResponse::FailedPause
                }
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
}
