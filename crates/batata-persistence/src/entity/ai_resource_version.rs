//! `SeaORM` Entity for ai_resource_version table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_resource_version")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub gmt_create: Option<DateTime>,
    pub gmt_modified: Option<DateTime>,
    #[sea_orm(column_name = "type")]
    pub r#type: String,
    pub author: Option<String>,
    pub name: String,
    pub c_desc: Option<String>,
    pub status: String,
    pub version: String,
    pub namespace_id: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub storage: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub publish_pipeline_info: Option<String>,
    pub download_count: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
