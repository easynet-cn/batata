use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_server_config")]
/// Represents the `Model` entity.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The `key` field.
    pub key: String,
    /// The `value` field.
    pub value: String,
    /// The `comment` field.
    pub comment: Option<String>,
    /// The `data_change_created_by` field.
    pub data_change_created_by: String,
    /// The `data_change_created_time` field.
    #[serde(with = "batata_common::datetime::naive_serde")]
    pub data_change_created_time: TimeDateTime,
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    /// The `data_change_last_time` field.
    #[serde(with = "batata_common::datetime::naive_serde::option")]
    pub data_change_last_time: Option<TimeDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_server_config` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
