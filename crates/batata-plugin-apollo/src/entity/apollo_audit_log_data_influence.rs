use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_audit_log_data_influence")]
/// The entity/field change influenced by one audit-log span.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The owning audit-log span.
    pub span_id: String,
    /// Influenced entity id.
    pub influence_entity_id: String,
    /// Influenced entity name.
    pub influence_entity_name: String,
    /// Changed field name.
    pub field_name: Option<String>,
    /// Value before the operation.
    pub field_old_value: Option<String>,
    /// Value after the operation.
    pub field_new_value: Option<String>,
    /// Soft-delete marker.
    pub is_deleted: bool,
    /// Soft-delete timestamp.
    pub deleted_at: i64,
    /// Creator of the row.
    pub data_change_created_by: Option<String>,
    /// Creation timestamp.
    #[serde(with = "batata_common::datetime::naive_serde")]
    pub data_change_created_time: TimeDateTime,
    /// Last modifier of the row.
    pub data_change_last_modified_by: String,
    /// Last update timestamp.
    #[serde(with = "batata_common::datetime::naive_serde")]
    pub data_change_last_time: TimeDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_audit_log_data_influence` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
