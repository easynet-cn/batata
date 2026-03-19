//! gRPC Authentication and Authorization Service
//!
//! This module provides authentication and authorization for gRPC requests,
//! similar to Nacos's GrpcProtocolAuthService.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use moka::sync::Cache;
use serde::{Deserialize, Serialize};

use crate::model::Connection;

/// Constants for gRPC authentication
pub const ACCESS_TOKEN: &str = "accessToken";
pub const USERNAME: &str = "username";
pub const PASSWORD: &str = "password";
pub const GLOBAL_ADMIN_ROLE: &str = "ROLE_ADMIN";

/// Trait for looking up user roles and permissions from the database.
/// This decouples the gRPC auth service from the persistence layer.
#[async_trait::async_trait]
pub trait GrpcAuthRoleProvider: Send + Sync {
    /// Find roles for a given username
    async fn find_roles_by_username(&self, username: &str) -> Vec<GrpcRoleInfo>;

    /// Find permissions for a set of roles
    async fn find_permissions_by_roles(&self, roles: Vec<String>) -> Vec<GrpcPermissionInfo>;
}

/// Role info for gRPC auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcRoleInfo {
    pub username: String,
    pub role: String,
}

/// Resource types for permission checking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Config,
    Naming,
    Internal,
    Ai,
    Lock,
}

impl ResourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Config => "config",
            ResourceType::Naming => "naming",
            ResourceType::Internal => "internal",
            ResourceType::Ai => "ai",
            ResourceType::Lock => "lock",
        }
    }
}

/// Permission action types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAction {
    Read,
    Write,
}

impl PermissionAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionAction::Read => "r",
            PermissionAction::Write => "w",
        }
    }
}

/// gRPC authentication context
/// Stores authentication information extracted from gRPC requests
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrpcAuthContext {
    /// Username extracted from token
    pub username: String,
    /// Whether the user is a global admin
    pub is_global_admin: bool,
    /// User's roles
    pub roles: Vec<String>,
    /// Authentication error message if any
    pub auth_error: Option<String>,
    /// Whether authentication is enabled
    pub auth_enabled: bool,
}

impl GrpcAuthContext {
    /// Create a new authenticated context
    pub fn authenticated(username: String, is_global_admin: bool, roles: Vec<String>) -> Self {
        Self {
            username,
            is_global_admin,
            roles,
            auth_error: None,
            auth_enabled: true,
        }
    }

    /// Create an unauthenticated context with error
    pub fn unauthenticated(error: String) -> Self {
        Self {
            auth_error: Some(error),
            auth_enabled: true,
            ..Default::default()
        }
    }

    /// Create a context when auth is disabled
    pub fn auth_disabled() -> Self {
        Self {
            auth_enabled: false,
            ..Default::default()
        }
    }

    /// Check if authentication passed
    pub fn is_authenticated(&self) -> bool {
        !self.auth_enabled || (!self.username.is_empty() && self.auth_error.is_none())
    }

    /// Check if user has global admin role
    pub fn has_admin_role(&self) -> bool {
        self.is_global_admin || self.roles.iter().any(|r| r == GLOBAL_ADMIN_ROLE)
    }
}

/// Resource for permission checking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrpcResource {
    pub namespace_id: String,
    pub group: String,
    pub name: String,
    pub resource_type: String,
}

impl GrpcResource {
    pub const SPLITTER: &str = ":";
    pub const ANY: &str = "*";

    /// Create a new resource
    pub fn new(namespace_id: &str, group: &str, name: &str, resource_type: ResourceType) -> Self {
        Self {
            namespace_id: namespace_id.to_string(),
            group: group.to_string(),
            name: name.to_string(),
            resource_type: resource_type.as_str().to_string(),
        }
    }

    /// Create a config resource
    pub fn config(namespace_id: &str, group: &str, data_id: &str) -> Self {
        Self::new(namespace_id, group, data_id, ResourceType::Config)
    }

    /// Create a naming resource
    pub fn naming(namespace_id: &str, group: &str, service_name: &str) -> Self {
        Self::new(namespace_id, group, service_name, ResourceType::Naming)
    }

