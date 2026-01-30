use crate::generic_reminder::GenericReminder;
use chrono_tz::Tz;
use sea_orm::ActiveModelTrait;

pub(crate) fn format_reminder<T: ActiveModelTrait + GenericReminder>(
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
