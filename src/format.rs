use crate::entity::cron_reminder;
use crate::generic_reminder::GenericReminder;
use chrono_tz::Tz;
use sea_orm::{ActiveModelTrait, IntoActiveModel};

pub fn format_reminder<T: ActiveModelTrait + GenericReminder>(
    reminder: &T,
    user_timezone: Tz,
) -> String {
    match reminder.user_id() {
        Some(user_id) if reminder.is_group() => {
            reminder.to_string_with_mention(user_timezone, user_id.0 as i64)
        }
        _ => reminder.to_string(user_timezone),
    }
}

pub fn format_cron_reminder(
    reminder: &cron_reminder::Model,
    next_reminder: Option<&cron_reminder::Model>,
    user_timezone: Tz,
) -> String {
    let formatted_reminder =
        format_reminder(&reminder.clone().into_active_model(), user_timezone);
    match next_reminder {
        Some(next_reminder) => format!(
            "{}\n\nNext time → {}",
            formatted_reminder,
            next_reminder
                .clone()
                .into_active_model()
                .serialize_time(user_timezone)
        ),
        None => formatted_reminder,
    }
}
