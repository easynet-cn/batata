//! HTTP test client for API testing
//!
//! Provides a lightweight HTTP client optimized for integration testing.
//! Uses `BatataHttpClient` from `batata-client` for HTTP transport,
//! adding test-specific features (raw responses, token management, error types).

use batata_client::{BatataHttpClient, HttpClientConfig};
use reqwest::{Client, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;

/// Nacos V2 API response wrapper
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NacosResponse<T> {
    /// Response code (0 = success)
    pub code: i32,
    /// Response message
    pub message: Option<String>,
    /// Response data
    pub data: Option<T>,
}

impl<T> NacosResponse<T> {
    /// Check if the response indicates success
    pub fn is_success(&self) -> bool {
        self.code == 0 || self.code == 200
    }

    /// Get data or return error
    pub fn into_data(self) -> Result<T, TestClientError> {
        if self.is_success() {
            self.data.ok_or(TestClientError::EmptyResponse)
        } else {
            Err(TestClientError::ApiError {
                code: self.code,
                message: self.message.unwrap_or_default(),
            })
        }
    }
}

/// Login response
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub access_token: String,
    pub token_ttl: i64,
    pub global_admin: bool,
    pub username: String,
}

/// Test HTTP client backed by `BatataHttpClient` for transport.
///
/// Adds test-specific features on top:
/// - Raw response access (`raw_get`, `raw_post_form`)
/// - Mutable token management (`set_token`, `clear_token`)
/// - Cross-port login (`login_via`)
/// - Test-specific error types (`TestClientError`)
pub struct TestClient {
    /// Low-level reqwest client for raw requests and login
    client: Client,
    base_url: String,
    access_token: Option<String>,
    /// High-level HTTP client for typed requests (initialized after login)
    http_client: Option<BatataHttpClient>,
}

impl TestClient {
    /// Create a new test client
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .no_proxy()
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token: None,
            http_client: None,
        }
    }

    /// Create a new test client and login
    pub async fn new_with_login(
        base_url: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, TestClientError> {
        let mut client = Self::new(base_url);
        client.login(username, password).await?;
        Ok(client)
    }

    /// Create a new test client with existing access token
    pub fn new_with_token(base_url: &str, token: &str) -> Self {
        let mut client = Self::new(base_url);
        client.access_token = Some(token.to_string());
        client
    }

    /// Create a new test client with existing cookies (as token)
    pub fn new_with_cookies(base_url: &str, cookies: String) -> Self {
        let mut client = Self::new(base_url);
        client.access_token = Some(cookies);
        client
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the access token (for sharing between clients)
    pub fn token(&self) -> Option<&String> {
        self.access_token.as_ref()
    }

    /// Get the cookies (as token string, for compatibility)
    pub fn cookies(&self) -> String {
        self.access_token.clone().unwrap_or_default()
    }

    /// Initialize the `BatataHttpClient` with credentials for typed requests.
    fn init_http_client(&mut self, username: &str, password: &str, auth_url: &str) {
        let config = HttpClientConfig::new(&self.base_url)
            .with_auth(username, password)
            .with_auth_endpoint(auth_url);

        // Use new_without_auth since we already have the token
        if let Ok(hc) = BatataHttpClient::new_without_auth(config) {
            self.http_client = Some(hc);
        }
    }

    /// Parse login response body, supporting both flat and wrapped formats.
    /// Flat format: `{"accessToken": "...", "tokenTtl": ..., ...}`
    /// Wrapped format: `{"code": 0, "data": {"accessToken": "...", ...}}`
    fn parse_login_response(&mut self, body: &str) -> Result<(), TestClientError> {
        // Try flat format first (console server returns this)
        if let Ok(data) = serde_json::from_str::<LoginResponse>(body) {
            self.access_token = Some(data.access_token);
            return Ok(());
        }

        // Try wrapped NacosResponse format
        if let Ok(result) = serde_json::from_str::<NacosResponse<LoginResponse>>(body)
            && result.is_success()
            && let Some(data) = result.data
        {
            self.access_token = Some(data.access_token);
            return Ok(());
        }

        Err(TestClientError::LoginFailed)
    }

    /// Login via a specific URL (for cross-port auth)
    /// Use this when the login endpoint is on a different server than the API endpoints.
    pub async fn login_via(
        &mut self,
        auth_base_url: &str,
        username: &str,
        password: &str,
    ) -> Result<(), TestClientError> {
        let url = format!("{}/v3/auth/user/login", auth_base_url.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        if response.status().is_success() {
            let body = response
                .text()
                .await
                .map_err(|e| TestClientError::ParseFailed(e.to_string()))?;
            self.parse_login_response(&body)?;
            self.init_http_client(username, password, "/v3/auth/user/login");
            return Ok(());
        }

        Err(TestClientError::LoginFailed)
    }

    /// Login and store access token
    pub async fn login(&mut self, username: &str, password: &str) -> Result<(), TestClientError> {
        let url = format!("{}/v3/auth/user/login", self.base_url);

        let response = self
            .client
            .post(&url)
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        if response.status().is_success() {
            let body = response
                .text()
                .await
                .map_err(|e| TestClientError::ParseFailed(e.to_string()))?;
            self.parse_login_response(&body)?;
            self.init_http_client(username, password, "/v3/auth/user/login");
            return Ok(());
        }

        Err(TestClientError::LoginFailed)
    }

    /// Set access token directly (for testing without login)
    pub fn set_token(&mut self, token: &str) {
        self.access_token = Some(token.to_string());
    }

    /// Clear access token
    pub fn clear_token(&mut self) {
        self.access_token = None;
    }

    /// Build full URL
    fn build_url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }

    /// Add authentication header to request builder
    fn add_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.access_token {
            builder.header("accessToken", token)
        } else {
            builder
        }
    }

    /// Make a GET request
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.get(&url));

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a GET request with query parameters
    pub async fn get_with_query<T: DeserializeOwned, Q: Serialize>(
        &self,
        path: &str,
        query: &Q,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.get(&url)).query(query);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a POST request with form data
    pub async fn post_form<T: DeserializeOwned, F: Serialize>(
        &self,
        path: &str,
        form: &F,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.post(&url)).form(form);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a POST request with form data and timeout
    pub async fn post_form_with_timeout<T: DeserializeOwned, F: Serialize>(
        &self,
        path: &str,
        form: &F,
        timeout: Duration,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.post(&url)).form(form);

        let response = tokio::time::timeout(timeout, builder.send())
            .await
            .map_err(|_| TestClientError::RequestFailed("Timeout".to_string()))?
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a POST request with JSON body
    pub async fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.post(&url)).json(body);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a PUT request with form data
    pub async fn put_form<T: DeserializeOwned, F: Serialize>(
        &self,
        path: &str,
        form: &F,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.put(&url)).form(form);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a PUT request with JSON body
    pub async fn put_json<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.put(&url)).json(body);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a PUT request with query parameters
    pub async fn put_with_query<T: DeserializeOwned, Q: Serialize>(
        &self,
        path: &str,
        query: &Q,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.put(&url)).query(query);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a PATCH request with form data
    pub async fn patch_form<T: DeserializeOwned, F: Serialize>(
        &self,
        path: &str,
        form: &F,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.patch(&url)).form(form);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a DELETE request
    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.delete(&url));

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a DELETE request with query parameters
    pub async fn delete_with_query<T: DeserializeOwned, Q: Serialize>(
        &self,
        path: &str,
        query: &Q,
    ) -> Result<T, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.delete(&url)).query(query);

        let response = builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a raw request and return the response without parsing
    pub async fn raw_get(&self, path: &str) -> Result<Response, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.get(&url));

        builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))
    }

    /// Make a raw POST request with form data
    pub async fn raw_post_form<F: Serialize>(
        &self,
        path: &str,
        form: &F,
    ) -> Result<Response, TestClientError> {
        let url = self.build_url(path);
        let builder = self.add_auth(self.client.post(&url)).form(form);

        builder
            .send()
            .await
            .map_err(|e| TestClientError::RequestFailed(e.to_string()))
    }

    /// Handle response and parse JSON
    async fn handle_response<T: DeserializeOwned>(
        &self,
        response: Response,
    ) -> Result<T, TestClientError> {
        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(TestClientError::Unauthorized);
        }

        if status == StatusCode::NOT_FOUND {
            return Err(TestClientError::NotFound);
        }

        let body = response
            .text()
            .await
            .map_err(|e| TestClientError::ParseFailed(e.to_string()))?;

        if status.is_success() || status == StatusCode::OK {
            serde_json::from_str(&body).map_err(|e| {
                TestClientError::ParseFailed(format!("Failed to parse: {} - body: {}", e, body))
            })
        } else {
            Err(TestClientError::HttpError {
                status: status.as_u16(),
                body,
            })
        }
    }

    /// Get the underlying `BatataHttpClient` (available after login).
    /// Useful for tests that want to use typed API client methods.
    pub fn http_client(&self) -> Option<&BatataHttpClient> {
        self.http_client.as_ref()
    }
}

