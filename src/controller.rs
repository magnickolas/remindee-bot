use crate::db;
use crate::err;
use crate::tg;
use crate::tz;

use chrono::Utc;
use chrono_tz::Tz;
use cron_parser::parse as parse_cron;
use itertools::Itertools;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
};
use teloxide::RequestError;
use tg::TgResponse;

impl db::Database {
    async fn get_sorted_reminders(
        &mut self,
        user_id: i64,
    ) -> Result<std::vec::IntoIter<Box<dyn tg::GenericReminder>>, db::Error>
    {
        let reminders_future = self.get_pending_user_reminders(user_id).await;
        let cron_reminders_future =
            self.get_pending_user_cron_reminders(user_id).await;
        let gen_rems = reminders_future.map(|v| {
            v.into_iter()
                .map::<Box<dyn tg::GenericReminder>, _>(|x| Box::new(x))
        });
        let gen_cron_rems = cron_reminders_future.map(|v| {
            v.into_iter()
                .map::<Box<dyn tg::GenericReminder>, _>(|x| Box::new(x))
        });
        gen_rems
            .and_then(|rems| {
                gen_cron_rems.map(|cron_rems| rems.chain(cron_rems))
            })
            .map(|rems| rems.sorted())
    }
}

pub struct TgBot<'a> {
    pub database: &'a mut db::Database,
    pub bot: &'a Bot,
}

