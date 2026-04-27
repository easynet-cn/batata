//! Authentication and authorization traits and types.

use chrono::{DateTime, Utc};
use thiserror::Error;

/// JWT token validation errors
#[derive(Error, Debug, Clone)]
pub enum TokenError {
    #[error("token is missing")]
    TokenMissing,

    #[error("token has expired at {expired_at}")]
    TokenExpired { expired_at: DateTime<Utc> },

    #[error("token is malformed")]
    TokenMalformed,

    #[error("token signature is invalid")]
    TokenSignatureInvalid,

    #[error("token audience mismatch")]
    TokenAudienceMismatch,

    #[error("token issuer mismatch")]
    TokenIssuerMismatch,

    #[error("token is not yet valid (valid from: {valid_from})")]
    TokenNotYetValid { valid_from: DateTime<Utc> },

    #[error("token has been revoked")]
    TokenRevoked,

    #[error("token decode error: {0}")]
    DecodeError(String),
}

/// Errors that can occur during authentication or authorization checks
#[derive(Error, Debug, Clone)]
pub enum AuthError {
    #[error("token error: {0}")]
    Token(#[from] TokenError),

    #[error("user not found: {username}")]
    UserNotFound { username: String },

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("permission denied: {resource}")]
    PermissionDenied { resource: String },

    #[error("user not authenticated")]
    NotAuthenticated,

    #[error("no roles found for user")]
    NoRolesFound,

    #[error("internal error: {0}")]
    InternalError(String),
}

/// Raw token extracted by middleware, stored in request extensions.
/// The middleware only extracts the token string -- all validation
/// (JWT decode, expiry check) is handled by the AuthPlugin.
#[derive(Debug, Clone, Default)]
pub struct RequestToken(pub Option<String>);

/// Identity context built from request token, enriched by AuthPlugin.
///
/// The `secured!` macro creates this from the `RequestToken` and passes
/// it to `AuthPlugin::validate_identity()` which fills in the remaining fields.
#[derive(Debug, Clone, Default)]
pub struct IdentityContext {
    /// Raw token from the request (populated by secured! macro from RequestToken)
    pub token: Option<String>,
    /// Username extracted from the token (set by AuthPlugin)
    pub username: String,
    /// Whether the identity has been successfully authenticated (set by AuthPlugin)
    pub authenticated: bool,
    /// Whether the user is a global admin (set by AuthPlugin)
    pub is_global_admin: bool,
}

/// Permission for authorization checking
#[derive(Debug, Clone)]
pub struct AuthPermission {
    /// Resource being accessed (format: "namespace:group:type/name")
    pub resource: String,
    /// Action being performed: "r" (read) or "w" (write)
    pub action: String,
}

/// Result of an authentication or authorization check
#[derive(Debug, Clone)]
pub struct AuthCheckResult {
    pub success: bool,
    /// Structured error for precise error handling.
    /// Callers can use `.error.as_ref().map(|e| e.to_string())` for display messages.
    pub error: Option<AuthError>,
}

impl AuthCheckResult {
    pub fn success() -> Self {
        Self {
            success: true,
            error: None,
        }
    }

    /// Create a failure result with a structured AuthError
    pub fn fail(error: AuthError) -> Self {
        Self {
            success: false,
            error: Some(error),
        }
    }
}

/// Result returned by a successful login
#[derive(Debug, Clone)]
pub struct LoginResult {
    /// JWT access token
    pub token: String,
    /// Token time-to-live in seconds
    pub token_ttl: i64,
    /// Authenticated username
    pub username: String,
    /// Whether the user is a global admin
    pub is_global_admin: bool,
}

/// Auth plugin trait -- the main SPI interface for pluggable authentication.
///
/// Each auth backend (nacos, ldap, oauth2) implements this trait.
/// The active plugin is selected via config key `batata.core.auth.system.type`.
///
/// Flow:
/// 1. Middleware extracts raw token -> stores as `RequestToken`
/// 2. `secured!` macro builds `IdentityContext` from `RequestToken`
/// 3. `secured!` calls `validate_identity()` -- plugin decodes token, checks validity,
///    sets username/is_global_admin. Handles expired/invalid/missing token errors.
/// 4. For non-admin users, `secured!` calls `validate_authority()` -- plugin checks
///    if user has permission for the requested resource+action.
#[async_trait::async_trait]
pub trait AuthPlugin: Send + Sync {
    /// Plugin identifier (e.g., "nacos", "ldap", "oauth2")
    fn plugin_name(&self) -> &str;

