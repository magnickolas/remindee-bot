use crate::db;
use crate::parsers;
use crate::tg;
use crate::tz;

use crate::generic_trait::GenericReminder;
use chrono_tz::Tz;
use entity::{cron_reminder, reminder};
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
        user_id: i64,
    ) -> Result<Vec<Box<dyn GenericReminder>>, db::Error> {
        let reminders_future = self.get_pending_user_reminders(user_id).await;
        let cron_reminders_future =
            self.get_pending_user_cron_reminders(user_id).await;
        let gen_rems = reminders_future.map(|mut v| {
            v.drain(..)
                .map(|x| -> Box<dyn GenericReminder> {
                    Box::<reminder::ActiveModel>::new(x.into())
                })
                .collect::<Vec<_>>()
        });
        let gen_cron_rems = cron_reminders_future.map(|mut v| {
            v.drain(..)
                .map(|x| -> Box<dyn GenericReminder> {
                    Box::<cron_reminder::ActiveModel>::new(x.into())
                })
                .collect::<Vec<_>>()
        });
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
}

pub struct TgBot<'a> {
    pub database: &'a db::Database,
    pub bot: &'a Bot,
}

impl TgBot<'_> {
    async fn reply<R: ToString>(
        &self,
        response: R,
        user_id: ChatId,
    ) -> Result<(), RequestError> {
        tg::send_silent_message(&response.to_string(), self.bot, user_id).await
    }

    pub async fn start(&self, user_id: ChatId) -> Result<(), RequestError> {
        self.reply(TgResponse::Hello, user_id).await
    }

    /// Send a list of all notifications
    pub async fn list(&mut self, user_id: ChatId) -> Result<(), RequestError> {
        // Format reminders
        let text = match self.database.get_user_timezone(user_id.0).await {
            Ok(Some(user_timezone)) => {
                match self.database.get_sorted_reminders(user_id.0).await {
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
        self.reply(&text, user_id).await
    }

    /// Send a markup with all timezones to select
    pub async fn choose_timezone(
        &self,
        user_id: ChatId,
    ) -> Result<(), RequestError> {
        tg::send_markup(
            &TgResponse::SelectTimezone.to_string(),
            self.get_markup_for_tz_page_idx(0),
            self.bot,
            user_id,
        )
        .await
    }

    /// Send user's timezone
    pub async fn get_timezone(
        &mut self,
        user_id: ChatId,
    ) -> Result<(), RequestError> {
        let response =
            match self.database.get_user_timezone_name(user_id.0).await {
                Ok(Some(tz_name)) => TgResponse::ChosenTimezone(tz_name),
                Ok(None) => TgResponse::NoChosenTimezone,
                Err(err) => {
                    dbg!(err);
                    TgResponse::NoChosenTimezone
                }
            };
        self.reply(response, user_id).await
    }

    /// General way to send a markup to select a reminder for some operation
    async fn start_alter(
        &self,
        user_id: ChatId,
        response: TgResponse,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        tg::send_markup(&response.to_string(), markup, self.bot, user_id).await
    }

    /// Send a markup to select a reminder for deleting
    pub async fn start_delete(
        &mut self,
        user_id: ChatId,
    ) -> Result<(), RequestError> {
        match self.database.get_user_timezone(user_id.0).await {
            Ok(Some(user_timezone)) => {
                let markup = self
                    .get_markup_for_reminders_page_deletion(
                        0,
                        user_id,
                        user_timezone,
                    )
                    .await;
                self.start_alter(
                    user_id,
                    TgResponse::ChooseDeleteReminder,
                    markup,
                )
                .await
            }
            _ => self.reply(TgResponse::NoChosenTimezone, user_id).await,
        }
    }

    /// Send a markup to select a reminder for editing
    pub async fn start_edit(
        &mut self,
        user_id: ChatId,
    ) -> Result<(), RequestError> {
        match self.database.get_user_timezone(user_id.0).await {
            Ok(Some(user_timezone)) => {
                let markup = self
                    .get_markup_for_reminders_page_editing(
                        0,
                        user_id,
                        user_timezone,
                    )
                    .await;
                self.start_alter(
                    user_id,
                    TgResponse::ChooseEditReminder,
                    markup,
                )
                .await
            }
            _ => self.reply(TgResponse::NoChosenTimezone, user_id).await,
        }
    }

    /// Send a list of supported commands
    pub async fn list_commands(
        &self,
        user_id: ChatId,
    ) -> Result<(), RequestError> {
        self.reply(TgResponse::CommandsHelp.to_string(), user_id)
            .await
    }

    /// Try to parse user's message into a one-time or periodic reminder and set it
    pub async fn set_reminder(
        &mut self,
        text: &str,
        user_id: ChatId,
        from_id_opt: Option<UserId>,
        silent_success: bool,
    ) -> Result<bool, RequestError> {
        match self.database.get_user_timezone(user_id.0).await {
            Ok(Some(user_timezone)) => {
                if let Some(cron_reminder) =
                    parsers::parse_cron_reminder(text, user_id.0, user_timezone)
                        .await
                {
                    match self
                        .database
                        .insert_cron_reminder(cron_reminder.clone())
                        .await
                    {
                        Ok(_) => {
                            if !silent_success {
                                let rem_str = cron_reminder
                                    .to_unescaped_string(user_timezone);
                                self.reply(
                                    TgResponse::SuccessPeriodicInsert(rem_str),
                                    user_id,
                                )
                                .await?;
                            };
                            Ok(true)
                        }
                        Err(err) => {
                            dbg!(err);
                            self.reply(TgResponse::FailedInsert, user_id)
                                .await?;
                            Ok(false)
                        }
                    }
                } else if let Some(reminder) =
                    parsers::parse_reminder(text, user_id.0, user_timezone)
                        .await
                {
                    match self.database.insert_reminder(reminder.clone()).await
                    {
                        Ok(_) => {
                            if !silent_success {
                                let rem_str =
                                    reminder.to_unescaped_string(user_timezone);
                                self.reply(
                                    TgResponse::SuccessInsert(rem_str),
                                    user_id,
                                )
                                .await?;
                            }
                            Ok(true)
                        }
                        Err(err) => {
                            dbg!(err);
                            self.reply(TgResponse::FailedInsert, user_id)
                                .await?;
                            Ok(false)
                        }
                    }
                } else if from_id_opt
                    .filter(|&from_id| from_id.0 == user_id.0 as u64)
                    .is_some()
                {
                    self.reply(TgResponse::IncorrectRequest, user_id).await?;
                    Ok(false)
                } else {
                    Ok(false)
                }
            }
            _ => {
                self.reply(TgResponse::NoChosenTimezone, user_id).await?;
                Ok(false)
            }
        }
    }

    pub async fn incorrect_request(
        &mut self,
        user_id: ChatId,
    ) -> Result<(), RequestError> {
        self.reply(TgResponse::IncorrectRequest, user_id).await
    }

    /// Switch the markup's page
    pub async fn select_timezone_set_page(
        &mut self,
        user_id: ChatId,
        page_num: usize,
        msg_id: MessageId,
    ) -> Result<(), RequestError> {
        tg::edit_markup(
            self.get_markup_for_tz_page_idx(page_num),
            self.bot,
            msg_id,
            user_id,
        )
        .await
    }

    pub async fn set_timezone(
        &mut self,
        user_id: ChatId,
        tz_name: &str,
    ) -> Result<(), RequestError> {
        let response = match self
            .database
            .set_user_timezone_name(user_id.0, tz_name)
            .await
        {
            Ok(_) => TgResponse::ChosenTimezone(tz_name.to_owned()),
            Err(err) => {
                dbg!(err);
                TgResponse::FailedSetTimezone(tz_name.to_owned())
            }
        };
        self.reply(response, user_id).await
    }

    async fn alter_reminder_set_page(
        &mut self,
        user_id: ChatId,
        msg_id: MessageId,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        tg::edit_markup(markup, self.bot, msg_id, user_id).await
    }

    pub async fn delete_reminder_set_page(
        &mut self,
        user_id: ChatId,
        page_num: usize,
        msg_id: MessageId,
    ) -> Result<(), RequestError> {
        if let Ok(Some(user_timezone)) =
            self.database.get_user_timezone(user_id.0).await
        {
            let markup = self
                .get_markup_for_reminders_page_deletion(
                    page_num,
                    user_id,
                    user_timezone,
                )
                .await;
            self.alter_reminder_set_page(user_id, msg_id, markup).await
        } else {
            self.reply(TgResponse::NoChosenTimezone, user_id).await
        }
    }

    pub async fn edit_reminder_set_page(
        &mut self,
        user_id: ChatId,
        page_num: usize,
        msg_id: MessageId,
    ) -> Result<(), RequestError> {
        if let Ok(Some(user_timezone)) =
            self.database.get_user_timezone(user_id.0).await
        {
            let markup = self
                .get_markup_for_reminders_page_editing(
                    page_num,
                    user_id,
                    user_timezone,
                )
                .await;
            self.alter_reminder_set_page(user_id, msg_id, markup).await
        } else {
            self.reply(TgResponse::NoChosenTimezone, user_id).await
        }
    }

    pub async fn delete_reminder(
        &mut self,
        user_id: ChatId,
        rem_id: i64,
        msg_id: MessageId,
    ) -> Result<(), RequestError> {
        let response = match self.database.mark_reminder_as_sent(rem_id).await {
            Ok(_) => TgResponse::SuccessDelete,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedDelete
            }
        };
        self.delete_reminder_set_page(user_id, 0, msg_id).await?;
        self.reply(response, user_id).await
    }

    pub async fn delete_cron_reminder(
        &mut self,
        user_id: ChatId,
        cron_rem_id: i64,
        msg_id: MessageId,
    ) -> Result<(), RequestError> {
        let response =
            match self.database.mark_cron_reminder_as_sent(cron_rem_id).await {
                Ok(_) => TgResponse::SuccessDelete,
                Err(err) => {
                    dbg!(err);
                    TgResponse::FailedDelete
                }
            };
        self.delete_reminder_set_page(user_id, 0, msg_id).await?;
        self.reply(response, user_id).await
    }

    pub async fn edit_reminder(
        &mut self,
        user_id: ChatId,
        rem_id: i64,
    ) -> Result<(), RequestError> {
        let response = match self
            .database
            .reset_reminders_edit(user_id.0)
            .await
            .and(self.database.mark_reminder_as_edit(rem_id).await)
        {
            Ok(_) => TgResponse::EnterNewReminder,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedEdit
            }
        };
        self.reply(response, user_id).await
    }

    pub async fn edit_cron_reminder(
        &mut self,
        user_id: ChatId,
        cron_rem_id: i64,
    ) -> Result<(), RequestError> {
        let response = match self
            .database
            .reset_cron_reminders_edit(user_id.0)
            .await
            .and(self.database.mark_cron_reminder_as_edit(cron_rem_id).await)
        {
            Ok(_) => TgResponse::EnterNewReminder,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedEdit
            }
        };
        self.reply(response, user_id).await
    }

    pub async fn replace_reminder(
        &mut self,
        text: &str,
        user_id: ChatId,
        rem_id: i64,
        from_id_opt: Option<UserId>,
    ) -> Result<(), RequestError> {
        if self.set_reminder(text, user_id, from_id_opt, true).await? {
            let response =
                match self.database.mark_reminder_as_sent(rem_id).await {
                    Ok(_) => TgResponse::SuccessEdit,
                    Err(err) => {
                        dbg!(err);
                        TgResponse::FailedEdit
                    }
                };
            self.reply(response, user_id).await
        } else {
            self.reply(TgResponse::FailedEdit, user_id).await
        }
    }

    pub async fn replace_cron_reminder(
        &mut self,
        text: &str,
        user_id: ChatId,
        rem_id: i64,
        from_id_opt: Option<UserId>,
    ) -> Result<(), RequestError> {
        if self.set_reminder(text, user_id, from_id_opt, true).await? {
            let response =
                match self.database.mark_cron_reminder_as_sent(rem_id).await {
                    Ok(_) => TgResponse::SuccessEdit,
                    Err(err) => {
                        dbg!(err);
                        TgResponse::FailedEdit
                    }
                };
            self.reply(response, user_id).await
        } else {
            self.reply(TgResponse::FailedEdit, user_id).await
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
        &mut self,
        num: usize,
        user_id: ChatId,
        cb_prefix: &str,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::default();
        let mut last_rem_page: bool = false;
        let sorted_reminders =
            self.database.get_sorted_reminders(user_id.0).await;
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
        &mut self,
        num: usize,
        user_id: ChatId,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            user_id,
            "delrem",
            user_timezone,
        )
        .await
    }

    pub async fn get_markup_for_reminders_page_editing(
        &mut self,
        num: usize,
        user_id: ChatId,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        self.get_markup_for_reminders_page_alteration(
            num,
            user_id,
            "editrem",
            user_timezone,
        )
        .await
    }

    pub async fn get_edit_reminder(
        &mut self,
        user_id: ChatId,
    ) -> Result<Option<reminder::Model>, db::Error> {
        self.database.get_edit_reminder(user_id.0).await
    }

    pub async fn get_edit_cron_reminder(
        &mut self,
        user_id: ChatId,
    ) -> Result<Option<cron_reminder::Model>, db::Error> {
        self.database.get_edit_cron_reminder(user_id.0).await
    }
}
