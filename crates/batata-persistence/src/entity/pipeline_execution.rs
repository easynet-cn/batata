//! `SeaORM` Entity for pipeline_execution table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "pipeline_execution")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub execution_id: String,
    pub resource_type: String,
    pub resource_name: String,
    pub namespace_id: Option<String>,
    pub version: Option<String>,
    pub status: String,
    #[sea_orm(column_type = "Text")]
    pub pipeline: String,
    pub create_time: i64,
    pub update_time: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