    /// Whether this plugin supports username/password login
    fn is_login_enabled(&self) -> bool {
        true
    }

    /// Validate identity: decode token, verify validity, load user info/roles.
    ///
    /// Handles all token errors (missing, expired, invalid) internally.
    /// On success, sets `identity.authenticated`, `identity.username`,
    /// `identity.is_global_admin`.
    async fn validate_identity(&self, identity: &mut IdentityContext) -> AuthCheckResult;

    /// Authorize: check if authenticated user has permission for resource+action.
    ///
    /// Only called for non-admin users (admin bypass is handled by the caller).
    async fn validate_authority(
        &self,
        identity: &IdentityContext,
        permission: &AuthPermission,
    ) -> AuthCheckResult;

    /// Login with username/password credentials.
    ///
    /// Returns a JWT token on success, or an error message on failure.
    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, String>;
}

/// OAuth/OIDC provider trait for pluggable external authentication.
///
/// Abstracts the OAuth2/OIDC service so that AppState can hold a trait object
/// instead of a concrete `OAuthService` type. This decouples `batata-server-common`
/// from `batata-auth`'s OAuth implementation.
#[async_trait::async_trait]
pub trait OAuthProvider: Send + Sync {
    /// Check if OAuth is enabled
    fn is_enabled(&self) -> bool;

    /// Get all enabled provider names
    fn get_enabled_providers(&self) -> Vec<String>;

    /// Generate authorization URL for a provider
    async fn get_authorization_url(
        &self,
        provider_name: &str,
        redirect_uri: &str,
    ) -> anyhow::Result<(String, String)>;

    /// Exchange authorization code for tokens
    async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        redirect_uri: &str,
        state: &str,
    ) -> anyhow::Result<OAuthTokenResponse>;

    /// Get user info from provider using access token
    async fn get_user_info(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> anyhow::Result<OAuthUserProfile>;
}

/// OAuth token response (provider-agnostic)
#[derive(Debug, Clone)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

/// OAuth user profile (provider-agnostic)
#[derive(Debug, Clone)]
pub struct OAuthUserProfile {
    pub provider_user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub groups: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_check_result() {
        let ok = AuthCheckResult::success();

        assert!(ok.success);
        assert!(ok.error.is_none());

        let fail = AuthCheckResult::fail(AuthError::Token(TokenError::TokenExpired {
            expired_at: Utc::now(),
        }));

        assert!(!fail.success);
        assert!(fail.error.is_some());

        // Verify error message via to_string()
        let msg = fail.error.as_ref().unwrap().to_string();
        assert!(msg.contains("expired"));
    }

    #[test]
    fn test_identity_context_default() {
        let ctx = IdentityContext::default();

        assert!(ctx.token.is_none());
        assert!(ctx.username.is_empty());
        assert!(!ctx.authenticated);
        assert!(!ctx.is_global_admin);
    }

    #[test]
    fn test_request_token_default() {
        let token = RequestToken::default();

        assert!(token.0.is_none());

        let token = RequestToken(Some("abc123".to_string()));

        assert_eq!(token.0.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_auth_permission() {
        let perm = AuthPermission {
            resource: "public:DEFAULT_GROUP:config/app.yaml".to_string(),
            action: "r".to_string(),
        };

        assert!(perm.resource.contains("config"));
        assert_eq!(perm.action, "r");
    }

    #[test]
    fn test_login_result() {
        let result = LoginResult {
            token: "jwt-token".to_string(),
            token_ttl: 18000,
            username: "nacos".to_string(),
            is_global_admin: true,
        };

        assert_eq!(result.username, "nacos");
        assert!(result.is_global_admin);
        assert_eq!(result.token_ttl, 18000);
    }
}
