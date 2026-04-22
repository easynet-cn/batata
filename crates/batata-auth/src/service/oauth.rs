//! OAuth2/OIDC Authentication Service
//!
//! Provides OAuth2 and OpenID Connect authentication support for Batata.
//!
//! Supported providers:
//! - Generic OAuth2
//! - OpenID Connect (OIDC) with discovery
//! - Google
//! - GitHub
//! - Microsoft Azure AD
//!
//! # Configuration
//!
//! OAuth providers are configured via environment variables or application.yml:
//! ```yaml
//! batata.core.auth.oauth.enabled: true
//! batata.core.auth.oauth.providers:
//!   google:
//!     client_id: ${GOOGLE_CLIENT_ID}
//!     client_secret: ${GOOGLE_CLIENT_SECRET}
//! ```

use std::collections::HashMap;
use std::time::Duration;

use moka::future::Cache;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

/// OAuth2 provider type
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProviderType {
    /// Generic OAuth2 provider
    Generic,
    /// OpenID Connect provider with discovery
    #[default]
    Oidc,
    /// Google OAuth2
    Google,
    /// GitHub OAuth2
    GitHub,
    /// Microsoft Azure AD
    Microsoft,
    /// Custom provider
    Custom(String),
}

/// OAuth2 provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthProviderConfig {
    /// Provider type
    #[serde(default)]
    pub provider_type: OAuthProviderType,
    /// Whether this provider is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OAuth2 client ID
    pub client_id: String,
    /// OAuth2 client secret
    pub client_secret: String,
    /// OIDC discovery URL (for OIDC providers)
    #[serde(default)]
    pub discovery_url: Option<String>,
    /// Authorization endpoint (for non-OIDC providers)
    #[serde(default)]
    pub authorize_endpoint: Option<String>,
    /// Token endpoint (for non-OIDC providers)
    #[serde(default)]
    pub token_endpoint: Option<String>,
    /// User info endpoint (for non-OIDC providers)
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    /// JWKS URI for token validation
    #[serde(default)]
    pub jwks_uri: Option<String>,
    /// OAuth2 scopes
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    /// Redirect URI for OAuth callback
    #[serde(default)]
    pub redirect_uri: Option<String>,
    /// User info claim mapping
    #[serde(default)]
    pub userinfo_mapping: UserInfoMapping,
}

fn default_true() -> bool {
    true
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
    ]
}

/// User info claim mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoMapping {
    /// Claim for username (default: "sub")
    #[serde(default = "default_username_claim")]
    pub username_claim: String,
    /// Claim for email (default: "email")
    #[serde(default = "default_email_claim")]
    pub email_claim: String,
    /// Claim for display name (default: "name")
    #[serde(default = "default_name_claim")]
    pub name_claim: String,
    /// Claim for groups/roles (optional)
    #[serde(default)]
    pub groups_claim: Option<String>,
}

impl Default for UserInfoMapping {
    fn default() -> Self {
        Self {
            username_claim: default_username_claim(),
            email_claim: default_email_claim(),
            name_claim: default_name_claim(),
            groups_claim: None,
        }
    }
}

fn default_username_claim() -> String {
    "sub".to_string()
}

fn default_email_claim() -> String {
    "email".to_string()
}

fn default_name_claim() -> String {
    "name".to_string()
}

/// Global OAuth2 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    /// Whether OAuth2 authentication is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Configured OAuth providers
    #[serde(default)]
    pub providers: HashMap<String, OAuthProviderConfig>,
    /// User creation mode: "auto" or "manual"
    #[serde(default = "default_user_creation")]
    pub user_creation: String,
    /// Role sync mode: "on_login" or "periodic"
    #[serde(default = "default_role_sync")]
    pub role_sync: String,
    /// Default redirect URI template
    #[serde(default)]
    pub redirect_uri: Option<String>,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            providers: HashMap::new(),
            user_creation: default_user_creation(),
            role_sync: default_role_sync(),
            redirect_uri: None,
        }
    }
}

fn default_user_creation() -> String {
    "auto".to_string()
}

fn default_role_sync() -> String {
    "on_login".to_string()
}

