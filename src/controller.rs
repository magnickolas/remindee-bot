use crate::db;
use crate::parsers;
use crate::tg;
use crate::tz;

use crate::entity::{cron_reminder, reminder};
use crate::generic_reminder::GenericReminder;
use chrono_tz::Tz;
use teloxide::prelude::*;
use teloxide::types::MessageId;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
};
use teloxide::RequestError;
use tg::TgResponse;

impl db::Database {
    async fn get_sorted_reminders(
        &self,
        chat_id: i64,
        exclude_reminders: bool,
        exclude_cron_reminders: bool,
    ) -> Result<Vec<Box<dyn GenericReminder>>, db::Error> {
        let reminders_future = self.get_pending_chat_reminders(chat_id).await;
        let cron_reminders_future =
            self.get_pending_chat_cron_reminders(chat_id).await;
        let gen_rems = if exclude_reminders {
            Ok(vec![])
        } else {
            reminders_future.map(|mut v| {
                v.drain(..)
                    .map(|x| -> Box<dyn GenericReminder> {
                        Box::<reminder::ActiveModel>::new(x.into())
                    })
                    .collect::<Vec<_>>()
            })
        };
        let gen_cron_rems = if exclude_cron_reminders {
            Ok(vec![])
        } else {
            cron_reminders_future.map(|mut v| {
                v.drain(..)
                    .map(|x| -> Box<dyn GenericReminder> {
                        Box::<cron_reminder::ActiveModel>::new(x.into())
                    })
                    .collect::<Vec<_>>()
            })
        };
        gen_rems
            .and_then(|mut rems| {
                gen_cron_rems.map(|mut cron_rems| {
                    rems.append(cron_rems.as_mut());
                    rems
                })
            })
            .map(|mut rems| {
                rems.sort();
                rems
            })
    }

    async fn get_sorted_all_reminders(
        &self,
        chat_id: i64,
    ) -> Result<Vec<Box<dyn GenericReminder>>, db::Error> {
        self.get_sorted_reminders(chat_id, false, false).await
    }
}

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

