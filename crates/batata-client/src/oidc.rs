//! OIDC client authentication (Nacos 3.3.0 GA parity, `F-NAC-CSDK-005`).
//!
//! GA release commit `b64bc9118` registers `OidcClientAuthServiceImpl` as the
//! third `AbstractClientAuthService` provider in
//! `client-basic/.../META-INF/services`. It implements the OAuth2 **Client
//! Credentials** grant: `OidcClientContext` resolves the token endpoint either
//! from a configured override or via OIDC Discovery
//! (`{issuer}/.well-known/openid-configuration` → `token_endpoint`), and
//! `OidcTokenHolder` POSTs a `grant_type=client_credentials` form, caches
//! `access_token`/`expires_in`, and refreshes inside a random
//! `[ttl/15, ttl/10)` window before expiry. The resulting
//! `LoginIdentityContext` carries **both** `Authorization: Bearer <token>` and
//! `accessToken` (`OidcProtocolConstants`), so both HTTP and gRPC requests are
//! authenticated.
//!
//! Batata mirrors that contract as a [`ClientAuthService`] implementation.
//! Configuration is opt-in: when no OIDC properties are supplied the provider
//! is disabled and every existing username/password path is unchanged.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use rand::RngExt;
use reqwest::{Client, StatusCode};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::auth::{ClientAuthService, IdentityContext, RequestResource};
use crate::tls::TlsOptions;

/// GA `OidcClientConstants.PROP_ISSUER_URI`.
pub const OIDC_ISSUER_URI_PROPERTY: &str = "nacos.client.auth.oidc.issuer-uri";
/// GA `OidcClientConstants.PROP_CLIENT_ID`.
pub const OIDC_CLIENT_ID_PROPERTY: &str = "nacos.client.auth.oidc.client-id";
/// GA `OidcClientConstants.PROP_CLIENT_SECRET`.
pub const OIDC_CLIENT_SECRET_PROPERTY: &str = "nacos.client.auth.oidc.client-secret";
/// GA `OidcClientConstants.PROP_SCOPE`.
pub const OIDC_SCOPE_PROPERTY: &str = "nacos.client.auth.oidc.scope";
/// GA `OidcClientConstants.PROP_TOKEN_ENDPOINT` (overrides Discovery).
pub const OIDC_TOKEN_ENDPOINT_PROPERTY: &str = "nacos.client.auth.oidc.token-endpoint";

/// GA `OidcClientConstants.DEFAULT_SCOPE`.
pub const OIDC_DEFAULT_SCOPE: &str = "openid";
/// GA `OidcProtocolConstants.WELL_KNOWN_PATH`.
pub const OIDC_WELL_KNOWN_PATH: &str = "/.well-known/openid-configuration";
/// GA `OidcProtocolConstants.AUTHORIZATION_HEADER`.
pub const OIDC_AUTHORIZATION_HEADER: &str = "Authorization";
/// GA `OidcProtocolConstants.BEARER_PREFIX`.
pub const OIDC_BEARER_PREFIX: &str = "Bearer ";
/// GA `OidcProtocolConstants.ACCESS_TOKEN_PARAM`.
pub const OIDC_ACCESS_TOKEN_PARAM: &str = "accessToken";
/// GA `OidcTokenHolder` default TTL when `expires_in` is missing.
pub const OIDC_DEFAULT_TOKEN_TTL_SECS: u64 = 300;

/// GA `OidcProtocolConstants.DEFAULT_CONNECT_TIMEOUT_MS`.
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);
/// GA `OidcProtocolConstants.DEFAULT_READ_TIMEOUT_MS`.
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_millis(10000);
const CONTENT_TYPE_JSON: &str = "application/json";
const DISCOVERY_TOKEN_ENDPOINT: &str = "token_endpoint";
const TOKEN_RESPONSE_ACCESS_TOKEN: &str = "access_token";
const TOKEN_RESPONSE_EXPIRES_IN: &str = "expires_in";
const GRANT_TYPE: &str = "grant_type";
const GRANT_TYPE_CLIENT_CREDENTIALS: &str = "client_credentials";
const PARAM_CLIENT_ID: &str = "client_id";
const PARAM_CLIENT_SECRET: &str = "client_secret";
const PARAM_SCOPE: &str = "scope";

