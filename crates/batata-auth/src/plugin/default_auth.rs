//! Default auth plugin and LDAP auth plugin
//!
//! DefaultAuthPlugin: JWT token validation + RBAC via persistence layer
//! LdapAuthPlugin: Composes DefaultAuthPlugin with LDAP authentication for login

use std::sync::Arc;

use batata_common::{
    AuthCheckResult, AuthPermission, AuthPlugin, DEFAULT_NAMESPACE_ID, IdentityContext, LoginResult,
};
use batata_persistence::PersistenceService;
use tracing::{debug, warn};

use crate::model::{
    NON_LOCAL_PASSWORD_SENTINEL, USER_SOURCE_LDAP, normalize_user_source,
    source_allows_password_login,
};
use crate::service::auth::{decode_jwt_token_cached, encode_jwt_token, unblacklist_user};
use crate::service::ldap::LdapAuthService;

/// Default auth plugin — the standard authentication backend.
///
/// Uses JWT tokens for authentication and RBAC via the persistence
/// layer for authorization. Plugin name is "default" for SDK compatibility.
pub struct DefaultAuthPlugin {
    secret_key: String,
    pub(crate) token_expire_seconds: i64,
    pub(crate) persistence: Arc<dyn PersistenceService>,
}

impl DefaultAuthPlugin {
    pub fn new(
        secret_key: String,
        token_expire_seconds: i64,
        persistence: Arc<dyn PersistenceService>,
    ) -> Self {
        Self {
            secret_key,
            token_expire_seconds,
            persistence,
        }
    }

    /// Validate a JWT token and return the username
    fn validate_token(&self, token: &str) -> Result<String, String> {
        match decode_jwt_token_cached(token, &self.secret_key) {
            Ok(data) => Ok(data.claims.sub),
            Err(e) => {
                let msg = match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        "token expired!".to_string()
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidToken => "token invalid!".to_string(),
                    _ => format!("token validation failed: {}", e),
                };
                Err(msg)
            }
        }
    }

    /// Generate a JWT token for a user
    pub(crate) fn generate_token(&self, username: &str) -> Result<String, String> {
        encode_jwt_token(username, &self.secret_key, self.token_expire_seconds)
            .map_err(|e| format!("failed to generate token: {}", e))
    }

    /// Check if a user has the global admin role
    pub(crate) async fn is_global_admin(&self, username: &str) -> bool {
        self.persistence
            .role_has_global_admin_by_username(username)
            .await
            .unwrap_or(false)
    }

    /// Verify password against stored hash.
    ///
    /// Returns `Err` (rather than `Ok(false)`) when the user exists but is
    /// owned by an external identity provider (OAuth, LDAP) — those users
    /// cannot authenticate with a local password and the caller should
    /// surface a specific error message rather than the generic "invalid
    /// credentials" produced by failed bcrypt comparison.
    async fn verify_credentials(&self, username: &str, password: &str) -> Result<bool, String> {
        let user = self
            .persistence
            .user_find_by_username(username)
            .await
            .map_err(|e| format!("failed to find user: {}", e))?;

        match user {
            Some(user) => {
                let source = normalize_user_source(Some(&user.source));

                if !source_allows_password_login(&source) {
                    return Err(format!(
                        "user '{}' is managed by '{}' and cannot log in with a password",
                        username, source
                    ));
                }

                let valid = bcrypt::verify(password, &user.password).unwrap_or(false);

                Ok(valid)
            }
            None => Ok(false),
        }
    }
}

#[async_trait::async_trait]
impl AuthPlugin for DefaultAuthPlugin {
    fn plugin_name(&self) -> &str {
        "default"
    }

    fn is_login_enabled(&self) -> bool {
        true
    }

    async fn validate_identity(&self, identity: &mut IdentityContext) -> AuthCheckResult {
        let token = match &identity.token {
            Some(t) if !t.is_empty() => t.clone(),
            _ => {
                return AuthCheckResult::fail("no token provided");
            }
        };

        match self.validate_token(&token) {
            Ok(username) => {
                identity.username = username.clone();
                identity.authenticated = true;
                identity.is_global_admin = self.is_global_admin(&username).await;

                AuthCheckResult::success()
            }
            Err(msg) => AuthCheckResult::fail(msg),
        }
    }

