//! HTTP client with authentication, retries, and failover
//!
//! This module provides a robust HTTP client for communicating with Batata/Nacos servers.

use bytes::Bytes;
use reqwest::{Client, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;
use std::{collections::HashMap, sync::RwLock, time::Duration};
use tracing::{debug, error, warn};

use crate::auth::{ClientAuthService, RequestResource};
use crate::oidc::OidcAuthProvider;
use crate::tls::TlsOptions;

/// Wire header mirroring Nacos 3.3.0 GA `HttpHeaderConsts.NACOS_REQUEST_ID`.
///
/// Every outgoing client HTTP request carries a fresh UUID under this header so
/// that upstream servers and log-correlation systems can trace individual
/// requests. This mirrors the GA `RequestIdHttpClientRequestInterceptor`, which
/// adds the same header to every request issued by the Nacos Java clients.
const NACOS_REQUEST_ID: &str = "Nacos-Request-Id";

/// Attach a fresh UUID correlation id to a request builder.
///
/// The id is generated per call (i.e. per physical request), matching the GA
/// interceptor which fires once per outgoing HTTP request rather than once per
/// logical operation.
fn with_request_id(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder.header(NACOS_REQUEST_ID, uuid::Uuid::new_v4().to_string())
}

/// Apply every identity header resolved by [`BatataHttpClient::ensure_identity_headers`].
///
/// GA's `LoginIdentityContext` can carry several parameters at once — the OIDC
/// provider emits both `Authorization: Bearer …` and `accessToken` — so the
/// transport must not assume a single auth header.
fn with_auth_headers(
    builder: reqwest::RequestBuilder,
    headers: &[(String, String)],
) -> reqwest::RequestBuilder {
    headers
        .iter()
        .fold(builder, |builder, (key, value)| builder.header(key, value))
}

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
    /// Server identity header key (when non-empty, uses identity auth instead of JWT)
    pub server_identity_key: String,
    /// Server identity header value
    pub server_identity_value: String,
    /// TLS settings (custom CA / mTLS identity) applied to the reqwest client
    pub tls: TlsOptions,
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
            server_identity_key: String::new(),
            server_identity_value: String::new(),
            tls: TlsOptions::default(),
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

    /// Set server identity for identity-based auth (bypasses JWT login)
    pub fn with_server_identity(mut self, key: &str, value: &str) -> Self {
        self.server_identity_key = key.to_string();
        self.server_identity_value = value.to_string();
        self
    }

    /// Apply TLS settings (custom CA / mTLS identity) to the reqwest client.
    pub fn with_tls(mut self, tls: TlsOptions) -> Self {
        self.tls = tls;
        self
    }

    /// Check if identity-based auth is configured
    pub fn has_server_identity(&self) -> bool {
        !self.server_identity_key.is_empty()
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
    /// GA OIDC Client Credentials provider, when `ClientConfig::oidc` is set.
    oidc: Option<Arc<OidcAuthProvider>>,
}

impl BatataHttpClient {
    /// Create an HTTP client from the unified `ClientConfig`.
    ///
    /// This is the preferred constructor — uses `ClientConfig` for proxy,
    /// timeouts, auth_endpoint, and context_path settings.
    pub async fn from_config(client_config: &crate::ClientConfig) -> anyhow::Result<Self> {
        let http_config = HttpClientConfig {
            server_addrs: client_config.server_addrs.clone(),
            context_path: client_config.context_path.clone(),
            auth_endpoint: format!(
                "{}{}",
                client_config.context_path, client_config.auth_endpoint
            ),
            username: client_config.username.clone(),
            password: client_config.password.clone(),
            connect_timeout_ms: client_config.connect_timeout.as_millis() as u64,
            read_timeout_ms: client_config.request_timeout.as_millis() as u64,
            tls: client_config.tls_options(),
            ..Default::default()
        };

        let reqwest_client = client_config.build_reqwest_client()?;

        let use_identity = http_config.has_server_identity();

        let oidc = if client_config.has_oidc_auth() {
            let provider = OidcAuthProvider::with_tls(
                client_config
                    .oidc
                    .as_ref()
                    .expect("has_oidc_auth implies oidc is Some")
                    .clone(),
                &client_config.tls_options(),
            )?;
            Some(Arc::new(provider))
        } else {
            None
        };

        let instance = Self {
            client: reqwest_client,
            config: http_config,
            current_server_index: RwLock::new(0),
            token: RwLock::new(None),
            oidc,
        };

        if let Some(provider) = &instance.oidc {
            // GA logs in through the SPI provider rather than the Nacos
            // username/password endpoint. A failed first login is retried on
            // demand by `ensure_identity_headers`.
            if !provider.login(&client_config.server_addrs).await {
                warn!("Initial OIDC login failed (will retry on demand)");
            }
        } else if !use_identity
            && !client_config.username.is_empty()
            && let Err(e) = instance.authenticate().await
        {
            tracing::warn!(
                "Initial authentication failed (will retry on demand): {}",
                e
            );
        }

        Ok(instance)
    }

    /// Create a new HTTP client
    pub async fn new(config: HttpClientConfig) -> anyhow::Result<Self> {
        let builder = Client::builder()
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .timeout(Duration::from_millis(config.read_timeout_ms))
            .no_proxy(); // Skip system proxy for direct server communication
        let client = config.tls.apply_to_reqwest(builder)?.build()?;

        let use_identity = config.has_server_identity();

        let instance = Self {
            client,
            config,
            current_server_index: RwLock::new(0),
            token: RwLock::new(None),
            oidc: None,
        };

        // Skip JWT login when using server identity authentication
        if !use_identity {
            // Try to authenticate on startup, but don't fail if it doesn't work.
            // The server may not be ready yet (e.g. self-connecting console datasource)
            // or no admin user may exist yet. ensure_token() will retry on each request.
            if let Err(e) = instance.authenticate().await {
                warn!(
                    "Initial authentication failed (will retry on demand): {}",
                    e
                );
            }
        } else {
            debug!("Using server identity authentication, skipping JWT login");
        }

        Ok(instance)
    }

    /// Create a client without initial authentication (for testing)
    pub fn new_without_auth(config: HttpClientConfig) -> anyhow::Result<Self> {
        let builder = Client::builder()
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .timeout(Duration::from_millis(config.read_timeout_ms))
            .no_proxy();
        let client = config.tls.apply_to_reqwest(builder)?.build()?;

        Ok(Self {
            client,
            config,
            current_server_index: RwLock::new(0),
            token: RwLock::new(None),
            oidc: None,
        })
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
        // Auth endpoint is an absolute path (already includes context path),
        // so use current_server() directly instead of build_url() which would
        // double the context path.
        let url = format!("{}{}", self.current_server(), self.config.auth_endpoint);

        debug!("Authenticating with server: {}", url);

        let response = with_request_id(self.client.post(&url).form(&[
            ("username", &self.config.username),
            ("password", &self.config.password),
        ]))
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

    /// Check if using server identity authentication
    fn use_identity_auth(&self) -> bool {
        self.config.has_server_identity()
    }

    /// Resolve all identity headers for the current auth mode.
    ///
    /// GA `ClientAuthPluginManager` merges every `LoginIdentityContext`
    /// parameter into a request; the OIDC provider contributes two headers
    /// (`Authorization: Bearer …` and `accessToken`), while JWT/identity auth
    /// contribute exactly one. Sorted for deterministic request construction.
    async fn ensure_identity_headers(&self) -> anyhow::Result<Vec<(String, String)>> {
        if self.use_identity_auth() {
            return Ok(vec![(
                self.config.server_identity_key.clone(),
                self.config.server_identity_value.clone(),
            )]);
        }
        if let Some(provider) = &self.oidc {
            if !provider.login(&[]).await {
                anyhow::bail!("OIDC login failed");
            }
            let mut headers: Vec<(String, String)> = provider
                .get_identity_context(&RequestResource::default())
                .headers
                .into_iter()
                .collect();
            headers.sort_by(|a, b| a.0.cmp(&b.0));
            if headers.is_empty() {
                anyhow::bail!("OIDC login produced no identity headers");
            }
            return Ok(headers);
        }
        let token = self.ensure_token().await?;
        Ok(vec![("accessToken".to_string(), token)])
    }

    /// Re-authenticate after a 401 using whichever provider is active.
    async fn reauthenticate(&self) -> anyhow::Result<()> {
        if let Some(provider) = &self.oidc {
            if provider.refresh().await {
                Ok(())
            } else {
                Err(anyhow::anyhow!("OIDC token refresh failed"))
            }
        } else {
            self.authenticate().await
        }
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
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.get(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
                    .send()
                    .await
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
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.get(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
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
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.post(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
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
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.post(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
                    .json(body)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a POST request with a JSON body and additional custom headers.
    ///
    /// The auth header is always applied on top of `extra_headers`, so callers
    /// only supply domain headers such as the RAD `clientId`. Retries across
    /// servers are skipped because the request closure is not `Clone`.
    pub async fn post_json_with_headers<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        extra_headers: &HashMap<String, String>,
    ) -> anyhow::Result<T> {
        let url = self.build_url(path);
        let auth_headers = self.ensure_identity_headers().await?;
        let mut builder =
            with_request_id(with_auth_headers(self.client.post(&url), &auth_headers).json(body));
        for (k, v) in extra_headers {
            builder = builder.header(k, v);
        }
        let response = builder.send().await?;
        self.handle_response(response).await
    }

    /// Make a POST request with query parameters and a JSON body
    ///
    /// Upstream endpoints such as `POST /v3/admin/cs/config/clone` combine
    /// `@RequestParam`s with an `@RequestBody`, so both have to be sent.
    pub async fn post_json_with_query<
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
        B: Serialize + ?Sized,
    >(
        &self,
        path: &str,
        query: &Q,
        body: &B,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.post(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
                    .query(query)
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
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.delete(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
                    .query(query)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a POST request with query parameters (no body)
    ///
    /// The Content-Type header is set even though there is no body: actix-web
    /// resolves the payload extractor from it, and a POST that arrives without
    /// one is rejected with 415 "Unsupported Media Type" before the handler
    /// ever sees the query. The Nacos clients always send one for POST.
    pub async fn post_with_query<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &Q,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.post(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
                    .header("Content-Type", "application/x-www-form-urlencoded")
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
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.put(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
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
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.put(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
                    .json(body)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a PUT request with both query parameters and a JSON body.
    ///
    /// The admin AI resource endpoints keep identity fields in `@RequestParam`s
    /// while taking the resource definition from an `@RequestBody`, so a plain
    /// `put_form` or `put_json` cannot express the wire shape on its own.
    pub async fn put_json_with_query<
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
        B: Serialize + ?Sized,
    >(
        &self,
        path: &str,
        query: &Q,
        body: &B,
    ) -> anyhow::Result<T> {
        self.request_with_retry(
            |client, url, auth_headers, request_id| async move {
                with_auth_headers(client.put(&url), &auth_headers)
                    .header(NACOS_REQUEST_ID, request_id)
                    .query(query)
                    .json(body)
                    .send()
                    .await
            },
            path,
        )
        .await
    }

    /// Make a GET request with query parameters and return raw bytes.
    ///
    /// The AI ADMIN download endpoints take identity parameters in the query
    /// and reply with a ZIP archive instead of a JSON `Result` envelope.
    pub async fn get_bytes_with_query<Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &Q,
    ) -> anyhow::Result<Vec<u8>> {
        let max_retries = self.config.server_addrs.len();
        let mut last_error = None;

        for _ in 0..max_retries {
            let url = self.build_url(path);
            let auth_headers = self.ensure_identity_headers().await?;

            match with_request_id(
                with_auth_headers(self.client.get(&url), &auth_headers).query(query),
            )
            .send()
            .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.bytes().await?.to_vec());
                    } else if response.status() == StatusCode::UNAUTHORIZED
                        && !self.use_identity_auth()
                    {
                        warn!("Token expired, re-authenticating...");
                        self.reauthenticate().await?;
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

    /// Make a GET request and return raw bytes
    pub async fn get_bytes(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let max_retries = self.config.server_addrs.len();
        let mut last_error = None;

        for _ in 0..max_retries {
            let url = self.build_url(path);
            let auth_headers = self.ensure_identity_headers().await?;

            match with_request_id(with_auth_headers(self.client.get(&url), &auth_headers))
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.bytes().await?.to_vec());
                    } else if response.status() == StatusCode::UNAUTHORIZED
                        && !self.use_identity_auth()
                    {
                        warn!("Token expired, re-authenticating...");
                        self.reauthenticate().await?;
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
            let auth_headers = self.ensure_identity_headers().await?;

            match with_request_id(
                with_auth_headers(self.client.post(&url), &auth_headers)
                    .header("Content-Type", "application/octet-stream")
                    .body(body.clone()), // Bytes clone is O(1)
            )
            .send()
            .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.bytes().await?.to_vec());
                    } else if response.status() == StatusCode::UNAUTHORIZED
                        && !self.use_identity_auth()
                    {
                        warn!("Token expired, re-authenticating...");
                        self.reauthenticate().await?;
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
        let auth_headers = self.ensure_identity_headers().await?;

        match with_request_id(
            with_auth_headers(self.client.post(url), &auth_headers).multipart(form),
        )
        .send()
        .await
        {
            Ok(response) => {
                if response.status() == StatusCode::UNAUTHORIZED && !self.use_identity_auth() {
                    warn!("Token expired, re-authenticating...");
                    self.reauthenticate().await?;
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

    /// Make a POST request with query parameters and multipart form data.
    ///
    /// The AI ADMIN ZIP upload endpoints keep `namespaceId`/`overwrite` in the
    /// query and the archive itself in a `file` multipart part. Multipart forms
    /// are not cloneable, so this request is not retried across servers.
    pub async fn post_multipart_with_query<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &Q,
        form: reqwest::multipart::Form,
    ) -> anyhow::Result<T> {
        let url = self.build_url(path);
        let auth_headers = self.ensure_identity_headers().await?;

        match with_request_id(
            with_auth_headers(self.client.post(&url), &auth_headers)
                .query(query)
                .multipart(form),
        )
        .send()
        .await
        {
            Ok(response) => {
                if response.status() == StatusCode::UNAUTHORIZED && !self.use_identity_auth() {
                    warn!("Token expired, re-authenticating...");
                    self.reauthenticate().await?;
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
        let auth_headers = self.ensure_identity_headers().await?;

        match with_request_id(
            with_auth_headers(self.client.post(&url), &auth_headers).multipart(form),
        )
        .send()
        .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    Ok(response.bytes().await?.to_vec())
                } else if response.status() == StatusCode::UNAUTHORIZED && !self.use_identity_auth()
                {
                    warn!("Token expired, re-authenticating...");
                    self.reauthenticate().await?;
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
        F: Fn(Client, String, Vec<(String, String)>, String) -> Fut,
        Fut: std::future::Future<Output = Result<Response, reqwest::Error>>,
    {
        let max_retries = self.config.server_addrs.len();
        let mut last_error = None;

        for _ in 0..max_retries {
            let url = self.build_url(path);
            let auth_headers = self.ensure_identity_headers().await?;
            let request_id = uuid::Uuid::new_v4().to_string();

            match request_fn(self.client.clone(), url, auth_headers, request_id).await {
                Ok(response) => {
                    if response.status() == StatusCode::UNAUTHORIZED && !self.use_identity_auth() {
                        warn!("Token expired, re-authenticating...");
                        self.reauthenticate().await?;
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

    #[test]
    fn test_config_with_server_identity() {
        let config = HttpClientConfig::new("http://localhost:8848")
            .with_server_identity("serverIdentity", "cluster-node-1");

        assert!(config.has_server_identity());
        assert_eq!(config.server_identity_key, "serverIdentity");
        assert_eq!(config.server_identity_value, "cluster-node-1");
    }

    #[test]
    fn test_config_without_server_identity() {
        let config = HttpClientConfig::default();
        assert!(!config.has_server_identity());
    }

    #[test]
    fn test_identity_auth_skips_jwt() {
        let config = HttpClientConfig::new("http://localhost:8848")
            .with_server_identity("serverIdentity", "cluster-node-1");
        let client = BatataHttpClient::new_without_auth(config).unwrap();

        assert!(client.use_identity_auth());
    }

    #[test]
    fn test_jwt_auth_when_no_identity() {
        let config = HttpClientConfig::new("http://localhost:8848");
        let client = BatataHttpClient::new_without_auth(config).unwrap();

        assert!(!client.use_identity_auth());
    }
}