/// OIDC client configuration, mirroring the five GA `OidcClientConstants`
/// properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcConfig {
    /// Issuer URI used for OIDC Discovery.
    pub issuer_uri: String,
    /// OAuth2 client id for the Client Credentials grant.
    pub client_id: String,
    /// OAuth2 client secret for the Client Credentials grant.
    pub client_secret: String,
    /// Requested scope; GA defaults to `openid`.
    pub scope: String,
    /// Direct token endpoint. When set, OIDC Discovery is skipped.
    pub token_endpoint: String,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            issuer_uri: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            scope: OIDC_DEFAULT_SCOPE.to_string(),
            token_endpoint: String::new(),
        }
    }
}

impl OidcConfig {
    /// Create a configuration with the GA default scope.
    pub fn new(client_id: &str, client_secret: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            ..Self::default()
        }
    }

    /// Set the issuer URI used for OIDC Discovery.
    pub fn with_issuer(mut self, issuer_uri: &str) -> Self {
        self.issuer_uri = issuer_uri.to_string();
        self
    }

    /// Set a direct token endpoint, which skips OIDC Discovery.
    pub fn with_token_endpoint(mut self, token_endpoint: &str) -> Self {
        self.token_endpoint = token_endpoint.to_string();
        self
    }

    /// Override the requested scope (GA default: `openid`).
    pub fn with_scope(mut self, scope: &str) -> Self {
        self.scope = scope.to_string();
        self
    }

    /// Whether GA `OidcClientContext.isConfigured()` would accept this config:
    /// non-blank client id/secret and either an issuer or a token endpoint.
    pub fn is_configured(&self) -> bool {
        !self.client_id.trim().is_empty()
            && !self.client_secret.trim().is_empty()
            && (!self.issuer_uri.trim().is_empty() || !self.token_endpoint.trim().is_empty())
    }

    /// Build a config from GA-style client properties.
    ///
    /// Returns `None` when no `nacos.client.auth.oidc.*` key is present, so
    /// OIDC stays entirely opt-in. A partially configured map still returns
    /// `Some` but [`Self::is_configured`] is false, matching GA's
    /// "unconfigured provider is a no-op" behavior.
    pub fn from_properties(properties: &HashMap<String, String>) -> Option<Self> {
        const KEYS: [&str; 5] = [
            OIDC_ISSUER_URI_PROPERTY,
            OIDC_CLIENT_ID_PROPERTY,
            OIDC_CLIENT_SECRET_PROPERTY,
            OIDC_SCOPE_PROPERTY,
            OIDC_TOKEN_ENDPOINT_PROPERTY,
        ];
        if !KEYS.iter().any(|key| properties.contains_key(*key)) {
            return None;
        }
        let mut config = Self::default();
        if let Some(value) = properties.get(OIDC_ISSUER_URI_PROPERTY) {
            config.issuer_uri = value.clone();
        }
        if let Some(value) = properties.get(OIDC_CLIENT_ID_PROPERTY) {
            config.client_id = value.clone();
        }
        if let Some(value) = properties.get(OIDC_CLIENT_SECRET_PROPERTY) {
            config.client_secret = value.clone();
        }
        if let Some(value) = properties.get(OIDC_SCOPE_PROPERTY) {
            config.scope = value.clone();
        }
        if let Some(value) = properties.get(OIDC_TOKEN_ENDPOINT_PROPERTY) {
            config.token_endpoint = value.clone();
        }
        Some(config)
    }

    /// OIDC Discovery URL, following GA's trailing-slash handling.
    pub fn discovery_url(&self) -> String {
        let issuer = self.issuer_uri.trim_end_matches('/');
        if issuer.is_empty() {
            String::new()
        } else {
            format!("{issuer}{OIDC_WELL_KNOWN_PATH}")
        }
    }

    /// The urlencoded Client Credentials form, in GA's field order.
    pub fn token_form(&self) -> Vec<(String, String)> {
        let mut form = vec![
            (
                GRANT_TYPE.to_string(),
                GRANT_TYPE_CLIENT_CREDENTIALS.to_string(),
            ),
            (PARAM_CLIENT_ID.to_string(), self.client_id.clone()),
            (PARAM_CLIENT_SECRET.to_string(), self.client_secret.clone()),
        ];
        if !self.scope.is_empty() {
            form.push((PARAM_SCOPE.to_string(), self.scope.clone()));
        }
        form
    }
}

