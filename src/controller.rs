use crate::db;
use crate::err;
use crate::tg;
use crate::tz;

use chrono::Utc;
use cron_parser::parse as parse_cron;
use itertools::Itertools;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
};
use tg::TgResponse;

pub async fn start(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    tg::send_message(&TgResponse::Hello.to_string(), bot, user_id).await
}

/// Send a list of all notifications
pub async fn list(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    let reminders_future = db::get_pending_user_reminders(user_id).map(|v| {
        v.into_iter()
            .map::<Box<dyn tg::GenericReminder>, _>(|x| Box::new(x))
    });
    let cron_reminders_future = db::get_pending_user_cron_reminders(user_id)
        .map(|v| {
            v.into_iter()
                .map::<Box<dyn tg::GenericReminder>, _>(|x| Box::new(x))
        });
    // Merge one-time and periodic reminders
    let all_reminders = reminders_future.and_then(|rems| {
        cron_reminders_future.map(|cron_rems| rems.chain(cron_rems))
    });
    // Sort all reminders ascending of their time appearance
    let sorted_reminders =
        all_reminders.map(|rems| rems.sorted().map(|x| x.to_string()));
    // Format reminders
    let text = sorted_reminders
        .map(|rems| {
            vec![TgResponse::RemindersListHeader.to_string()]
                .into_iter()
                .chain(rems)
                .collect::<Vec<String>>()
                .join("\n")
        })
        .unwrap_or_else(|err| {
            dbg!(err);
            TgResponse::QueryingError.to_string()
        });
    tg::send_message(&text, bot, user_id).await
}

/// Send a markup with all timezones to select
pub async fn choose_timezone(
    bot: &Bot,
    user_id: i64,
) -> Result<(), RequestError> {
    tg::send_markup(
        &TgResponse::SelectTimezone.to_string(),
        get_markup_for_tz_page_idx(0),
        bot,
        user_id,
    )
    .await
}

/// Send user's timezone
pub async fn get_timezone(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    let response = match db::get_user_timezone_name(user_id) {
        Ok(tz_name) => TgResponse::ChosenTimezone(tz_name),
        Err(err) => {
            dbg!(err);
            TgResponse::NoChosenTimezone
        }
    };
    tg::send_message(&response.to_string(), bot, user_id).await
}

/// General way to send a markup to select a reminder for some operation
async fn start_alter(
    bot: &Bot,
    user_id: i64,
    response: TgResponse,
    get_markup: fn(usize, i64) -> InlineKeyboardMarkup,
) -> Result<(), RequestError> {
    tg::send_markup(&response.to_string(), get_markup(0, user_id), bot, user_id)
        .await
}

/// Send a markup to select a reminder for deleting
pub async fn start_delete(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    start_alter(
        bot,
        user_id,
        TgResponse::ChooseDeleteReminder,
        get_markup_for_reminders_page_deletion,
    )
    .await
}

/// Send a markup to select a reminder for editing
pub async fn start_edit(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    start_alter(
        bot,
        user_id,
        TgResponse::ChooseEditReminder,
        get_markup_for_reminders_page_editing,
    )
    .await
}

/// Send a list of supported commands
pub async fn list_commands(
    bot: &Bot,
    user_id: i64,
) -> Result<(), RequestError> {
    tg::send_message(&TgResponse::CommandsHelp.to_string(), bot, user_id).await
}

/// Try to parse user's message into a one-time or periodic reminder and set it
pub async fn set_reminder(
    text: &str,
    bot: &Bot,
    user_id: i64,
    from_id_opt: Option<i32>,
    silent_success: bool,
) -> Result<bool, RequestError> {
    if let Some(reminder) = tg::parse_req(text, user_id) {
        let mut ok = true;
        let response = match db::insert_reminder(&reminder) {
            Ok(_) => TgResponse::SuccessInsert,
            Err(err) => {
                ok = false;
                dbg!(err);
                TgResponse::FailedInsert
            }
        };

        if !silent_success {
            tg::send_message(&response.to_string(), bot, user_id).await?
        }
        Ok(ok)
    } else if let Ok((cron_expr, time)) = {
        let cron_fields: Vec<&str> = text.split_whitespace().take(5).collect();
        if cron_fields.len() < 5 {
            Err(err::Error::CronFewFields)
        } else {
            let cron_expr = cron_fields.join(" ");
            tz::get_user_timezone(user_id).and_then(|timezone| {
                let time = parse_cron(
                    &cron_expr,
                    &Utc::now().with_timezone(&timezone),
                )?
                .with_timezone(&Utc);
                Ok((cron_expr, time))
            })
        }
    } {
        let mut ok = true;
        let response = match db::insert_cron_reminder(&db::CronReminder {
            id: 0,
            user_id,
            cron_expr: cron_expr.clone(),
            time,
            desc: text
                .strip_prefix(&(cron_expr.to_owned() + " "))
                .unwrap_or("")
                .to_owned(),
            sent: false,
            edit: false,
        }) {
            Ok(_) => TgResponse::SuccessInsert,
            Err(err) => {
                ok = false;
                dbg!(err);
                TgResponse::FailedInsert
            }
        };
        tg::send_message(&response.to_string(), bot, user_id).await?;
        Ok(ok)
    } else if from_id_opt
        .filter(|&from_id| from_id as i64 == user_id)
        .is_some()
    {
        let response = if tz::get_user_timezone(user_id).is_err() {
            TgResponse::NoChosenTimezone
        } else {
            TgResponse::IncorrectRequest
        };
        tg::send_message(&response.to_string(), bot, user_id).await?;
        Ok(false)
    } else {
        Ok(false)
    }
}