impl OAuthConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("BATATA_OAUTH_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(false),
            providers: HashMap::new(), // Providers should be configured via YAML
            user_creation: std::env::var("BATATA_OAUTH_USER_CREATION")
                .unwrap_or_else(|_| "auto".to_string()),
            role_sync: std::env::var("BATATA_OAUTH_ROLE_SYNC")
                .unwrap_or_else(|_| "on_login".to_string()),
            redirect_uri: std::env::var("BATATA_OAUTH_REDIRECT_URI").ok(),
        }
    }

    /// Get a provider configuration by name
    pub fn get_provider(&self, name: &str) -> Option<&OAuthProviderConfig> {
        self.providers.get(name).filter(|p| p.enabled)
    }

    /// Get all enabled providers
    pub fn enabled_providers(&self) -> Vec<(&String, &OAuthProviderConfig)> {
        self.providers.iter().filter(|(_, p)| p.enabled).collect()
    }
}

/// OIDC discovery document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: String,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub response_types_supported: Vec<String>,
    #[serde(default)]
    pub grant_types_supported: Vec<String>,
}

/// OAuth2 token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// OAuth2 user info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    /// Provider user ID (from "sub" claim)
    pub provider_user_id: String,
    /// Username for Batata
    pub username: String,
    /// Email address
    #[serde(default)]
    pub email: Option<String>,
    /// Display name
    #[serde(default)]
    pub name: Option<String>,
    /// Groups/roles from provider
    #[serde(default)]
    pub groups: Vec<String>,
    /// Raw claims
    #[serde(default)]
    pub raw_claims: HashMap<String, serde_json::Value>,
}

/// OAuth2 authorization state (for CSRF protection)
#[derive(Debug, Clone)]
pub struct OAuthState {
    /// Random state value
    pub state: String,
    /// Provider name
    pub provider: String,
    /// Creation time
    pub created_at: std::time::Instant,
    /// Optional nonce for OIDC
    pub nonce: Option<String>,
    /// PKCE code verifier
    pub code_verifier: Option<String>,
}

/// Configuration for OAuth cache tuning
#[derive(Clone, Debug)]
pub struct OAuthCacheConfig {
    /// Discovery document cache TTL in seconds (default: 3600)
    pub discovery_ttl_secs: u64,
    /// Discovery document cache max capacity (default: 100)
    pub discovery_capacity: u64,
    /// State cache TTL in seconds (default: 600)
    pub state_ttl_secs: u64,
    /// State cache max capacity (default: 10000)
    pub state_capacity: u64,
    /// HTTP client timeout in seconds (default: 30)
    pub http_timeout_secs: u64,
}

impl Default for OAuthCacheConfig {
    fn default() -> Self {
        Self {
            discovery_ttl_secs: 3600,
            discovery_capacity: 100,
            state_ttl_secs: 600,
            state_capacity: 10000,
            http_timeout_secs: 30,
        }
    }
}

/// OAuth2 service for handling authentication
pub struct OAuthService {
    config: OAuthConfig,
    http_client: reqwest::Client,
    /// Cache for OIDC discovery documents
    discovery_cache: Cache<String, OidcDiscovery>,
    /// Cache for authorization states (CSRF protection)
    state_cache: Cache<String, OAuthState>,
}

impl OAuthService {
    /// Create a new OAuth2 service with default cache config
    pub fn new(config: OAuthConfig) -> anyhow::Result<Self> {
        Self::with_cache_config(config, OAuthCacheConfig::default())
    }

