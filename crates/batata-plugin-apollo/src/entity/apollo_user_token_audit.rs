use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_user_token_audit")]
/// One authenticated request made with a portal user token.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The user token that authenticated the request.
    pub token_id: Option<i64>,
    /// The resolved portal account name.
    pub user_id: String,
    /// The request URI.
    pub uri: String,
    /// The HTTP method.
    pub method: String,
    /// Creation timestamp.
    #[serde(with = "batata_common::datetime::naive_serde")]
    pub data_change_created_time: TimeDateTime,
    /// Last update timestamp.
    #[serde(with = "batata_common::datetime::naive_serde")]
    pub data_change_last_time: TimeDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_user_token_audit` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
