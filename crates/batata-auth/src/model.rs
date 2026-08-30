//! Authentication and authorization models
//!
//! This file defines data structures for users, roles, permissions, and JWT tokens

use std::collections::HashMap;

use jsonwebtoken::errors::ErrorKind;
use serde::{Deserialize, Serialize};

use batata_persistence::entity::{permissions, roles, users};

// Auth configuration keys
/// Config key enabling the auth subsystem globally.
pub const AUTH_ENABLED_KEY: &str = "batata.core.auth.enabled";
/// Config key enabling the console (UI) auth.
pub const AUTH_CONSOLE_ENABLED_KEY: &str = "batata.core.auth.console.enabled";
/// Config key enabling the admin API auth.
pub const AUTH_ADMIN_ENABLED_KEY: &str = "batata.core.auth.admin.enabled";
/// Config key selecting the active auth system type (e.g. "nacos", "ldap").
pub const AUTH_SYSTEM_TYPE_KEY: &str = "batata.core.auth.system.type";
/// Config property holding the server identity key.
pub const AUTH_SERVER_IDENTITY_KEY_PROP: &str = "batata.core.auth.server.identity.key";
/// Config property holding the server identity value.
pub const AUTH_SERVER_IDENTITY_VALUE_PROP: &str = "batata.core.auth.server.identity.value";

/// Role name granted global administrator privileges.
pub const GLOBAL_ADMIN_ROLE: &str = "ROLE_ADMIN";
/// HTTP header used to carry credentials (`Authorization`).
pub const AUTHORIZATION_HEADER: &str = "Authorization";
/// Prefix prepended to a JWT in the `Authorization` header.
pub const TOKEN_PREFIX: &str = "Bearer ";
/// Prefix for console-scoped resource names.
pub const CONSOLE_RESOURCE_NAME_PREFIX: &str = "console/";
/// Console endpoint used to update the current user's password.
pub const UPDATE_PASSWORD_ENTRY_POINT: &str = "console/user/password";

/// Config key controlling the default JWT token lifetime, in seconds.
pub const TOKEN_EXPIRE_SECONDS: &str = "batata.core.auth.plugin.default.token.expire.seconds";
/// Default JWT token lifetime, in seconds (5 hours).
pub const DEFAULT_TOKEN_EXPIRE_SECONDS: i64 = 18000;

// LDAP configuration keys
/// Config key for the LDAP server URL.
pub const AUTH_LDAP_URL: &str = "batata.core.auth.ldap.url";
/// Config key for the LDAP base distinguished name.
pub const AUTH_LDAP_BASE_DC: &str = "batata.core.auth.ldap.base_dc";
/// Config key for the LDAP bind (admin) DN.
pub const AUTH_LDAP_BIND_DN: &str = "batata.core.auth.ldap.bind_dn";
/// Config key for the LDAP bind password.
pub const AUTH_LDAP_PASSWORD: &str = "batata.core.auth.ldap.password";
/// Config key for the LDAP user DN pattern (e.g. `cn={0},dc=example,dc=org`).
pub const AUTH_LDADP_USER_DN_PATTERN: &str = "batata.core.auth.ldap.user_dn_pattern";
/// Config key for the LDAP user search filter prefix (e.g. `uid`).
pub const AUTH_LDAP_FILTER_PREFIX: &str = "batata.core.auth.ldap.filter.prefix";
/// Config key for the LDAP connection timeout, in milliseconds.
pub const AUTH_LDAP_TIMEOUT: &str = "batata.core.auth.ldap.timeout";
/// Config key toggling case-sensitive LDAP username comparison.
pub const AUTH_LDAP_CASE_SENSITIVE: &str = "batata.core.auth.ldap.case.sensitive";
/// Config key toggling whether to ignore LDAP partial result exceptions.
pub const AUTH_LDAP_IGNORE_PARTIAL_RESULT_EXCEPTION: &str =
    "batata.core.auth.ldap.ignore.partial.result.exception";

/// Maximum accepted password length, matching the bcrypt input limit.
pub const MAX_PASSWORD_LENGTH: i32 = 72;
/// Marker value indicating only an identity (no password) is provided.
pub const ONLY_IDENTITY: &str = "only_identity";

