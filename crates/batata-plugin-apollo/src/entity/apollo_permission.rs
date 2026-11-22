use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_permission")]
/// Represents the `Model` entity.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The `permission_type` field.
    pub permission_type: i32,
    /// The `target_id` field.
    pub target_id: String,
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
/// SeaORM relation definitions for the `apollo_permission` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
