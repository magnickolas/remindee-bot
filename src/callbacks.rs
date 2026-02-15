const SELECT_TIMEZONE_PREFIX: &str = "seltz::";
const SELECT_TIMEZONE_PAGE_PREFIX: &str = "seltz::page::";
const SELECT_TIMEZONE_TZ_PREFIX: &str = "seltz::tz::";

const SET_LANGUAGE_PREFIX: &str = "setlang::";
const SET_LANGUAGE_CODE_PREFIX: &str = "setlang::lang::";

const SETTINGS_PREFIX: &str = "settings::";
const SETTINGS_CHANGE_LANGUAGE: &str = "settings::change_lang";

const DONE_OCCURRENCE_PREFIX: &str = "donerem::occ::";

const EDIT_MODE_TIME_PATTERN_PREFIX: &str = "edit_rem_mode::rem_time_pattern::";
const EDIT_MODE_DESCRIPTION_PREFIX: &str = "edit_rem_mode::rem_description::";

#[derive(Clone, Copy)]
pub(crate) enum ReminderListKind {
    Delete,
    Edit,
    Pause,
}

impl ReminderListKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Delete => "delrem",
            Self::Edit => "editrem",
            Self::Pause => "pauserem",
        }
    }
}

fn parse_i64_with_prefix(prefix: &str, data: &str) -> Option<i64> {
    data.strip_prefix(prefix)
        .and_then(|x| x.parse::<i64>().ok())
}

fn parse_usize_with_prefix(prefix: &str, data: &str) -> Option<usize> {
    data.strip_prefix(prefix)
        .and_then(|x| x.parse::<usize>().ok())
}

pub(crate) fn is_select_timezone(data: &str) -> bool {
    data.starts_with(SELECT_TIMEZONE_PREFIX)
}

pub(crate) fn select_timezone_page(page_num: usize) -> String {
    format!("{SELECT_TIMEZONE_PAGE_PREFIX}{page_num}")
}

pub(crate) fn parse_select_timezone_page(data: &str) -> Option<usize> {
    parse_usize_with_prefix(SELECT_TIMEZONE_PAGE_PREFIX, data)
}

pub(crate) fn select_timezone_tz(tz_name: &str) -> String {
    format!("{SELECT_TIMEZONE_TZ_PREFIX}{tz_name}")
}

pub(crate) fn parse_select_timezone_tz(data: &str) -> Option<&str> {
    data.strip_prefix(SELECT_TIMEZONE_TZ_PREFIX)
}

pub(crate) fn is_set_language(data: &str) -> bool {
    data.starts_with(SET_LANGUAGE_PREFIX)
}

pub(crate) fn set_language(lang_code: &str) -> String {
    format!("{SET_LANGUAGE_CODE_PREFIX}{lang_code}")
}

pub(crate) fn parse_set_language(data: &str) -> Option<&str> {
    data.strip_prefix(SET_LANGUAGE_CODE_PREFIX)
}

pub(crate) fn is_settings(data: &str) -> bool {
    data.starts_with(SETTINGS_PREFIX)
}

pub(crate) fn settings_change_language() -> String {
    SETTINGS_CHANGE_LANGUAGE.to_owned()
}

pub(crate) fn is_settings_change_language(data: &str) -> bool {
    data == SETTINGS_CHANGE_LANGUAGE
}

pub(crate) fn done_occurrence(occ_id: i64) -> String {
    format!("{DONE_OCCURRENCE_PREFIX}{occ_id}")
}

pub(crate) fn is_done_occurrence(data: &str) -> bool {
    data.starts_with(DONE_OCCURRENCE_PREFIX)
}

pub(crate) fn parse_done_occurrence(data: &str) -> Option<i64> {
    parse_i64_with_prefix(DONE_OCCURRENCE_PREFIX, data)
}

pub(crate) fn reminder_page(kind: ReminderListKind, page_num: usize) -> String {
    format!("{}::page::{page_num}", kind.prefix())
}