// ============================================================================
// User source markers
// ============================================================================
//
// Aligned with Nacos: a user account may originate from different identity
// providers. Local-source users authenticate via password; OAuth/LDAP users
// authenticate via their respective providers and password login is rejected.

/// Built-in account whose password lives in the database.
pub const USER_SOURCE_LOCAL: &str = "local";
/// User auto-provisioned during an OAuth2/OIDC login.
pub const USER_SOURCE_OAUTH: &str = "oauth";
/// User auto-provisioned during an LDAP login.
pub const USER_SOURCE_LDAP: &str = "ldap";

/// Sentinel stored in the `password` column for non-local users. The leading
/// `!` makes it impossible for `bcrypt::verify` to ever match (bcrypt hashes
/// always start with `$2`), giving us a guaranteed unmatchable password hash
/// without resorting to NULL columns or random bcrypt of a UUID.
pub const NON_LOCAL_PASSWORD_SENTINEL: &str = "!non-local-user!";

/// Resolve a possibly missing source string to a canonical value.
///
/// Existing rows from older schemas may have NULL/empty `source`; treat those
/// as `local` for backwards compatibility.
pub fn normalize_user_source(source: Option<&str>) -> String {
    match source {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => USER_SOURCE_LOCAL.to_string(),
    }
}

/// Returns true when the given source allows password-based login.
pub fn source_allows_password_login(source: &str) -> bool {
    source == USER_SOURCE_LOCAL
}

/// Basic user information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique login name for the user.
    pub username: String,
    /// Hashed password (bcrypt) for local users; sentinel for external users.
    pub password: String,
    /// Identity provider that owns this account: one of
    /// [`USER_SOURCE_LOCAL`], [`USER_SOURCE_OAUTH`], [`USER_SOURCE_LDAP`].
    #[serde(default = "default_user_source")]
    pub source: String,
}

fn default_user_source() -> String {
    USER_SOURCE_LOCAL.to_string()
}

impl From<users::Model> for User {
    fn from(value: users::Model) -> Self {
        Self {
            username: value.username,
            password: value.password,
            source: normalize_user_source(value.source.as_deref()),
        }
    }
}

impl From<&users::Model> for User {
    fn from(value: &users::Model) -> Self {
        Self {
            username: value.username.to_string(),
            password: value.password.to_string(),
            source: normalize_user_source(value.source.as_deref()),
        }
    }
}

/// Authenticated user with JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUser {
    /// Authenticated username.
    pub username: String,
    /// Hashed password for the user (may be a sentinel for external users).
    pub password: String,
    /// Issued JWT bearer token.
    pub token: String,
    /// Whether the user holds the global admin role.
    pub global_admin: bool,
}

/// JWT payload for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JwtPayload {
    /// Subject (username) the token was issued for.
    pub sub: String,
    /// Expiry timestamp in seconds since the UNIX epoch.
    pub exp: i64,
}

/// Role information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    /// Role name bound to the user.
    pub role: String,
    /// Username that holds the role.
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
    /// Role this permission is granted to.
    pub role: String,
    /// Resource pattern the permission applies to.
    pub resource: String,
    /// Allowed action(s), e.g. `r`, `w`, or `rw`.
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
    /// Namespace identifier the resource belongs to.
    pub namespace_id: String,
    /// Group the resource belongs to.
    pub group: String,
    /// Resource name.
    pub name: String,
    /// Resource type (e.g. `config`, `naming`).
    pub r#type: String,
    /// Arbitrary properties attached to the resource.
    pub properties: HashMap<String, serde_json::Value>,
}

impl Resource {
    /// Separator used between resource identifier segments.
    pub const SPLITTER: &str = ":";
    /// Wildcard matching any value in a resource segment.
    pub const ANY: &str = "*";
    /// Property key for the requested action.
    pub const ACTION: &str = "action";
    /// Property key for the request class.
    pub const REQUEST_CLASS: &str = "requestClass";
}

/// Auth context passed through request extensions
#[derive(Debug, Default, Clone)]
pub struct AuthContext {
    /// Username resolved from the request, if any.
    pub username: String,
    /// Parsing/validation error associated with the JWT, if any.
    pub jwt_error: Option<jsonwebtoken::errors::Error>,
    /// Whether a token was supplied in the request.
    pub token_provided: bool,
}

