//! Authentication provider for gRPC connections
//!
//! Handles HTTP-based JWT authentication against the Nacos server,
//! caching the token and refreshing before expiry.

use std::sync::RwLock;
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
    username: String,
    password: String,
    token: RwLock<Option<TokenInfo>>,
}

/// Token refresh buffer: refresh 5 minutes before expiry
const TOKEN_REFRESH_BUFFER_SECS: u64 = 300;

impl AuthProvider {
    /// Create a new AuthProvider.
    ///
    /// `server_addr` should be the base HTTP address, e.g. `http://127.0.0.1:8848`.
    pub fn new(server_addr: &str, username: &str, password: &str) -> Result<Self> {
        let http_client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ClientError::Other(e.into()))?;

        Ok(Self {
            http_client,
            server_addr: server_addr.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
            token: RwLock::new(None),
        })
    }

    /// Create an AuthProvider that skips authentication (no credentials).
    pub fn none() -> Self {
        Self {
            http_client: Client::new(),
            server_addr: String::new(),
            username: String::new(),
            password: String::new(),
            token: RwLock::new(None),
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
                if info.expires_at > now + Duration::from_secs(TOKEN_REFRESH_BUFFER_SECS) {
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
        let url = format!("{}/nacos/v3/auth/user/login", self.server_addr);

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

    /// Force re-authentication (e.g., after a reconnect).
    pub async fn refresh(&self) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }
        self.login().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_provider_none() {
        let provider = AuthProvider::none();
        assert!(!provider.is_enabled());
    }

    #[test]
    fn test_auth_provider_enabled() {
        let provider = AuthProvider::new("http://localhost:8848", "nacos", "nacos").unwrap();
        assert!(provider.is_enabled());
    }

    #[tokio::test]
    async fn test_no_auth_returns_none() {
        let provider = AuthProvider::none();
        let token = provider.get_token().await.unwrap();
        assert!(token.is_none());
    }
}