#[derive(Debug, Default)]
struct OidcState {
    token: Option<String>,
    ttl_secs: u64,
    obtained_at: Option<Instant>,
    token_endpoint: String,
}

/// GA `OidcTokenHolder.generateTokenRefreshWindow`: a random window in
/// `[ttl/15, ttl/10)` seconds before expiry.
fn refresh_window(ttl_secs: u64) -> u64 {
    if ttl_secs == 0 {
        return 0;
    }
    let start = ttl_secs / 15;
    let end = ttl_secs / 10;
    if start >= end {
        start
    } else {
        rand::rng().random_range(start..end)
    }
}

/// OIDC Client Credentials auth provider.
///
/// Implements [`ClientAuthService`] so it can sit alongside the JWT and
/// AccessKey providers in `SecurityProxy`, and is also consumed directly by
/// the HTTP and gRPC transports so every outgoing request carries the GA
/// dual identity headers.
pub struct OidcAuthProvider {
    config: OidcConfig,
    http: Client,
    state: RwLock<OidcState>,
    refresh_lock: Mutex<()>,
}

impl OidcAuthProvider {
    /// Create a provider using GA's 5s connect / 10s read timeouts.
    pub fn new(config: OidcConfig) -> anyhow::Result<Self> {
        Self::with_tls(config, &TlsOptions::default())
    }

    /// Create a provider whose HTTP client honours the SDK TLS settings, so
    /// Discovery and token fetches work against a private-CA IdP.
    pub fn with_tls(config: OidcConfig, tls: &TlsOptions) -> anyhow::Result<Self> {
        let builder = Client::builder()
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .timeout(DEFAULT_READ_TIMEOUT)
            .no_proxy();
        let http = tls.apply_to_reqwest(builder)?.build()?;
        Ok(Self {
            config,
            http,
            state: RwLock::new(OidcState::default()),
            refresh_lock: Mutex::new(()),
        })
    }

    /// The configuration this provider was built with.
    pub fn config(&self) -> &OidcConfig {
        &self.config
    }

    /// Return the cached access token, or `None` when it is missing or inside
    /// the proactive refresh window.
    pub fn access_token(&self) -> Option<String> {
        let state = self.state.read();
        let token = state.token.as_ref()?;
        if token.is_empty() {
            return None;
        }
        let ttl = state.ttl_secs;
        let elapsed = state
            .obtained_at
            .map(|instant| instant.elapsed().as_secs())
            .unwrap_or(ttl);
        let window = refresh_window(ttl);
        if elapsed >= ttl.saturating_sub(window) {
            return None;
        }
        Some(token.clone())
    }

    /// TTL of the cached token in seconds (0 before the first successful fetch).
    pub fn token_ttl_secs(&self) -> u64 {
        self.state.read().ttl_secs
    }

    fn needs_refresh(&self) -> bool {
        self.access_token().is_none()
    }

    fn resolved_endpoint(&self) -> Option<String> {
        if !self.config.token_endpoint.trim().is_empty() {
            return Some(self.config.token_endpoint.clone());
        }
        let endpoint = self.state.read().token_endpoint.clone();
        if endpoint.is_empty() {
            None
        } else {
            Some(endpoint)
        }
    }