pub async fn incorrect_request(
    bot: &Bot,
    user_id: i64,
) -> Result<(), RequestError> {
    tg::send_message(&TgResponse::IncorrectRequest.to_string(), bot, user_id)
        .await
}

/// Switch the markup's page
pub async fn select_timezone_set_page(
    bot: &Bot,
    user_id: i64,
    page_num: usize,
    msg_id: i32,
) -> Result<(), RequestError> {
    tg::edit_markup(get_markup_for_tz_page_idx(page_num), bot, msg_id, user_id)
        .await
}

pub async fn set_timezone(
    bot: &Bot,
    user_id: i64,
    tz_name: &str,
) -> Result<(), RequestError> {
    let response = match db::set_user_timezone_name(user_id, tz_name) {
        Ok(_) => TgResponse::ChosenTimezone(tz_name.to_owned()),
        Err(err) => {
            dbg!(err);
            TgResponse::FailedSetTimezone(tz_name.to_owned())
        }
    };
    tg::send_message(&response.to_string(), bot, user_id).await
}

async fn alter_reminder_set_page(
    bot: &Bot,
    user_id: i64,
    page_num: usize,
    msg_id: i32,
    get_markup: fn(usize, i64) -> InlineKeyboardMarkup,
) -> Result<(), RequestError> {
    tg::edit_markup(get_markup(page_num, user_id), bot, msg_id, user_id).await
}

pub async fn delete_reminder_set_page(
    bot: &Bot,
    user_id: i64,
    page_num: usize,
    msg_id: i32,
) -> Result<(), RequestError> {
    alter_reminder_set_page(
        bot,
        user_id,
        page_num,
        msg_id,
        get_markup_for_reminders_page_deletion,
    )
    .await
}

pub async fn edit_reminder_set_page(
    bot: &Bot,
    user_id: i64,
    page_num: usize,
    msg_id: i32,
) -> Result<(), RequestError> {
    alter_reminder_set_page(
        bot,
        user_id,
        page_num,
        msg_id,
        get_markup_for_reminders_page_editing,
    )
    .await
}

async fn delete_arbitrary_reminder(
    bot: &Bot,
    user_id: i64,
    rem_id: u32,
    msg_id: i32,
    mark_sent: fn(u32) -> Result<(), rusqlite::Error>,
) -> Result<(), RequestError> {
    let response = match mark_sent(rem_id) {
        Ok(_) => TgResponse::SuccessDelete,
        Err(err) => {
            dbg!(err);
            TgResponse::FailedDelete
        }
    };
    delete_reminder_set_page(bot, user_id, 0, msg_id).await?;
    tg::send_message(&response.to_string(), bot, user_id).await
}

pub async fn delete_reminder(
    bot: &Bot,
    user_id: i64,
    rem_id: u32,
    msg_id: i32,
) -> Result<(), RequestError> {
    delete_arbitrary_reminder(
        bot,
        user_id,
        rem_id,
        msg_id,
        db::mark_reminder_as_sent,
    )
    .await
}

pub async fn delete_cron_reminder(
    bot: &Bot,
    user_id: i64,
    cron_rem_id: u32,
    msg_id: i32,
) -> Result<(), RequestError> {
    delete_arbitrary_reminder(
        bot,
        user_id,
        cron_rem_id,
        msg_id,
        db::mark_cron_reminder_as_sent,
    )
    .await
}

async fn edit_arbitrary_reminder(
    bot: &Bot,
    user_id: i64,
    rem_id: u32,
    mark_edit: fn(u32) -> Result<(), rusqlite::Error>,
) -> Result<(), RequestError> {
    let response = match db::reset_reminders_edit(user_id)
        .and(db::reset_cron_reminders_edit(user_id))
        .and(mark_edit(rem_id))
    {
        Ok(_) => TgResponse::EnterNewReminder,
        Err(err) => {
            dbg!(err);
            TgResponse::FailedEdit
        }
    };
    tg::send_message(&response.to_string(), bot, user_id).await
}

