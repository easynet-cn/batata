//! `SeaORM` Entity for cluster_info table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "cluster_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub service_id: i64,
    pub cluster_name: String,
    pub health_check_type: Option<String>,
    pub health_check_port: Option<i32>,
    pub health_check_path: Option<String>,
    pub use_instance_port: Option<bool>,
    #[sea_orm(column_type = "Text", nullable)]
    pub metadata: Option<String>,
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