    async fn validate_authority(
        &self,
        identity: &IdentityContext,
        permission: &AuthPermission,
    ) -> AuthCheckResult {
        if !identity.authenticated {
            return AuthCheckResult::fail("user not authenticated");
        }

        // Load roles
        let roles = self
            .persistence
            .role_find_by_username(&identity.username)
            .await
            .unwrap_or_default();

        if roles.is_empty() {
            return AuthCheckResult::fail("no roles found for user");
        }

        // Load permissions for all roles
        let role_names: Vec<String> = roles.iter().map(|r| r.role.clone()).collect();
        let permissions = self
            .persistence
            .permission_find_by_roles(role_names)
            .await
            .unwrap_or_default();

        // Check if any permission matches the requested resource+action
        // Uses the same regex escaping logic as the original secured! macro
        let has_permission = roles.iter().any(|role| {
            permissions.iter().filter(|p| p.role == role.role).any(|p| {
                let mut perm_resource = regex::escape(&p.resource).replace("\\*", ".*");

                if perm_resource.starts_with(':') {
                    perm_resource = format!("{}{}", DEFAULT_NAMESPACE_ID, perm_resource);
                }

                let regex_match =
                    batata_common::regex_matches(&perm_resource, &permission.resource);

                p.action.contains(&permission.action) && regex_match
            })
        });

        if has_permission {
            AuthCheckResult::success()
        } else {
            debug!(
                username = %identity.username,
                resource = %permission.resource,
                action = %permission.action,
                "authorization failed"
            );
            AuthCheckResult::fail("authorization failed!")
        }
    }

    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, String> {
        let valid = self.verify_credentials(username, password).await?;

        if !valid {
            warn!(username, "login failed: invalid credentials");
            return Err("invalid credentials".to_string());
        }

        // Clear user from blacklist on successful login
        unblacklist_user(username);

        let token = self.generate_token(username)?;
        let is_admin = self.is_global_admin(username).await;

        Ok(LoginResult {
            token,
            token_ttl: self.token_expire_seconds,
            username: username.to_string(),
            is_global_admin: is_admin,
        })
    }
}

// ============================================================================
// LDAP Auth Plugin
// ============================================================================

/// LDAP auth plugin — composes DefaultAuthPlugin with LDAP for login.
///
/// Token validation and RBAC are shared with the inner DefaultAuthPlugin
/// because LDAP users also get JWT tokens after login.
/// Login flow: admin users → default plugin; others → LDAP bind.
pub struct LdapAuthPlugin {
    default_plugin: DefaultAuthPlugin,
    ldap_service: LdapAuthService,
}

impl LdapAuthPlugin {
    pub fn new(default_plugin: DefaultAuthPlugin, ldap_service: LdapAuthService) -> Self {
        Self {
            default_plugin,
            ldap_service,
        }
    }
}

#[async_trait::async_trait]
impl AuthPlugin for LdapAuthPlugin {
    fn plugin_name(&self) -> &str {
        "ldap"
    }

    fn is_login_enabled(&self) -> bool {
        true
    }

    async fn validate_identity(&self, identity: &mut IdentityContext) -> AuthCheckResult {
        self.default_plugin.validate_identity(identity).await
    }

