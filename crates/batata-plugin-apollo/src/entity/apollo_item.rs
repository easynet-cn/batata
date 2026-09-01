use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_item")]
/// Represents the `Model` entity.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The `namespace_id` field.
    pub namespace_id: i64,
    /// The `key` field.
    pub key: String,
    /// The `type` field.
    pub r#type: i32,
    #[sea_orm(column_type = "custom(\"LONGTEXT\")")]
    /// The `value` field.
    pub value: String,
    /// The `comment` field.
    pub comment: Option<String>,
    /// The `line_num` field.
    pub line_num: i32,
    /// The `is_deleted` field.
    pub is_deleted: bool,
    /// The `deleted_at` field.
    pub deleted_at: i64,
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
/// SeaORM relation definitions for the `apollo_item` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
