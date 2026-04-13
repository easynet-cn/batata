//! Authentication provider for gRPC connections
//!
//! Handles HTTP-based JWT authentication against the Batata server,
//! caching the token and refreshing before expiry.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use reqwest::Client;
use tracing::debug;

use crate::error::{ClientError, Result};

/// Cached token info
#[derive(Clone, Debug)]
struct TokenInfo {
    access_token: String,
    expires_at: Instant,
}

/// Authentication provider that obtains JWT tokens via HTTP login
/// and injects them into gRPC metadata headers.
pub struct AuthProvider {
    http_client: Client,
    server_addr: String,
    context_path: String,
    username: String,
    password: String,
    token: Arc<RwLock<Option<TokenInfo>>>,
    /// Token refresh buffer in seconds
    token_refresh_buffer_secs: u64,
    /// Token refresh check interval in seconds
    token_refresh_interval_secs: u64,
}

/// Default token refresh buffer: refresh 5 minutes before expiry
const DEFAULT_TOKEN_REFRESH_BUFFER_SECS: u64 = 300;

/// Default background token refresh check interval (matches Nacos Java SDK: 5 seconds)
const DEFAULT_TOKEN_REFRESH_CHECK_INTERVAL_SECS: u64 = 5;

/// Default auth HTTP connect timeout in seconds
#[allow(dead_code)]
const DEFAULT_AUTH_CONNECT_TIMEOUT_SECS: u64 = 5;

/// Default auth HTTP request timeout in seconds
#[allow(dead_code)]
const DEFAULT_AUTH_REQUEST_TIMEOUT_SECS: u64 = 10;

impl AuthProvider {
    /// Create a new AuthProvider.
    ///
    /// `server_addr` should be the base HTTP address, e.g. `http://127.0.0.1:8848`.
    /// `context_path` is the server context path, e.g. `/nacos` (default if empty).
    pub fn new(server_addr: &str, username: &str, password: &str) -> Result<Self> {
        Self::with_context_path(server_addr, "/nacos", username, password)
    }

    /// Create a new AuthProvider with a custom context path.
    ///
    /// `server_addr` should be the base HTTP address, e.g. `http://127.0.0.1:8848`.
    /// `context_path` is the server context path, e.g. `/nacos`. Use empty string for root.
    pub fn with_context_path(
        server_addr: &str,
        context_path: &str,
        username: &str,
        password: &str,
    ) -> Result<Self> {
        let http_client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .no_proxy() // Skip system proxy for direct server communication
            .build()
            .map_err(|e| ClientError::Other(e.into()))?;

        // Normalize context path: ensure it starts with "/" if non-empty, or is empty for root
        let normalized_context_path = if context_path.is_empty() || context_path == "/" {
            String::new()
        } else if context_path.starts_with('/') {
            context_path.to_string()
        } else {
            format!("/{context_path}")
        };

        Ok(Self {
            http_client,
            server_addr: server_addr.trim_end_matches('/').to_string(),
            context_path: normalized_context_path,
            username: username.to_string(),
            password: password.to_string(),
            token: Arc::new(RwLock::new(None)),
            token_refresh_buffer_secs: DEFAULT_TOKEN_REFRESH_BUFFER_SECS,
            token_refresh_interval_secs: DEFAULT_TOKEN_REFRESH_CHECK_INTERVAL_SECS,
        })
    }

    /// Create an AuthProvider that skips authentication (no credentials).
    pub fn none() -> Self {
        Self {
            http_client: Client::new(),
            server_addr: String::new(),
            context_path: String::new(),
            username: String::new(),
            password: String::new(),
            token: Arc::new(RwLock::new(None)),
            token_refresh_buffer_secs: DEFAULT_TOKEN_REFRESH_BUFFER_SECS,
            token_refresh_interval_secs: DEFAULT_TOKEN_REFRESH_CHECK_INTERVAL_SECS,
        }
    }

    /// Whether authentication is enabled
    pub fn is_enabled(&self) -> bool {
        !self.username.is_empty()
    }

    /// Get a valid access token, refreshing if needed.
    pub async fn get_token(&self) -> Result<Option<String>> {
        if !self.is_enabled() {
            return Ok(None);
        }

        // Check cached token
        {
            let guard = self.token.read().unwrap_or_else(|e| e.into_inner());
            if let Some(ref info) = *guard {
                let now = Instant::now();
                if info.expires_at > now + Duration::from_secs(self.token_refresh_buffer_secs) {
                    return Ok(Some(info.access_token.clone()));
                }
            }
        }

        // Token expired or not set, re-authenticate
        self.login().await?;

        let guard = self.token.read().unwrap_or_else(|e| e.into_inner());
        Ok(guard.as_ref().map(|info| info.access_token.clone()))
    }

