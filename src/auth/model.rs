// Authentication and authorization models
// This file defines data structures for users, roles, permissions, and JWT tokens

use std::collections::HashMap;

use jsonwebtoken::errors::ErrorKind;
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
pub const UPDATE_PASSWORD_ENTRY_POINT: &str = "console/user/password";
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

pub const ONLY_IDENTITY: &str = "only_identity";

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

impl From<&entity::users::Model> for User {
    fn from(value: &entity::users::Model) -> Self {
        Self {
            username: value.username.to_string(),
            password: value.password.to_string(),
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

impl From<&entity::roles::Model> for RoleInfo {
    fn from(value: &entity::roles::Model) -> Self {
        Self {
            username: value.username.to_string(),
            role: value.role.to_string(),
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

impl From<&entity::permissions::Model> for PermissionInfo {
    fn from(value: &entity::permissions::Model) -> Self {
        Self {
            role: value.role.to_string(),
            resource: value.resource.to_string(),
            action: value.action.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub namespace_id: String,
    pub group: String,
    pub name: String,
    pub r#type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

impl Resource {
    pub const SPLITTER: &str = ":";
    pub const ANY: &str = "*";
    pub const ACTION: &str = "action";
    pub const REQUEST_CLASS: &str = "requestClass";
}

#[derive(Debug, Default, Clone)]
pub struct AuthContext {
    pub username: String,
    pub jwt_error: Option<jsonwebtoken::errors::Error>,
}

impl AuthContext {
    pub fn jwt_error_string(&self) -> String {
        if let Some(e) = &self.jwt_error {
            match e.kind() {
                ErrorKind::ExpiredSignature => "token expired!".to_string(),
                _ => e.to_string(),
            }
        } else {
            String::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User {
            username: "testuser".to_string(),
            password: "password123".to_string(),
        };
        assert_eq!(user.username, "testuser");
        assert_eq!(user.password, "password123");
    }

    #[test]
    fn test_nacos_user_creation() {
        let user = NacosUser {
            username: "admin".to_string(),
            password: "admin123".to_string(),
            token: "jwt_token".to_string(),
            global_admin: true,
        };
        assert_eq!(user.username, "admin");
        assert!(user.global_admin);
    }

    #[test]
    fn test_nacos_jwt_payload() {
        let payload = NacosJwtPayload {
            sub: "testuser".to_string(),
            exp: 1234567890,
        };
        assert_eq!(payload.sub, "testuser");
        assert_eq!(payload.exp, 1234567890);
    }

    #[test]
    fn test_role_info() {
        let role = RoleInfo {
            role: GLOBAL_ADMIN_ROLE.to_string(),
            username: "admin".to_string(),
        };
        assert_eq!(role.role, "ROLE_ADMIN");
        assert_eq!(role.username, "admin");
    }

    #[test]
    fn test_permission_info() {
        let perm = PermissionInfo {
            role: "developer".to_string(),
            resource: "config/*".to_string(),
            action: "rw".to_string(),
        };
        assert_eq!(perm.role, "developer");
        assert_eq!(perm.resource, "config/*");
        assert_eq!(perm.action, "rw");
    }

    #[test]
    fn test_resource_default() {
        let resource = Resource::default();
        assert!(resource.namespace_id.is_empty());
        assert!(resource.group.is_empty());
        assert!(resource.name.is_empty());
        assert!(resource.r#type.is_empty());
        assert!(resource.properties.is_empty());
    }

    #[test]
    fn test_resource_constants() {
        assert_eq!(Resource::SPLITTER, ":");
        assert_eq!(Resource::ANY, "*");
        assert_eq!(Resource::ACTION, "action");
        assert_eq!(Resource::REQUEST_CLASS, "requestClass");
    }

    #[test]
    fn test_resource_with_properties() {
        let mut props = HashMap::new();
        props.insert("key".to_string(), serde_json::json!("value"));

        let resource = Resource {
            namespace_id: "ns1".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            name: "my-config".to_string(),
            r#type: "config".to_string(),
            properties: props,
        };

        assert_eq!(resource.namespace_id, "ns1");
        assert_eq!(resource.group, "DEFAULT_GROUP");
        assert_eq!(resource.properties.get("key").unwrap(), "value");
    }

    #[test]
    fn test_auth_context_default() {
        let ctx = AuthContext::default();
        assert!(ctx.username.is_empty());
        assert!(ctx.jwt_error.is_none());
    }

    #[test]
    fn test_auth_context_with_username() {
        let ctx = AuthContext {
            username: "testuser".to_string(),
            jwt_error: None,
        };
        assert_eq!(ctx.username, "testuser");
        assert_eq!(ctx.jwt_error_string(), "");
    }

    #[test]
    fn test_auth_constants() {
        assert_eq!(GLOBAL_ADMIN_ROLE, "ROLE_ADMIN");
        assert_eq!(AUTHORIZATION_HEADER, "Authorization");
        assert_eq!(TOKEN_PREFIX, "Bearer ");
        assert_eq!(DEFAULT_USER, "nacos");
        assert_eq!(PARAM_USERNAME, "username");
        assert_eq!(PARAM_PASSWORD, "password");
        assert_eq!(DEFAULT_TOKEN_EXPIRE_SECONDS, 18000);
        assert_eq!(MAX_PASSWORD_LENGTH, 72);
    }

    #[test]
    fn test_nacos_user_serialization() {
        let user = NacosUser {
            username: "test".to_string(),
            password: "pass".to_string(),
            token: "tok".to_string(),
            global_admin: false,
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("globalAdmin"));
        assert!(json.contains("\"globalAdmin\":false"));
    }

    #[test]
    fn test_role_info_serialization() {
        let role = RoleInfo {
            role: "admin".to_string(),
            username: "user1".to_string(),
        };
        let json = serde_json::to_string(&role).unwrap();
        assert!(json.contains("\"role\":\"admin\""));
        assert!(json.contains("\"username\":\"user1\""));
    }
}
