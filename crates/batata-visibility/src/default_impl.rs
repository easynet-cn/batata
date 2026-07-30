//! Default visibility service implementation
//!
//! Mirrors the built-in behavior of Nacos when no custom VisibilityService
//! is registered. Rules:
//! - Public resources: readable by everyone, writable only by owner/admin
//! - Private resources: readable/writable only by owner/admin
//! - Admin (ROLE_ADMIN) bypasses all checks

use async_trait::async_trait;

use std::sync::Arc;

use crate::constants::{ACTION_READ, ACTION_WRITE, SCOPE_PRIVATE, SCOPE_PUBLIC};
use crate::model::{
    BaseVisibilityPredicate, QueryAdvisor, ValidationResult, VisibilityQueryContext,
    VisibilityResource,
};
use crate::spi::VisibilityService;

/// Default visibility service name.
pub const DEFAULT_SERVICE_NAME: &str = "default";

/// Built-in visibility service implementing standard PUBLIC/PRIVATE logic.
///
/// # Rules
/// | Scope   | Read                | Write               |
/// |---------|---------------------|---------------------|
/// | PUBLIC  | anyone              | owner / admin only  |
/// | PRIVATE | owner / admin only  | owner / admin only  |
///
/// Admin is determined by checking if the identity equals the admin username
/// or if an `is_admin` hint is provided in the context.
pub struct DefaultVisibilityService {
    admin_usernames: Vec<String>,
    /// Whether auth is disabled (all checks bypassed).
    /// Mirrors Nacos' isAuthDisabled() check.
    auth_disabled: bool,
    /// Optional auth plugin for global admin checks.
    /// When present, admin status is verified through the auth system
    /// in addition to the hardcoded admin_usernames list.
    auth_plugin: Option<Arc<dyn batata_common::AuthPlugin>>,
}

impl DefaultVisibilityService {
    pub fn new() -> Self {
        Self {
            admin_usernames: vec!["nacos".to_string(), "admin".to_string()],
            auth_disabled: false,
            auth_plugin: None,
        }
    }

    pub fn with_admin_usernames(mut self, admins: Vec<String>) -> Self {
        self.admin_usernames = admins;
        self
    }

    /// Set whether auth is disabled. When disabled, all visibility checks are bypassed.
    /// Mirrors Nacos' isAuthDisabled() behavior.
    pub fn with_auth_disabled(mut self, disabled: bool) -> Self {
        self.auth_disabled = disabled;
        self
    }

    /// Set the auth plugin for global admin verification.
    pub fn with_auth_plugin(mut self, plugin: Arc<dyn batata_common::AuthPlugin>) -> Self {
        self.auth_plugin = Some(plugin);
        self
    }

    fn is_admin(&self, identity: &str) -> bool {
        if identity.is_empty() {
            return false;
        }
        // Hardcoded admin usernames (backward compatibility)
        if self.admin_usernames
            .iter()
            .any(|a| a.eq_ignore_ascii_case(identity))
        {
            return true;
        }
        // Auth plugin admin check (when available)
        false
    }

    async fn is_admin_async(&self, identity: &str) -> bool {
        if self.is_admin(identity) {
            return true;
        }
        if let Some(ref plugin) = self.auth_plugin {
            return plugin.is_global_admin(identity).await;
        }
        false
    }

    fn is_owner(&self, identity: &str, owner: &str) -> bool {
        !identity.is_empty() && identity == owner
    }
}

impl Default for DefaultVisibilityService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VisibilityService for DefaultVisibilityService {
    fn visibility_service_name(&self) -> &str {
        DEFAULT_SERVICE_NAME
    }

    fn resolve_default_scope_for_create(
        &self,
        _identity: &str,
        _api_type: &str,
        _resource_type: &str,
    ) -> String {
        SCOPE_PRIVATE.to_string()
    }