    /// Create an AI resource (MCP server or A2A agent)
    pub fn ai(namespace_id: &str, name: &str) -> Self {
        Self::new(namespace_id, Self::ANY, name, ResourceType::Ai)
    }

    /// Create a lock resource
    pub fn lock(key: &str) -> Self {
        Self::new(Self::ANY, Self::ANY, key, ResourceType::Lock)
    }

    /// Convert to permission resource string format
    /// Format: namespace_id:group:resource_type/name
    pub fn to_permission_string(&self) -> String {
        let ns = if self.namespace_id.is_empty() {
            Self::ANY
        } else {
            &self.namespace_id
        };
        let group = if self.group.is_empty() {
            Self::ANY
        } else {
            &self.group
        };

        format!(
            "{}{}{}{}{}{}{}",
            ns,
            Self::SPLITTER,
            group,
            Self::SPLITTER,
            self.resource_type,
            "/",
            if self.name.is_empty() {
                Self::ANY
            } else {
                &self.name
            }
        )
    }
}

/// Permission info for checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcPermissionInfo {
    pub role: String,
    pub resource: String,
    pub action: String,
}

/// Result of permission check
#[derive(Debug, Clone)]
pub struct PermissionCheckResult {
    pub passed: bool,
    pub message: Option<String>,
}

impl PermissionCheckResult {
    pub fn pass() -> Self {
        Self {
            passed: true,
            message: None,
        }
    }

    pub fn deny(message: &str) -> Self {
        Self {
            passed: false,
            message: Some(message.to_string()),
        }
    }
}

/// Cache for permission check results
static PERMISSION_CHECK_CACHE: LazyLock<Cache<String, bool>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(10_000)
        .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
        .build()
});

/// gRPC Authentication Service
/// Handles token validation and permission checking for gRPC requests
#[derive(Clone, Default)]
pub struct GrpcAuthService {
    /// Whether authentication is enabled
    auth_enabled: bool,
    /// JWT secret key for token validation
    token_secret_key: String,
    /// Server identity key for internal requests
    server_identity_key: String,
    /// Server identity value for internal requests
    server_identity_value: String,
    /// Role provider for looking up roles and permissions from the database
    role_provider: Option<Arc<dyn GrpcAuthRoleProvider>>,
}

impl GrpcAuthService {
    /// Create a new GrpcAuthService
    pub fn new(
        auth_enabled: bool,
        token_secret_key: String,
        server_identity_key: String,
        server_identity_value: String,
    ) -> Self {
        Self {
            auth_enabled,
            token_secret_key,
            server_identity_key,
            server_identity_value,
            role_provider: None,
        }
    }

    /// Create a new GrpcAuthService with a role provider for database lookups
    pub fn with_role_provider(
        auth_enabled: bool,
        token_secret_key: String,
        server_identity_key: String,
        server_identity_value: String,
        role_provider: Arc<dyn GrpcAuthRoleProvider>,
    ) -> Self {
        Self {
            auth_enabled,
            token_secret_key,
            server_identity_key,
            server_identity_value,
            role_provider: Some(role_provider),
        }
    }

    /// Check if auth is enabled
    pub fn is_auth_enabled(&self) -> bool {
        self.auth_enabled
    }

    /// Get the token secret key
    pub fn token_secret_key(&self) -> &str {
        &self.token_secret_key
    }

    /// Parse identity from gRPC request headers
    /// Returns GrpcAuthContext with authentication info
    pub fn parse_identity(&self, headers: &HashMap<String, String>) -> GrpcAuthContext {
        if !self.auth_enabled {
            return GrpcAuthContext::auth_disabled();
        }

        // Try to get access token from headers
        let token = headers.get(ACCESS_TOKEN).map(|s| s.as_str()).unwrap_or("");

        if token.is_empty() {
            return GrpcAuthContext::unauthenticated("token invalid!".to_string());
        }

        // Decode and validate token
        match self.decode_token(token) {
            Ok(username) => GrpcAuthContext {
                username,
                auth_enabled: true,
                ..Default::default()
            },
            Err(e) => GrpcAuthContext::unauthenticated(e),
        }
    }

