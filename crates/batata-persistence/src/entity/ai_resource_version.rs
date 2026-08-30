//! `SeaORM` Entity for ai_resource_version table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_resource_version")]
/// ORM model for a row in the `ai_resource_version` table.
pub struct Model {
    /// Primary key.
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Creation timestamp.
    pub gmt_create: Option<DateTime>,
    /// Last modification timestamp.
    pub gmt_modified: Option<DateTime>,
    /// Resource type (stored in the `type` column).
    #[sea_orm(column_name = "type")]
    pub r#type: String,
    /// Optional author username.
    pub author: Option<String>,
    /// Resource name.
    pub name: String,
    /// Optional description.
    pub c_desc: Option<String>,
    /// Lifecycle status.
    pub status: String,
    /// Version string.
    pub version: String,
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Optional serialized storage location.
    #[sea_orm(column_type = "Text", nullable)]
    pub storage: Option<String>,
    /// Optional serialized publish pipeline metadata.
    #[sea_orm(column_type = "Text", nullable)]
    pub publish_pipeline_info: Option<String>,
    /// Number of downloads.
    pub download_count: i64,
}

/// Relation definitions for the `ai_resource_version` entity.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
