use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "apollo_user_token")]
/// A persisted portal user token (open-api / portal session token).
///
/// Upstream reference: `apollo-portal/.../entity/UserToken.java` plus
/// `UserTokenService`, which stores the token split into a `tokenPrefix`
/// (the first 32 chars, indexed for lookup) and a `tokenHash`
/// (`sha256` of the full token). The plaintext token is returned to the
/// caller exactly once at creation time.
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    /// The `id` field.
    pub id: i64,
    /// The `user_id` field (the owning `apollo_users.username`).
    pub user_id: String,
    /// The `name` field (human readable token name).
    pub name: String,
    /// The `token_prefix` field (first 32 chars of the plaintext token).
    pub token_prefix: String,
    /// The `token_hash` field (`sha256` of the full plaintext token).
    pub token_hash: String,
    /// The `scopes` field (optional comma separated scope list).
    pub scopes: Option<String>,
    /// The `rate_limit` field (0 means unlimited).
    pub rate_limit: i32,
    /// The `expires` field (expiry timestamp).
    pub expires: DateTime,
    /// The `last_used_time` field.
    pub last_used_time: Option<DateTime>,
    /// The `last_used_ip` field.
    pub last_used_ip: Option<String>,
    /// The `last_used_user_agent` field.
    pub last_used_user_agent: Option<String>,
    /// The `revoked_at` field.
    pub revoked_at: Option<DateTime>,
    /// The `revoked_by` field.
    pub revoked_by: Option<String>,
    /// The `is_deleted` field.
    pub is_deleted: bool,
    /// The `deleted_at` field (soft-delete marker).
    pub deleted_at: i64,
    /// The `data_change_created_by` field.
    pub data_change_created_by: String,
    /// The `data_change_created_time` field.
    pub data_change_created_time: DateTime,
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// SeaORM relation definitions for the `apollo_user_token` entity.
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