impl AuthContext {
    /// Return a stable, human-readable description of the JWT error, or an
    /// empty string when no error is present (e.g. no token was supplied).
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

/// Escape special characters in an LDAP filter value per RFC 4515.
fn ldap_escape_filter_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => escaped.push_str("\\5c"),
            '*' => escaped.push_str("\\2a"),
            '(' => escaped.push_str("\\28"),
            ')' => escaped.push_str("\\29"),
            '\0' => escaped.push_str("\\00"),
            _ => escaped.push(c),
        }
    }
    escaped
}

impl LdapConfig {
    /// Check if LDAP is configured (has a URL)
    pub fn is_configured(&self) -> bool {
        !self.url.is_empty()
    }

    /// Build the user DN from the pattern and username
    pub fn build_user_dn(&self, username: &str) -> String {
        let escaped = ldap_escape_filter_value(username);
        if self.user_dn_pattern.is_empty() {
            // Default pattern: uid=username,base_dn
            format!("{}={},{}", self.filter_prefix, escaped, self.base_dn)
        } else {
            self.user_dn_pattern.replace("{0}", &escaped)
        }
    }

    /// Build the search filter for a user
    pub fn build_search_filter(&self, username: &str) -> String {
        let escaped = ldap_escape_filter_value(username);
        format!("({}={})", self.filter_prefix, escaped)
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
        assert_eq!(DEFAULT_TOKEN_EXPIRE_SECONDS, 18000);
    }

    #[test]
    fn test_ldap_constants() {
        assert_eq!(AUTH_LDAP_URL, "batata.core.auth.ldap.url");
        assert_eq!(AUTH_LDAP_BASE_DC, "batata.core.auth.ldap.base_dc");
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
    fn test_ldap_escape_filter_value() {
        assert_eq!(ldap_escape_filter_value("normal"), "normal");
        assert_eq!(ldap_escape_filter_value("user*"), "user\\2a");
        assert_eq!(ldap_escape_filter_value("user(name)"), "user\\28name\\29");
        assert_eq!(ldap_escape_filter_value("back\\slash"), "back\\5cslash");
        assert_eq!(ldap_escape_filter_value("null\0byte"), "null\\00byte");
        assert_eq!(ldap_escape_filter_value("*()\\\0"), "\\2a\\28\\29\\5c\\00");
    }

    #[test]
    fn test_ldap_filter_injection_prevented() {
        let config = LdapConfig {
            filter_prefix: "uid".to_string(),
            ..Default::default()
        };
        assert_eq!(
            config.build_search_filter("user)(uid=*)"),
            "(uid=user\\29\\28uid=\\2a\\29)"
        );
    }

    #[test]
    fn test_user_creation() {
        let user = User {
            username: "test".to_string(),
            password: "password".to_string(),
            source: USER_SOURCE_LOCAL.to_string(),
        };
        assert_eq!(user.username, "test");
        assert_eq!(user.source, "local");
    }

    #[test]
    fn test_authenticated_user_serialization() {
        let user = AuthenticatedUser {
            username: "admin".to_string(),
            password: "hashed".to_string(),
            token: "jwt.token.here".to_string(),
            global_admin: true,
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("\"globalAdmin\":true"));
        assert!(json.contains("\"username\":\"admin\""));
    }

    #[test]
    fn test_authenticated_user_deserialization() {
        let json = r#"{"username":"admin","password":"pass","token":"tok","globalAdmin":false}"#;
        let user: AuthenticatedUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.username, "admin");
        assert!(!user.global_admin);
    }

    #[test]
    fn test_jwt_payload_serialization() {
        let payload = JwtPayload {
            sub: "testuser".to_string(),
            exp: 1700000000,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: JwtPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.sub, "testuser");
        assert_eq!(deserialized.exp, 1700000000);
    }

