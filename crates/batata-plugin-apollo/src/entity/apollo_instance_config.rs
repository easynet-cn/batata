use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_instance_config")]
/// Represents the `Model` entity.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The `instance_id` field.
    pub instance_id: i64,
    /// The `config_app_id` field.
    pub config_app_id: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `release_key` field.
    pub release_key: String,
    /// The `configurations` field.
    pub configurations: Option<String>,
    /// The `data_change_created_by` field.
    pub data_change_created_by: String,
    /// The `data_change_created_time` field.
    #[serde(with = "batata_common::datetime::naive_serde")]
    pub data_change_created_time: TimeDateTime,
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    /// The `data_change_last_time` field.
    #[serde(with = "batata_common::datetime::naive_serde::option")]
    pub data_change_last_time: Option<TimeDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_instance_config` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
