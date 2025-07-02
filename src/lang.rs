#[cfg(not(test))]
use crate::db::Database;
#[cfg(test)]
use crate::db::MockDatabase as Database;
use teloxide::types::UserId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Language {
    English,
    Dutch,
    Russian,
}

impl Language {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Dutch => "nl",
            Language::Russian => "ru",
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Dutch => "Nederlands",
            Language::Russian => "Русский",
        }
    }

    pub(crate) fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "nl" => Some(Self::Dutch),
            "ru" => Some(Self::Russian),
            _ => None,
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        DEFAULT_LANGUAGE
    }
}

const DEFAULT_LANGUAGE: Language = Language::English;
pub const LANGUAGES: &[Language] =
    &[Language::English, Language::Dutch, Language::Russian];

pub async fn get_user_language(db: &Database, user_id: UserId) -> Language {
    db.get_user_language_name(user_id.0 as i64)
        .await
        .ok()
        .flatten()
        .and_then(|code| Language::from_code(&code))
        .unwrap_or_default()
}
