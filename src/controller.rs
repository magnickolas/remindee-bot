use crate::db;
use crate::err;
use crate::tg;
use crate::tz;

use chrono::Utc;
use cron_parser::parse as parse_cron;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
};
use tg::TgResponse;

pub async fn start(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    tg::send_message(&TgResponse::Hello.to_string(), &bot, user_id).await
}

pub async fn list(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    let reminders_str_fut = db::get_pending_user_reminders(user_id)
        .map(|v| v.into_iter().map(|x| x.to_string()));
    let cron_reminders_str_fut = db::get_pending_user_cron_reminders(user_id)
        .map(|v| v.into_iter().map(|x| x.to_string()));
    let all_reminders_str = reminders_str_fut.and_then(|rems_str| {
        cron_reminders_str_fut
            .map(|cron_rems_str| rems_str.chain(cron_rems_str))
    });

    let text = all_reminders_str
        .map(|rems_str| {
            vec![TgResponse::RemindersListHeader.to_string()]
                .into_iter()
                .chain(rems_str)
                .collect::<Vec<String>>()
                .join("\n")
        })
        .unwrap_or_else(|_| TgResponse::QueryingError.to_string());

    tg::send_message(&text, &bot, user_id).await
}

pub async fn choose_timezone(
    bot: &Bot,
    user_id: i64,
) -> Result<(), RequestError> {
    tg::send_markup(
        &TgResponse::SelectTimezone.to_string(),
        get_markup_for_page_idx(0),
        &bot,
        user_id,
    )
    .await
}

pub async fn get_timezone(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    let response = match db::get_user_timezone_name(user_id) {
        Ok(tz_name) => TgResponse::ChosenTimezone(tz_name),
        Err(err) => {
            dbg!(err);
            TgResponse::NoChosenTimezone
        }
    };
    tg::send_message(&response.to_string(), &bot, user_id).await
}

pub async fn start_delete(bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    tg::send_markup(
        &TgResponse::ChooseDeleteReminder.to_string(),
        get_markup_for_reminders_page_deletion(0, user_id),
        &bot,
        user_id,
    )
    .await
}

pub async fn list_commands(
    bot: &Bot,
    user_id: i64,
) -> Result<(), RequestError> {
    tg::send_message(&TgResponse::CommandsHelp.to_string(), &bot, user_id).await
}

pub async fn set_reminder(
    text: &str,
    bot: &Bot,
    user_id: i64,
    from_id: Option<i32>,
) -> Result<(), RequestError> {
    if let Some(reminder) = tg::parse_req(text, user_id) {
        let response = match db::insert_reminder(&reminder) {
            Ok(_) => TgResponse::SuccessInsert,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedInsert
            }
        };

        tg::send_message(&response.to_string(), &bot, user_id).await
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
        let response = match db::insert_cron_reminder(&db::CronReminder {
            id: 0,
            user_id,
            cron_expr: cron_expr.clone(),
            time,
            desc: text
                .strip_prefix(&(cron_expr.to_string() + " "))
                .unwrap_or("")
                .to_string(),
            sent: false,
        }) {
            Ok(_) => TgResponse::SuccessInsert,
            Err(err) => {
                dbg!(err);
                TgResponse::FailedInsert
            }
        };
        tg::send_message(&response.to_string(), &bot, user_id).await
    } else if let Some(id) = from_id {
        if id as i64 == user_id {
            let response = if tz::get_user_timezone(user_id).is_err() {
                TgResponse::NoChosenTimezone
            } else {
                TgResponse::IncorrectRequest
            };
            tg::send_message(&response.to_string(), &bot, user_id).await
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

pub async fn incorrect_request(
    bot: &Bot,
    user_id: i64,
) -> Result<(), RequestError> {
    tg::send_message(&TgResponse::IncorrectRequest.to_string(), &bot, user_id)
        .await
}

pub async fn select_timezone_set_page(
    bot: &Bot,
    user_id: i64,
    page_num: usize,
    msg_id: i32,
) -> Result<(), RequestError> {
    tg::edit_markup(get_markup_for_page_idx(page_num), &bot, msg_id, user_id)
        .await
}

pub async fn set_timezone(
    bot: &Bot,
    user_id: i64,
    tz_name: &str,
) -> Result<(), RequestError> {
    let response = match db::set_user_timezone_name(user_id, tz_name) {
        Ok(_) => TgResponse::ChosenTimezone(tz_name.to_string()),
        Err(err) => {
            dbg!(err);
            TgResponse::FailedSetTimezone(tz_name.to_string())
        }
    };
    tg::send_message(&response.to_string(), &bot, user_id).await
}

pub async fn delete_reminder_set_page(
    bot: &Bot,
    user_id: i64,
    page_num: usize,
    msg_id: i32,
) -> Result<(), RequestError> {
    tg::edit_markup(
        get_markup_for_reminders_page_deletion(page_num, user_id),
        &bot,
        msg_id,
        user_id,
    )
    .await
}

pub async fn delete_reminder(
    bot: &Bot,
    user_id: i64,
    rem_id: u32,
    msg_id: i32,
) -> Result<(), RequestError> {
    let response = match db::mark_reminder_as_sent(rem_id) {
        Ok(_) => TgResponse::SuccessDelete,
        Err(err) => {
            dbg!(err);
            TgResponse::FailedDelete
        }
    };
    tg::edit_markup(
        get_markup_for_reminders_page_deletion(0, user_id),
        &bot,
        msg_id,
        user_id,
    )
    .await?;
    tg::send_message(&response.to_string(), &bot, user_id).await
}

pub async fn delete_cron_reminder(
    bot: &Bot,
    user_id: i64,
    cron_rem_id: u32,
    msg_id: i32,
) -> Result<(), RequestError> {
    let response = match db::mark_cron_reminder_as_sent(cron_rem_id) {
        Ok(_) => TgResponse::SuccessDelete,
        Err(err) => {
            dbg!(err);
            TgResponse::FailedDelete
        }
    };
    tg::edit_markup(
        get_markup_for_reminders_page_deletion(0, user_id),
        &bot,
        msg_id,
        user_id,
    )
    .await?;
    tg::send_message(&response.to_string(), &bot, user_id).await
}

pub fn get_markup_for_page_idx(num: usize) -> InlineKeyboardMarkup {
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
                                "seltz::tz::".to_string() + tz_name,
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
                "seltz::page::".to_string() + &(num - 1).to_string(),
            ),
        ))
    }
    if !last_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData(
                "seltz::page::".to_string() + &(num + 1).to_string(),
            ),
        ))
    }
    markup.append_row(move_buttons)
}

pub fn get_markup_for_reminders_page_deletion(
    num: usize,
    user_id: i64,
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
                                "delrem::del::".to_string()
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
                                "delrem::cron_del::".to_string()
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
                "delrem::page::".to_string() + &(num - 1).to_string(),
            ),
        ))
    }
    if !last_rem_page || !last_cron_rem_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData(
                "delrem::page::".to_string() + &(num + 1).to_string(),
            ),
        ))
    }
    markup.append_row(move_buttons)
}