    /// Create a new OAuth2 service with custom cache configuration
    pub fn with_cache_config(
        config: OAuthConfig,
        cache_config: OAuthCacheConfig,
    ) -> anyhow::Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cache_config.http_timeout_secs))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            config,
            http_client,
            discovery_cache: Cache::builder()
                .time_to_live(Duration::from_secs(cache_config.discovery_ttl_secs))
                .max_capacity(cache_config.discovery_capacity)
                .build(),
            state_cache: Cache::builder()
                .time_to_live(Duration::from_secs(cache_config.state_ttl_secs))
                .max_capacity(cache_config.state_capacity)
                .build(),
        })
    }

    /// Check if OAuth is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get all enabled provider names
    pub fn get_enabled_providers(&self) -> Vec<String> {
        self.config
            .enabled_providers()
            .iter()
            .map(|(name, _)| (*name).clone())
            .collect()
    }

    /// Generate authorization URL for a provider
    pub async fn get_authorization_url(
        &self,
        provider_name: &str,
        redirect_uri: &str,
    ) -> anyhow::Result<(String, String)> {
        let provider = self
            .config
            .get_provider(provider_name)
            .ok_or_else(|| anyhow::anyhow!("OAuth provider not found: {}", provider_name))?;

        // Get authorization endpoint
        let authorize_endpoint = if let Some(ref discovery_url) = provider.discovery_url {
            let discovery = self.get_oidc_discovery(discovery_url).await?;

            discovery.authorization_endpoint
        } else {
            provider
                .authorize_endpoint
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No authorization endpoint configured"))?
        };

        // Generate state for CSRF protection
        let state = generate_random_string(32);
        let nonce = generate_random_string(32);

        // Store state
        self.state_cache
            .insert(
                state.clone(),
                OAuthState {
                    state: state.clone(),
                    provider: provider_name.to_string(),
                    created_at: std::time::Instant::now(),
                    nonce: Some(nonce.clone()),
                    code_verifier: None,
                },
            )
            .await;

        // Build authorization URL
        let mut url = url::Url::parse(&authorize_endpoint)?;

        url.query_pairs_mut()
            .append_pair("client_id", &provider.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", &provider.scopes.join(" "))
            .append_pair("state", &state)
            .append_pair("nonce", &nonce);

        debug!("Generated authorization URL for provider {}", provider_name);

        Ok((url.to_string(), state))
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        redirect_uri: &str,
        state: &str,
    ) -> anyhow::Result<TokenResponse> {
        // Validate state
        let oauth_state = self
            .state_cache
            .get(state)
            .await
            .ok_or_else(|| anyhow::anyhow!("Invalid or expired OAuth state"))?;

        if oauth_state.provider != provider_name {
            return Err(anyhow::anyhow!("State provider mismatch"));
        }

        // Remove state (one-time use)
        self.state_cache.remove(state).await;

        let provider = self
            .config
            .get_provider(provider_name)
            .ok_or_else(|| anyhow::anyhow!("OAuth provider not found: {}", provider_name))?;

        // Get token endpoint
        let token_endpoint = if let Some(ref discovery_url) = provider.discovery_url {
            let discovery = self.get_oidc_discovery(discovery_url).await?;

            discovery.token_endpoint
        } else {
            provider
                .token_endpoint
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No token endpoint configured"))?
        };

        // Exchange code for tokens
        let mut params = HashMap::new();

        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("redirect_uri", redirect_uri);
        params.insert("client_id", &provider.client_id);
        params.insert("client_secret", &provider.client_secret);

        let response = self
            .http_client
            .post(&token_endpoint)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Token exchange failed: {}", error_text);
            return Err(anyhow::anyhow!("Token exchange failed: {}", error_text));
        }

        let token_response: TokenResponse = response.json().await?;

        info!(
            "Successfully exchanged code for tokens from provider {}",
            provider_name
        );

        Ok(token_response)
    }

    /// Get user info from provider
    pub async fn get_user_info(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> anyhow::Result<OAuthUserInfo> {
        let provider = self
            .config
            .get_provider(provider_name)
            .ok_or_else(|| anyhow::anyhow!("OAuth provider not found: {}", provider_name))?;

        // Get userinfo endpoint
        let userinfo_endpoint = if let Some(ref discovery_url) = provider.discovery_url {
            let discovery = self.get_oidc_discovery(discovery_url).await?;

            discovery
                .userinfo_endpoint
                .ok_or_else(|| anyhow::anyhow!("No userinfo endpoint in discovery"))?
        } else {
            provider
                .userinfo_endpoint
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No userinfo endpoint configured"))?
        };

        // Fetch user info
        let response = self
            .http_client
            .get(&userinfo_endpoint)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();

            return Err(anyhow::anyhow!("Failed to get user info: {}", error_text));
        }

        let raw_claims: HashMap<String, serde_json::Value> = response.json().await?;

        // Map claims to user info
        let mapping = &provider.userinfo_mapping;

        let provider_user_id = raw_claims
            .get("sub")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let username = raw_claims
            .get(&mapping.username_claim)
            .and_then(|v| v.as_str())
            .unwrap_or(&provider_user_id)
            .to_string();

        let email = raw_claims
            .get(&mapping.email_claim)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let name = raw_claims
            .get(&mapping.name_claim)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let groups = if let Some(ref groups_claim) = mapping.groups_claim {
            raw_claims
                .get(groups_claim)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(OAuthUserInfo {
            provider_user_id,
            username,
            email,
            name,
            groups,
            raw_claims,
        })
    }

    /// Get OIDC discovery document (cached)
    async fn get_oidc_discovery(&self, discovery_url: &str) -> anyhow::Result<OidcDiscovery> {
        if let Some(discovery) = self.discovery_cache.get(discovery_url).await {
            return Ok(discovery);
        }

        info!("Fetching OIDC discovery from {}", discovery_url);

        let response = self.http_client.get(discovery_url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch OIDC discovery"));
        }

        let discovery: OidcDiscovery = response.json().await?;
        self.discovery_cache
            .insert(discovery_url.to_string(), discovery.clone())
            .await;

        Ok(discovery)
    }

    /// Validate and parse an ID token (basic validation)
    pub fn parse_id_token(
        &self,
        id_token: &str,
    ) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        // Split JWT into parts
        let parts: Vec<&str> = id_token.split('.').collect();

        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid ID token format"));
        }

        // Decode payload (middle part)
        let payload = base64_decode_url_safe(parts[1])?;
        let claims: HashMap<String, serde_json::Value> = serde_json::from_slice(&payload)?;

        // Basic validation
        if let Some(exp) = claims.get("exp").and_then(|v| v.as_i64()) {
            let now = chrono::Utc::now().timestamp();

            if now > exp {
                return Err(anyhow::anyhow!("ID token has expired"));
            }
        }

        Ok(claims)
    }
}

