use chrono_tz::Tz;
use dptree::case;
use teloxide::{
    dispatching::{dialogue, UpdateHandler},
    prelude::*,
    types::Location,
    utils::command::BotCommands,
};

#[cfg(not(test))]
use teloxide::dispatching::dialogue::ErasedStorage;
#[cfg(test)]
use teloxide::dispatching::dialogue::InMemStorage;

use crate::{
    callbacks,
    controller::{
        EditMode, ReminderUpdate, TgCallbackController, TgMessageController,
    },
    tg::TgResponse,
    tz::{self, get_timezone_name_of_location},
};

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum State {
    #[default]
    Default,
    Edit {
        id: i64,
        mode: EditMode,
    },
}

#[cfg(not(test))]
type MyStorage = ErasedStorage<State>;
#[cfg(test)]
type MyStorage = InMemStorage<State>;

type MyDialogue = Dialogue<State, MyStorage>;

#[derive(BotCommands, Clone)]
#[command(description = "Commands:", rename_rule = "lowercase")]
pub(crate) enum Command {
    #[command(description = "show the set reminders")]
    List,
    #[command(description = "choose reminders to delete")]
    Delete,
    #[command(description = "choose reminders to edit")]
    Edit,
    #[command(description = "cancel editing")]
    Cancel,
    #[command(description = "choose reminders to pause")]
    Pause,
    #[command(description = "set a new reminder")]
    Set(String),
    #[command(description = "select a timezone")]
    SetTimezone,
    #[command(description = "show your timezone")]
    Timezone,
    #[command(description = "bot settings")]
    Settings,
    #[command(description = "show this text")]
    Help,
    #[command(description = "show the greeting message")]
    Start,
}

pub(crate) fn get_handler(
) -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    dialogue::enter::<Update, MyStorage, State, _>()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .filter_map(TgMessageController::from_msg)
                .branch(case![Command::Help].endpoint(help_handler))
                .branch(
                    case![Command::Start]
                        .branch(
                            dptree::filter(|msg: Message| {
                                msg.chat.id.is_user()
                            })
                            .endpoint(start_handler),
                        )
                        .endpoint(start_group_handler),
                )
                .branch(
                    case![Command::SetTimezone].endpoint(set_timezone_handler),
                )
                .branch(case![Command::Settings].endpoint(settings_handler))
                .branch(
                    dptree::filter_map_async(get_user_timezone)
                        .branch(case![Command::List].endpoint(list_handler))
                        .branch(
                            case![Command::Timezone].endpoint(timezone_handler),
                        )
                        .branch(case![Command::Delete].endpoint(delete_handler))
                        .branch(case![Command::Edit].endpoint(edit_handler))
                        .branch(case![Command::Cancel].endpoint(cancel_handler))
                        .branch(case![Command::Pause].endpoint(pause_handler))
                        .branch(case![Command::Set(text)].endpoint(set_handler))
                        .endpoint(incorrect_request_handler),
                )
                .endpoint(set_timezone_handler),
        )
        .branch(
            Update::filter_message()
                .filter(|msg: Message| msg.chat.id.is_user())
                .filter_map(TgMessageController::from_msg)
                .branch(
                    dptree::filter_map(|msg: Message| msg.location().copied())
                        .endpoint(location_handler),
                )
                .branch(
                    dptree::filter_map_async(get_user_timezone)
                        .branch(
                            dptree::filter_map(|msg: Message| {
                                msg.text().map(|text| text.to_owned())
                            })
                            .branch(
                                case![State::Edit { id, mode }]
                                    .endpoint(edit_message_handler),
                            )
                            .endpoint(message_handler),
                        )
                        .endpoint(incorrect_request_handler),
                )
                .endpoint(set_timezone_handler),
        )
        .branch(
            Update::filter_edited_message()
                .filter_command::<Command>()
                .filter_map(TgMessageController::from_msg)
                .branch(
                    dptree::filter_map_async(get_user_timezone)
                        .branch(
                            case![Command::Set(text)]
                                .endpoint(set_edited_handler),
                        )
                        .endpoint(incorrect_request_handler),
                )
                .endpoint(set_timezone_handler),
        )
        .branch(
            Update::filter_edited_message()
                .filter(|msg: Message| msg.chat.id.is_user())
                .filter_map(TgMessageController::from_msg)
                .branch(
                    dptree::filter_map_async(get_user_timezone)
                        .endpoint(edited_message_handler),
                )
                .endpoint(set_timezone_handler),
        )
        .branch(
            Update::filter_callback_query()
                .filter_map(TgCallbackController::new)
                .filter_map(|cb_query: CallbackQuery| cb_query.data)
                .branch(
                    dptree::filter(|cb_data: String| {
                        callbacks::is_select_timezone(&cb_data)
                    })
                    .endpoint(select_timezone_handler),
                )
                .branch(
                    dptree::filter(|cb_data: String| {
                        callbacks::is_set_language(&cb_data)
                    })
                    .endpoint(select_language_handler),
                )
                .branch(
                    dptree::filter(|cb_data: String| {
                        callbacks::is_settings(&cb_data)
                    })
                    .endpoint(settings_menu_handler),
                )
                .branch(
                    dptree::filter(|cb_data: String| {
                        callbacks::is_done_occurrence(&cb_data)
                    })
                    .endpoint(done_occurrence_handler),
                )
                .branch(
                    dptree::map(|cb_ctl: TgCallbackController| cb_ctl.msg_ctl)
                        .filter_map_async(get_user_timezone)
                        .endpoint(callback_handler),
                ),
        )
}

