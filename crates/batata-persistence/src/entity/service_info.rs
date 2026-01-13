//! `SeaORM` Entity for service_info table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "service_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub protect_threshold: f32,
    #[sea_orm(column_type = "Text", nullable)]
    pub metadata: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub selector: Option<String>,
    pub enabled: bool,
    pub gmt_create: DateTime,
    pub gmt_modified: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::cluster_info::Entity")]
    ClusterInfo,
    #[sea_orm(has_many = "super::instance_info::Entity")]
    InstanceInfo,
}

impl Related<super::cluster_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ClusterInfo.def()
    }
}

impl Related<super::instance_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InstanceInfo.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