#[async_trait::async_trait]
impl batata_common::OAuthProvider for OAuthService {
    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn get_enabled_providers(&self) -> Vec<String> {
        self.config
            .enabled_providers()
            .iter()
            .map(|(name, _)| (*name).clone())
            .collect()
    }

    async fn get_authorization_url(
        &self,
        provider_name: &str,
        redirect_uri: &str,
    ) -> anyhow::Result<(String, String)> {
        self.get_authorization_url(provider_name, redirect_uri)
            .await
    }

    async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        redirect_uri: &str,
        state: &str,
    ) -> anyhow::Result<batata_common::OAuthTokenResponse> {
        let resp = self
            .exchange_code(provider_name, code, redirect_uri, state)
            .await?;

        Ok(batata_common::OAuthTokenResponse {
            access_token: resp.access_token,
            token_type: resp.token_type,
            expires_in: resp.expires_in,
            refresh_token: resp.refresh_token,
            id_token: resp.id_token,
            scope: resp.scope,
        })
    }

    async fn get_user_info(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> anyhow::Result<batata_common::OAuthUserProfile> {
        let info = self.get_user_info(provider_name, access_token).await?;

        Ok(batata_common::OAuthUserProfile {
            provider_user_id: info.provider_user_id,
            username: info.username,
            email: info.email,
            name: info.name,
            groups: info.groups,
        })
    }
}

/// Generate a random string for state/nonce
fn generate_random_string(len: usize) -> String {
    use std::iter;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let rng = std::collections::hash_map::RandomState::new();

    iter::repeat_with(|| {
        let idx = (std::hash::BuildHasher::hash_one(&rng, std::time::Instant::now()) as usize)
            % CHARSET.len();
        CHARSET[idx] as char
    })
    .take(len)
    .collect()
}