async fn get_user_timezone(ctl: TgMessageController) -> Option<Tz> {
    tz::get_user_timezone(&ctl.db, ctl.user_id)
        .await
        .ok()
        .flatten()
}

async fn help_handler(
    ctl: TgMessageController,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.help().await.map_err(From::from)
}

async fn start_handler(
    ctl: TgMessageController,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.start().await.map_err(From::from)
}

async fn start_group_handler(
    ctl: TgMessageController,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.start_group().await.map_err(From::from)
}

async fn list_handler(
    ctl: TgMessageController,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.list(user_tz).await.map_err(From::from)
}

async fn timezone_handler(
    ctl: TgMessageController,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.get_timezone(user_tz).await.map_err(From::from)
}

async fn delete_handler(
    ctl: TgMessageController,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.start_delete(user_tz).await.map_err(From::from)
}

async fn edit_handler(
    ctl: TgMessageController,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.start_edit(user_tz).await.map_err(From::from)
}

async fn cancel_handler(
    ctl: TgMessageController,
    dialogue: MyDialogue,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.cancel_edit().await?;
    #[allow(clippy::useless_conversion)]
    dialogue.update(State::Default).await.map_err(From::from)
}

async fn pause_handler(
    ctl: TgMessageController,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.start_pause(user_tz).await.map_err(From::from)
}

async fn set_handler(
    ctl: TgMessageController,
    reminder_text: String,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.set_new_reminder(&reminder_text, user_tz)
        .await
        .map(|_| ())
        .map_err(From::from)
}

async fn set_edited_handler(
    ctl: TgMessageController,
    reminder_text: String,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Ok(ctl
        .edit_reminder_from_edited_message(&reminder_text, user_tz)
        .await?)
}

async fn edited_message_handler(
    ctl: TgMessageController,
    msg: Message,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        Ok(ctl.edit_reminder_from_edited_message(text, user_tz).await?)
    } else {
        ctl.incorrect_request().await.map_err(From::from)
    }
}

async fn set_timezone_handler(
    ctl: TgMessageController,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.choose_timezone().await.map_err(From::from)
}

async fn settings_handler(
    ctl: TgMessageController,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.choose_settings().await.map_err(From::from)
}

async fn location_handler(
    ctl: TgMessageController,
    loc: Location,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.set_timezone(get_timezone_name_of_location(loc.longitude, loc.latitude))
        .await
        .map_err(From::from)
}

async fn incorrect_request_handler(
    ctl: TgMessageController,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.incorrect_request().await.map_err(From::from)
}

async fn edit_message_handler(
    ctl: TgMessageController,
    text: String,
    rem_update: (i64, EditMode),
    user_tz: Tz,
    dialogue: MyDialogue,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match rem_update.1 {
        EditMode::TimePattern => {
            ctl.edit_reminder(
                ReminderUpdate::ReminderTimePattern(rem_update.0, text),
                user_tz,
            )
            .await?
        }
        EditMode::Description => {
            ctl.edit_reminder(
                ReminderUpdate::ReminderDescription(rem_update.0, text),
                user_tz,
            )
            .await?
        }
    }
    #[allow(clippy::useless_conversion)]
    dialogue.update(State::Default).await.map_err(From::from)
}

async fn message_handler(
    ctl: TgMessageController,
    text: String,
    user_tz: Tz,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ctl.set_new_reminder(&text, user_tz)
        .await
        .map(|_| ())
        .map_err(From::from)
}

async fn select_timezone_handler(
    ctl: TgCallbackController,
    cb_data: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(page_num) = callbacks::parse_select_timezone_page(&cb_data) {
        ctl.msg_ctl
            .select_timezone_set_page(page_num)
            .await
            .map_err(From::from)
    } else if let Some(tz_name) = callbacks::parse_select_timezone_tz(&cb_data)
    {
        ctl.set_timezone(tz_name).await.map_err(From::from)
    } else {
        ctl.msg_ctl.reply(TgResponse::IncorrectRequest).await?;
        ctl.acknowledge_callback().await.map_err(From::from)
    }
}

