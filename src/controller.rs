use crate::db;
use crate::entity::common::EditMode;
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

pub struct TgMessageController<'a> {
    pub db: &'a db::Database,
    pub bot: &'a Bot,
    pub chat_id: ChatId,
    pub user_id: UserId,
    pub msg_id: MessageId,
}

pub struct TgCallbackController<'a> {
    pub msg_ctl: TgMessageController<'a>,
    pub cb_id: &'a str,
}

pub enum Reminder {
    Reminder(reminder::ActiveModel),
    CronReminder(cron_reminder::ActiveModel),
}

impl TgMessageController<'_> {
    pub async fn reply<R: ToString>(
        &self,
        response: R,
    ) -> Result<Message, RequestError> {
        tg::send_silent_message(&response.to_string(), self.bot, self.chat_id)
            .await
    }

    pub async fn start(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::Hello).await.map(|_| ())
    }

    /// Send a list of all notifications
    pub async fn list(&self) -> Result<(), RequestError> {
        // Format reminders
        let text = match tz::get_user_timezone(self.db, self.user_id).await {
            Ok(Some(user_timezone)) => {
                match self.db.get_sorted_all_reminders(self.chat_id.0).await {
                    Ok(sorted_reminders) => std::iter::once(
                        TgResponse::RemindersListHeader.to_string(),
                    )
                    .chain(sorted_reminders.into_iter().map(|rem| {
                        rem.to_string(user_timezone).replace('@', "@\u{200B}")
                    }))
                    .collect::<Vec<String>>()
                    .join("\n"),
                    Err(err) => {
                        log::error!("{}", err);
                        TgResponse::QueryingError.to_string()
                    }
                }
            }
            _ => TgResponse::NoChosenTimezone.to_string(),
        };
        self.reply(&text).await.map(|_| ())
    }

    /// Send a markup with all timezones to select
    pub async fn choose_timezone(&self) -> Result<(), RequestError> {
        tg::send_markup(
            &TgResponse::SelectTimezone.to_string(),
            self.get_markup_for_tz_page_idx(0),
            self.bot,
            self.chat_id,
        )
        .await
    }

    /// Send user's timezone
    pub async fn get_timezone(&self) -> Result<(), RequestError> {
        let response =
            match self.db.get_user_timezone_name(self.user_id.0 as i64).await {
                Ok(Some(tz_name)) => TgResponse::ChosenTimezone(tz_name),
                Ok(None) => TgResponse::NoChosenTimezone,
                Err(err) => {
                    log::error!("{}", err);
                    TgResponse::NoChosenTimezone
                }
            };
        self.reply(response).await.map(|_| ())
    }

    /// General way to send a markup to select a reminder for some operation
    async fn start_alter(
        &self,
        response: TgResponse,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        tg::send_markup(&response.to_string(), markup, self.bot, self.chat_id)
            .await
    }

    /// Send a markup to select a reminder for deleting
    pub async fn start_delete(&self) -> Result<(), RequestError> {
        match tz::get_user_timezone(self.db, self.user_id).await {
            Ok(Some(user_timezone)) => {
                let markup = self
                    .get_markup_for_reminders_page_deletion(0, user_timezone)
                    .await;
                self.start_alter(TgResponse::ChooseDeleteReminder, markup)
                    .await
            }
            _ => self.reply(TgResponse::NoChosenTimezone).await.map(|_| ()),
        }
    }

    /// Send a markup to select a reminder for editing
    pub async fn start_edit(&self) -> Result<(), RequestError> {
        match tz::get_user_timezone(self.db, self.user_id).await {
            Ok(Some(user_timezone)) => {
                let markup = self
                    .get_markup_for_reminders_page_editing(0, user_timezone)
                    .await;
                self.start_alter(TgResponse::ChooseEditReminder, markup)
                    .await
            }
            _ => self.reply(TgResponse::NoChosenTimezone).await.map(|_| ()),
        }
    }

    /// Cancel ongoing reminder editing
    pub async fn cancel_edit(&self) -> Result<(), RequestError> {
        let response = match self
            .db
            .reset_reminders_edit(self.chat_id.0)
            .await
            .and(self.db.reset_cron_reminders_edit(self.chat_id.0).await)
        {
            Ok(()) => TgResponse::CancelEdit,
            Err(err) => {
                log::error!("{}", err);
                TgResponse::FailedCancelEdit
            }
        };
        self.reply(response).await.map(|_| ())
    }

    /// Send a markup to select a reminder for pausing
    pub async fn start_pause(&self) -> Result<(), RequestError> {
        match tz::get_user_timezone(self.db, self.user_id).await {
            Ok(Some(user_timezone)) => {
                let markup = self
                    .get_markup_for_reminders_page_pausing(0, user_timezone)
                    .await;
                self.start_alter(TgResponse::ChoosePauseReminder, markup)
                    .await
            }
            _ => self.reply(TgResponse::NoChosenTimezone).await.map(|_| ()),
        }
    }

    async fn parse_reminder(&self, text: &str, tz: Tz) -> Option<Reminder> {
        parsers::parse_cron_reminder(
            text,
            self.chat_id.0,
            self.user_id.0,
            self.msg_id.0,
            tz,
        )
        .await
        .map(Reminder::CronReminder)
        .or(parsers::parse_reminder(
            text,
            self.chat_id.0,
            self.user_id.0,
            self.msg_id.0,
            tz,
        )
        .await
        .map(Reminder::Reminder))
    }

    /// Try to parse user's message into a one-time or periodic reminder and set it
    async fn _set_reminder(
        &self,
        text: &str,
    ) -> (Option<Reminder>, Option<TgResponse>) {
        match tz::get_user_timezone(self.db, self.user_id).await {
            Ok(Some(user_timezone)) => {
                match self.parse_reminder(text, user_timezone).await {
                    Some(Reminder::Reminder(reminder)) => {
                        match self.db.insert_reminder(reminder.clone()).await {
                            Ok(reminder) => {
                                let rem_str = reminder
                                    .to_unescaped_string(user_timezone)
                                    .replace('@', "@\u{200B}");
                                (
                                    Some(Reminder::Reminder(reminder)),
                                    Some(TgResponse::SuccessInsert(rem_str)),
                                )
                            }
                            Err(err) => {
                                log::error!("{}", err);
                                (None, Some(TgResponse::FailedInsert))
                            }
                        }
                    }
                    Some(Reminder::CronReminder(cron_reminder)) => {
                        match self
                            .db
                            .insert_cron_reminder(cron_reminder.clone())
                            .await
                        {
                            Ok(_) => {
                                let rem_str = cron_reminder
                                    .to_unescaped_string(user_timezone);
                                (
                                    Some(Reminder::CronReminder(cron_reminder)),
                                    Some(TgResponse::SuccessPeriodicInsert(
                                        rem_str,
                                    )),
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
            _ => (None, Some(TgResponse::NoChosenTimezone)),
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
    ) -> Result<(Option<Reminder>, Option<Message>), RequestError> {
        let (reminder, response) = self._set_reminder(text).await;
        match response {
            Some(response) => {
                self.reply(response).await.map(|msg| (reminder, Some(msg)))
            }
            None => Ok((reminder, None)),
        }
    }

    async fn set_reminder_silently(&self, text: &str) -> Option<Reminder> {
        self._set_reminder(text).await.0
    }

    pub async fn incorrect_request(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::IncorrectRequest).await.map(|_| ())
    }

    /// Switch the markup's page
    pub async fn select_timezone_set_page(
        &self,
        page_num: usize,
    ) -> Result<(), RequestError> {
        tg::edit_markup(
            self.get_markup_for_tz_page_idx(page_num),
            self.bot,
            self.msg_id,
            self.chat_id,
        )
        .await
    }

    async fn alter_reminder_set_page(
        &self,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        tg::edit_markup(markup, self.bot, self.msg_id, self.chat_id).await
    }

    pub async fn delete_reminder_set_page(
        &self,
        page_num: usize,
    ) -> Result<(), RequestError> {
        if let Ok(Some(user_timezone)) =
            tz::get_user_timezone(self.db, self.user_id).await
        {
            let markup = self
                .get_markup_for_reminders_page_deletion(page_num, user_timezone)
                .await;
            self.alter_reminder_set_page(markup).await
        } else {
            self.reply(TgResponse::NoChosenTimezone).await.map(|_| ())
        }
    }

    pub async fn edit_reminder_set_page(
        &self,
        page_num: usize,
    ) -> Result<(), RequestError> {
        if let Ok(Some(user_timezone)) =
            tz::get_user_timezone(self.db, self.user_id).await
        {
            let markup = self
                .get_markup_for_reminders_page_editing(page_num, user_timezone)
                .await;
            self.alter_reminder_set_page(markup).await
        } else {
            self.reply(TgResponse::NoChosenTimezone).await.map(|_| ())
        }
    }

    pub async fn pause_reminder_set_page(
        &self,
        page_num: usize,
    ) -> Result<(), RequestError> {
        if let Ok(Some(user_timezone)) =
            tz::get_user_timezone(self.db, self.user_id).await
        {
            let markup = self
                .get_markup_for_reminders_page_pausing(page_num, user_timezone)
                .await;
            self.alter_reminder_set_page(markup).await
        } else {
            self.reply(TgResponse::NoChosenTimezone).await.map(|_| ())
        }
    }

    pub fn get_markup_for_tz_page_idx(
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
        exclude_reminders: bool,
        exclude_cron_reminders: bool,
    ) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::default();
        let mut last_rem_page: bool = false;
        let sorted_reminders = self
            .db
            .get_sorted_reminders(
                self.chat_id.0,
                exclude_reminders,
                exclude_cron_reminders,
            )
            .await;
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

    pub async fn get_markup_for_reminders_page_deletion(
        &self,
        num: usize,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            "delrem",
            user_timezone,
            false,
            false,
        )
        .await
    }

    pub async fn get_markup_for_reminders_page_editing(
        &self,
        num: usize,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            "editrem",
            user_timezone,
            false,
            false,
        )
        .await
    }

    pub async fn get_markup_for_reminders_page_pausing(
        &self,
        num: usize,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            "pauserem",
            user_timezone,
            false,
            false,
        )
        .await
    }

    pub async fn replace_reminder(
        &self,
        text: &str,
        rem_id: i64,
    ) -> Result<(Option<Reminder>, Message), RequestError> {
        let (set_result, response) =
            match tz::get_user_timezone(self.db, self.user_id).await {
                Ok(Some(user_timezone)) => {
                    match self.db.get_reminder(rem_id).await {
                        Ok(Some(old_reminder)) => {
                            match self.set_reminder_silently(text).await {
                                Some(Reminder::Reminder(new_reminder)) => {
                                    match self.db.delete_reminder(rem_id).await
                                    {
                                        Ok(()) => {
                                            let new_reminder_str = new_reminder
                                                .to_unescaped_string(
                                                    user_timezone,
                                                );
                                            (
                                                Some(Reminder::Reminder(
                                                    new_reminder,
                                                )),
                                                TgResponse::SuccessEdit(
                                                    old_reminder
                                                        .into_active_model()
                                                        .to_unescaped_string(
                                                            user_timezone,
                                                        ),
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
                    }
                }
                _ => (None, TgResponse::NoChosenTimezone),
            };

        self.reply(response).await.map(|msg| (set_result, msg))
    }

    pub async fn replace_cron_reminder(
        &self,
        text: &str,
        cron_rem_id: i64,
    ) -> Result<(Option<Reminder>, Message), RequestError> {
        let (set_result, response) =
            match tz::get_user_timezone(self.db, self.user_id).await {
                Ok(Some(user_timezone)) => {
                    match self.db.get_cron_reminder(cron_rem_id).await {
                        Ok(Some(old_cron_reminder)) => {
                            match self.set_reminder_silently(text).await {
                                Some(Reminder::CronReminder(
                                    new_cron_reminder,
                                )) => {
                                    match self
                                        .db
                                        .delete_cron_reminder(cron_rem_id)
                                        .await
                                    {
                                        Ok(()) => {
                                            let new_cron_reminder_str =
                                                new_cron_reminder
                                                    .to_unescaped_string(
                                                        user_timezone,
                                                    );
                                            (
                                                Some(Reminder::CronReminder(
                                                    new_cron_reminder,
                                                )),
                                                TgResponse::SuccessEdit(
                                                    old_cron_reminder
                                                        .into_active_model()
                                                        .to_unescaped_string(
                                                            user_timezone,
                                                        ),
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
                            log::error!(
                                "missing cron reminder with id: {}",
                                cron_rem_id
                            );
                            (None, TgResponse::FailedEdit)
                        }
                    }
                }
                _ => (None, TgResponse::NoChosenTimezone),
            };

        self.reply(response).await.map(|msg| (set_result, msg))
    }

    pub async fn set_or_edit_reminder(
        &self,
        text: &str,
    ) -> Result<(Option<Reminder>, Option<Message>), Error> {
        let (reminder, old_reply_id, reply) = match (
            self.get_edit_reminder().await?,
            self.get_edit_cron_reminder().await?,
        ) {
            (Some(old_reminder), _) => match old_reminder.edit_mode {
                EditMode::TimePattern => self
                    .replace_reminder(
                        &(text.to_owned() + " " + &old_reminder.desc),
                        old_reminder.id,
                    )
                    .await
                    .map(|(set_result, msg)| {
                        (set_result, old_reminder.reply_id, Some(msg))
                    }),
                EditMode::Description => {
                    let (set_result, response) = match tz::get_user_timezone(
                        self.db,
                        self.user_id,
                    )
                    .await
                    {
                        Ok(Some(user_timezone)) => {
                            let mut new_reminder = old_reminder.clone();
                            text.clone_into(&mut new_reminder.desc);
                            dbg!(&new_reminder);
                            match self
                                .db
                                .update_edited_reminder_description(
                                    new_reminder.clone(),
                                )
                                .await
                            {
                                Ok(()) => (
                                    Some(Reminder::Reminder(
                                        new_reminder
                                            .clone()
                                            .into_active_model(),
                                    )),
                                    TgResponse::SuccessEdit(
                                        old_reminder
                                            .clone()
                                            .into_active_model()
                                            .to_unescaped_string(user_timezone),
                                        new_reminder
                                            .into_active_model()
                                            .to_unescaped_string(user_timezone),
                                    ),
                                ),
                                Err(_) => (None, TgResponse::FailedEdit),
                            }
                        }
                        _ => (None, TgResponse::FailedEdit),
                    };
                    self.reply(response).await.map(|msg| {
                        (set_result, old_reminder.reply_id, Some(msg))
                    })
                }
                EditMode::None => self
                    .reply(TgResponse::FailedEdit)
                    .await
                    .map(|msg| (None, None, Some(msg))),
            },
            (_, Some(old_cron_reminder)) => self
                .replace_cron_reminder(text, old_cron_reminder.id)
                .await
                .map(|(set_result, msg)| {
                    (set_result, old_cron_reminder.reply_id, Some(msg))
                }),
            _ => self
                .set_reminder(text)
                .await
                .map(|(rem, msg)| (rem, None, msg)),
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

        Ok((reminder, reply))
    }

    pub async fn update_reply_link(
        &self,
        reminder: &Reminder,
        reply: &Message,
        old_reply_id: Option<MessageId>,
    ) -> Result<(), Error> {
        if let Some(old_reply_id) = old_reply_id {
            tg::delete_message(self.bot, self.chat_id, old_reply_id).await?;
        }
        match reminder {
            Reminder::Reminder(ref reminder) => {
                self.link_reminder_with_reply_msg(reminder.clone(), reply)
                    .await
            }
            Reminder::CronReminder(ref cron_reminder) => {
                self.link_cron_reminder_with_reply_msg(
                    cron_reminder.clone(),
                    reply,
                )
                .await
            }
        }
    }

    pub async fn get_edit_reminder(
        &self,
    ) -> Result<Option<reminder::Model>, db::Error> {
        self.db.get_edit_reminder(self.chat_id.0).await
    }

    pub async fn get_edit_cron_reminder(
        &self,
    ) -> Result<Option<cron_reminder::Model>, db::Error> {
        self.db.get_edit_cron_reminder(self.chat_id.0).await
    }

    pub async fn set_timezone(
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

    pub async fn edit_reminder_from_edited_message(
        &self,
        text: &str,
    ) -> Result<(), Error> {
        let (reminder, old_reply_id, reply) = match self
            .db
            .get_reminder_by_msg_id(self.msg_id.0)
            .await?
        {
            Some(old_rem) => self
                .replace_reminder(text, old_rem.id)
                .await
                .map(|(rem, msg)| (rem, old_rem.reply_id, Some(msg))),
            None => {
                match self.db.get_cron_reminder_by_msg_id(self.msg_id.0).await?
                {
                    Some(old_cron_rem) => self
                        .replace_cron_reminder(text, old_cron_rem.id)
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

impl TgCallbackController<'_> {
    async fn answer_callback_query(
        &self,
        response: TgResponse,
    ) -> Result<(), RequestError> {
        self.msg_ctl.reply(response).await?;
        self.acknowledge_callback().await
    }

    async fn acknowledge_callback(&self) -> Result<(), RequestError> {
        self.msg_ctl
            .bot
            .answer_callback_query(self.cb_id)
            .send()
            .await
            .map(|_| ())
    }

    pub async fn set_timezone(
        &self,
        tz_name: &str,
    ) -> Result<(), RequestError> {
        self.msg_ctl.set_timezone(tz_name).await?;
        self.acknowledge_callback().await
    }

    pub async fn delete_reminder(
        &self,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        let response =
            match tz::get_user_timezone(self.msg_ctl.db, self.msg_ctl.user_id)
                .await
            {
                Ok(Some(user_timezone)) => {
                    match self.msg_ctl.db.get_reminder(rem_id).await {
                        Ok(Some(reminder)) => {
                            match self.msg_ctl.db.delete_reminder(rem_id).await
                            {
                                Ok(()) => TgResponse::SuccessDelete(
                                    reminder
                                        .into_active_model()
                                        .to_unescaped_string(user_timezone),
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
                    }
                }
                _ => TgResponse::NoChosenTimezone,
            };
        self.msg_ctl.delete_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }

    pub async fn delete_cron_reminder(
        &self,
        cron_rem_id: i64,
    ) -> Result<(), RequestError> {
        let response =
            match tz::get_user_timezone(self.msg_ctl.db, self.msg_ctl.user_id)
                .await
            {
                Ok(Some(user_timezone)) => {
                    match self.msg_ctl.db.get_cron_reminder(cron_rem_id).await {
                        Ok(Some(cron_reminder)) => {
                            match self
                                .msg_ctl
                                .db
                                .delete_cron_reminder(cron_rem_id)
                                .await
                            {
                                Ok(()) => TgResponse::SuccessDelete(
                                    cron_reminder
                                        .into_active_model()
                                        .to_unescaped_string(user_timezone),
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
                            log::error!(
                                "missing cron reminder with id: {}",
                                cron_rem_id
                            );
                            TgResponse::FailedDelete
                        }
                    }
                }
                _ => TgResponse::NoChosenTimezone,
            };
        self.msg_ctl.delete_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }

    pub async fn choose_edit_mode_reminder(
        &self,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        match self
            .msg_ctl
            .db
            .mark_reminder_as_edit(rem_id, self.msg_ctl.chat_id.0)
            .await
        {
            Ok(()) => {
                let markup = InlineKeyboardMarkup::default().append_row(vec![
                    InlineKeyboardButton::new(
                        "Time pattern",
                        InlineKeyboardButtonKind::CallbackData(
                            "edit_rem_mode::rem_time_pattern".to_owned(),
                        ),
                    ),
                    InlineKeyboardButton::new(
                        "Description",
                        InlineKeyboardButtonKind::CallbackData(
                            "edit_rem_mode::rem_description".to_owned(),
                        ),
                    ),
                ]);
                tg::send_markup(
                    "What would you like to edit?",
                    markup,
                    self.msg_ctl.bot,
                    self.msg_ctl.chat_id,
                )
                .await?;
                self.acknowledge_callback().await
            }
            Err(err) => {
                log::error!("{}", err);
                self.answer_callback_query(TgResponse::FailedEdit).await
            }
        }
    }

    pub async fn edit_cron_reminder(
        &self,
        cron_rem_id: i64,
    ) -> Result<(), RequestError> {
        let response = match self
            .msg_ctl
            .db
            .mark_cron_reminder_as_edit(cron_rem_id, self.msg_ctl.chat_id.0)
            .await
        {
            Ok(()) => TgResponse::EnterNewReminder,
            Err(err) => {
                log::error!("{}", err);
                TgResponse::FailedEdit
            }
        };
        self.answer_callback_query(response).await
    }

    pub async fn pause_reminder(
        &self,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        let response =
            match tz::get_user_timezone(self.msg_ctl.db, self.msg_ctl.user_id)
                .await
            {
                Ok(Some(user_timezone)) => {
                    match self.msg_ctl.db.get_reminder(rem_id).await {
                        Ok(Some(reminder)) => {
                            match self
                                .msg_ctl
                                .db
                                .toggle_reminder_paused(rem_id)
                                .await
                            {
                                Ok(true) => TgResponse::SuccessPause(
                                    reminder
                                        .into_active_model()
                                        .to_unescaped_string(user_timezone),
                                ),
                                Ok(false) => TgResponse::SuccessResume(
                                    reminder
                                        .into_active_model()
                                        .to_unescaped_string(user_timezone),
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
                    }
                }
                _ => TgResponse::NoChosenTimezone,
            };
        self.msg_ctl.pause_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }

    pub async fn pause_cron_reminder(
        &self,
        cron_rem_id: i64,
    ) -> Result<(), RequestError> {
        let response =
            match tz::get_user_timezone(self.msg_ctl.db, self.msg_ctl.user_id)
                .await
            {
                Ok(Some(user_timezone)) => {
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
                                        .to_unescaped_string(user_timezone),
                                ),
                                Ok(false) => TgResponse::SuccessResume(
                                    cron_reminder
                                        .into_active_model()
                                        .to_unescaped_string(user_timezone),
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
                    }
                }
                _ => TgResponse::NoChosenTimezone,
            };
        self.msg_ctl.pause_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }

    pub async fn set_edit_mode_reminder(
        &self,
        edit_mode: EditMode,
    ) -> Result<(), RequestError> {
        let response = match self
            .msg_ctl
            .db
            .set_edit_mode_reminder(self.msg_ctl.chat_id.0, edit_mode)
            .await
        {
            Ok(()) => match edit_mode {
                EditMode::TimePattern => TgResponse::EnterNewTimePattern,
                EditMode::Description => TgResponse::EnterNewDescription,
                EditMode::None => TgResponse::FailedEdit,
            },
            Err(_) => TgResponse::FailedEdit,
        };
        self.answer_callback_query(response).await
    }

    pub async fn set_edit_mode_cron_reminder(
        &self,
        edit_mode: EditMode,
    ) -> Result<(), RequestError> {
        match self
            .msg_ctl
            .db
            .set_edit_mode_cron_reminder(self.msg_ctl.chat_id.0, edit_mode)
            .await
        {
            Ok(()) => self.acknowledge_callback().await,
            Err(_) => self.answer_callback_query(TgResponse::FailedEdit).await,
        }
    }
}
