//! Apollo Release Message entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_release_message")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub message: String,
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
