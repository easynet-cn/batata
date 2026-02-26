//! Authentication and authorization models
//!
//! This file defines data structures for users, roles, permissions, and JWT tokens

use std::collections::HashMap;

use jsonwebtoken::errors::ErrorKind;
use serde::{Deserialize, Serialize};

use batata_persistence::entity::{permissions, roles, users};

// Auth configuration keys
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

// LDAP configuration keys
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

/// Basic user information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password: String,
}

impl From<users::Model> for User {
    fn from(value: users::Model) -> Self {
        Self {
            username: value.username,
            password: value.password,
        }
    }
}

impl From<&users::Model> for User {
    fn from(value: &users::Model) -> Self {
        Self {
            username: value.username.to_string(),
            password: value.password.to_string(),
        }
    }
}

/// Nacos user with authentication token
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosUser {
    pub username: String,
    pub password: String,
    pub token: String,
    pub global_admin: bool,
}

/// JWT payload for Nacos authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosJwtPayload {
    pub sub: String,
    pub exp: i64,
}

/// Role information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    pub role: String,
    pub username: String,
}

impl From<roles::Model> for RoleInfo {
    fn from(value: roles::Model) -> Self {
        Self {
            username: value.username,
            role: value.role,
        }
    }
}

impl From<&roles::Model> for RoleInfo {
    fn from(value: &roles::Model) -> Self {
        Self {
            username: value.username.to_string(),
            role: value.role.to_string(),
        }
    }
}

/// Permission information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionInfo {
    pub role: String,
    pub resource: String,
    pub action: String,
}

impl From<permissions::Model> for PermissionInfo {
    fn from(value: permissions::Model) -> Self {
        Self {
            role: value.role,
            resource: value.resource,
            action: value.action,
        }
    }
}

impl From<&permissions::Model> for PermissionInfo {
    fn from(value: &permissions::Model) -> Self {
        Self {
            role: value.role.to_string(),
            resource: value.resource.to_string(),
            action: value.action.to_string(),
        }
    }
}

/// Resource for permission checking
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

/// Auth context passed through request extensions
#[derive(Debug, Default, Clone)]
pub struct AuthContext {
    pub username: String,
    pub jwt_error: Option<jsonwebtoken::errors::Error>,
    pub token_provided: bool,
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

/// LDAP configuration for authentication
#[derive(Debug, Clone)]
pub struct LdapConfig {
    /// LDAP server URL (e.g., ldap://localhost:389 or ldaps://localhost:636)
    pub url: String,
    /// Base DN for user search (e.g., dc=example,dc=org)
    pub base_dn: String,
    /// Admin/bind user DN for initial connection
    pub bind_dn: String,
    /// Admin/bind user password
    pub bind_password: String,
    /// User DN pattern for authentication (e.g., cn={0},dc=example,dc=org)
    /// {0} will be replaced with the username
    pub user_dn_pattern: String,
    /// Filter prefix for user search (default: uid)
    pub filter_prefix: String,
    /// Connection timeout in milliseconds
    pub timeout_ms: u64,
    /// Case-sensitive username comparison
    pub case_sensitive: bool,
    /// Ignore partial result exceptions
    pub ignore_partial_result_exception: bool,
}

impl Default for LdapConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            base_dn: String::new(),
            bind_dn: String::new(),
            bind_password: String::new(),
            user_dn_pattern: String::new(),
            filter_prefix: "uid".to_string(),
            timeout_ms: 5000,
            case_sensitive: true,
            ignore_partial_result_exception: false,
        }
    }
}

impl LdapConfig {
    /// Check if LDAP is configured (has a URL)
    pub fn is_configured(&self) -> bool {
        !self.url.is_empty()
    }

    /// Build the user DN from the pattern and username
    pub fn build_user_dn(&self, username: &str) -> String {
        if self.user_dn_pattern.is_empty() {
            // Default pattern: uid=username,base_dn
            format!("{}={},{}", self.filter_prefix, username, self.base_dn)
        } else {
            self.user_dn_pattern.replace("{0}", username)
        }
    }

    /// Build the search filter for a user
    pub fn build_search_filter(&self, username: &str) -> String {
        format!("({}={})", self.filter_prefix, username)
    }
}

/// Authentication result from any auth provider
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// Whether authentication was successful
    pub success: bool,
    /// Username (may be normalized)
    pub username: String,
    /// Error message if authentication failed
    pub error_message: Option<String>,
    /// Whether this is an LDAP user (for potential sync)
    pub is_ldap_user: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_constants() {
        assert_eq!(GLOBAL_ADMIN_ROLE, "ROLE_ADMIN");
        assert_eq!(AUTHORIZATION_HEADER, "Authorization");
        assert_eq!(TOKEN_PREFIX, "Bearer ");
        assert_eq!(DEFAULT_USER, "nacos");
        assert_eq!(DEFAULT_TOKEN_EXPIRE_SECONDS, 18000);
    }

    #[test]
    fn test_ldap_constants() {
        assert_eq!(LDAP_AUTH_PLUGIN_TYPE, "ldap");
        assert_eq!(NACOS_CORE_AUTH_LDAP_URL, "nacos.core.auth.ldap.url");
        assert_eq!(NACOS_CORE_AUTH_LDAP_BASEDC, "nacos.core.auth.ldap.basedc");
        assert_eq!(LDAP_PREFIX, "LDAP_");
    }

    #[test]
    fn test_resource_constants() {
        assert_eq!(Resource::SPLITTER, ":");
        assert_eq!(Resource::ANY, "*");
        assert_eq!(Resource::ACTION, "action");
    }

    #[test]
    fn test_auth_context_default() {
        let ctx = AuthContext::default();
        assert!(ctx.username.is_empty());
        assert!(ctx.jwt_error.is_none());
        assert!(!ctx.token_provided);
        assert_eq!(ctx.jwt_error_string(), "");
    }

    #[test]
    fn test_auth_context_token_provided() {
        let mut ctx = AuthContext::default();
        assert!(!ctx.token_provided);

        ctx.token_provided = true;
        assert!(ctx.token_provided);

        ctx.username = "admin".to_string();
        assert_eq!(ctx.username, "admin");
        assert!(ctx.token_provided);
        assert!(ctx.jwt_error.is_none());
    }

    #[test]
    fn test_ldap_config_default() {
        let config = LdapConfig::default();
        assert!(!config.is_configured());
        assert_eq!(config.filter_prefix, "uid");
        assert_eq!(config.timeout_ms, 5000);
        assert!(config.case_sensitive);
    }

    #[test]
    fn test_ldap_config_build_user_dn() {
        let mut config = LdapConfig {
            base_dn: "dc=example,dc=org".to_string(),
            filter_prefix: "uid".to_string(),
            ..Default::default()
        };

        // Default pattern
        assert_eq!(config.build_user_dn("john"), "uid=john,dc=example,dc=org");

        // Custom pattern
        config.user_dn_pattern = "cn={0},ou=users,dc=example,dc=org".to_string();
        assert_eq!(
            config.build_user_dn("john"),
            "cn=john,ou=users,dc=example,dc=org"
        );
    }

    #[test]
    fn test_ldap_config_build_search_filter() {
        let mut config = LdapConfig {
            filter_prefix: "uid".to_string(),
            ..Default::default()
        };

        assert_eq!(config.build_search_filter("john"), "(uid=john)");

        config.filter_prefix = "cn".to_string();
        assert_eq!(config.build_search_filter("john"), "(cn=john)");
    }

    #[test]
    fn test_user_creation() {
        let user = User {
            username: "test".to_string(),
            password: "password".to_string(),
        };
        assert_eq!(user.username, "test");
    }
}
