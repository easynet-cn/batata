use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_authorities")]
/// Spring Security authority granted to a portal user.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The owning portal account name.
    pub username: String,
    /// The granted authority, e.g. `ROLE_ADMIN`.
    pub authority: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_authorities` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
