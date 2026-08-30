use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_audit")]
/// Represents the `Model` entity.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i32,
    /// The `audit_key` field.
    pub audit_key: String,
    /// The `entity_name` field.
    pub entity_name: String,
    /// The `entity_id` field.
    pub entity_id: String,
    /// The `op_name` field.
    pub op_name: String,
    /// The `op_time` field.
    pub op_time: DateTime,
    /// The `op_by` field.
    pub op_by: String,
    /// The `op_client_ip` field.
    pub op_client_ip: String,
    #[sea_orm(column_type = "custom(\"LONGTEXT\")")]
    /// The `detail` field.
    pub detail: Option<String>,
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
/// SeaORM relation definitions for the `apollo_audit` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
