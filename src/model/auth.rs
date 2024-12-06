use serde::{Deserialize, Serialize};

use crate::entity;

pub const DEFAULT_TOKEN_EXPIRE_SECONDS: i64 = 1800;
pub const GLOBAL_ADMIN_ROLE: &str = "ROLE_ADMIN";
pub const DEFAULT_USER: &str = "nacos";

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password: String,
}

impl From<entity::users::Model> for User {
    fn from(value: entity::users::Model) -> Self {
        Self {
            username: value.username,
            password: value.password,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosUser {
    pub username: String,
    pub password: String,
    pub token: String,
    pub global_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosJwtPayload {
    pub sub: String,
    pub exp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    pub role: String,
    pub username: String,
}

impl From<entity::roles::Model> for RoleInfo {
    fn from(value: entity::roles::Model) -> Self {
        Self {
            username: value.username,
            role: value.role,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionInfo {
    pub role: String,
    pub resource: String,
    pub action: String,
}

impl From<entity::permissions::Model> for PermissionInfo {
    fn from(value: entity::permissions::Model) -> Self {
        Self {
            role: value.role,
            resource: value.resource,
            action: value.action,
        }
    }
}