    async fn validate_authority(
        &self,
        identity: &IdentityContext,
        permission: &AuthPermission,
    ) -> AuthCheckResult {
        self.default_plugin
            .validate_authority(identity, permission)
            .await
    }

    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, String> {
        // Admin users always use local auth (bypass LDAP)
        if self.default_plugin.is_global_admin(username).await {
            return self.default_plugin.login(username, password).await;
        }

        // Try LDAP authentication (returns AuthResult directly, not Result)
        let ldap_result = self.ldap_service.authenticate(username, password).await;

        if ldap_result.success {
            debug!(username, "LDAP authentication successful");

            // Ensure user exists in local DB for RBAC
            let user_exists = self
                .default_plugin
                .persistence
                .user_find_by_username(username)
                .await
                .ok()
                .flatten()
                .is_some();

            if !user_exists {
                // Auto-create the LDAP-backed user with the non-local sentinel
                // and an explicit source marker so password login is rejected.
                let _ = self
                    .default_plugin
                    .persistence
                    .user_create_with_source(
                        username,
                        NON_LOCAL_PASSWORD_SENTINEL,
                        true,
                        USER_SOURCE_LDAP,
                    )
                    .await;
                debug!(username, "auto-created local user for LDAP user");
            }

            // Generate JWT token
            unblacklist_user(username);

            let token = self.default_plugin.generate_token(username)?;
            let is_admin = self.default_plugin.is_global_admin(username).await;

            Ok(LoginResult {
                token,
                token_ttl: self.default_plugin.token_expire_seconds,
                username: username.to_string(),
                is_global_admin: is_admin,
            })
        } else {
            // LDAP auth failed, try local fallback
            debug!(username, "LDAP auth failed, trying local fallback");

            self.default_plugin.login(username, password).await
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use batata_consistency::RocksStateMachine;
    use batata_persistence::{AuthPersistence, EmbeddedPersistService};
    use tempfile::TempDir;

    /// Bcrypt cost used for fast test hashing. The library minimum is 4;
    /// we use that everywhere in tests to keep the suite snappy.
    const TEST_BCRYPT_COST: u32 = 4;
    const TEST_JWT_SECRET: &str = "dGVzdC1zZWNyZXQta2V5LWZvci11bml0LXRlc3RzLW9ubHk=";
    const TEST_TOKEN_TTL_SECS: i64 = 3600;

    async fn build_plugin() -> (DefaultAuthPlugin, Arc<EmbeddedPersistService>, TempDir) {
        let tmp = TempDir::new().unwrap();
        let sm = RocksStateMachine::new(tmp.path()).await.unwrap();
        let svc = Arc::new(EmbeddedPersistService::from_state_machine(&sm));
        let plugin = DefaultAuthPlugin::new(
            TEST_JWT_SECRET.to_string(),
            TEST_TOKEN_TTL_SECS,
            svc.clone() as Arc<dyn PersistenceService>,
        );
        (plugin, svc, tmp)
    }

    #[tokio::test]
    async fn local_user_can_log_in_with_password() {
        let (plugin, svc, _tmp) = build_plugin().await;

        let password = "correct horse battery staple";
        let hash = bcrypt::hash(password, TEST_BCRYPT_COST).unwrap();

        svc.user_create_with_source("alice", &hash, true, "local")
            .await
            .unwrap();

        let result = plugin.login("alice", password).await;

        assert!(result.is_ok(), "local login should succeed: {:?}", result);

        let login = result.unwrap();

        assert_eq!(login.username, "alice");
        assert!(!login.token.is_empty(), "JWT token must be issued");
    }

    #[tokio::test]
    async fn local_user_with_wrong_password_is_rejected() {
        let (plugin, svc, _tmp) = build_plugin().await;

        let hash = bcrypt::hash("right", TEST_BCRYPT_COST).unwrap();

        svc.user_create_with_source("alice", &hash, true, "local")
            .await
            .unwrap();

        let err = plugin.login("alice", "wrong").await.unwrap_err();

        assert_eq!(
            err, "invalid credentials",
            "wrong password must return generic 'invalid credentials'"
        );
    }

    #[tokio::test]
    async fn oauth_user_cannot_log_in_with_password() {
        let (plugin, svc, _tmp) = build_plugin().await;

        // Simulate what the OAuth handler does: store the sentinel, not a
        // bcrypt of a random UUID, and tag source = "oauth".
        svc.user_create_with_source(
            "oauth_github_123",
            crate::model::NON_LOCAL_PASSWORD_SENTINEL,
            true,
            "oauth",
        )
        .await
        .unwrap();

        let err = plugin
            .login("oauth_github_123", "anything")
            .await
            .unwrap_err();

        assert!(
            err.contains("oauth") && err.contains("cannot log in with a password"),
            "oauth user login must be rejected with a source-specific error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn ldap_user_cannot_log_in_with_password() {
        let (plugin, svc, _tmp) = build_plugin().await;

        svc.user_create_with_source(
            "ldap_user",
            crate::model::NON_LOCAL_PASSWORD_SENTINEL,
            true,
            "ldap",
        )
        .await
        .unwrap();

        let err = plugin.login("ldap_user", "password").await.unwrap_err();

        assert!(
            err.contains("ldap"),
            "ldap user must get an ldap-specific rejection, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn user_list_response_exposes_source_for_both_types() {
        let (_plugin, svc, _tmp) = build_plugin().await;

        let hash = bcrypt::hash("pw", TEST_BCRYPT_COST).unwrap();

        svc.user_create_with_source("alice", &hash, true, "local")
            .await
            .unwrap();
        svc.user_create_with_source(
            "oauth_user",
            crate::model::NON_LOCAL_PASSWORD_SENTINEL,
            true,
            "oauth",
        )
        .await
        .unwrap();

        let page = svc.user_find_page("", 1, 10, false).await.unwrap();

        assert_eq!(page.total_count, 2);

        let alice = page
            .page_items
            .iter()
            .find(|u| u.username == "alice")
            .expect("alice present");

        assert_eq!(alice.source, "local");

        let oauth = page
            .page_items
            .iter()
            .find(|u| u.username == "oauth_user")
            .expect("oauth user present");

        assert_eq!(oauth.source, "oauth");
        assert_eq!(
            oauth.password,
            crate::model::NON_LOCAL_PASSWORD_SENTINEL,
            "oauth user's password must be the unmatchable sentinel, not a bcrypt hash"
        );
    }

    #[tokio::test]
    async fn sentinel_never_matches_any_password_via_bcrypt() {
        // Defensive regression: even if someone accidentally feeds the
        // sentinel through bcrypt::verify, it must never succeed.
        let matched = bcrypt::verify("", crate::model::NON_LOCAL_PASSWORD_SENTINEL)
            .ok()
            .unwrap_or(false);

        assert!(
            !matched,
            "sentinel must be a non-bcrypt string so verify() never returns true"
        );
    }
}