    async fn validate_visibility(
        &self,
        identity: &str,
        action: &str,
        _api_type: &str,
        resource: &dyn VisibilityResource,
    ) -> ValidationResult {
        // Auth disabled bypass (Nacos: isAuthDisabled)
        if self.auth_disabled {
            return ValidationResult::allow();
        }

        let scope = resource.scope();
        let owner = resource.owner();

        // Admin bypass (Nacos: isCurrentIdentityGlobalAdmin)
        if self.is_admin_async(identity).await {
            return ValidationResult::allow();
        }

        match (scope, action) {
            (SCOPE_PUBLIC, ACTION_READ) => ValidationResult::allow(),
            (SCOPE_PUBLIC, ACTION_WRITE) => {
                if self.is_owner(identity, owner) {
                    ValidationResult::allow()
                } else {
                    ValidationResult::deny(
                        "Write permission denied: public resource can only be modified by owner or admin",
                    )
                }
            }
            (SCOPE_PRIVATE, ACTION_READ) | (SCOPE_PRIVATE, ACTION_WRITE) => {
                if self.is_owner(identity, owner) {
                    ValidationResult::allow()
                } else {
                    ValidationResult::deny(
                        "Permission denied: private resource is only accessible to owner or admin",
                    )
                }
            }
            _ => ValidationResult::deny("Unknown scope or action"),
        }
    }

    async fn advise_query(
        &self,
        identity: &str,
        action: &str,
        _api_type: &str,
        context: &VisibilityQueryContext,
    ) -> QueryAdvisor {
        // Auth disabled or admin: see everything (Nacos: ALL)
        if self.auth_disabled || self.is_admin_async(identity).await {
            return QueryAdvisor::new().with_base_predicate(BaseVisibilityPredicate::All);
        }

        // Non-read (write) operations: only see own resources (Nacos: OWNER)
        if action != ACTION_READ {
            let mut advisor = QueryAdvisor::new().with_base_predicate(BaseVisibilityPredicate::Owner);
            // Populate authorized predicate with resource type (matching Nacos behavior)
            advisor = advisor.with_authorized_resources(&context.resource_type, Vec::new());
            return advisor;
        }

        // Read operations: anonymous sees PUBLIC, authenticated sees PUBLIC_AND_OWNER
        let base_predicate = if identity.is_empty() {
            BaseVisibilityPredicate::Public
        } else {
            BaseVisibilityPredicate::PublicAndOwner
        };

        let mut advisor = QueryAdvisor::new().with_base_predicate(base_predicate);
        // Populate authorized predicate with resource type (matching Nacos behavior)
        advisor = advisor.with_authorized_resources(&context.resource_type, Vec::new());
        advisor
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GenericVisibilityResource, VisibilityQueryContext};

    fn make_resource(scope: &str, owner: &str) -> GenericVisibilityResource {
        GenericVisibilityResource {
            namespace_id: "public".to_string(),
            resource_name: "test-resource".to_string(),
            resource_type: "skill".to_string(),
            scope: scope.to_string(),
            owner: owner.to_string(),
        }
    }

    fn make_query_ctx() -> VisibilityQueryContext {
        VisibilityQueryContext {
            namespace_id: "public".to_string(),
            resource_type: "skill".to_string(),
        }
    }

    // ============================================================
    // validate_visibility tests
    // ============================================================

    #[tokio::test]
    async fn test_auth_disabled_allows_everything() {
        let service = DefaultVisibilityService::new().with_auth_disabled(true);
        let resource = make_resource(SCOPE_PRIVATE, "someone_else");

        let result = service
            .validate_visibility("", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "auth disabled should allow all reads");

        let result = service
            .validate_visibility("", ACTION_WRITE, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "auth disabled should allow all writes");
    }

    #[tokio::test]
    async fn test_admin_bypasses_all_checks() {
        let service = DefaultVisibilityService::new();
        let resource = make_resource(SCOPE_PRIVATE, "someone_else");

        // "nacos" is a built-in admin
        let result = service
            .validate_visibility("nacos", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "admin should read private resources");

        let result = service
            .validate_visibility("nacos", ACTION_WRITE, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "admin should write private resources");

        // "admin" is also a built-in admin
        let result = service
            .validate_visibility("admin", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "admin user should read private resources");
    }

    #[tokio::test]
    async fn test_public_resource_readable_by_anyone() {
        let service = DefaultVisibilityService::new();
        let resource = make_resource(SCOPE_PUBLIC, "owner_user");

        // Anonymous can read public
        let result = service
            .validate_visibility("", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "anonymous should read public resources");

        // Authenticated non-owner can read public
        let result = service
            .validate_visibility("other_user", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "non-owner should read public resources");
    }

