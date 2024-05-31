use sea_orm::entity::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum EditMode {
    #[sea_orm(string_value = "time_pattern")]
    TimePattern,
    #[sea_orm(string_value = "description")]
    Description,
    #[sea_orm(string_value = "none")]
    None,
}
