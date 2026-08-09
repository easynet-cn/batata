//! Visibility grant service
//!
//! Implements Nacos-compatible visibility authorization grants.
//! Maps to Nacos `VisibilityGrantService` / `DefaultVisibilityGrantService`
//! and `VisibilityGrantRoleHelper`.
//!
//! Visibility grants allow a resource owner or administrator to delegate
//! read (or read-write) visibility on a specific resource to another user.
//! Internally each grantee gets a dedicated role whose name is derived from
//! a SHA-256 prefix of the username, and a permission row whose resource
//! identifier encodes the namespace / resource-type / resource-name triple.

use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use batata_persistence::PersistenceService;

// ============================================================================
// Constants — aligned with Nacos AuthConstants / VisibilityGrantRoleHelper
// ============================================================================

/// Visibility API path (Nacos: `AuthConstants.VISIBILITY_PATH`).
pub const VISIBILITY_PATH: &str = "/v3/auth/visibility";

/// Secured resource identifier for the visibility grant endpoint
/// (Nacos: `AuthConstants.VISIBILITY_RESOURCE`).
pub const VISIBILITY_RESOURCE: &str = "auth/visibility";

/// Prefix for internal visibility grant roles
/// (Nacos: `AuthConstants.VISIBILITY_GRANT_ROLE_PREFIX`).
pub const VISIBILITY_GRANT_ROLE_PREFIX: &str = "__nacos_vis__.";

/// Prefix of the persisted resource identifier.
const RESOURCE_IDENTIFIER_PREFIX: &str = "@@visibility/";

/// Marker inserted between the role prefix and the username hash.
const USER_ROLE_MARKER: &str = "u.";

/// Number of hex characters taken from the SHA-256 digest.
/// Keeps role names within the `roles.role` varchar(50) limit.
const USER_ROLE_HASH_HEX_LENGTH: usize = 32;

/// Default namespace when none is supplied.
const DEFAULT_NAMESPACE_ID: &str = "public";

// ============================================================================
// Trait
// ============================================================================