    /// Perform OIDC Discovery (GA `OidcClientContext.discover()`).
    async fn discover(&self) -> bool {
        let url = self.config.discovery_url();
        if url.is_empty() {
            warn!("OIDC issuer-uri is not configured, cannot perform Discovery");
            return false;
        }
        debug!("Performing OIDC Discovery from {url}");
        let response = match self
            .http
            .get(&url)
            .header("Accept", CONTENT_TYPE_JSON)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                warn!("OIDC Discovery request failed: {e}");
                return false;
            }
        };
        if response.status() != StatusCode::OK {
            warn!("OIDC Discovery failed, HTTP status {}", response.status());
            return false;
        }
        let body: serde_json::Value = match response.json().await {
            Ok(body) => body,
            Err(e) => {
                warn!("OIDC Discovery response is not JSON: {e}");
                return false;
            }
        };
        let endpoint = match body.get(DISCOVERY_TOKEN_ENDPOINT) {
            Some(value) if !value.is_null() => value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string()),
            _ => {
                warn!("OIDC Discovery response missing 'token_endpoint'");
                return false;
            }
        };
        if endpoint.is_empty() {
            warn!("OIDC Discovery returned an empty 'token_endpoint'");
            return false;
        }
        self.state.write().token_endpoint = endpoint;
        true
    }

    /// Fetch a token (GA `OidcTokenHolder.fetchToken()`).
    async fn fetch_token(&self) -> bool {
        let Some(endpoint) = self.resolved_endpoint() else {
            warn!("OIDC token endpoint is not available");
            return false;
        };
        let response = match self
            .http
            .post(&endpoint)
            .header("Accept", CONTENT_TYPE_JSON)
            .form(&self.config.token_form())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                warn!("OIDC token request failed: {e}");
                return false;
            }
        };
        if response.status() != StatusCode::OK {
            let body = response.text().await.unwrap_or_default();
            warn!("OIDC token request returned non-200: {body}");
            return false;
        }
        let body: serde_json::Value = match response.json().await {
            Ok(body) => body,
            Err(e) => {
                warn!("OIDC token response is not JSON: {e}");
                return false;
            }
        };
        let token = match body
            .get(TOKEN_RESPONSE_ACCESS_TOKEN)
            .and_then(|value| value.as_str())
        {
            Some(token) if !token.is_empty() => token.to_string(),
            _ => {
                warn!("OIDC token response missing 'access_token'");
                return false;
            }
        };
        let ttl = body
            .get(TOKEN_RESPONSE_EXPIRES_IN)
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|s| s.parse::<u64>().ok()))
            })
            .unwrap_or(OIDC_DEFAULT_TOKEN_TTL_SECS);
        {
            let mut state = self.state.write();
            state.token = Some(token);
            state.ttl_secs = ttl;
            state.obtained_at = Some(Instant::now());
        }
        debug!("OIDC access token obtained, expires_in={ttl}s");
        true
    }

    /// Force a token refresh (used after a 401/403 or an explicit reconnect).
    pub async fn refresh(&self) -> bool {
        let _guard = self.refresh_lock.lock().await;
        self.state.write().token = None;
        if self.resolved_endpoint().is_none() && !self.discover().await {
            return false;
        }
        self.fetch_token().await
    }
}

#[async_trait::async_trait]
impl ClientAuthService for OidcAuthProvider {
    fn name(&self) -> &str {
        "oidc"
    }

    async fn login(&self, _server_list: &[String]) -> bool {
        // GA: an unconfigured provider is a no-op success, not a login failure.
        if !self.config.is_configured() {
            return true;
        }
        let _guard = self.refresh_lock.lock().await;
        if self.resolved_endpoint().is_none() && !self.discover().await {
            return false;
        }
        if self.needs_refresh() && !self.fetch_token().await {
            return false;
        }
        true
    }

    fn get_identity_context(&self, _resource: &RequestResource) -> IdentityContext {
        match self.access_token() {
            Some(token) => IdentityContext::new()
                .with_header(
                    OIDC_AUTHORIZATION_HEADER,
                    &format!("{OIDC_BEARER_PREFIX}{token}"),
                )
                .with_header(OIDC_ACCESS_TOKEN_PARAM, &token),
            None => IdentityContext::new(),
        }
    }