    #[tokio::test]
    async fn test_public_resource_write_only_by_owner() {
        let service = DefaultVisibilityService::new();
        let resource = make_resource(SCOPE_PUBLIC, "owner_user");

        // Owner can write
        let result = service
            .validate_visibility("owner_user", ACTION_WRITE, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "owner should write public resources");

        // Non-owner cannot write
        let result = service
            .validate_visibility("other_user", ACTION_WRITE, "admin", &resource)
            .await;
        assert!(!result.is_allowed(), "non-owner should not write public resources");
        assert!(result.reason().is_some());

        // Anonymous cannot write
        let result = service
            .validate_visibility("", ACTION_WRITE, "admin", &resource)
            .await;
        assert!(!result.is_allowed(), "anonymous should not write public resources");
    }

    #[tokio::test]
    async fn test_private_resource_only_owner_access() {
        let service = DefaultVisibilityService::new();
        let resource = make_resource(SCOPE_PRIVATE, "owner_user");

        // Owner can read
        let result = service
            .validate_visibility("owner_user", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "owner should read private resources");

        // Owner can write
        let result = service
            .validate_visibility("owner_user", ACTION_WRITE, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "owner should write private resources");

        // Non-owner cannot read
        let result = service
            .validate_visibility("other_user", ACTION_READ, "admin", &resource)
            .await;
        assert!(!result.is_allowed(), "non-owner should not read private resources");

        // Non-owner cannot write
        let result = service
            .validate_visibility("other_user", ACTION_WRITE, "admin", &resource)
            .await;
        assert!(!result.is_allowed(), "non-owner should not write private resources");

        // Anonymous cannot read
        let result = service
            .validate_visibility("", ACTION_READ, "admin", &resource)
            .await;
        assert!(!result.is_allowed(), "anonymous should not read private resources");
    }

    #[tokio::test]
    async fn test_custom_admin_usernames() {
        let service = DefaultVisibilityService::new()
            .with_admin_usernames(vec!["superuser".to_string()]);
        let resource = make_resource(SCOPE_PRIVATE, "someone_else");

        // Custom admin can access
        let result = service
            .validate_visibility("superuser", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "custom admin should read private resources");

        // Default admin no longer works
        let result = service
            .validate_visibility("nacos", ACTION_READ, "admin", &resource)
            .await;
        assert!(!result.is_allowed(), "default admin should not work with custom list");
    }

    // ============================================================
    // advise_query tests
    // ============================================================

    #[tokio::test]
    async fn test_advise_query_auth_disabled_returns_all() {
        let service = DefaultVisibilityService::new().with_auth_disabled(true);
        let ctx = make_query_ctx();

        let advisor = service.advise_query("", ACTION_READ, "admin", &ctx).await;
        assert_eq!(advisor.base_predicate, BaseVisibilityPredicate::All);
    }

    #[tokio::test]
    async fn test_advise_query_admin_returns_all() {
        let service = DefaultVisibilityService::new();
        let ctx = make_query_ctx();

        let advisor = service.advise_query("nacos", ACTION_READ, "admin", &ctx).await;
        assert_eq!(advisor.base_predicate, BaseVisibilityPredicate::All);
    }

    #[tokio::test]
    async fn test_advise_query_write_returns_owner() {
        let service = DefaultVisibilityService::new();
        let ctx = make_query_ctx();

        // Write action for authenticated non-admin returns OWNER
        let advisor = service.advise_query("user1", ACTION_WRITE, "admin", &ctx).await;
        assert_eq!(advisor.base_predicate, BaseVisibilityPredicate::Owner);
        assert_eq!(advisor.authorized_predicate.resource_type, "skill");
    }

    #[tokio::test]
    async fn test_advise_query_anonymous_read_returns_public() {
        let service = DefaultVisibilityService::new();
        let ctx = make_query_ctx();

        let advisor = service.advise_query("", ACTION_READ, "admin", &ctx).await;
        assert_eq!(advisor.base_predicate, BaseVisibilityPredicate::Public);
        assert_eq!(advisor.authorized_predicate.resource_type, "skill");
    }

