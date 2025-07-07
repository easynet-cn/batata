use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::entity;

pub const NACOS_CORE_AUTH_ENABLED: &str = "nacos.core.auth.enabled";
pub const NACOS_CORE_AUTH_CONSOLE_ENABLED: &str = "nacos.core.auth.console.enabled";
pub const NACOS_CORE_AUTH_ADMIN_ENABLED: &str = "nacos.core.auth.admin.enabled";
pub const NACOS_CORE_AUTH_SYSTEM_TYPE: &str = "nacos.core.auth.system.type";
pub const NACOS_CORE_AUTH_CACHING_ENABLED: &str = "nacos.core.auth.caching.enabled";
pub const NACOS_CORE_AUTH_SERVER_IDENTITY_KEY: &str = "nacos.core.auth.server.identity.key";
pub const NACOS_CORE_AUTH_SERVER_IDENTITY_VALUE: &str = "nacos.core.auth.server.identity.value";

pub const AUTH_PLUGIN_TYPE: &str = "nacos";
pub const LDAP_AUTH_PLUGIN_TYPE: &str = "ldap";
pub const GLOBAL_ADMIN_ROLE: &str = "ROLE_ADMIN";
pub const AUTHORIZATION_HEADER: &str = "Authorization";
pub const TOKEN_PREFIX: &str = "Bearer ";
pub const DEFAULT_USER: &str = "nacos";
pub const PARAM_USERNAME: &str = "username";
pub const PARAM_PASSWORD: &str = "password";
pub const CONSOLE_RESOURCE_NAME_PREFIX: &str = "console/";
pub const UPDATE_PASSWORD_ENTRY_POINT: &str = "onsole/user/password";
pub const LOCK_OPERATOR_POINT: &str = "grpc/lock";
pub const NACOS_USER_KEY: &str = "nacosuser";
pub const TOKEN_SECRET_KEY: &str = "nacos.core.auth.plugin.nacos.token.secret.key";
pub const DEFAULT_TOKEN_SECRET_KEY: &str = "";
pub const TOKEN_EXPIRE_SECONDS: &str = "nacos.core.auth.plugin.nacos.token.expire.seconds";
pub const DEFAULT_TOKEN_EXPIRE_SECONDS: i64 = 18000;
pub const NACOS_CORE_AUTH_LDAP_URL: &str = "nacos.core.auth.ldap.url";
pub const NACOS_CORE_AUTH_LDAP_BASEDC: &str = "nacos.core.auth.ldap.basedc";
pub const NACOS_CORE_AUTH_LDAP_TIMEOUT: &str = "nacos.core.auth.ldap.timeout";
pub const NACOS_CORE_AUTH_LDAP_USERDN: &str = "nacos.core.auth.ldap.userDn";
pub const NACOS_CORE_AUTH_LDAP_PASSWORD: &str = "nacos.core.auth.ldap.password";
pub const NACOS_CORE_AUTH_LDAP_FILTER_PREFIX: &str = "nacos.core.auth.ldap.filter.prefix";
pub const NACOS_CORE_AUTH_CASE_SENSITIVE: &str = "nacos.core.auth.ldap.case.sensitive";
pub const NACOS_CORE_AUTH_IGNORE_PARTIAL_RESULT_EXCEPTION: &str =
    "nacos.core.auth.ldap.ignore.partial.result.exception";
pub const LDAP_PREFIX: &str = "LDAP_";
pub const MAX_PASSWORD_LENGTH: i32 = 72;
pub const USER_PATH: &str = "/v3/auth/user";
pub const ROLE_PATH: &str = "/v3/auth/role";
pub const PERMISSION_PATH: &str = "/v3/auth/permission";

pub const USER_NOT_FOUND_MESSAGE: &str =
    "User not found! Please check user exist or password is right!";

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub namespace_id: String,
    pub group: String,
    pub name: String,
    pub _type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Default, Clone)]
pub struct AuthContext {
    pub username: String,
    pub jwt_error: Option<jsonwebtoken::errors::Error>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionTypes {
    Read,
    Write,
}

impl ActionTypes {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionTypes::Read => "r",
            ActionTypes::Write => "w",
        }
    }
}

impl std::fmt::Display for ActionTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct SignType {}

impl SignType {
    pub const NAMING: &str = "naming";
    pub const CONFIG: &str = "config";
    pub const LOCK: &str = "lock";
    pub const AI: &str = "ai";
    pub const CONSOLE: &str = "console";
    pub const SPECIFIED: &str = "specified";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiType {
    AdminApi,
    ConsoleApi,
    OpenApi,
    InnerApi,
}

impl ApiType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ApiType::AdminApi => "ADMIN_API",
            ApiType::ConsoleApi => "CONSOLE_API",
            ApiType::OpenApi => "OPEN_API",
            ApiType::InnerApi => "INNER_API",
        }
    }
}

impl std::fmt::Display for ApiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