    fn is_enabled(&self) -> bool {
        self.config.is_configured()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_ga_properties() {
        let mut props = HashMap::new();
        assert!(OidcConfig::from_properties(&props).is_none());

        props.insert(OIDC_CLIENT_ID_PROPERTY.to_string(), "sdk".to_string());
        props.insert(
            OIDC_CLIENT_SECRET_PROPERTY.to_string(),
            "secret".to_string(),
        );
        props.insert(
            OIDC_ISSUER_URI_PROPERTY.to_string(),
            "https://idp.test".to_string(),
        );
        let config = OidcConfig::from_properties(&props).unwrap();
        assert_eq!(config.client_id, "sdk");
        assert_eq!(config.scope, OIDC_DEFAULT_SCOPE);
        assert!(config.is_configured());

        props.insert(OIDC_SCOPE_PROPERTY.to_string(), "nacos".to_string());
        props.insert(
            OIDC_TOKEN_ENDPOINT_PROPERTY.to_string(),
            "https://idp.test/token".to_string(),
        );
        let config = OidcConfig::from_properties(&props).unwrap();
        assert_eq!(config.scope, "nacos");
        assert_eq!(config.token_endpoint, "https://idp.test/token");
    }

    #[test]
    fn is_configured_requires_credentials_and_endpoint() {
        assert!(!OidcConfig::default().is_configured());
        assert!(!OidcConfig::new("id", "secret").is_configured());
        assert!(
            OidcConfig::new("id", "secret")
                .with_issuer("https://idp.test")
                .is_configured()
        );
        assert!(
            OidcConfig::new("id", "secret")
                .with_token_endpoint("https://idp.test/token")
                .is_configured()
        );
        assert!(
            !OidcConfig::new(" ", "secret")
                .with_issuer("https://idp.test")
                .is_configured()
        );
    }

    #[test]
    fn discovery_url_handles_trailing_slash() {
        assert_eq!(
            OidcConfig::new("id", "secret")
                .with_issuer("https://idp.test")
                .discovery_url(),
            "https://idp.test/.well-known/openid-configuration"
        );
        assert_eq!(
            OidcConfig::new("id", "secret")
                .with_issuer("https://idp.test/")
                .discovery_url(),
            "https://idp.test/.well-known/openid-configuration"
        );
        assert_eq!(OidcConfig::new("id", "secret").discovery_url(), "");
    }

    #[test]
    fn token_form_matches_ga_field_order_and_scope_guard() {
        let config = OidcConfig::new("id", "sec ret").with_scope("nacos");
        assert_eq!(
            config.token_form(),
            vec![
                ("grant_type".to_string(), "client_credentials".to_string()),
                ("client_id".to_string(), "id".to_string()),
                ("client_secret".to_string(), "sec ret".to_string()),
                ("scope".to_string(), "nacos".to_string()),
            ]
        );
        let empty_scope = OidcConfig::new("id", "secret").with_scope("");
        assert_eq!(empty_scope.token_form().len(), 3);
    }

    #[test]
    fn refresh_window_stays_inside_ga_bounds() {
        assert_eq!(refresh_window(0), 0);
        assert_eq!(refresh_window(5), 0);
        for _ in 0..50 {
            let window = refresh_window(3600);
            assert!((240..360).contains(&window), "window={window}");
        }
    }

    #[test]
    fn transports_consume_oidc_config() {
        // "declaration != call site": a config field and an auth provider are
        // useless unless the HTTP and gRPC transports actually consume them.
        const CONFIG: &str = include_str!("client_config.rs");
        const HTTP: &str = include_str!("http.rs");
        const GRPC: &str = include_str!("grpc/mod.rs");
        const GRPC_AUTH: &str = include_str!("grpc/auth.rs");
        assert!(CONFIG.contains(concat!("OidcConfig::", "from_properties(")));
        assert!(HTTP.contains(concat!("client_config.has_oidc_auth", "()")));
        assert!(GRPC.contains(concat!("client_config.has_oidc_auth", "()")));
        assert!(GRPC_AUTH.contains(concat!("oidc.login", "(&[])")));
        assert!(HTTP.contains(concat!("with_auth_headers", "(client.get(&url)")));
        assert!(GRPC.contains(concat!("self.auth_provider.", "uses_oidc()")));
    }

    #[test]
    fn unconfigured_provider_is_disabled_with_a_noop_login() {
        let provider = OidcAuthProvider::new(OidcConfig::default()).unwrap();
        assert!(!provider.is_enabled());
        assert_eq!(provider.name(), "oidc");
        assert!(
            provider
                .get_identity_context(&RequestResource::default())
                .headers
                .is_empty()
        );
    }

    #[tokio::test]
    async fn unconfigured_login_is_a_noop() {
        // GA: an unconfigured OIDC provider returns success without touching
        // Discovery or the token endpoint.
        let provider = OidcAuthProvider::new(OidcConfig::default()).unwrap();
        assert!(provider.login(&[]).await);
        assert!(provider.access_token().is_none());
    }
}
