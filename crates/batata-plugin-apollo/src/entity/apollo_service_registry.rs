use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Upstream `ServiceRegistry` table (database-discovery mode):
/// services self-register with a heartbeat and discovery serves rows whose
/// `DataChange_LastTime` is within the health-check window.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_service_registry")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The `service_name` field.
    pub service_name: String,
    /// The `uri` field.
    pub uri: String,
    /// The `cluster` field.
    pub cluster: String,
    /// The `metadata` field.
    pub metadata: Option<String>,
    /// The `data_change_created_time` field.
    pub data_change_created_time: DateTime,
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_service_registry` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