impl TgMessageController<'_> {
    pub async fn reply<R: ToString>(
        &self,
        response: R,
    ) -> Result<(), RequestError> {
        tg::send_silent_message(&response.to_string(), self.bot, self.chat_id)
            .await
    }

    pub async fn start(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::Hello).await
    }

    /// Send a list of all notifications
    pub async fn list(&self) -> Result<(), RequestError> {
        // Format reminders
        let text = match tz::get_user_timezone(self.db, self.user_id).await {
            Ok(Some(user_timezone)) => {
                match self.db.get_sorted_all_reminders(self.chat_id.0).await {
                    Ok(sorted_reminders) => {
                        vec![TgResponse::RemindersListHeader.to_string()]
                            .into_iter()
                            .chain(
                                sorted_reminders
                                    .into_iter()
                                    .map(|rem| rem.to_string(user_timezone)),
                            )
                            .collect::<Vec<String>>()
                            .join("\n")
                    }
                    Err(err) => {
                        dbg!(err);
                        TgResponse::QueryingError.to_string()
                    }
                }
            }
            _ => TgResponse::NoChosenTimezone.to_string(),
        };
        self.reply(&text).await
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
                    dbg!(err);
                    TgResponse::NoChosenTimezone
                }
            };
        self.reply(response).await
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
            _ => self.reply(TgResponse::NoChosenTimezone).await,
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
            _ => self.reply(TgResponse::NoChosenTimezone).await,
        }
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
            _ => self.reply(TgResponse::NoChosenTimezone).await,
        }
    }

    /// Try to parse user's message into a one-time or periodic reminder and set it
    pub async fn set_reminder(
        &self,
        text: &str,
        silent_success: bool,
    ) -> Result<bool, RequestError> {
        let user_id_raw = self.user_id.0;
        match tz::get_user_timezone(self.db, self.user_id).await {
            Ok(Some(user_timezone)) => {
                if let Some(cron_reminder) = parsers::parse_cron_reminder(
                    text,
                    self.chat_id.0,
                    user_id_raw,
                    user_timezone,
                )
                .await
                {
                    match self
                        .db
                        .insert_cron_reminder(cron_reminder.clone())
                        .await
                    {
                        Ok(()) => {
                            if !silent_success {
                                let rem_str = cron_reminder
                                    .to_unescaped_string(user_timezone);
                                self.reply(TgResponse::SuccessPeriodicInsert(
                                    rem_str,
                                ))
                                .await?;
                            };
                            Ok(true)
                        }
                        Err(err) => {
                            dbg!(err);
                            self.reply(TgResponse::FailedInsert).await?;
                            Ok(false)
                        }
                    }
                } else if let Some(reminder) = parsers::parse_reminder(
                    text,
                    self.chat_id.0,
                    user_id_raw,
                    user_timezone,
                )
                .await
                {
                    match self.db.insert_reminder(reminder.clone()).await {
                        Ok(()) => {
                            if !silent_success {
                                let rem_str =
                                    reminder.to_unescaped_string(user_timezone);
                                self.reply(TgResponse::SuccessInsert(rem_str))
                                    .await?;
                            }
                            Ok(true)
                        }
                        Err(err) => {
                            dbg!(err);
                            self.reply(TgResponse::FailedInsert).await?;
                            Ok(false)
                        }
                    }
                } else if user_id_raw == self.chat_id.0 as u64 {
                    self.reply(TgResponse::IncorrectRequest).await?;
                    Ok(false)
                } else {
                    Ok(false)
                }
            }
            _ => {
                self.reply(TgResponse::NoChosenTimezone).await?;
                Ok(false)
            }
        }
    }

    pub async fn incorrect_request(&self) -> Result<(), RequestError> {
        self.reply(TgResponse::IncorrectRequest).await
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
            self.reply(TgResponse::NoChosenTimezone).await
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
            self.reply(TgResponse::NoChosenTimezone).await
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
            self.reply(TgResponse::NoChosenTimezone).await
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
            .and_then(|rems| rems.chunks(45).into_iter().nth(num))
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
    ) -> Result<(), RequestError> {
        if self.set_reminder(text, true).await? {
            let response = match self.db.delete_reminder(rem_id).await {
                Ok(()) => TgResponse::SuccessEdit,
                Err(err) => {
                    dbg!(err);
                    TgResponse::FailedEdit
                }
            };
            self.reply(response).await
        } else {
            self.reply(TgResponse::FailedEdit).await
        }
    }

    pub async fn replace_cron_reminder(
        &self,
        text: &str,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        if self.set_reminder(text, true).await? {
            let response = match self.db.delete_cron_reminder(rem_id).await {
                Ok(()) => TgResponse::SuccessEdit,
                Err(err) => {
                    dbg!(err);
                    TgResponse::FailedEdit
                }
            };
            self.reply(response).await
        } else {
            self.reply(TgResponse::FailedEdit).await
        }
    }

    pub async fn set_or_edit_reminder(
        &self,
        text: &str,
    ) -> Result<(), RequestError> {
        match (
            self.get_edit_reminder().await,
            self.get_edit_cron_reminder().await,
        ) {
            (Ok(Some(edit_reminder)), _) => {
                self.replace_reminder(text, edit_reminder.id).await
            }
            (_, Ok(Some(edit_cron_reminder))) => {
                self.replace_cron_reminder(text, edit_cron_reminder.id)
                    .await
            }
            _ => self.set_reminder(text, false).await.map(|_| ()),
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
}

impl TgCallbackController<'_> {
    async fn answer_callback_query(
        &self,
        response: TgResponse,
    ) -> Result<(), RequestError> {
        tg::answer_callback_query(
            self.msg_ctl.bot,
            self.cb_id,
            &response.to_unescaped_string(),
        )
        .await
    }

    pub async fn set_timezone(
        &self,
        tz_name: &str,
    ) -> Result<(), RequestError> {
        let response = match self
            .msg_ctl
            .db
            .insert_or_update_user_timezone(
                self.msg_ctl.user_id.0 as i64,
                tz_name,
            )
            .await
        {
            Ok(()) => TgResponse::ChosenTimezone(tz_name.to_owned()),
            Err(err) => {
                dbg!(err);
                TgResponse::FailedSetTimezone(tz_name.to_owned())
            }
        };
        self.answer_callback_query(response).await
    }

    pub async fn delete_reminder(
        &self,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        let response = match self.msg_ctl.db.delete_reminder(rem_id).await {
            Ok(()) => TgResponse::SuccessDelete,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedDelete
            }
        };
        self.msg_ctl.delete_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }

    pub async fn delete_cron_reminder(
        &self,
        cron_rem_id: i64,
    ) -> Result<(), RequestError> {
        let response =
            match self.msg_ctl.db.delete_cron_reminder(cron_rem_id).await {
                Ok(()) => TgResponse::SuccessDelete,
                Err(err) => {
                    dbg!(err);
                    TgResponse::FailedDelete
                }
            };
        self.msg_ctl.delete_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }

    pub async fn edit_reminder(&self, rem_id: i64) -> Result<(), RequestError> {
        let response = match self
            .msg_ctl
            .db
            .reset_reminders_edit(self.msg_ctl.chat_id.0)
            .await
            .and(self.msg_ctl.db.mark_reminder_as_edit(rem_id).await)
        {
            Ok(()) => TgResponse::EnterNewReminder,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedEdit
            }
        };
        self.answer_callback_query(response).await
    }

    pub async fn edit_cron_reminder(
        &self,
        cron_rem_id: i64,
    ) -> Result<(), RequestError> {
        let response = match self
            .msg_ctl
            .db
            .reset_cron_reminders_edit(self.msg_ctl.chat_id.0)
            .await
            .and(
                self.msg_ctl
                    .db
                    .mark_cron_reminder_as_edit(cron_rem_id)
                    .await,
            ) {
            Ok(()) => TgResponse::EnterNewReminder,
            Err(err) => {
                dbg!(err);
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
            match self.msg_ctl.db.toggle_reminder_paused(rem_id).await {
                Ok(true) => TgResponse::SuccessPause,
                Ok(false) => TgResponse::SuccessResume,
                Err(err) => {
                    dbg!(err);
                    TgResponse::FailedPause
                }
            };
        self.msg_ctl.pause_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }

    pub async fn pause_cron_reminder(
        &self,
        cron_rem_id: i64,
    ) -> Result<(), RequestError> {
        let response = match self
            .msg_ctl
            .db
            .toggle_cron_reminder_paused(cron_rem_id)
            .await
        {
            Ok(true) => TgResponse::SuccessPause,
            Ok(false) => TgResponse::SuccessResume,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedPause
            }
        };
        self.msg_ctl.pause_reminder_set_page(0).await?;
        self.answer_callback_query(response).await
    }
}