/// Service for plugin-owned visibility grants.
///
/// Equivalent to the Nacos `VisibilityGrantService` interface.
#[async_trait]
pub trait VisibilityGrantService: Send + Sync {
    /// Grant visibility access to one user.
    ///
    /// * `namespace_id` - namespace ID, blank for the default namespace
    /// * `resource_type` - resource type (e.g. "config", "naming")
    /// * `resource_name` - resource name
    /// * `username` - grantee username
    /// * `action` - requested grant action: `r`, `w`, or `rw`
    async fn grant(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()>;

    /// Revoke visibility access from one user.
    ///
    /// * `namespace_id` - namespace ID, blank for the default namespace
    /// * `resource_type` - resource type
    /// * `resource_name` - resource name
    /// * `username` - grantee username
    /// * `action` - grant action to revoke: `r`, `w`, or `rw`
    async fn revoke(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()>;

    /// Find explicitly authorized resource names for one user and query action.
    ///
    /// * `username` - grantee username
    /// * `namespace_id` - namespace ID, blank for the default namespace
    /// * `resource_type` - resource type
    /// * `action` - query action (`r` matches `r` and `rw`; `rw` matches only `rw`)
    async fn find_authorized_resource_names(
        &self,
        username: &str,
        namespace_id: &str,
        resource_type: &str,
        action: &str,
    ) -> anyhow::Result<Vec<String>>;
}

// ============================================================================
// Default implementation
// ============================================================================

/// Default implementation of [`VisibilityGrantService`].
///
/// Wraps a [`PersistenceService`] to create / delete dedicated visibility
/// roles and permission rows. Local role / permission caches are invalidated
/// in-place; the caller (API handler) is responsible for clearing the gRPC
/// auth cache via `GrpcAuthService::clear_cache()`.
pub struct DefaultVisibilityGrantService {
    persistence: Arc<dyn PersistenceService>,
}

impl DefaultVisibilityGrantService {
    /// Create a new service backed by the given persistence layer.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }
}

#[async_trait]
impl VisibilityGrantService for DefaultVisibilityGrantService {
    async fn grant(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        // a. Validate resourceType and resourceName non-empty
        validate_resource_type_and_name(resource_type, resource_name)?;

        // b. Validate username non-empty and user exists
        validate_username(username)?;
        let user = self.persistence.user_find_by_username(username).await?;
        if user.is_none() {
            return Err(anyhow::anyhow!("user '{}' not found", username));
        }

        // c. Normalize action
        let stored_action = normalize_stored_action(action)?;

        // d. Build role name and resource identifier
        let role_name = build_user_role_name(username);
        let resource_id = build_resource_identifier(namespace_id, resource_type, resource_name);

        // e. If user already has the dedicated role, only add the permission
        //    (skip when the permission row already exists — dedup check).
        let roles = self.persistence.role_find_by_username(username).await?;
        if roles.iter().any(|r| r.role == role_name) {
            let existing = self
                .persistence
                .permission_find_by_id(&role_name, &resource_id, &stored_action)
                .await?;
            if existing.is_none() {
                self.persistence
                    .permission_grant(&role_name, &resource_id, &stored_action)
                    .await?;
                invalidate_caches(&role_name, username);
            }
            return Ok(());
        }

        // f. Otherwise create the role first, then attach the permission.
        //    If permission creation fails, roll back the role to keep the
        //    grant operation atomic.
        self.persistence.role_create(&role_name, username).await?;
        match self
            .persistence
            .permission_grant(&role_name, &resource_id, &stored_action)
            .await
        {
            Ok(()) => {
                invalidate_caches(&role_name, username);
                Ok(())
            }
            Err(e) => {
                // Rollback the role binding we just created.
                let _ = self.persistence.role_delete(&role_name, username).await;
                invalidate_caches(&role_name, username);
                Err(e)
            }
        }
    }

    async fn revoke(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        // a. Validate resourceType and resourceName non-empty
        validate_resource_type_and_name(resource_type, resource_name)?;

        // b. Normalize action
        let stored_action = normalize_stored_action(action)?;

        // c. Delete permission (keep the empty role binding so future
        //    grants remain idempotent).
        let role_name = build_user_role_name(username);
        let resource_id = build_resource_identifier(namespace_id, resource_type, resource_name);
        self.persistence
            .permission_revoke(&role_name, &resource_id, &stored_action)
            .await?;
        invalidate_caches(&role_name, username);
        Ok(())
    }

    async fn find_authorized_resource_names(
        &self,
        username: &str,
        namespace_id: &str,
        resource_type: &str,
        action: &str,
    ) -> anyhow::Result<Vec<String>> {
        if username.trim().is_empty()
            || resource_type.trim().is_empty()
            || action.trim().is_empty()
        {
            return Ok(vec![]);
        }

        let roles = self.persistence.role_find_by_username(username).await?;
        if roles.is_empty() {
            return Ok(vec![]);
        }

        let dedicated_role_name = build_user_role_name(username);
        let has_dedicated_role = roles.iter().any(|r| r.role == dedicated_role_name);
        if !has_dedicated_role {
            return Ok(vec![]);
        }

        let resolved_namespace_id = normalize_namespace_id(namespace_id);
        let normalized_resource_type = normalize_resource_type(resource_type);

        let permissions = self
            .persistence
            .permission_find_by_role(&dedicated_role_name)
            .await?;
        if permissions.is_empty() {
            return Ok(vec![]);
        }

        let mut names = Vec::new();
        for perm in &permissions {
            if let Some((ns, rt, name)) = try_parse_resource_identifier(&perm.resource) {
                if ns != resolved_namespace_id || rt != normalized_resource_type {
                    continue;
                }
                if matches_requested_action(&perm.action, action) {
                    names.push(name);
                }
            }
        }
        Ok(names)
    }
}

// ============================================================================
// Helper functions — mirror VisibilityGrantRoleHelper
// ============================================================================

/// Invalidate local role and permission caches for the given role/user.
fn invalidate_caches(role_name: &str, username: &str) {
    crate::service::role::invalidate_roles_cache(username);
    crate::service::permission::invalidate_permissions_cache_for_role(role_name);
}

/// Normalize namespace ID: blank → "public" (Nacos: `Constants.DEFAULT_NAMESPACE_ID`).
fn normalize_namespace_id(namespace_id: &str) -> String {
    if namespace_id.trim().is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        namespace_id.to_string()
    }
}

/// Normalize resource type: trim + lowercase (Nacos: `normalizeResourceType`).
fn normalize_resource_type(resource_type: &str) -> String {
    resource_type.trim().to_lowercase()
}

/// Normalize and validate the grant action.
///
/// * `r` → `r`
/// * `w` / `rw` → `rw` (write grants imply read visibility)
///
/// Returns an error for blank or unsupported actions.
fn normalize_stored_action(action: &str) -> anyhow::Result<String> {
    if action.trim().is_empty() {
        return Err(anyhow::anyhow!("action is blank"));
    }
    let normalized = action.trim().to_lowercase();
    match normalized.as_str() {
        "r" => Ok("r".to_string()),
        "w" | "rw" => Ok("rw".to_string()),
        _ => Err(anyhow::anyhow!("unsupported action: {}", action)),
    }
}

/// Check whether a stored action satisfies the requested action.
///
/// * Requesting `rw` matches only stored `rw`.
/// * Requesting `r` matches stored `r` or `rw`.
fn matches_requested_action(stored_action: &str, requested_action: &str) -> bool {
    let normalized_requested = match normalize_stored_action(requested_action) {
        Ok(a) => a,
        Err(_) => return false,
    };
    if normalized_requested == "rw" {
        return stored_action == "rw";
    }
    stored_action == "r" || stored_action == "rw"
}

/// Build the dedicated visibility role name for a user.
///
/// Format: `__nacos_vis__.u.` + SHA-256(username)[0:32]
///
/// Uses a deterministic short SHA-256 prefix so internal role names stay
/// within the `roles.role` varchar(50) limit and do not expose user names.
fn build_user_role_name(username: &str) -> String {
    let hash = Sha256::digest(username.as_bytes());
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    format!(
        "{}{}{}",
        VISIBILITY_GRANT_ROLE_PREFIX,
        USER_ROLE_MARKER,
        &hex[..USER_ROLE_HASH_HEX_LENGTH]
    )
}

/// Build the persisted resource identifier for a visibility grant.
///
/// Format: `@@visibility/{namespaceId}/{resourceType}/{resourceName}`
fn build_resource_identifier(
    namespace_id: &str,
    resource_type: &str,
    resource_name: &str,
) -> String {
    let ns = normalize_namespace_id(namespace_id);
    let rt = normalize_resource_type(resource_type);
    format!(
        "{}{}/{}/{}",
        RESOURCE_IDENTIFIER_PREFIX, ns, rt, resource_name
    )
}

/// Parse a visibility resource identifier back into its components.
///
/// Returns `Some((namespace_id, resource_type, resource_name))` when the
/// identifier matches the `@@visibility/...` format with three non-empty
/// parts, or `None` otherwise.
fn try_parse_resource_identifier(
    resource: &str,
) -> Option<(String, String, String)> {
    if !resource.starts_with(RESOURCE_IDENTIFIER_PREFIX) {
        return None;
    }
    let body = &resource[RESOURCE_IDENTIFIER_PREFIX.len()..];
    let parts: Vec<&str> = body.splitn(3, '/').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
}

/// Validate that resource type and name are non-empty.
fn validate_resource_type_and_name(
    resource_type: &str,
    resource_name: &str,
) -> anyhow::Result<()> {
    if resource_type.trim().is_empty() {
        return Err(anyhow::anyhow!("resourceType is blank"));
    }
    if resource_name.trim().is_empty() {
        return Err(anyhow::anyhow!("resourceName is blank"));
    }
    Ok(())
}

/// Validate that username is non-empty.
fn validate_username(username: &str) -> anyhow::Result<()> {
    if username.trim().is_empty() {
        return Err(anyhow::anyhow!("username is blank"));
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_namespace_id() {
        assert_eq!(normalize_namespace_id(""), "public");
        assert_eq!(normalize_namespace_id("   "), "public");
        assert_eq!(normalize_namespace_id("dev"), "dev");
    }

    #[test]
    fn test_normalize_resource_type() {
        assert_eq!(normalize_resource_type("Config"), "config");
        assert_eq!(normalize_resource_type("  Naming "), "naming");
        assert_eq!(normalize_resource_type(""), "");
    }

    #[test]
    fn test_normalize_stored_action() {
        assert_eq!(normalize_stored_action("r").unwrap(), "r");
        assert_eq!(normalize_stored_action("R").unwrap(), "r");
        assert_eq!(normalize_stored_action("w").unwrap(), "rw");
        assert_eq!(normalize_stored_action("W").unwrap(), "rw");
        assert_eq!(normalize_stored_action("rw").unwrap(), "rw");
        assert_eq!(normalize_stored_action("RW").unwrap(), "rw");
        assert_eq!(normalize_stored_action(" r ").unwrap(), "r");
        assert!(normalize_stored_action("").is_err());
        assert!(normalize_stored_action("x").is_err());
    }

    #[test]
    fn test_matches_requested_action() {
        // Requesting "r" matches "r" and "rw"
        assert!(matches_requested_action("r", "r"));
        assert!(matches_requested_action("rw", "r"));
        // Requesting "rw" matches only "rw"
        assert!(!matches_requested_action("r", "rw"));
        assert!(matches_requested_action("rw", "rw"));
        // Invalid requested action never matches
        assert!(!matches_requested_action("rw", "x"));
    }

    #[test]
    fn test_build_user_role_name() {
        let role = build_user_role_name("alice");
        assert!(role.starts_with("__nacos_vis__.u."));
        // SHA-256 prefix is 32 hex chars
        let hash_part = &role["__nacos_vis__.u.".len()..];
        assert_eq!(hash_part.len(), USER_ROLE_HASH_HEX_LENGTH);
        // Deterministic
        assert_eq!(role, build_user_role_name("alice"));
        // Different users produce different roles
        assert_ne!(role, build_user_role_name("bob"));
    }

    #[test]
    fn test_build_resource_identifier() {
        let id = build_resource_identifier("dev", "config", "app.yml");
        assert_eq!(id, "@@visibility/dev/config/app.yml");

        // Blank namespace defaults to "public"
        let id2 = build_resource_identifier("", "Config", "app.yml");
        assert_eq!(id2, "@@visibility/public/config/app.yml");

        // Resource name can contain slashes
        let id3 = build_resource_identifier("dev", "naming", "group/service");
        assert_eq!(id3, "@@visibility/dev/naming/group/service");
    }

    #[test]
    fn test_try_parse_resource_identifier() {
        // Valid
        let parsed = try_parse_resource_identifier("@@visibility/dev/config/app.yml");
        assert_eq!(
            parsed,
            Some(("dev".to_string(), "config".to_string(), "app.yml".to_string()))
        );

        // Resource name with slashes
        let parsed = try_parse_resource_identifier("@@visibility/dev/naming/group/service");
        assert_eq!(
            parsed,
            Some((
                "dev".to_string(),
                "naming".to_string(),
                "group/service".to_string()
            ))
        );

        // Not a visibility identifier
        assert!(try_parse_resource_identifier("public:*:config/*").is_none());

        // Missing parts
        assert!(try_parse_resource_identifier("@@visibility/dev/config").is_none());
        assert!(try_parse_resource_identifier("@@visibility//config/app.yml").is_none());
        assert!(try_parse_resource_identifier("@@visibility/dev//app.yml").is_none());
        assert!(try_parse_resource_identifier("@@visibility/dev/config/").is_none());
    }

    #[test]
    fn test_validate_resource_type_and_name() {
        assert!(validate_resource_type_and_name("config", "app.yml").is_ok());
        assert!(validate_resource_type_and_name("", "app.yml").is_err());
        assert!(validate_resource_type_and_name("config", "").is_err());
        assert!(validate_resource_type_and_name("  ", "  ").is_err());
    }

    #[test]
    fn test_validate_username() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("   ").is_err());
    }

    #[test]
    fn test_constants_align_with_nacos() {
        assert_eq!(VISIBILITY_PATH, "/v3/auth/visibility");
        assert_eq!(VISIBILITY_RESOURCE, "auth/visibility");
        assert_eq!(VISIBILITY_GRANT_ROLE_PREFIX, "__nacos_vis__.");
    }
}