    #[tokio::test]
    async fn test_advise_query_authenticated_read_returns_public_and_owner() {
        let service = DefaultVisibilityService::new();
        let ctx = make_query_ctx();

        let advisor = service.advise_query("user1", ACTION_READ, "admin", &ctx).await;
        assert_eq!(advisor.base_predicate, BaseVisibilityPredicate::PublicAndOwner);
        assert_eq!(advisor.authorized_predicate.resource_type, "skill");
    }

    // ============================================================
    // resolve_default_scope_for_create tests
    // ============================================================

    #[test]
    fn test_resolve_default_scope_is_private() {
        let service = DefaultVisibilityService::new();
        let scope = service.resolve_default_scope_for_create("user1", "admin", "skill");
        assert_eq!(scope, SCOPE_PRIVATE);
    }

    // ============================================================
    // visibility_service_name test
    // ============================================================

    #[test]
    fn test_service_name_is_default() {
        let service = DefaultVisibilityService::new();
        assert_eq!(service.visibility_service_name(), DEFAULT_SERVICE_NAME);
    }

    // ============================================================
    // AuthPlugin integration tests
    // ============================================================

    use batata_common::{AuthCheckResult, AuthPermission, IdentityContext};

    struct MockAuthPlugin {
        global_admins: Vec<String>,
    }

    #[async_trait::async_trait]
    impl batata_common::AuthPlugin for MockAuthPlugin {
        fn plugin_name(&self) -> &str {
            "mock"
        }

        async fn validate_identity(&self, _identity: &mut IdentityContext) -> AuthCheckResult {
            AuthCheckResult::success()
        }

        async fn validate_authority(
            &self,
            _identity: &IdentityContext,
            _permission: &AuthPermission,
        ) -> AuthCheckResult {
            AuthCheckResult::success()
        }

        async fn login(&self, _username: &str, _password: &str) -> Result<batata_common::LoginResult, batata_common::LoginError> {
            unimplemented!()
        }

        async fn is_global_admin(&self, username: &str) -> bool {
            self.global_admins.iter().any(|a| a == username)
        }
    }

    #[tokio::test]
    async fn test_auth_plugin_global_admin_bypass_validate() {
        let plugin = Arc::new(MockAuthPlugin {
            global_admins: vec!["global_admin".to_string()],
        });
        let service = DefaultVisibilityService::new().with_auth_plugin(plugin);
        let resource = make_resource(SCOPE_PRIVATE, "someone_else");

        let result = service
            .validate_visibility("global_admin", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "auth plugin global admin should bypass visibility checks");
    }

    #[tokio::test]
    async fn test_auth_plugin_non_admin_uses_scope_checks() {
        let plugin = Arc::new(MockAuthPlugin {
            global_admins: vec!["global_admin".to_string()],
        });
        let service = DefaultVisibilityService::new().with_auth_plugin(plugin);
        let resource = make_resource(SCOPE_PRIVATE, "owner_user");

        // Non-admin cannot read private resource
        let result = service
            .validate_visibility("regular_user", ACTION_READ, "admin", &resource)
            .await;
        assert!(!result.is_allowed(), "non-admin should not read private resources");

        // Owner can read
        let result = service
            .validate_visibility("owner_user", ACTION_READ, "admin", &resource)
            .await;
        assert!(result.is_allowed(), "owner should read private resources");
    }

    #[tokio::test]
    async fn test_auth_plugin_global_admin_bypass_advise_query() {
        let plugin = Arc::new(MockAuthPlugin {
            global_admins: vec!["global_admin".to_string()],
        });
        let service = DefaultVisibilityService::new().with_auth_plugin(plugin);
        let ctx = make_query_ctx();

        let advisor = service.advise_query("global_admin", ACTION_READ, "admin", &ctx).await;
        assert_eq!(advisor.base_predicate, BaseVisibilityPredicate::All);
    }

    #[tokio::test]
    async fn test_auth_plugin_non_admin_advise_query_unchanged() {
        let plugin = Arc::new(MockAuthPlugin {
            global_admins: vec!["global_admin".to_string()],
        });
        let service = DefaultVisibilityService::new().with_auth_plugin(plugin);
        let ctx = make_query_ctx();

        let advisor = service.advise_query("regular_user", ACTION_READ, "admin", &ctx).await;
        assert_eq!(advisor.base_predicate, BaseVisibilityPredicate::PublicAndOwner);
    }
}
