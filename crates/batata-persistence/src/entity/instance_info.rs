//! `SeaORM` Entity for instance_info table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "instance_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub instance_id: String,
    pub service_id: i64,
    pub cluster_name: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    #[sea_orm(column_type = "Text", nullable)]
    pub metadata: Option<String>,
    pub heartbeat_interval: Option<i32>,
    pub heartbeat_timeout: Option<i32>,
    pub ip_delete_timeout: Option<i32>,
    pub last_heartbeat: Option<DateTime>,
    pub gmt_create: DateTime,
    pub gmt_modified: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::service_info::Entity",
        from = "Column::ServiceId",
        to = "super::service_info::Column::Id"
    )]
    ServiceInfo,
}

impl Related<super::service_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ServiceInfo.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