impl TgBot<'_> {
    pub async fn start(&self, user_id: i64) -> Result<(), RequestError> {
        tg::send_message(&TgResponse::Hello.to_string(), self.bot, user_id)
            .await
    }

    /// Send a list of all notifications
    pub async fn list(&mut self, user_id: i64) -> Result<(), RequestError> {
        // Format reminders
        let text = match self.database.get_user_timezone(user_id).await {
            Ok(Some(user_timezone)) => {
                match self.database.get_sorted_reminders(user_id).await {
                    Ok(sorted_reminders) => {
                        vec![TgResponse::RemindersListHeader.to_string()]
                            .into_iter()
                            .chain(
                                sorted_reminders
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
        tg::send_message(&text, self.bot, user_id).await
    }

    /// Send a markup with all timezones to select
    pub async fn choose_timezone(
        &self,
        user_id: i64,
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
        user_id: i64,
    ) -> Result<(), RequestError> {
        let response = match self.database.get_user_timezone_name(user_id).await
        {
            Ok(Some(tz_name)) => TgResponse::ChosenTimezone(tz_name),
            Ok(None) => TgResponse::NoChosenTimezone,
            Err(err) => {
                dbg!(err);
                TgResponse::NoChosenTimezone
            }
        };
        tg::send_message(&response.to_string(), self.bot, user_id).await
    }

    /// General way to send a markup to select a reminder for some operation
    async fn start_alter(
        &self,
        user_id: i64,
        response: TgResponse,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        tg::send_markup(&response.to_string(), markup, self.bot, user_id).await
    }

    /// Send a markup to select a reminder for deleting
    pub async fn start_delete(
        &mut self,
        user_id: i64,
    ) -> Result<(), RequestError> {
        match self.database.get_user_timezone(user_id).await {
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
            _ => {
                tg::send_message(
                    &TgResponse::NoChosenTimezone.to_string(),
                    self.bot,
                    user_id,
                )
                .await
            }
        }
    }

    /// Send a markup to select a reminder for editing
    pub async fn start_edit(
        &mut self,
        user_id: i64,
    ) -> Result<(), RequestError> {
        match self.database.get_user_timezone(user_id).await {
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
            _ => {
                tg::send_message(
                    &TgResponse::NoChosenTimezone.to_string(),
                    self.bot,
                    user_id,
                )
                .await
            }
        }
    }

    /// Send a list of supported commands
    pub async fn list_commands(
        &self,
        user_id: i64,
    ) -> Result<(), RequestError> {
        tg::send_message(
            &TgResponse::CommandsHelp.to_string(),
            self.bot,
            user_id,
        )
        .await
    }

    /// Try to parse user's message into a one-time or periodic reminder and set it
    pub async fn set_reminder(
        &mut self,
        text: &str,
        user_id: i64,
        from_id_opt: Option<i64>,
        silent_success: bool,
    ) -> Result<bool, RequestError> {
        match self.database.get_user_timezone(user_id).await {
            Ok(Some(user_timezone)) => {
                if let Some(reminder) =
                    tg::parse_req(text, user_id, user_timezone).await
                {
                    let mut is_ok = true;
                    let response =
                        match self.database.insert_reminder(&reminder).await {
                            Ok(_) => TgResponse::SuccessInsert,
                            Err(err) => {
                                is_ok = false;
                                dbg!(err);
                                TgResponse::FailedInsert
                            }
                        };

                    if !silent_success {
                        tg::send_message(
                            &response.to_string(),
                            self.bot,
                            user_id,
                        )
                        .await?
                    }
                    Ok(is_ok)
                } else if let Ok(Some((cron_expr, time))) = {
                    let cron_fields: Vec<&str> =
                        text.split_whitespace().take(5).collect();
                    if cron_fields.len() < 5 {
                        Err(err::Error::CronFewFields)
                    } else {
                        let cron_expr = cron_fields.join(" ");
                        self.database.get_user_timezone(user_id).await.and_then(
                            |timezone_opt| {
                                timezone_opt
                                    .map(|timezone| {
                                        let time = parse_cron(
                                            &cron_expr,
                                            &Utc::now()
                                                .with_timezone(&timezone),
                                        )?
                                        .with_timezone(&Utc);
                                        Ok((cron_expr, time))
                                    })
                                    .transpose()
                            },
                        )
                    }
                } {
                    let mut is_ok = true;
                    let response = match self
                        .database
                        .insert_cron_reminder(&db::CronReminderStruct {
                            id: 0,
                            user_id,
                            cron_expr: cron_expr.clone(),
                            time: time.naive_utc(),
                            desc: text
                                .strip_prefix(&(cron_expr.to_owned()))
                                .unwrap_or("")
                                .trim()
                                .to_owned(),
                            sent: false,
                            edit: false,
                        })
                        .await
                    {
                        Ok(_) => TgResponse::SuccessInsert,
                        Err(err) => {
                            is_ok = false;
                            dbg!(err);
                            TgResponse::FailedInsert
                        }
                    };
                    tg::send_message(&response.to_string(), self.bot, user_id)
                        .await?;
                    Ok(is_ok)
                } else if from_id_opt
                    .filter(|&from_id| from_id == user_id)
                    .is_some()
                {
                    tg::send_message(
                        &TgResponse::IncorrectRequest.to_string(),
                        self.bot,
                        user_id,
                    )
                    .await?;
                    Ok(false)
                } else {
                    Ok(false)
                }
            }
            _ => {
                tg::send_message(
                    &TgResponse::NoChosenTimezone.to_string(),
                    self.bot,
                    user_id,
                )
                .await?;
                Ok(false)
            }
        }
    }

    pub async fn incorrect_request(
        &mut self,
        user_id: i64,
    ) -> Result<(), RequestError> {
        tg::send_message(
            &TgResponse::IncorrectRequest.to_string(),
            self.bot,
            user_id,
        )
        .await
    }

    /// Switch the markup's page
    pub async fn select_timezone_set_page(
        &mut self,
        user_id: i64,
        page_num: usize,
        msg_id: i32,
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
        user_id: i64,
        tz_name: &str,
    ) -> Result<(), RequestError> {
        let response = match self
            .database
            .set_user_timezone_name(user_id, tz_name)
            .await
        {
            Ok(_) => TgResponse::ChosenTimezone(tz_name.to_owned()),
            Err(err) => {
                dbg!(err);
                TgResponse::FailedSetTimezone(tz_name.to_owned())
            }
        };
        tg::send_message(&response.to_string(), self.bot, user_id).await
    }

    async fn alter_reminder_set_page(
        &mut self,
        user_id: i64,
        msg_id: i32,
        markup: InlineKeyboardMarkup,
    ) -> Result<(), RequestError> {
        tg::edit_markup(markup, self.bot, msg_id, user_id).await
    }

    pub async fn delete_reminder_set_page(
        &mut self,
        user_id: i64,
        page_num: usize,
        msg_id: i32,
    ) -> Result<(), RequestError> {
        if let Ok(Some(user_timezone)) =
            self.database.get_user_timezone(user_id).await
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
            tg::send_message(&TgResponse::NoChosenTimezone.to_string(), self.bot, user_id).await
        }
    }

    pub async fn edit_reminder_set_page(
        &mut self,
        user_id: i64,
        page_num: usize,
        msg_id: i32,
    ) -> Result<(), RequestError> {
        if let Ok(Some(user_timezone)) =
            self.database.get_user_timezone(user_id).await
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
            tg::send_message(&TgResponse::NoChosenTimezone.to_string(), self.bot, user_id).await
        }
    }

    pub async fn delete_reminder(
        &mut self,
        user_id: i64,
        rem_id: u32,
        msg_id: i32,
    ) -> Result<(), RequestError> {
        let response = match self.database.mark_reminder_as_sent(rem_id).await {
            Ok(_) => TgResponse::SuccessDelete,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedDelete
            }
        };
        self.delete_reminder_set_page(user_id, 0, msg_id).await?;
        tg::send_message(&response.to_string(), self.bot, user_id).await
    }

    pub async fn delete_cron_reminder(
        &mut self,
        user_id: i64,
        cron_rem_id: u32,
        msg_id: i32,
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
        tg::send_message(&response.to_string(), self.bot, user_id).await
    }

    pub async fn edit_reminder(
        &mut self,
        user_id: i64,
        rem_id: u32,
    ) -> Result<(), RequestError> {
        let response = match self
            .database
            .reset_reminders_edit(user_id)
            .await
            .and(self.database.mark_reminder_as_edit(rem_id).await)
        {
            Ok(_) => TgResponse::EnterNewReminder,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedEdit
            }
        };
        tg::send_message(&response.to_string(), self.bot, user_id).await
    }

    pub async fn edit_cron_reminder(
        &mut self,
        user_id: i64,
        cron_rem_id: u32,
    ) -> Result<(), RequestError> {
        let response =
            match self.database.reset_cron_reminders_edit(user_id).await.and(
                self.database.mark_cron_reminder_as_edit(cron_rem_id).await,
            ) {
                Ok(_) => TgResponse::EnterNewReminder,
                Err(err) => {
                    dbg!(err);
                    TgResponse::FailedEdit
                }
            };
        tg::send_message(&response.to_string(), self.bot, user_id).await
    }

    pub async fn replace_reminder(
        &mut self,
        text: &str,
        user_id: i64,
        rem_id: u32,
        from_id_opt: Option<i64>,
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
            tg::send_message(&response.to_string(), self.bot, user_id).await
        } else {
            tg::send_message(
                &TgResponse::FailedEdit.to_string(),
                self.bot,
                user_id,
            )
            .await
        }
    }

    pub async fn replace_cron_reminder(
        &mut self,
        text: &str,
        user_id: i64,
        rem_id: u32,
        from_id_opt: Option<i64>,
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
            tg::send_message(&response.to_string(), self.bot, user_id).await
        } else {
            tg::send_message(
                &TgResponse::FailedEdit.to_string(),
                self.bot,
                user_id,
            )
            .await
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
                        .to_vec()
                        .into_iter()
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
        user_id: i64,
        cb_prefix: &str,
        user_timezone: Tz,
    ) -> InlineKeyboardMarkup {
        let mut markup = InlineKeyboardMarkup::default();
        let mut last_rem_page: bool = false;
        let sorted_reminders = self
            .database
            .get_sorted_reminders(user_id)
            .await
            .map(|rems| rems.collect::<Vec<_>>());
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
                                + if rem.is_cron_reminder() {
                                    "::cron_alt::"
                                } else {
                                    "::alt::"
                                }
                                + &rem.get_id().to_string(),
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
        user_id: i64,
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
        user_id: i64,
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
        user_id: i64,
    ) -> Result<Option<db::ReminderStruct>, db::Error> {
        self.database.get_edit_reminder(user_id).await
    }

    pub async fn get_edit_cron_reminder(
        &mut self,
        user_id: i64,
    ) -> Result<Option<db::CronReminderStruct>, db::Error> {
        self.database.get_edit_cron_reminder(user_id).await
    }
}
