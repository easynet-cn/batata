use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_instance")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: u32,
    pub app_id: String,
    pub instance_id: String,
    pub instance_ip: String,
    pub data_center_info: Option<String>,
    pub client_ip: String,
    pub machine_name: Option<String>,
    pub host_name: Option<String>,
    pub data_change_created_by: String,
    pub data_change_created_time: DateTime,
    pub data_change_last_modified_by: Option<String>,
    pub data_change_last_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