pub async fn edit_reminder(
    bot: &Bot,
    user_id: i64,
    rem_id: u32,
) -> Result<(), RequestError> {
    edit_arbitrary_reminder(bot, user_id, rem_id, db::mark_reminder_as_edit)
        .await
}

pub async fn edit_cron_reminder(
    bot: &Bot,
    user_id: i64,
    cron_rem_id: u32,
) -> Result<(), RequestError> {
    edit_arbitrary_reminder(
        bot,
        user_id,
        cron_rem_id,
        db::mark_cron_reminder_as_edit,
    )
    .await
}

pub async fn replace_reminder(
    text: &str,
    bot: &Bot,
    user_id: i64,
    rem_id: u32,
    from_id_opt: Option<i32>,
) -> Result<(), RequestError> {
    if set_reminder(text, bot, user_id, from_id_opt, true).await? {
        let response = match db::mark_reminder_as_sent(rem_id) {
            Ok(_) => TgResponse::SuccessEdit,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedEdit
            }
        };
        tg::send_message(&response.to_string(), bot, user_id).await
    } else {
        tg::send_message(&TgResponse::FailedEdit.to_string(), bot, user_id)
            .await
    }
}

pub async fn replace_cron_reminder(
    text: &str,
    bot: &Bot,
    user_id: i64,
    rem_id: u32,
    from_id_opt: Option<i32>,
) -> Result<(), RequestError> {
    if set_reminder(text, bot, user_id, from_id_opt, true).await? {
        let response = match db::mark_cron_reminder_as_sent(rem_id) {
            Ok(_) => TgResponse::SuccessEdit,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedEdit
            }
        };
        tg::send_message(&response.to_string(), bot, user_id).await
    } else {
        tg::send_message(&TgResponse::FailedEdit.to_string(), bot, user_id)
            .await
    }
}

pub fn get_markup_for_tz_page_idx(num: usize) -> InlineKeyboardMarkup {
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
                    .collect(),
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

fn get_markup_for_reminders_page_alteration(
    num: usize,
    user_id: i64,
    cb_prefix: &str,
) -> InlineKeyboardMarkup {
    let mut markup = InlineKeyboardMarkup::default();
    let mut last_rem_page: bool = false;
    let mut last_cron_rem_page: bool = false;
    if let Some(reminders) = db::get_pending_user_reminders(user_id)
        .ok()
        .as_ref()
        .and_then(|rems| rems.chunks(45).into_iter().nth(num))
    {
        for chunk in reminders.chunks(1) {
            markup = markup.append_row(
                chunk
                    .to_vec()
                    .into_iter()
                    .map(|rem| {
                        InlineKeyboardButton::new(
                            rem.to_unescaped_string(),
                            InlineKeyboardButtonKind::CallbackData(
                                cb_prefix.to_owned()
                                    + "::alt::"
                                    + &rem.id.to_string(),
                            ),
                        )
                    })
                    .collect(),
            );
        }
    } else {
        last_rem_page = true;
    }
    if let Some(cron_reminders) = db::get_pending_user_cron_reminders(user_id)
        .ok()
        .as_ref()
        .and_then(|cron_rems| cron_rems.chunks(45).into_iter().nth(num))
    {
        for chunk in cron_reminders.chunks(1) {
            markup = markup.append_row(
                chunk
                    .to_vec()
                    .into_iter()
                    .map(|cron_rem| {
                        InlineKeyboardButton::new(
                            cron_rem.to_unescaped_string(),
                            InlineKeyboardButtonKind::CallbackData(
                                cb_prefix.to_owned()
                                    + "::cron_alt::"
                                    + &cron_rem.id.to_string(),
                            ),
                        )
                    })
                    .collect(),
            );
        }
    } else {
        last_cron_rem_page = true;
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
    if !last_rem_page || !last_cron_rem_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData(
                cb_prefix.to_owned() + "::page::" + &(num + 1).to_string(),
            ),
        ))
    }
    markup.append_row(move_buttons)
}

pub fn get_markup_for_reminders_page_deletion(
    num: usize,
    user_id: i64,
) -> InlineKeyboardMarkup {
    get_markup_for_reminders_page_alteration(num, user_id, "delrem")
}

pub fn get_markup_for_reminders_page_editing(
    num: usize,
    user_id: i64,
) -> InlineKeyboardMarkup {
    get_markup_for_reminders_page_alteration(num, user_id, "editrem")
}