pub(crate) fn parse_reminder_page(
    kind: ReminderListKind,
    data: &str,
) -> Option<usize> {
    parse_usize_with_prefix(&format!("{}::page::", kind.prefix()), data)
}

pub(crate) fn reminder_alter(
    kind: ReminderListKind,
    rem_type: &str,
    rem_id: i64,
) -> String {
    format!("{}::{rem_type}_alt::{rem_id}", kind.prefix())
}

pub(crate) fn parse_reminder_alter(
    kind: ReminderListKind,
    rem_type: &str,
    data: &str,
) -> Option<i64> {
    parse_i64_with_prefix(&format!("{}::{rem_type}_alt::", kind.prefix()), data)
}

pub(crate) fn edit_mode_time_pattern(rem_id: i64) -> String {
    format!("{EDIT_MODE_TIME_PATTERN_PREFIX}{rem_id}")
}

pub(crate) fn parse_edit_mode_time_pattern(data: &str) -> Option<i64> {
    parse_i64_with_prefix(EDIT_MODE_TIME_PATTERN_PREFIX, data)
}

pub(crate) fn edit_mode_description(rem_id: i64) -> String {
    format!("{EDIT_MODE_DESCRIPTION_PREFIX}{rem_id}")
}

pub(crate) fn parse_edit_mode_description(data: &str) -> Option<i64> {
    parse_i64_with_prefix(EDIT_MODE_DESCRIPTION_PREFIX, data)
}

#[cfg(test)]
mod tests {
    use super::{
        done_occurrence, edit_mode_description, edit_mode_time_pattern,
        is_done_occurrence, is_select_timezone, is_set_language, is_settings,
        is_settings_change_language, parse_done_occurrence,
        parse_edit_mode_description, parse_edit_mode_time_pattern,
        parse_reminder_alter, parse_reminder_page, parse_select_timezone_page,
        parse_select_timezone_tz, parse_set_language, reminder_alter,
        reminder_page, select_timezone_page, select_timezone_tz, set_language,
        settings_change_language, ReminderListKind,
    };

    #[test]
    fn done_occurrence_roundtrip() {
        let data = done_occurrence(42);
        assert!(is_done_occurrence(&data));
        assert_eq!(parse_done_occurrence(&data), Some(42));
    }

    #[test]
    fn parse_done_occurrence_rejects_invalid_data() {
        assert_eq!(parse_done_occurrence("donerem::occ::abc"), None);
        assert_eq!(parse_done_occurrence("settings::change_lang"), None);
    }

    #[test]
    fn timezone_callbacks_roundtrip() {
        let page = select_timezone_page(3);
        assert!(is_select_timezone(&page));
        assert_eq!(parse_select_timezone_page(&page), Some(3));

        let tz = select_timezone_tz("Europe/Amsterdam");
        assert!(is_select_timezone(&tz));
        assert_eq!(parse_select_timezone_tz(&tz), Some("Europe/Amsterdam"));
    }

    #[test]
    fn language_callbacks_roundtrip() {
        let lang = set_language("nl");
        assert!(is_set_language(&lang));
        assert_eq!(parse_set_language(&lang), Some("nl"));
    }

    #[test]
    fn settings_callbacks_roundtrip() {
        let data = settings_change_language();
        assert!(is_settings(&data));
        assert!(is_settings_change_language(&data));
    }

    #[test]
    fn reminder_callbacks_roundtrip() {
        let page = reminder_page(ReminderListKind::Delete, 1);
        assert_eq!(
            parse_reminder_page(ReminderListKind::Delete, &page),
            Some(1)
        );

        let rem = reminder_alter(ReminderListKind::Pause, "rem", 11);
        assert_eq!(
            parse_reminder_alter(ReminderListKind::Pause, "rem", &rem),
            Some(11)
        );
    }

    #[test]
    fn edit_mode_callbacks_roundtrip() {
        let time = edit_mode_time_pattern(7);
        assert_eq!(parse_edit_mode_time_pattern(&time), Some(7));

        let desc = edit_mode_description(8);
        assert_eq!(parse_edit_mode_description(&desc), Some(8));
    }
}