async fn select_language_handler(
    ctl: TgCallbackController,
    cb_data: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(lang_code) = callbacks::parse_set_language(&cb_data) {
        ctl.set_language(lang_code).await.map_err(From::from)
    } else {
        ctl.msg_ctl.reply(TgResponse::IncorrectRequest).await?;
        ctl.acknowledge_callback().await.map_err(From::from)
    }
}

async fn settings_menu_handler(
    ctl: TgCallbackController,
    cb_data: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if callbacks::is_settings_change_language(&cb_data) {
        ctl.msg_ctl.choose_language().await?;
        ctl.acknowledge_callback().await.map_err(From::from)
    } else {
        ctl.msg_ctl.reply(TgResponse::IncorrectRequest).await?;
        ctl.acknowledge_callback().await.map_err(From::from)
    }
}

async fn done_occurrence_handler(
    ctl: TgCallbackController,
    cb_data: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(occ_id) = callbacks::parse_done_occurrence(&cb_data) {
        ctl.done_occurrence(occ_id).await.map_err(From::from)
    } else {
        ctl.msg_ctl.reply(TgResponse::IncorrectRequest).await?;
        ctl.acknowledge_callback().await.map_err(From::from)
    }
}

async fn callback_handler(
    ctl: TgCallbackController,
    cb_data: String,
    user_tz: Tz,
    dialogue: MyDialogue,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(page_num) = callbacks::parse_select_timezone_page(&cb_data) {
        ctl.msg_ctl
            .select_timezone_set_page(page_num)
            .await
            .map_err(From::from)
    } else if let Some(tz_name) = callbacks::parse_select_timezone_tz(&cb_data)
    {
        ctl.set_timezone(tz_name).await.map_err(From::from)
    } else if callbacks::is_settings_change_language(&cb_data) {
        ctl.msg_ctl.choose_language().await?;
        ctl.acknowledge_callback().await.map_err(From::from)
    } else if let Some(lang_code) = callbacks::parse_set_language(&cb_data) {
        ctl.set_language(lang_code).await.map_err(From::from)
    } else if let Some(page_num) = callbacks::parse_reminder_page(
        callbacks::ReminderListKind::Delete,
        &cb_data,
    ) {
        ctl.msg_ctl
            .delete_reminder_set_page(page_num, user_tz)
            .await
            .map_err(From::from)
    } else if let Some(rem_id) = callbacks::parse_reminder_alter(
        callbacks::ReminderListKind::Delete,
        "rem",
        &cb_data,
    ) {
        ctl.delete_reminder(rem_id, user_tz)
            .await
            .map_err(From::from)
    } else if let Some(page_num) = callbacks::parse_reminder_page(
        callbacks::ReminderListKind::Edit,
        &cb_data,
    ) {
        ctl.msg_ctl
            .edit_reminder_set_page(page_num, user_tz)
            .await
            .map_err(From::from)
    } else if let Some(rem_id) = callbacks::parse_reminder_alter(
        callbacks::ReminderListKind::Edit,
        "rem",
        &cb_data,
    ) {
        ctl.choose_edit_mode_reminder(rem_id)
            .await
            .map_err(From::from)
    } else if let Some(page_num) = callbacks::parse_reminder_page(
        callbacks::ReminderListKind::Pause,
        &cb_data,
    ) {
        ctl.msg_ctl
            .pause_reminder_set_page(page_num, user_tz)
            .await
            .map_err(From::from)
    } else if let Some(rem_id) = callbacks::parse_reminder_alter(
        callbacks::ReminderListKind::Pause,
        "rem",
        &cb_data,
    ) {
        ctl.pause_reminder(rem_id, user_tz)
            .await
            .map_err(From::from)
    } else if let Some(rem_id) =
        callbacks::parse_edit_mode_time_pattern(&cb_data)
    {
        ctl.set_edit_mode_reminder(EditMode::TimePattern).await?;
        #[allow(clippy::useless_conversion)]
        dialogue
            .update(State::Edit {
                id: rem_id,
                mode: EditMode::TimePattern,
            })
            .await
            .map_err(From::from)
    } else if let Some(rem_id) =
        callbacks::parse_edit_mode_description(&cb_data)
    {
        ctl.set_edit_mode_reminder(EditMode::Description).await?;
        #[allow(clippy::useless_conversion)]
        dialogue
            .update(State::Edit {
                id: rem_id,
                mode: EditMode::Description,
            })
            .await
            .map_err(From::from)
    } else {
        ctl.msg_ctl.reply(TgResponse::IncorrectRequest).await?;
        ctl.acknowledge_callback().await.map_err(From::from)
    }
}
