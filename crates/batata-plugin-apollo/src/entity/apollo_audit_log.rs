use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_audit_log")]
/// A distributed audit-log entry, mirroring upstream `AuditLog`.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// Trace identifier shared by entries from one request.
    pub trace_id: String,
    /// Span identifier of this operation.
    pub span_id: String,
    /// Parent span identifier, when nested.
    pub parent_span_id: Option<String>,
    /// Follows-from span identifier, when applicable.
    pub follows_from_span_id: Option<String>,
    /// The operator account or consumer id.
    pub operator: String,
    /// Upstream operation type.
    pub op_type: String,
    /// Upstream operation name.
    pub op_name: String,
    /// Human readable description.
    pub description: Option<String>,
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
/// SeaORM relation definitions for the `apollo_audit_log` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
