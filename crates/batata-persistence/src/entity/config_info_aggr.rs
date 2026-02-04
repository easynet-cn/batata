//! Aggregate configuration entity
//!
//! Stores aggregate configuration items grouped by dataId, groupId, and datumId.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "config_info_aggr")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    /// Parent configuration data ID
    pub data_id: String,
    /// Configuration group
    pub group_id: String,
    /// Unique datum ID within the aggregate
    pub datum_id: String,
    /// Configuration content
    #[sea_orm(column_type = "Text")]
    pub content: String,
    /// MD5 hash of content
    pub md5: Option<String>,
    /// Tenant/Namespace ID
    pub tenant_id: String,
    /// Application name
    pub app_name: Option<String>,
    /// Creation timestamp
    pub gmt_create: DateTime,
    /// Modification timestamp
    pub gmt_modified: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
