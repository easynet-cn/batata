//! `SeaORM` Entity for pipeline_execution table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "pipeline_execution")]
/// ORM model for a row in the `pipeline_execution` table.
pub struct Model {
    /// Primary key: execution ID.
    #[sea_orm(primary_key, auto_increment = false)]
    pub execution_id: String,
    /// Resource type the pipeline ran for.
    pub resource_type: String,
    /// Resource name the pipeline ran for.
    pub resource_name: String,
    /// Optional owning namespace ID.
    pub namespace_id: Option<String>,
    /// Optional resource version.
    pub version: Option<String>,
    /// Execution status.
    pub status: String,
    /// Serialized pipeline definition.
    #[sea_orm(column_type = "Text")]
    pub pipeline: String,
    /// Creation time (epoch millis).
    pub create_time: i64,
    /// Last update time (epoch millis).
    pub update_time: i64,
}

/// Relation definitions for the `pipeline_execution` entity.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
