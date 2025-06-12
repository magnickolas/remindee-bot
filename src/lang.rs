#[cfg(not(test))]
use crate::db::Database;
#[cfg(test)]
use crate::db::MockDatabase as Database;
use crate::err;
use teloxide::types::UserId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Dutch,
}

pub const LANGUAGES: &[(&str, Language)] =
    &[("English", Language::English), ("Dutch", Language::Dutch)];

pub fn parse_language(name: &str) -> Option<Language> {
    match name {
        "English" => Some(Language::English),
        "Dutch" => Some(Language::Dutch),
        _ => None,
    }
}

#[cfg(not(test))]
pub async fn get_user_language(
    db: &Database,
    user_id: UserId,
) -> Result<Option<Language>, err::Error> {
    let lang_name_opt = db.get_user_language_name(user_id.0 as i64).await?;
    Ok(lang_name_opt.and_then(|name| parse_language(&name)))
}

#[cfg(test)]
pub async fn get_user_language(
    _db: &Database,
    _user_id: UserId,
) -> Result<Option<Language>, err::Error> {
    Ok(Some(Language::English))
}