/// Errors that can occur when using the test client
#[derive(Debug)]
pub enum TestClientError {
    /// HTTP request failed
    RequestFailed(String),
    /// Failed to parse response
    ParseFailed(String),
    /// Login failed
    LoginFailed,
    /// Unauthorized (401)
    Unauthorized,
    /// Not found (404)
    NotFound,
    /// Empty response data
    EmptyResponse,
    /// API returned an error
    ApiError { code: i32, message: String },
    /// HTTP error with status code
    HttpError { status: u16, body: String },
}

impl std::fmt::Display for TestClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestFailed(e) => write!(f, "Request failed: {}", e),
            Self::ParseFailed(e) => write!(f, "Parse failed: {}", e),
            Self::LoginFailed => write!(f, "Login failed"),
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::NotFound => write!(f, "Not found"),
            Self::EmptyResponse => write!(f, "Empty response"),
            Self::ApiError { code, message } => write!(f, "API error {}: {}", code, message),
            Self::HttpError { status, body } => write!(f, "HTTP error {}: {}", status, body),
        }
    }
}

impl std::error::Error for TestClientError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url() {
        let client = TestClient::new("http://localhost:8848");
        assert_eq!(
            client.build_url("/nacos/v2/cs/config"),
            "http://localhost:8848/nacos/v2/cs/config"
        );
        assert_eq!(
            client.build_url("nacos/v2/cs/config"),
            "http://localhost:8848/nacos/v2/cs/config"
        );
    }

    #[test]
    fn test_build_url_trailing_slash() {
        let client = TestClient::new("http://localhost:8848/");
        assert_eq!(
            client.build_url("/nacos/v2/cs/config"),
            "http://localhost:8848/nacos/v2/cs/config"
        );
    }

    #[test]
    fn test_nacos_response_success() {
        let response: NacosResponse<String> = NacosResponse {
            code: 0,
            message: Some("success".to_string()),
            data: Some("test".to_string()),
        };
        assert!(response.is_success());
        assert_eq!(response.into_data().unwrap(), "test");
    }

    #[test]
    fn test_nacos_response_error() {
        let response: NacosResponse<String> = NacosResponse {
            code: 500,
            message: Some("error".to_string()),
            data: None,
        };
        assert!(!response.is_success());
        assert!(response.into_data().is_err());
    }
}
