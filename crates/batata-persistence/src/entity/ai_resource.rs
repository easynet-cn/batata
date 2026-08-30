//! `SeaORM` Entity for ai_resource table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_resource")]
/// ORM model for a row in the `ai_resource` table.
pub struct Model {
    /// Primary key.
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Creation timestamp.
    pub gmt_create: Option<DateTime>,
    /// Last modification timestamp.
    pub gmt_modified: Option<DateTime>,
    /// Resource name.
    pub name: String,
    /// Resource type (stored in the `type` column).
    #[sea_orm(column_name = "type")]
    pub r#type: String,
    /// Optional description.
    pub c_desc: Option<String>,
    /// Optional lifecycle status.
    pub status: Option<String>,
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Optional comma-separated business tags.
    pub biz_tags: Option<String>,
    /// Optional opaque extension JSON.
    #[sea_orm(column_type = "Text", nullable)]
    pub ext: Option<String>,
    /// Origin of the resource.
    pub c_from: String,
    /// Optional serialized version metadata.
    #[sea_orm(column_type = "Text", nullable)]
    pub version_info: Option<String>,
    /// Optimistic-lock version for `version_info` updates.
    pub meta_version: i64,
    /// Visibility scope (e.g. `PUBLIC`, `PRIVATE`).
    pub scope: String,
    /// Owner username.
    pub owner: String,
    /// Number of downloads.
    pub download_count: i64,
}

/// Relation definitions for the `ai_resource` entity.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
