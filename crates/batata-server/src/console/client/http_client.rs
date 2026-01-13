// HTTP client wrapper for remote console mode
// Handles authentication, retries, and failover

use reqwest::{Client, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use std::{sync::RwLock, time::Duration};
use tracing::{debug, error, warn};

use crate::model::common::Configuration;

/// Configuration for remote console HTTP client
#[derive(Clone, Debug)]
pub struct RemoteConsoleConfig {
    pub server_addrs: Vec<String>,
    pub username: String,
    pub password: String,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
    pub context_path: String,
}

impl RemoteConsoleConfig {
    pub fn from_configuration(config: &Configuration) -> Self {
        let server_addr = config.console_remote_server_addr();
        let server_addrs: Vec<String> = server_addr
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            server_addrs,
            username: config.console_remote_username(),
            password: config.console_remote_password(),
            connect_timeout_ms: config.console_remote_connect_timeout_ms(),
            read_timeout_ms: config.console_remote_read_timeout_ms(),
            context_path: config.server_context_path(),
        }
    }
}

/// Token info for authentication
#[derive(Clone, Debug)]
struct TokenInfo {
    access_token: String,
    expires_at: std::time::Instant,
}

/// HTTP client for remote console operations
pub struct ConsoleHttpClient {
    client: Client,
    config: RemoteConsoleConfig,
    current_server_index: RwLock<usize>,
    token: RwLock<Option<TokenInfo>>,
}

impl ConsoleHttpClient {
    /// Create a new console HTTP client
    pub async fn new(config: RemoteConsoleConfig) -> anyhow::Result<Self> {
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

        // Authenticate on startup
        instance.authenticate().await?;

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

    /// Authenticate with the remote server
    pub async fn authenticate(&self) -> anyhow::Result<()> {
        let url = self.build_url("/v1/auth/login");

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
