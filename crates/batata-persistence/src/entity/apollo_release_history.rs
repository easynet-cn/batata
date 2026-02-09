//! Apollo Release History entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_release_history")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub branch_name: Option<String>,
    pub release_id: i64,
    pub previous_release_id: i64,
    pub operation: i16,
    #[sea_orm(column_type = "Text", nullable)]
    pub operation_context: Option<String>,
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