    /// Perform HTTP login to get JWT token.
    async fn login(&self) -> Result<()> {
        let url = format!(
            "{}{}/v3/auth/user/login",
            self.server_addr, self.context_path
        );

        debug!("Authenticating with server: {}", url);

        let response = self
            .http_client
            .post(&url)
            .form(&[("username", &self.username), ("password", &self.password)])
            .send()
            .await
            .map_err(|e| ClientError::AuthFailed(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ClientError::AuthFailed(format!(
                "Login failed with status {}: {}",
                status, body
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            ClientError::AuthFailed(format!("Failed to parse login response: {}", e))
        })?;

        // Try to extract from data.accessToken (V3 response format) or accessToken (V2)
        let access_token = result
            .get("data")
            .and_then(|d| d.get("accessToken"))
            .or_else(|| result.get("accessToken"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ClientError::AuthFailed(format!("No accessToken in login response: {}", result))
            })?;

        let ttl = result
            .get("data")
            .and_then(|d| d.get("tokenTtl"))
            .or_else(|| result.get("tokenTtl"))
            .and_then(|v| v.as_i64())
            .unwrap_or(18000);

        let expires_at = Instant::now() + Duration::from_secs(ttl as u64);

        {
            let mut guard = self.token.write().unwrap_or_else(|e| e.into_inner());
            *guard = Some(TokenInfo {
                access_token: access_token.to_string(),
                expires_at,
            });
        }

        debug!(
            "Authentication successful, token expires in {} seconds",
            ttl
        );

        Ok(())
    }

    /// Force re-authentication (e.g., after a reconnect or 403 response).
    pub async fn refresh(&self) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }
        // Clear cached token to force re-login
        {
            let mut guard = self.token.write().unwrap_or_else(|e| e.into_inner());
            *guard = None;
        }
        self.login().await
    }

    /// Start a background task that periodically checks and refreshes the token.
    ///
    /// Follows Nacos Java SDK pattern: `scheduleWithFixedDelay(login, 0, 5s)`.
    /// The actual HTTP login only happens when the token is close to expiry
    /// (within `tokenRefreshWindow` of the TTL), so most calls are cheap no-ops.
    pub fn start_token_refresh_task(&self) {
        if !self.is_enabled() {
            return;
        }

        let server_addr = self.server_addr.clone();
        let context_path = self.context_path.clone();
        let username = self.username.clone();
        let password = self.password.clone();
        let http_client = self.http_client.clone();
        let token = self.token.clone();
        let refresh_interval = self.token_refresh_interval_secs;
        let refresh_buffer = self.token_refresh_buffer_secs;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(refresh_interval));
            loop {
                interval.tick().await;

                // Check if token needs refresh
                let needs_refresh = {
                    let guard = token.read().unwrap_or_else(|e| e.into_inner());
                    match guard.as_ref() {
                        Some(info) => {
                            // Refresh when token is within TOKEN_REFRESH_BUFFER_SECS of expiry
                            info.expires_at <= Instant::now() + Duration::from_secs(refresh_buffer)
                        }
                        None => true,
                    }
                };

                if !needs_refresh {
                    continue;
                }

                // Perform login
                let url = format!("{}{}/v3/auth/user/login", server_addr, context_path);
                match http_client
                    .post(&url)
                    .form(&[("username", &username), ("password", &password)])
                    .send()
                    .await
                {
                    Ok(response) if response.status().is_success() => {
                        if let Ok(result) = response.json::<serde_json::Value>().await {
                            let access_token = result
                                .get("data")
                                .and_then(|d| d.get("accessToken"))
                                .or_else(|| result.get("accessToken"))
                                .and_then(|v| v.as_str());
                            let ttl = result
                                .get("data")
                                .and_then(|d| d.get("tokenTtl"))
                                .or_else(|| result.get("tokenTtl"))
                                .and_then(|v| v.as_i64())
                                .unwrap_or(18000);

                            if let Some(at) = access_token {
                                let mut guard = token.write().unwrap_or_else(|e| e.into_inner());
                                *guard = Some(TokenInfo {
                                    access_token: at.to_string(),
                                    expires_at: Instant::now() + Duration::from_secs(ttl as u64),
                                });
                                debug!("Token refreshed, expires in {}s", ttl);
                            }
                        }
                    }
                    Ok(response) => {
                        tracing::warn!(
                            "Token refresh login failed with status {}",
                            response.status()
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Token refresh login error: {}", e);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_provider_none() {
        let provider = AuthProvider::none();
        assert!(!provider.is_enabled());
        assert!(provider.username.is_empty());
        assert!(provider.server_addr.is_empty());
    }

    #[test]
    fn test_auth_provider_enabled() {
        let provider = AuthProvider::new("http://localhost:8848", "nacos", "nacos").unwrap();
        assert!(provider.is_enabled());
    }

    #[test]
    fn test_auth_provider_empty_username() {
        let provider = AuthProvider::new("http://localhost:8848", "", "password").unwrap();
        assert!(!provider.is_enabled());
    }

    #[test]
    fn test_auth_provider_trims_trailing_slash() {
        let provider = AuthProvider::new("http://localhost:8848/", "user", "pass").unwrap();
        assert_eq!(provider.server_addr, "http://localhost:8848");
    }

    #[test]
    fn test_auth_provider_trims_multiple_trailing_slashes() {
        let provider = AuthProvider::new("http://localhost:8848///", "user", "pass").unwrap();
        // trim_end_matches removes all trailing slashes
        assert!(!provider.server_addr.ends_with('/'));
    }

    #[tokio::test]
    async fn test_no_auth_returns_none() {
        let provider = AuthProvider::none();
        let token = provider.get_token().await.unwrap();
        assert!(token.is_none());
    }

    #[tokio::test]
    async fn test_refresh_no_auth() {
        let provider = AuthProvider::none();
        // Refresh on a disabled provider should be no-op
        provider.refresh().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_token_requires_login() {
        // Provider with credentials but unreachable server
        let provider = AuthProvider::new("http://127.0.0.1:1", "user", "pass").unwrap();
        // Should fail because server is not reachable
        let result = provider.get_token().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_token_refresh_buffer() {
        assert_eq!(DEFAULT_TOKEN_REFRESH_BUFFER_SECS, 300);
    }

    #[test]
    fn test_token_info_struct() {
        let token = TokenInfo {
            access_token: "test-token".to_string(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        };
        assert_eq!(token.access_token, "test-token");
        assert!(token.expires_at > Instant::now());
    }
}
