//! `SeaORM` Entity for ai_resource table

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_resource")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub gmt_create: Option<DateTime>,
    pub gmt_modified: Option<DateTime>,
    pub name: String,
    #[sea_orm(column_name = "type")]
    pub r#type: String,
    pub c_desc: Option<String>,
    pub status: Option<String>,
    pub namespace_id: String,
    pub biz_tags: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub ext: Option<String>,
    pub c_from: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub version_info: Option<String>,
    pub meta_version: i64,
    pub scope: String,
    pub owner: String,
    pub download_count: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
