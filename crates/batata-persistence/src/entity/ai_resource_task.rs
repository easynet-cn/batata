//! `SeaORM` Entity for ai_resource_task table
//!
//! Mirrors upstream Nacos `ai_resource_task`: a durable, lease-based async task
//! queue. Upstream uses it to drive search index convergence (see
//! `AiResourceIndexTaskConsumer`), polling due tasks, taking a lease and
//! advancing through stages.
//!
//! Upstream DDL: `nacos/.../mysql-schema.sql:302-321`, `pg-schema.sql:645-661`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_resource_task")]
/// ORM model for a row in the `ai_resource_task` table.
pub struct Model {
    /// Unique task key (primary key, not auto-increment).
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_key: String,
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Task type (upstream: `search_index`).
    pub task_type: String,
    /// Current stage (upstream: `base_index`, `llm_enhancement`).
    pub task_stage: String,
    /// Task status.
    pub status: String,
    /// Serialized task input JSON.
    #[sea_orm(column_type = "Text")]
    pub task_payload: String,
    /// Serialized task result JSON, set once the task finishes.
    #[sea_orm(column_type = "Text", nullable)]
    pub task_result: Option<String>,
    /// Number of retries spent on the current stage.
    pub retry_count: i32,
    /// Optimistic-lock revision of the task row.
    pub revision: i64,
    /// Lease token held by the worker currently processing the task.
    pub lease_token: i64,
    /// Earliest execution time, Unix epoch milliseconds.
    pub next_execute_at: i64,
    /// Lease expiry time, Unix epoch milliseconds.
    pub lease_expire_at: Option<i64>,
    /// Most recent error message.
    pub last_error: Option<String>,
    /// Creation timestamp.
    pub gmt_create: Option<DateTime>,
    /// Last modification timestamp.
    pub gmt_modified: Option<DateTime>,
}

/// Relation definitions for the `ai_resource_task` entity.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