    #[test]
    fn test_role_info_serialization() {
        let role = RoleInfo {
            role: "ROLE_ADMIN".to_string(),
            username: "admin".to_string(),
        };
        let json = serde_json::to_string(&role).unwrap();
        assert!(json.contains("\"role\":\"ROLE_ADMIN\""));

        let deserialized: RoleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, "ROLE_ADMIN");
        assert_eq!(deserialized.username, "admin");
    }

    #[test]
    fn test_permission_info_serialization() {
        let perm = PermissionInfo {
            role: "developer".to_string(),
            resource: "public:*:config/*".to_string(),
            action: "rw".to_string(),
        };
        let json = serde_json::to_string(&perm).unwrap();
        let deserialized: PermissionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, "developer");
        assert_eq!(deserialized.resource, "public:*:config/*");
        assert_eq!(deserialized.action, "rw");
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
    fn test_resource_with_properties() {
        let mut resource = Resource {
            namespace_id: "public".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            name: "app.properties".to_string(),
            r#type: "config".to_string(),
            ..Default::default()
        };
        resource.properties.insert(
            Resource::ACTION.to_string(),
            serde_json::Value::String("r".to_string()),
        );
        assert_eq!(resource.properties.len(), 1);
    }

    #[test]
    fn test_auth_context_jwt_error_expired() {
        let ctx = AuthContext {
            jwt_error: Some(jsonwebtoken::errors::Error::from(
                ErrorKind::ExpiredSignature,
            )),
            ..Default::default()
        };
        assert_eq!(ctx.jwt_error_string(), "token expired!");
    }

    #[test]
    fn test_auth_context_jwt_error_invalid() {
        let ctx = AuthContext {
            jwt_error: Some(jsonwebtoken::errors::Error::from(ErrorKind::InvalidToken)),
            ..Default::default()
        };
        let error_str = ctx.jwt_error_string();
        assert!(!error_str.is_empty());
        assert_ne!(error_str, "token expired!");
    }

    #[test]
    fn test_auth_result_creation() {
        let result = AuthResult {
            success: true,
            username: "admin".to_string(),
            error_message: None,
            is_ldap_user: false,
        };
        assert!(result.success);
        assert!(!result.is_ldap_user);
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_auth_result_failure() {
        let result = AuthResult {
            success: false,
            username: "user".to_string(),
            error_message: Some("Invalid credentials".to_string()),
            is_ldap_user: true,
        };
        assert!(!result.success);
        assert!(result.is_ldap_user);
        assert_eq!(result.error_message.unwrap(), "Invalid credentials");
    }

    #[test]
    fn test_ldap_config_not_configured_when_empty_url() {
        let config = LdapConfig {
            url: "".to_string(),
            ..Default::default()
        };
        assert!(!config.is_configured());
    }

    #[test]
    fn test_ldap_config_configured_with_url() {
        let config = LdapConfig {
            url: "ldap://localhost:389".to_string(),
            ..Default::default()
        };
        assert!(config.is_configured());
    }

    #[test]
    fn test_ldap_config_build_user_dn_with_custom_prefix() {
        let config = LdapConfig {
            base_dn: "dc=company,dc=com".to_string(),
            filter_prefix: "cn".to_string(),
            ..Default::default()
        };
        assert_eq!(config.build_user_dn("john"), "cn=john,dc=company,dc=com");
    }

    #[test]
    fn test_user_from_model() {
        let model = users::Model {
            username: "testuser".to_string(),
            password: "hashed_password".to_string(),
            enabled: true,
            source: Some(USER_SOURCE_LOCAL.to_string()),
        };
        let user = User::from(model);
        assert_eq!(user.username, "testuser");
        assert_eq!(user.password, "hashed_password");
        assert_eq!(user.source, "local");
    }

    #[test]
    fn test_role_info_from_model() {
        let model = roles::Model {
            username: "admin".to_string(),
            role: "ROLE_ADMIN".to_string(),
        };
        let role_info = RoleInfo::from(&model);
        assert_eq!(role_info.username, "admin");
        assert_eq!(role_info.role, "ROLE_ADMIN");
    }

    #[test]
    fn test_permission_info_from_model() {
        let model = permissions::Model {
            role: "dev".to_string(),
            resource: "public:*:config/*".to_string(),
            action: "r".to_string(),
        };
        let perm = PermissionInfo::from(&model);
        assert_eq!(perm.role, "dev");
        assert_eq!(perm.resource, "public:*:config/*");
        assert_eq!(perm.action, "r");
    }

    #[test]
    fn test_max_password_length() {
        assert_eq!(MAX_PASSWORD_LENGTH, 72);
    }
}
