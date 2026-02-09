//! Apollo Gray Release Rule entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_gray_release_rule")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub branch_name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub rules: Option<String>,
    pub release_id: i64,
    pub branch_status: i16,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime>,
    pub created_by: Option<String>,
    pub created_time: Option<DateTime>,
    pub last_modified_by: Option<String>,
    pub last_modified_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