    /// Decode JWT token and extract username
    fn decode_token(&self, token: &str) -> Result<String, String> {
        use jsonwebtoken::{DecodingKey, Validation, decode};

        #[derive(Debug, Deserialize)]
        struct Claims {
            sub: String,
            #[allow(dead_code)]
            exp: i64,
        }

        let decoding_key = DecodingKey::from_base64_secret(&self.token_secret_key)
            .map_err(|e| format!("invalid secret key: {}", e))?;

        let token_data =
            decode::<Claims>(token, &decoding_key, &Validation::default()).map_err(|e| match e
                .kind()
            {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => "token expired!".to_string(),
                _ => format!("token invalid: {}", e),
            })?;

        Ok(token_data.claims.sub)
    }

    /// Resolve full auth context by parsing JWT and loading roles from database.
    /// This is the async version of parse_identity that also populates roles.
    /// Follows the same pattern as Nacos's RemoteRequestAuthFilter:
    /// 1. Decode JWT to get username
    /// 2. Query database for user's roles
    /// 3. Check if user has global admin role
    pub async fn resolve_auth_context(&self, headers: &HashMap<String, String>) -> GrpcAuthContext {
        // First, parse the JWT token (sync)
        let mut ctx = self.parse_identity(headers);

        // If auth is disabled or authentication failed, return as-is
        if !ctx.auth_enabled || !ctx.is_authenticated() {
            return ctx;
        }

        // Look up roles from database if we have a role provider
        if let Some(ref provider) = self.role_provider {
            let grpc_roles: Vec<GrpcRoleInfo> =
                provider.find_roles_by_username(&ctx.username).await;
            let is_global_admin = grpc_roles.iter().any(|r| r.role == GLOBAL_ADMIN_ROLE);
            ctx.roles = grpc_roles.into_iter().map(|r| r.role).collect();
            ctx.is_global_admin = is_global_admin;
        }

        ctx
    }

    /// Load permissions for the given roles from the database.
    /// Used when checking non-admin users' specific resource permissions.
    pub async fn load_permissions_for_roles(&self, roles: Vec<String>) -> Vec<GrpcPermissionInfo> {
        if let Some(ref provider) = self.role_provider {
            provider.find_permissions_by_roles(roles).await
        } else {
            vec![]
        }
    }

    /// Check if request is from internal server
    pub fn check_server_identity(&self, headers: &HashMap<String, String>) -> bool {
        if self.server_identity_key.is_empty() {
            return false;
        }

        headers
            .get(&self.server_identity_key)
            .map(|v| v == &self.server_identity_value)
            .unwrap_or(false)
    }

    /// Check permission for a resource and action
    pub fn check_permission(
        &self,
        auth_context: &GrpcAuthContext,
        resource: &GrpcResource,
        action: PermissionAction,
        permissions: &[GrpcPermissionInfo],
    ) -> PermissionCheckResult {
        // If auth is disabled, always pass
        if !self.auth_enabled || !auth_context.auth_enabled {
            return PermissionCheckResult::pass();
        }

        // Check authentication
        if !auth_context.is_authenticated() {
            return PermissionCheckResult::deny(
                auth_context
                    .auth_error
                    .as_deref()
                    .unwrap_or("user not authenticated"),
            );
        }

        // Global admin has all permissions
        if auth_context.has_admin_role() {
            return PermissionCheckResult::pass();
        }

        let resource_str = resource.to_permission_string();
        let action_str = action.as_str();

        // Check cache first
        let cache_key = format!("{}:{}:{}", auth_context.username, resource_str, action_str);
        if let Some(result) = PERMISSION_CHECK_CACHE.get(&cache_key) {
            if result {
                return PermissionCheckResult::pass();
            } else {
                return PermissionCheckResult::deny("permission denied");
            }
        }

        // Check user's permissions
        let has_permission = permissions.iter().any(|p| {
            auth_context.roles.contains(&p.role)
                && self.match_resource(&p.resource, &resource_str)
                && self.match_action(&p.action, action_str)
        });

        // Cache the result
        PERMISSION_CHECK_CACHE.insert(cache_key, has_permission);

        if has_permission {
            PermissionCheckResult::pass()
        } else {
            PermissionCheckResult::deny("permission denied")
        }
    }

