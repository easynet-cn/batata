use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Upstream `ServiceRegistry` table (database-discovery mode):
/// services self-register with a heartbeat and discovery serves rows whose
/// `DataChange_LastTime` is within the health-check window.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_service_registry")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    pub service_name: String,
    pub uri: String,
    pub cluster: String,
    pub metadata: Option<String>,
    pub data_change_created_time: DateTime,
    pub data_change_last_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
