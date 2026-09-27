//! `SeaORM` Entity for ai_resource_search_document table
//!
//! Mirrors upstream Nacos `ai_resource_search_document`: one search document per
//! AI resource version, uniquely keyed by
//! (`namespace_id`, `resource_type`, `resource_name`, `resource_version`).
//!
//! Upstream DDL: `nacos/.../mysql-schema.sql:251-271`, `pg-schema.sql:570-587`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_resource_search_document")]
/// ORM model for a row in the `ai_resource_search_document` table.
pub struct Model {
    /// Primary key.
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Creation timestamp.
    pub gmt_create: Option<DateTime>,
    /// Last modification timestamp.
    pub gmt_modified: Option<DateTime>,
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Resource type (for example `skill`, `mcp`).
    pub resource_type: String,
    /// Resource name.
    pub resource_name: String,
    /// Resource version.
    pub resource_version: String,
    /// Name shown in search results.
    pub display_name: String,
    /// Optional description.
    pub c_desc: Option<String>,
    /// Serialized tags JSON.
    #[sea_orm(column_type = "Text", nullable)]
    pub tags: Option<String>,
    /// Serialized capabilities JSON.
    #[sea_orm(column_type = "Text", nullable)]
    pub capabilities: Option<String>,
    /// Serialized representative queries JSON.
    #[sea_orm(column_type = "Text", nullable)]
    pub representative_queries: Option<String>,
    /// Serialized metadata JSON.
    #[sea_orm(column_type = "Text", nullable)]
    pub metadata: Option<String>,
    /// Digest of the indexed source content, used to skip unchanged rebuilds.
    pub source_digest: String,
    /// Document status (`enabled` or `pending`).
    pub status: String,
    /// How the document was generated (`auto` upstream).
    pub generate_mode: String,
}

/// Relation definitions for the `ai_resource_search_document` entity.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
