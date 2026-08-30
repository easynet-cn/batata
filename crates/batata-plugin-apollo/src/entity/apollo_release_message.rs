use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_release_message")]
/// Represents the `Model` entity.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i32,
    /// The `message` field.
    pub message: String,
    /// The `data_change_created_time` field.
    pub data_change_created_time: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_release_message` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