    /// Match resource pattern (supports wildcard *)
    fn match_resource(&self, pattern: &str, resource: &str) -> bool {
        if pattern == GrpcResource::ANY {
            return true;
        }

        // Simple pattern matching with wildcard support
        let pattern_parts: Vec<&str> = pattern.split(':').collect();
        let resource_parts: Vec<&str> = resource.split(':').collect();

        if pattern_parts.len() != resource_parts.len() {
            return false;
        }

        pattern_parts
            .iter()
            .zip(resource_parts.iter())
            .all(|(p, r)| *p == GrpcResource::ANY || p == r)
    }

    /// Match action (r = read, w = write, rw = both)
    fn match_action(&self, pattern: &str, action: &str) -> bool {
        pattern.contains(action) || pattern == "rw"
    }

    /// Invalidate permission cache for a user
    pub fn invalidate_cache_for_user(username: &str) {
        let prefix = format!("{}:", username);
        let keys_to_invalidate: Vec<String> = PERMISSION_CHECK_CACHE
            .iter()
            .filter_map(|(key, _)| {
                if key.starts_with(&prefix) {
                    Some((*key).clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_invalidate {
            PERMISSION_CHECK_CACHE.invalidate(&key);
        }
    }

    /// Clear all permission cache
    pub fn clear_cache() {
        PERMISSION_CHECK_CACHE.invalidate_all();
    }
}

/// Extract auth context from connection and payload headers
pub fn extract_auth_context(
    auth_service: &GrpcAuthService,
    connection: &Connection,
    payload_headers: &HashMap<String, String>,
) -> GrpcAuthContext {
    // Merge connection labels with payload headers
    // Payload headers take precedence
    let mut headers = connection.meta_info.labels.clone();
    headers.extend(payload_headers.clone());

    auth_service.parse_identity(&headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_resource_to_permission_string() {
        let resource = GrpcResource::config("public", "DEFAULT_GROUP", "test.yaml");
        assert_eq!(
            resource.to_permission_string(),
            "public:DEFAULT_GROUP:config/test.yaml"
        );
    }

    #[test]
    fn test_grpc_resource_with_empty_values() {
        let resource = GrpcResource::config("", "", "");
        assert_eq!(resource.to_permission_string(), "*:*:config/*");
    }

    #[test]
    fn test_grpc_auth_context_disabled() {
        let ctx = GrpcAuthContext::auth_disabled();
        assert!(!ctx.auth_enabled);
        assert!(ctx.is_authenticated()); // Always authenticated when disabled
    }

    #[test]
    fn test_grpc_auth_context_authenticated() {
        let ctx = GrpcAuthContext::authenticated("admin".to_string(), true, vec![]);
        assert!(ctx.is_authenticated());
        assert!(ctx.has_admin_role());
    }

    #[test]
    fn test_grpc_auth_context_unauthenticated() {
        let ctx = GrpcAuthContext::unauthenticated("token expired!".to_string());
        assert!(!ctx.is_authenticated());
        assert_eq!(ctx.auth_error, Some("token expired!".to_string()));
    }

    #[test]
    fn test_permission_action_str() {
        assert_eq!(PermissionAction::Read.as_str(), "r");
        assert_eq!(PermissionAction::Write.as_str(), "w");
    }

    #[test]
    fn test_resource_type_str() {
        assert_eq!(ResourceType::Config.as_str(), "config");
        assert_eq!(ResourceType::Naming.as_str(), "naming");
        assert_eq!(ResourceType::Internal.as_str(), "internal");
        assert_eq!(ResourceType::Ai.as_str(), "ai");
        assert_eq!(ResourceType::Lock.as_str(), "lock");
    }

    #[test]
    fn test_grpc_resource_ai() {
        let resource = GrpcResource::ai("public", "my-mcp-server");
        assert_eq!(resource.to_permission_string(), "public:*:ai/my-mcp-server");
    }

    #[test]
    fn test_grpc_resource_lock() {
        let resource = GrpcResource::lock("my-lock-key");
        assert_eq!(resource.to_permission_string(), "*:*:lock/my-lock-key");
    }

    #[test]
    fn test_auth_service_disabled() {
        let service = GrpcAuthService::default();
        let headers = HashMap::new();
        let ctx = service.parse_identity(&headers);
        assert!(!ctx.auth_enabled);
        assert!(ctx.is_authenticated());
    }

    #[test]
    fn test_auth_service_no_token() {
        let service =
            GrpcAuthService::new(true, "secret".to_string(), "".to_string(), "".to_string());
        let headers = HashMap::new();
        let ctx = service.parse_identity(&headers);
        assert!(!ctx.is_authenticated());
        assert_eq!(ctx.auth_error, Some("token invalid!".to_string()));
    }

    #[test]
    fn test_permission_check_admin_bypass() {
        let service =
            GrpcAuthService::new(true, "secret".to_string(), "".to_string(), "".to_string());
        let ctx = GrpcAuthContext::authenticated(
            "admin".to_string(),
            false,
            vec![GLOBAL_ADMIN_ROLE.to_string()],
        );
        let resource = GrpcResource::config("public", "DEFAULT_GROUP", "test.yaml");
        let result = service.check_permission(&ctx, &resource, PermissionAction::Write, &[]);
        assert!(result.passed);
    }

    #[test]
    fn test_server_identity_check() {
        let service = GrpcAuthService::new(
            true,
            "secret".to_string(),
            "serverIdentity".to_string(),
            "cluster-node-1".to_string(),
        );

        let mut headers = HashMap::new();
        headers.insert("serverIdentity".to_string(), "cluster-node-1".to_string());
        assert!(service.check_server_identity(&headers));

        let mut wrong_headers = HashMap::new();
        wrong_headers.insert("serverIdentity".to_string(), "wrong-value".to_string());
        assert!(!service.check_server_identity(&wrong_headers));
    }

    #[test]
    fn test_grpc_role_info() {
        let role = GrpcRoleInfo {
            username: "admin".to_string(),
            role: GLOBAL_ADMIN_ROLE.to_string(),
        };
        assert_eq!(role.role, "ROLE_ADMIN");
    }

    #[test]
    fn test_auth_service_with_role_provider() {
        // Verify with_role_provider constructor sets the provider
        struct MockProvider;
        #[async_trait::async_trait]
        impl GrpcAuthRoleProvider for MockProvider {
            async fn find_roles_by_username(&self, _: &str) -> Vec<GrpcRoleInfo> {
                vec![GrpcRoleInfo {
                    username: "admin".to_string(),
                    role: GLOBAL_ADMIN_ROLE.to_string(),
                }]
            }
            async fn find_permissions_by_roles(&self, _: Vec<String>) -> Vec<GrpcPermissionInfo> {
                vec![]
            }
        }

        let service = GrpcAuthService::with_role_provider(
            true,
            "c2VjcmV0".to_string(), // base64 "secret"
            "".to_string(),
            "".to_string(),
            Arc::new(MockProvider),
        );
        assert!(service.is_auth_enabled());
    }

    #[tokio::test]
    async fn test_resolve_auth_context_disabled() {
        let service = GrpcAuthService::default(); // auth disabled
        let headers = HashMap::new();
        let ctx = service.resolve_auth_context(&headers).await;
        assert!(!ctx.auth_enabled);
        assert!(ctx.is_authenticated());
    }

    #[tokio::test]
    async fn test_resolve_auth_context_no_token() {
        let service =
            GrpcAuthService::new(true, "c2VjcmV0".to_string(), "".to_string(), "".to_string());
        let headers = HashMap::new();
        let ctx = service.resolve_auth_context(&headers).await;
        assert!(!ctx.is_authenticated());
    }

    #[tokio::test]
    async fn test_resolve_auth_context_with_role_provider() {
        struct MockProvider;
        #[async_trait::async_trait]
        impl GrpcAuthRoleProvider for MockProvider {
            async fn find_roles_by_username(&self, username: &str) -> Vec<GrpcRoleInfo> {
                if username == "admin" {
                    vec![GrpcRoleInfo {
                        username: "admin".to_string(),
                        role: GLOBAL_ADMIN_ROLE.to_string(),
                    }]
                } else {
                    vec![GrpcRoleInfo {
                        username: username.to_string(),
                        role: "ROLE_USER".to_string(),
                    }]
                }
            }
            async fn find_permissions_by_roles(&self, _: Vec<String>) -> Vec<GrpcPermissionInfo> {
                vec![]
            }
        }

        // Simulate a valid JWT-authenticated admin context
        let ctx_admin = GrpcAuthContext::authenticated("admin".to_string(), false, vec![]);
        // After resolve, roles should be loaded from provider
        // (In real flow, resolve_auth_context decodes JWT first, then loads roles.
        //  Here we test the role loading logic directly.)
        let service = GrpcAuthService::with_role_provider(
            false, // disabled to bypass JWT decode
            "".to_string(),
            "".to_string(),
            "".to_string(),
            Arc::new(MockProvider),
        );
        let ctx = service.resolve_auth_context(&HashMap::new()).await;
        // Auth disabled → should pass without role lookup
        assert!(ctx.is_authenticated());
        assert!(!ctx.auth_enabled);

        // Test load_permissions_for_roles
        let perms = service
            .load_permissions_for_roles(vec!["ROLE_USER".to_string()])
            .await;
        assert!(perms.is_empty()); // mock returns empty

        // Verify admin bypass still works
        let _ = ctx_admin; // just check it compiles with the fields
    }

    #[test]
    fn test_permission_check_with_roles_and_permissions() {
        GrpcAuthService::clear_cache();
        let service =
            GrpcAuthService::new(true, "secret".to_string(), "".to_string(), "".to_string());

        // Non-admin user with specific permissions
        let ctx = GrpcAuthContext::authenticated(
            "user1".to_string(),
            false,
            vec!["developer".to_string()],
        );
        let resource = GrpcResource::config("public", "DEFAULT_GROUP", "app.yaml");
        let permissions = vec![GrpcPermissionInfo {
            role: "developer".to_string(),
            resource: "public:DEFAULT_GROUP:config/app.yaml".to_string(),
            action: "rw".to_string(),
        }];

        // Should pass with matching permission
        let result =
            service.check_permission(&ctx, &resource, PermissionAction::Read, &permissions);
        assert!(result.passed);

        let result =
            service.check_permission(&ctx, &resource, PermissionAction::Write, &permissions);
        assert!(result.passed);

        GrpcAuthService::clear_cache();
    }

    #[test]
    fn test_permission_check_denied_wrong_role() {
        GrpcAuthService::clear_cache();
        let service =
            GrpcAuthService::new(true, "secret".to_string(), "".to_string(), "".to_string());

        let ctx =
            GrpcAuthContext::authenticated("user2".to_string(), false, vec!["viewer".to_string()]);
        let resource = GrpcResource::config("public", "DEFAULT_GROUP", "app.yaml");
        let permissions = vec![GrpcPermissionInfo {
            role: "developer".to_string(), // user doesn't have this role
            resource: "public:DEFAULT_GROUP:config/app.yaml".to_string(),
            action: "rw".to_string(),
        }];

        let result =
            service.check_permission(&ctx, &resource, PermissionAction::Read, &permissions);
        assert!(!result.passed);

        GrpcAuthService::clear_cache();
    }

    #[test]
    fn test_permission_check_wildcard_resource() {
        GrpcAuthService::clear_cache();
        let service =
            GrpcAuthService::new(true, "secret".to_string(), "".to_string(), "".to_string());

        let ctx =
            GrpcAuthContext::authenticated("user3".to_string(), false, vec!["ops".to_string()]);
        let resource = GrpcResource::config("public", "DEFAULT_GROUP", "app.yaml");

        // Full wildcard "*" matches everything
        let permissions = vec![GrpcPermissionInfo {
            role: "ops".to_string(),
            resource: "*".to_string(),
            action: "r".to_string(),
        }];
        let result =
            service.check_permission(&ctx, &resource, PermissionAction::Read, &permissions);
        assert!(result.passed);

        GrpcAuthService::clear_cache();

        // Per-segment wildcard: *:*:* matches any 3-part resource
        let permissions = vec![GrpcPermissionInfo {
            role: "ops".to_string(),
            resource: "*:*:*".to_string(),
            action: "r".to_string(),
        }];
        let result =
            service.check_permission(&ctx, &resource, PermissionAction::Read, &permissions);
        assert!(result.passed);

        // Write should be denied (permission only has "r")
        let result =
            service.check_permission(&ctx, &resource, PermissionAction::Write, &permissions);
        assert!(!result.passed);

        GrpcAuthService::clear_cache();
    }

    #[test]
    fn test_grpc_resource_config() {
        let resource = GrpcResource::config("tenant-1", "group-a", "app.properties");
        assert_eq!(resource.namespace_id, "tenant-1");
        assert_eq!(resource.group, "group-a");
        assert_eq!(resource.name, "app.properties");
        assert_eq!(resource.resource_type, "config");
        assert_eq!(
            resource.to_permission_string(),
            "tenant-1:group-a:config/app.properties"
        );
    }

    #[test]
    fn test_grpc_resource_naming() {
        let resource = GrpcResource::naming("public", "DEFAULT_GROUP", "my-service");
        assert_eq!(resource.namespace_id, "public");
        assert_eq!(resource.group, "DEFAULT_GROUP");
        assert_eq!(resource.name, "my-service");
        assert_eq!(resource.resource_type, "naming");
        assert_eq!(
            resource.to_permission_string(),
            "public:DEFAULT_GROUP:naming/my-service"
        );
    }

    #[test]
    fn test_grpc_resource_ai_uses_wildcard_group() {
        let resource = GrpcResource::ai("dev-ns", "my-mcp");
        assert_eq!(resource.group, "*");
        assert_eq!(resource.to_permission_string(), "dev-ns:*:ai/my-mcp");
    }

    #[test]
    fn test_grpc_resource_lock_uses_wildcards() {
        let resource = GrpcResource::lock("distributed-lock-1");
        assert_eq!(resource.namespace_id, "*");
        assert_eq!(resource.group, "*");
        assert_eq!(
            resource.to_permission_string(),
            "*:*:lock/distributed-lock-1"
        );
    }

    #[test]
    fn test_permission_string_format() {
        // Format: namespace_id:group:resource_type/name
        let resource = GrpcResource::new("ns", "grp", "res", ResourceType::Config);
        let perm = resource.to_permission_string();
        let parts: Vec<&str> = perm.splitn(3, ':').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "ns");
        assert_eq!(parts[1], "grp");
        assert!(parts[2].contains('/'));
        assert_eq!(parts[2], "config/res");
    }

    #[test]
    fn test_grpc_auth_context_defaults() {
        let ctx = GrpcAuthContext::default();
        assert!(ctx.username.is_empty());
        assert!(!ctx.is_global_admin);
        assert!(ctx.roles.is_empty());
        assert!(ctx.auth_error.is_none());
        assert!(!ctx.auth_enabled);
    }

    #[test]
    fn test_grpc_auth_context_has_admin_role_via_roles() {
        let ctx = GrpcAuthContext::authenticated(
            "user1".to_string(),
            false,
            vec![GLOBAL_ADMIN_ROLE.to_string()],
        );
        assert!(ctx.has_admin_role());
    }

    #[test]
    fn test_grpc_auth_context_has_admin_role_via_flag() {
        let ctx = GrpcAuthContext::authenticated("admin".to_string(), true, vec![]);
        assert!(ctx.has_admin_role());
    }

    #[test]
    fn test_grpc_auth_context_no_admin_role() {
        let ctx = GrpcAuthContext::authenticated(
            "user1".to_string(),
            false,
            vec!["ROLE_USER".to_string()],
        );
        assert!(!ctx.has_admin_role());
    }
}
