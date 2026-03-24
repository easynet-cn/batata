//! Nacos auth plugin (default) and LDAP auth plugin
//!
//! NacosAuthPlugin: JWT token validation + RBAC via persistence layer
//! LdapAuthPlugin: Composes NacosAuthPlugin with LDAP authentication for login

use std::sync::Arc;

use batata_common::{AuthCheckResult, AuthPermission, AuthPlugin, IdentityContext, LoginResult};
use batata_persistence::PersistenceService;
use tracing::{debug, warn};

use crate::service::auth::{decode_jwt_token_cached, encode_jwt_token, unblacklist_user};
use crate::service::ldap::LdapAuthService;

const DEFAULT_NAMESPACE_ID: &str = "public";

/// Nacos auth plugin — the default authentication backend.
///
/// Uses JWT tokens for authentication and RBAC via the persistence
/// layer for authorization. This is the standard Nacos auth model.
pub struct NacosAuthPlugin {
    secret_key: String,
    pub(crate) token_expire_seconds: i64,
    pub(crate) persistence: Arc<dyn PersistenceService>,
}

impl NacosAuthPlugin {
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

    /// Verify password against stored hash
    async fn verify_credentials(&self, username: &str, password: &str) -> Result<bool, String> {
        let user = self
            .persistence
            .user_find_by_username(username)
            .await
            .map_err(|e| format!("failed to find user: {}", e))?;

        match user {
            Some(user) => {
                let valid = bcrypt::verify(password, &user.password).unwrap_or(false);
                Ok(valid)
            }
            None => Ok(false),
        }
    }
}

#[async_trait::async_trait]
impl AuthPlugin for NacosAuthPlugin {
    fn plugin_name(&self) -> &str {
        "nacos"
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

/// LDAP auth plugin — composes NacosAuthPlugin with LDAP for login.
///
/// Token validation and RBAC are shared with the inner NacosAuthPlugin
/// because LDAP users also get JWT tokens after login.
/// Login flow: admin users → nacos plugin; others → LDAP bind.
pub struct LdapAuthPlugin {
    nacos_plugin: NacosAuthPlugin,
    ldap_service: LdapAuthService,
}

impl LdapAuthPlugin {
    pub fn new(nacos_plugin: NacosAuthPlugin, ldap_service: LdapAuthService) -> Self {
        Self {
            nacos_plugin,
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
        self.nacos_plugin.validate_identity(identity).await
    }

    async fn validate_authority(
        &self,
        identity: &IdentityContext,
        permission: &AuthPermission,
    ) -> AuthCheckResult {
        self.nacos_plugin
            .validate_authority(identity, permission)
            .await
    }

    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, String> {
        // Admin users always use local auth (bypass LDAP)
        if self.nacos_plugin.is_global_admin(username).await {
            return self.nacos_plugin.login(username, password).await;
        }

        // Try LDAP authentication (returns AuthResult directly, not Result)
        let ldap_result = self.ldap_service.authenticate(username, password).await;

        if ldap_result.success {
            debug!(username, "LDAP authentication successful");

            // Ensure user exists in local DB for RBAC
            let user_exists = self
                .nacos_plugin
                .persistence
                .user_find_by_username(username)
                .await
                .ok()
                .flatten()
                .is_some();

            if !user_exists {
                // Auto-create LDAP user in local DB with unusable password
                let placeholder_hash =
                    bcrypt::hash("LDAP_USER_NO_LOCAL_PASSWORD", 4).unwrap_or_default();
                let _ = self
                    .nacos_plugin
                    .persistence
                    .user_create(username, &placeholder_hash, true)
                    .await;
                debug!(username, "auto-created local user for LDAP user");
            }

            // Generate JWT token
            unblacklist_user(username);
            let token = self.nacos_plugin.generate_token(username)?;
            let is_admin = self.nacos_plugin.is_global_admin(username).await;

            Ok(LoginResult {
                token,
                token_ttl: self.nacos_plugin.token_expire_seconds,
                username: username.to_string(),
                is_global_admin: is_admin,
            })
        } else {
            // LDAP auth failed, try local fallback
            debug!(username, "LDAP auth failed, trying local fallback");
            self.nacos_plugin.login(username, password).await
        }
    }
}
