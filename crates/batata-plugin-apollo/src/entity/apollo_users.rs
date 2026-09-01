use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_users")]
/// Represents the `Model` entity.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The `username` field.
    pub username: String,
    /// The `password` field.
    pub password: String,
    /// The `email` field.
    pub email: String,
    /// The `enabled` field.
    pub enabled: bool,
    /// The `data_change_created_by` field.
    pub data_change_created_by: String,
    /// The `data_change_created_time` field.
    pub data_change_created_time: DateTime,
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_users` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
