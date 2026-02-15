use crate::callbacks;
use crate::lang::Language;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
};

pub(crate) struct ReminderMarkupEntry {
    pub(crate) text: String,
    pub(crate) rem_type: &'static str,
    pub(crate) rem_id: i64,
}

pub(crate) fn done_markup(lang: Language, occ_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![InlineKeyboardButton::new(
        t!("Done", locale = lang.code()),
        InlineKeyboardButtonKind::CallbackData(callbacks::done_occurrence(
            occ_id,
        )),
    )])
}

pub(crate) fn edit_mode_markup(
    lang: Language,
    rem_id: i64,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![
        InlineKeyboardButton::new(
            t!("TimePattern", locale = lang.code()),
            InlineKeyboardButtonKind::CallbackData(
                callbacks::edit_mode_time_pattern(rem_id),
            ),
        ),
        InlineKeyboardButton::new(
            t!("Description", locale = lang.code()),
            InlineKeyboardButtonKind::CallbackData(
                callbacks::edit_mode_description(rem_id),
            ),
        ),
    ])
}

pub(crate) fn timezone_page_markup(
    num: usize,
    tz_names: Option<Vec<&'static str>>,
) -> InlineKeyboardMarkup {
    let mut markup = InlineKeyboardMarkup::default();
    let mut last_page = false;

    if let Some(tz_names) = tz_names {
        for chunk in tz_names.chunks(2) {
            markup = markup.append_row(
                chunk
                    .iter()
                    .copied()
                    .map(|tz_name| {
                        InlineKeyboardButton::new(
                            tz_name,
                            InlineKeyboardButtonKind::CallbackData(
                                callbacks::select_timezone_tz(tz_name),
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
                callbacks::select_timezone_page(num - 1),
            ),
        ));
    }
    if !last_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData(
                callbacks::select_timezone_page(num + 1),
            ),
        ));
    }

    markup.append_row(move_buttons)
}

pub(crate) fn languages_markup(
    languages: &[crate::lang::Language],
) -> InlineKeyboardMarkup {
    let row = languages
        .iter()
        .map(|lang| {
            InlineKeyboardButton::new(
                lang.name(),
                InlineKeyboardButtonKind::CallbackData(
                    callbacks::set_language(lang.code()),
                ),
            )
        })
        .collect::<Vec<_>>();
    InlineKeyboardMarkup::default().append_row(row)
}

pub(crate) fn settings_markup(lang: Language) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![InlineKeyboardButton::new(
        t!("ChangeLanguage", locale = lang.code()),
        InlineKeyboardButtonKind::CallbackData(
            callbacks::settings_change_language(),
        ),
    )])
}

pub(crate) fn reminders_page_markup(
    num: usize,
    callback_kind: callbacks::ReminderListKind,
    reminders: Option<Vec<ReminderMarkupEntry>>,
) -> InlineKeyboardMarkup {
    let mut markup = InlineKeyboardMarkup::default();
    let mut last_rem_page = false;

    if let Some(reminders) = reminders {
        for reminder in reminders {
            markup = markup.append_row(vec![InlineKeyboardButton::new(
                reminder.text,
                InlineKeyboardButtonKind::CallbackData(
                    callbacks::reminder_alter(
                        callback_kind,
                        reminder.rem_type,
                        reminder.rem_id,
                    ),
                ),
            )]);
        }
    } else {
        last_rem_page = true;
    }

    let mut move_buttons = vec![];
    if num > 0 {
        move_buttons.push(InlineKeyboardButton::new(
            "⬅️",
            InlineKeyboardButtonKind::CallbackData(callbacks::reminder_page(
                callback_kind,
                num - 1,
            )),
        ));
    }
    if !last_rem_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData(callbacks::reminder_page(
                callback_kind,
                num + 1,
            )),
        ));
    }

    markup.append_row(move_buttons)
}
