//! HTTP client with authentication, retries, and failover
//!
//! This module provides a robust HTTP client for communicating with Batata/Nacos servers.

use bytes::Bytes;
use reqwest::{Client, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use std::{sync::RwLock, time::Duration};
use tracing::{debug, error, warn};

/// Configuration for the HTTP client
#[derive(Clone, Debug)]
pub struct HttpClientConfig {
    /// List of server addresses to connect to
    pub server_addrs: Vec<String>,
    /// Username for authentication
    pub username: String,
    /// Password for authentication
    pub password: String,
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Read timeout in milliseconds
    pub read_timeout_ms: u64,
    /// Context path (e.g., "/nacos")
    pub context_path: String,
    /// Auth endpoint path (default: "/v1/auth/login")
    pub auth_endpoint: String,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            server_addrs: vec!["http://127.0.0.1:8848".to_string()],
            username: "nacos".to_string(),
            password: "nacos".to_string(),
            connect_timeout_ms: 5000,
            read_timeout_ms: 30000,
            context_path: String::new(),
            auth_endpoint: "/v1/auth/login".to_string(),
        }
    }
}

impl HttpClientConfig {
    /// Create a new config with a single server address
    pub fn new(server_addr: &str) -> Self {
        Self {
            server_addrs: vec![server_addr.to_string()],
            ..Default::default()
        }
    }

    /// Create a config with multiple server addresses
    pub fn with_servers(server_addrs: Vec<String>) -> Self {
        Self {
            server_addrs,
            ..Default::default()
        }
    }

    /// Set authentication credentials
    pub fn with_auth(mut self, username: &str, password: &str) -> Self {
        self.username = username.to_string();
        self.password = password.to_string();
        self
    }

    /// Set timeouts
    pub fn with_timeouts(mut self, connect_ms: u64, read_ms: u64) -> Self {
        self.connect_timeout_ms = connect_ms;
        self.read_timeout_ms = read_ms;
        self
    }

    /// Set context path
    pub fn with_context_path(mut self, path: &str) -> Self {
        self.context_path = path.to_string();
        self
    }

    /// Set auth endpoint path
    pub fn with_auth_endpoint(mut self, endpoint: &str) -> Self {
        self.auth_endpoint = endpoint.to_string();
        self
    }
}

/// Token info for authentication
#[derive(Clone, Debug)]
struct TokenInfo {
    access_token: String,
    expires_at: std::time::Instant,
}

/// HTTP client with authentication and failover support
pub struct BatataHttpClient {
    client: Client,
    config: HttpClientConfig,
    current_server_index: RwLock<usize>,
    token: RwLock<Option<TokenInfo>>,
}

impl BatataHttpClient {
    /// Create a new HTTP client
    pub async fn new(config: HttpClientConfig) -> anyhow::Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .timeout(Duration::from_millis(config.read_timeout_ms))
            .build()?;

        let instance = Self {
            client,
            config,
            current_server_index: RwLock::new(0),
            token: RwLock::new(None),
        };

        // Try to authenticate on startup, but don't fail if it doesn't work.
        // The server may not be ready yet (e.g. self-connecting console datasource)
        // or no admin user may exist yet. ensure_token() will retry on each request.
        if let Err(e) = instance.authenticate().await {
            warn!("Initial authentication failed (will retry on demand): {}", e);
        }

