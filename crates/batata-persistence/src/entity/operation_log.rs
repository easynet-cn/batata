//! Operation log entity for audit logging
//!
//! Tracks all operations performed on the system for audit purposes.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "operation_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    /// Operation type: CREATE, UPDATE, DELETE, QUERY, LOGIN, LOGOUT, etc.
    pub operation: String,
    /// Resource type: CONFIG, SERVICE, INSTANCE, NAMESPACE, USER, ROLE, PERMISSION
    pub resource_type: String,
    /// Resource identifier (e.g., dataId@@groupId@@tenantId for config)
    #[sea_orm(column_type = "Text", nullable)]
    pub resource_id: Option<String>,
    /// Tenant/Namespace ID
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
    /// User who performed the operation
    pub operator: String,
    /// Source IP address
    #[sea_orm(column_type = "Text", nullable)]
    pub source_ip: Option<String>,
    /// Operation result: SUCCESS, FAILURE
    pub result: String,
    /// Error message if operation failed
    #[sea_orm(column_type = "Text", nullable)]
    pub error_message: Option<String>,
    /// Additional details in JSON format
    #[sea_orm(column_type = "Text", nullable)]
    pub details: Option<String>,
    /// When the operation occurred
    pub gmt_create: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