/// Base64 URL-safe decode
fn base64_decode_url_safe(input: &str) -> anyhow::Result<Vec<u8>> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    Ok(engine.decode(input)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_config_default() {
        let config = OAuthConfig::default();

        assert!(!config.enabled);
        assert!(config.providers.is_empty());
        assert_eq!(config.user_creation, "auto");
    }

    #[test]
    fn test_userinfo_mapping_default() {
        let mapping = UserInfoMapping::default();

        assert_eq!(mapping.username_claim, "sub");
        assert_eq!(mapping.email_claim, "email");
        assert_eq!(mapping.name_claim, "name");
    }

    #[test]
    fn test_generate_random_string() {
        let s1 = generate_random_string(32);
        let s2 = generate_random_string(32);

        assert_eq!(s1.len(), 32);
        assert_eq!(s2.len(), 32);
        // They should be different (with very high probability)
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_oauth_provider_type_variants() {
        assert_eq!(OAuthProviderType::default(), OAuthProviderType::Oidc);

        let types = vec![
            OAuthProviderType::Generic,
            OAuthProviderType::Oidc,
            OAuthProviderType::Google,
            OAuthProviderType::GitHub,
            OAuthProviderType::Microsoft,
            OAuthProviderType::Custom("keycloak".to_string()),
        ];
        for t in types {
            // Ensure debug works
            let _ = format!("{:?}", t);
        }
    }

    #[test]
    fn test_oauth_config_disabled_by_default() {
        let config = OAuthConfig::from_env();

        assert!(!config.enabled);
        assert!(config.providers.is_empty());
    }

    #[test]
    fn test_oauth_config_get_provider() {
        let mut config = OAuthConfig::from_env();

        config.providers.insert(
            "test".to_string(),
            OAuthProviderConfig {
                provider_type: OAuthProviderType::Generic,
                enabled: true,
                client_id: "client123".to_string(),
                client_secret: "secret456".to_string(),
                discovery_url: None,
                authorize_endpoint: Some("https://auth.example.com/authorize".to_string()),
                token_endpoint: Some("https://auth.example.com/token".to_string()),
                userinfo_endpoint: Some("https://auth.example.com/userinfo".to_string()),
                jwks_uri: None,
                scopes: vec!["openid".to_string(), "profile".to_string()],
                redirect_uri: None,
                userinfo_mapping: UserInfoMapping::default(),
            },
        );

        assert!(config.get_provider("test").is_some());
        assert!(config.get_provider("nonexistent").is_none());
    }

    #[test]
    fn test_oauth_config_enabled_providers() {
        let mut config = OAuthConfig::from_env();

        config.providers.insert(
            "enabled1".to_string(),
            OAuthProviderConfig {
                provider_type: OAuthProviderType::Google,
                enabled: true,
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                discovery_url: None,
                authorize_endpoint: None,
                token_endpoint: None,
                userinfo_endpoint: None,
                jwks_uri: None,
                scopes: vec![],
                redirect_uri: None,
                userinfo_mapping: UserInfoMapping::default(),
            },
        );
        config.providers.insert(
            "disabled1".to_string(),
            OAuthProviderConfig {
                provider_type: OAuthProviderType::GitHub,
                enabled: false,
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                discovery_url: None,
                authorize_endpoint: None,
                token_endpoint: None,
                userinfo_endpoint: None,
                jwks_uri: None,
                scopes: vec![],
                redirect_uri: None,
                userinfo_mapping: UserInfoMapping::default(),
            },
        );

        let enabled = config.enabled_providers();

        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "enabled1");
    }

    #[test]
    fn test_oauth_service_creation() {
        let config = OAuthConfig::from_env();
        let service = OAuthService::new(config).unwrap();

        assert!(!service.is_enabled());
        assert!(service.get_enabled_providers().is_empty());
    }

    #[test]
    fn test_oauth_service_enabled() {
        let mut config = OAuthConfig::from_env();

        config.enabled = true;

        let service = OAuthService::new(config).unwrap();

        assert!(service.is_enabled());
    }

    #[test]
    fn test_parse_id_token_expired() {
        let config = OAuthConfig::from_env();
        let service = OAuthService::new(config).unwrap();
        // Create a JWT-like token with expired exp claim
        use base64::Engine;
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(format!(r#"{{"sub":"user","exp":{}}}"#, 1000000));
        let token = format!("{}.{}.sig", header, payload);
        let result = service.parse_id_token(&token);

        assert!(result.is_err());
    }

    #[test]
    fn test_token_response_fields() {
        let json = r#"{
            "access_token": "at123",
            "token_type": "bearer",
            "expires_in": 3600,
            "refresh_token": "rt456",
            "id_token": "id789",
            "scope": "openid profile"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.access_token, "at123");
        assert_eq!(response.token_type, "bearer");
        assert_eq!(response.expires_in, Some(3600));
        assert_eq!(response.refresh_token.as_deref(), Some("rt456"));
    }

    #[test]
    fn test_oidc_discovery_deserialization() {
        let json = r#"{
            "issuer": "https://auth.example.com",
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token",
            "userinfo_endpoint": "https://auth.example.com/userinfo",
            "jwks_uri": "https://auth.example.com/.well-known/jwks.json",
            "scopes_supported": ["openid", "profile", "email"],
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code"]
        }"#;

        let discovery: OidcDiscovery = serde_json::from_str(json).unwrap();

        assert_eq!(discovery.issuer, "https://auth.example.com");
        assert_eq!(discovery.scopes_supported.len(), 3);
    }
}