        Ok(instance)
    }

    /// Create a client without initial authentication (for testing)
    pub fn new_without_auth(config: HttpClientConfig) -> anyhow::Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .timeout(Duration::from_millis(config.read_timeout_ms))
            .build()?;

        Ok(Self {
            client,
            config,
            current_server_index: RwLock::new(0),
            token: RwLock::new(None),
        })
    }

    /// Create a client with a pre-generated token, bypassing HTTP login.
    /// Used for embedded mode where the server connects to itself locally.
    /// Disables proxy to avoid system proxy intercepting localhost requests.
    pub fn new_with_token(
        config: HttpClientConfig,
        token: String,
        ttl_seconds: i64,
    ) -> anyhow::Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .timeout(Duration::from_millis(config.read_timeout_ms))
            .no_proxy()
            .build()?;

        let instance = Self {
            client,
            config,
            current_server_index: RwLock::new(0),
            token: RwLock::new(None),
        };

        instance.set_token(token, ttl_seconds);
        Ok(instance)
    }

    /// Get the current server URL
    fn current_server(&self) -> String {
        let index = *self
            .current_server_index
            .read()
            .unwrap_or_else(|e| e.into_inner());
        self.config.server_addrs[index].clone()
    }

    /// Switch to the next server (for failover)
    fn switch_to_next_server(&self) {
        let mut index = self
            .current_server_index
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *index = (*index + 1) % self.config.server_addrs.len();
        debug!("Switched to server index: {}", *index);
    }

    /// Build full URL with context path
    fn build_url(&self, path: &str) -> String {
        let base_url = self.current_server();
        let context_path = &self.config.context_path;

        if context_path.is_empty() {
            format!("{}{}", base_url, path)
        } else {
            format!(
                "{}/{}{}",
                base_url,
                context_path.trim_start_matches('/'),
                path
            )
        }
    }

    /// Get the current access token
    fn get_token(&self) -> Option<String> {
        let token_guard = self.token.read().unwrap_or_else(|e| e.into_inner());
        token_guard.as_ref().and_then(|t| {
            // Check if token is still valid (with 5 minute buffer)
            if t.expires_at > std::time::Instant::now() + Duration::from_secs(300) {
                Some(t.access_token.clone())
            } else {
                None
            }
        })
    }

    /// Set the access token
    fn set_token(&self, access_token: String, ttl_seconds: i64) {
        let expires_at = std::time::Instant::now() + Duration::from_secs(ttl_seconds as u64);
        let mut token_guard = self.token.write().unwrap_or_else(|e| e.into_inner());
        *token_guard = Some(TokenInfo {
            access_token,
            expires_at,
        });
    }

    /// Authenticate with the server
    pub async fn authenticate(&self) -> anyhow::Result<()> {
        let url = self.build_url(&self.config.auth_endpoint);

        debug!("Authenticating with server: {}", url);

        let response = self
            .client
            .post(&url)
            .form(&[
                ("username", &self.config.username),
                ("password", &self.config.password),
            ])
            .send()
            .await?;

        if response.status().is_success() {
            let result: serde_json::Value = response.json().await?;

            if let Some(access_token) = result.get("accessToken").and_then(|v| v.as_str()) {
                let ttl = result
                    .get("tokenTtl")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(18000);

                self.set_token(access_token.to_string(), ttl);
                debug!(
                    "Authentication successful, token expires in {} seconds",
                    ttl
                );
                return Ok(());
            }
        }

        Err(anyhow::anyhow!("Authentication failed"))
    }

    /// Ensure we have a valid token, refreshing if needed
    async fn ensure_token(&self) -> anyhow::Result<String> {
        if let Some(token) = self.get_token() {
            return Ok(token);
        }

        // Token expired or not set, re-authenticate
        self.authenticate().await?;

        self.get_token()
            .ok_or_else(|| anyhow::anyhow!("Failed to get token after authentication"))
    }

    /// Make a GET request
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client.get(&url).header("accessToken", &token).send().await
            },
            path,
        )
        .await
    }

    /// Make a GET request with query parameters
    pub async fn get_with_query<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &Q,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client
                    .get(&url)
                    .header("accessToken", &token)
                    .query(query)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a POST request with form data
    pub async fn post_form<T: DeserializeOwned, F: Serialize + ?Sized>(
        &self,
        path: &str,
        form: &F,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client
                    .post(&url)
                    .header("accessToken", &token)
                    .form(form)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a POST request with JSON body
    pub async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client
                    .post(&url)
                    .header("accessToken", &token)
                    .json(body)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a DELETE request with query parameters
    pub async fn delete_with_query<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &Q,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client
                    .delete(&url)
                    .header("accessToken", &token)
                    .query(query)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a POST request with query parameters (no body)
    pub async fn post_with_query<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &Q,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client
                    .post(&url)
                    .header("accessToken", &token)
                    .query(query)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a PUT request with form data
    pub async fn put_form<T: DeserializeOwned, F: Serialize + ?Sized>(
        &self,
        path: &str,
        form: &F,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client
                    .put(&url)
                    .header("accessToken", &token)
                    .form(form)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a PUT request with JSON body
    pub async fn put_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, token| async move {
                client
                    .put(&url)
                    .header("accessToken", &token)
                    .json(body)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a GET request and return raw bytes
    pub async fn get_bytes(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let max_retries = self.config.server_addrs.len();
        let mut last_error = None;

        for _ in 0..max_retries {
            let url = self.build_url(path);
            let token = self.ensure_token().await?;

            match self
                .client
                .get(&url)
                .header("accessToken", &token)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.bytes().await?.to_vec());
                    } else if response.status() == StatusCode::UNAUTHORIZED {
                        warn!("Token expired, re-authenticating...");
                        self.authenticate().await?;
                        continue;
                    } else {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        last_error = Some(anyhow::anyhow!(
                            "Request failed with status {}: {}",
                            status,
                            body
                        ));
                    }
                }
                Err(e) => {
                    warn!("Request failed: {}, switching to next server", e);
                    self.switch_to_next_server();
                    last_error = Some(e.into());
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All servers failed")))
    }

    /// Make a POST request with bytes body
    pub async fn post_bytes(&self, path: &str, bytes: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        let max_retries = self.config.server_addrs.len();
        let mut last_error = None;
        // Convert to Bytes for O(1) cloning in retry loop
        let body = Bytes::from(bytes);

        for _ in 0..max_retries {
            let url = self.build_url(path);
            let token = self.ensure_token().await?;

            match self
                .client
                .post(&url)
                .header("accessToken", &token)
                .header("Content-Type", "application/octet-stream")
                .body(body.clone()) // Bytes clone is O(1)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.bytes().await?.to_vec());
                    } else if response.status() == StatusCode::UNAUTHORIZED {
                        warn!("Token expired, re-authenticating...");
                        self.authenticate().await?;
                        continue;
                    } else {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        last_error = Some(anyhow::anyhow!(
                            "Request failed with status {}: {}",
                            status,
                            body
                        ));
                    }
                }
                Err(e) => {
                    warn!("Request failed: {}, switching to next server", e);
                    self.switch_to_next_server();
                    last_error = Some(e.into());
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All servers failed")))
    }

    /// Make a POST request with multipart form data (for file uploads)
    /// Note: multipart requests cannot be retried across servers since Form cannot be cloned
    pub async fn post_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> anyhow::Result<T> {
        let url = self.build_url(path).clone();
        let token = self.ensure_token().await?;

        match self
            .client
            .post(url)
            .header("accessToken", &token)
            .multipart(form)
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == StatusCode::UNAUTHORIZED {
                    warn!("Token expired, re-authenticating...");
                    self.authenticate().await?;
                    // Token expired, caller should retry
                    return Err(anyhow::anyhow!("Token expired, please retry"));
                }
                self.handle_response(response).await
            }
            Err(e) => {
                warn!("Request failed: {}", e);
                Err(e.into())
            }
        }
    }

    /// Make a POST request with multipart form data and return raw bytes
    /// Note: multipart requests cannot be retried across servers since Form cannot be cloned
    pub async fn post_multipart_bytes(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> anyhow::Result<Vec<u8>> {
        let url = self.build_url(path);
        let token = self.ensure_token().await?;

        match self
            .client
            .post(&url)
            .header("accessToken", &token)
            .multipart(form)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    Ok(response.bytes().await?.to_vec())
                } else if response.status() == StatusCode::UNAUTHORIZED {
                    warn!("Token expired, re-authenticating...");
                    self.authenticate().await?;
                    Err(anyhow::anyhow!("Token expired, please retry"))
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    Err(anyhow::anyhow!(
                        "Request failed with status {}: {}",
                        status,
                        body
                    ))
                }
            }
            Err(e) => {
                warn!("Request failed: {}", e);
                Err(e.into())
            }
        }
    }

    /// Generic request with retry logic
    async fn request_with_retry<T, F, Fut>(&self, request_fn: F, path: &str) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
        F: Fn(Client, String, String) -> Fut,
        Fut: std::future::Future<Output = Result<Response, reqwest::Error>>,
    {
        let max_retries = self.config.server_addrs.len();
        let mut last_error = None;

        for _ in 0..max_retries {
            let url = self.build_url(path);
            let token = self.ensure_token().await?;

            match request_fn(self.client.clone(), url, token).await {
                Ok(response) => {
                    if response.status() == StatusCode::UNAUTHORIZED {
                        warn!("Token expired, re-authenticating...");
                        self.authenticate().await?;
                        continue;
                    }
                    return self.handle_response(response).await;
                }
                Err(e) => {
                    warn!("Request failed: {}, switching to next server", e);
                    self.switch_to_next_server();
                    last_error = Some(e.into());
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All servers failed")))
    }

    /// Handle response and parse JSON
    async fn handle_response<T: DeserializeOwned>(&self, response: Response) -> anyhow::Result<T> {
        let status = response.status();

        if status.is_success() {
            let result = response.json::<T>().await?;
            Ok(result)
        } else {
            let body = response.text().await.unwrap_or_default();
            error!("Request failed with status {}: {}", status, body);
            Err(anyhow::anyhow!(
                "Request failed with status {}: {}",
                status,
                body
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = HttpClientConfig::default();
        assert_eq!(config.server_addrs.len(), 1);
        assert_eq!(config.username, "nacos");
        assert_eq!(config.connect_timeout_ms, 5000);
    }

    #[test]
    fn test_config_builder() {
        let config = HttpClientConfig::new("http://localhost:8848")
            .with_auth("admin", "secret")
            .with_timeouts(3000, 15000)
            .with_context_path("/nacos");

        assert_eq!(config.server_addrs[0], "http://localhost:8848");
        assert_eq!(config.username, "admin");
        assert_eq!(config.password, "secret");
        assert_eq!(config.connect_timeout_ms, 3000);
        assert_eq!(config.read_timeout_ms, 15000);
        assert_eq!(config.context_path, "/nacos");
    }

    #[test]
    fn test_config_with_servers() {
        let config = HttpClientConfig::with_servers(vec![
            "http://server1:8848".to_string(),
            "http://server2:8848".to_string(),
        ]);

        assert_eq!(config.server_addrs.len(), 2);
    }

    #[test]
    fn test_build_url_no_context() {
        let config = HttpClientConfig::new("http://localhost:8848");
        let client = BatataHttpClient::new_without_auth(config).unwrap();

        assert_eq!(
            client.build_url("/v1/test"),
            "http://localhost:8848/v1/test"
        );
    }

    #[test]
    fn test_build_url_with_context() {
        let config = HttpClientConfig::new("http://localhost:8848").with_context_path("/nacos");
        let client = BatataHttpClient::new_without_auth(config).unwrap();

        assert_eq!(
            client.build_url("/v1/test"),
            "http://localhost:8848/nacos/v1/test"
        );
    }
}
