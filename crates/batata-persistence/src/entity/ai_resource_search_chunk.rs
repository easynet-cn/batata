//! `SeaORM` Entity for ai_resource_search_chunk table
//!
//! Mirrors upstream Nacos `ai_resource_search_chunk`: text chunks split out of a
//! search document, each with a content hash so unchanged chunks can be skipped.
//!
//! Upstream DDL: `nacos/.../mysql-schema.sql:276-297`, `pg-schema.sql:608-624`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_resource_search_chunk")]
/// ORM model for a row in the `ai_resource_search_chunk` table.
pub struct Model {
    /// Primary key.
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Creation timestamp.
    pub gmt_create: Option<DateTime>,
    /// Last modification timestamp.
    pub gmt_modified: Option<DateTime>,
    /// Owning search document ID.
    pub document_id: i64,
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Resource type (for example `skill`, `mcp`).
    pub resource_type: String,
    /// Resource name.
    pub resource_name: String,
    /// Resource version.
    pub resource_version: String,
    /// Chunk type (for example `description`, `capability`, `skill_content`).
    pub chunk_type: String,
    /// Raw chunk text.
    #[sea_orm(column_type = "Text")]
    pub chunk_text: String,
    /// Normalized chunk text used for matching.
    #[sea_orm(column_type = "Text")]
    pub canonical_text: String,
    /// Optional language code.
    pub language: Option<String>,
    /// Hash of the chunk content, used to skip unchanged rebuilds.
    pub chunk_hash: String,
    /// Serialized metadata JSON.
    #[sea_orm(column_type = "Text", nullable)]
    pub metadata: Option<String>,
    /// Chunk status.
    pub status: String,
}

/// Relation definitions for the `ai_resource_search_chunk` entity.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
