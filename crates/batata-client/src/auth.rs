//! Pluggable authentication provider system
//!
//! Supports multiple auth providers (JWT, AccessKey/SecretKey) compatible
//! with Nacos ClientAuthService/SecurityProxy architecture.

use std::collections::HashMap;
use std::sync::Arc;

/// Identity context containing auth headers to attach to requests
#[derive(Debug, Clone, Default)]
pub struct IdentityContext {
    /// Headers to add to HTTP/gRPC requests
    pub headers: HashMap<String, String>,
}

impl IdentityContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Merge another context's headers into this one
    pub fn merge(&mut self, other: &IdentityContext) {
        self.headers.extend(other.headers.clone());
    }
}

/// Resource being accessed (for signing/authorization)
#[derive(Debug, Clone, Default)]
pub struct RequestResource {
    pub namespace: String,
    pub group: String,
    pub resource: String,
    pub resource_type: String,
}

/// Trait for pluggable auth providers.
///
/// Compatible with Nacos ClientAuthService SPI.
#[async_trait::async_trait]
pub trait ClientAuthService: Send + Sync {
    /// Provider name
    fn name(&self) -> &str;

    /// Authenticate with the server. Returns true if login succeeded.
    async fn login(&self, server_list: &[String]) -> bool;

    /// Get identity headers to attach to a request.
    fn get_identity_context(&self, resource: &RequestResource) -> IdentityContext;

    /// Whether this provider is enabled
    fn is_enabled(&self) -> bool {
        true
    }
}

/// JWT (username/password) auth provider — the default Nacos auth mechanism.
pub struct JwtAuthProvider {
    username: String,
    password: String,
    context_path: String,
    token: parking_lot::RwLock<Option<String>>,
    token_ttl: parking_lot::RwLock<i64>,
    last_login: parking_lot::RwLock<std::time::Instant>,
}

impl JwtAuthProvider {
    pub fn new(username: &str, password: &str, context_path: &str) -> Self {
        Self {
            username: username.to_string(),
            password: password.to_string(),
            context_path: context_path.to_string(),
            token: parking_lot::RwLock::new(None),
            token_ttl: parking_lot::RwLock::new(18000),
            last_login: parking_lot::RwLock::new(std::time::Instant::now()),
        }
    }

    fn has_valid_token(&self) -> bool {
        let token = self.token.read();
        if token.is_none() {
            return false;
        }
        let elapsed = self.last_login.read().elapsed().as_secs() as i64;
        let ttl = *self.token_ttl.read();
        elapsed < ttl - 300 // 5-minute buffer
    }
}

#[async_trait::async_trait]
impl ClientAuthService for JwtAuthProvider {
    fn name(&self) -> &str {
        "jwt"
    }

    async fn login(&self, server_list: &[String]) -> bool {
        if self.username.is_empty() {
            return false;
        }
        if self.has_valid_token() {
            return true;
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        for server in server_list {
            let url = format!(
                "{}{}/v3/auth/user/login",
                server.trim_end_matches('/'),
                self.context_path
            );

            let result = client
                .post(&url)
                .form(&[("username", &self.username), ("password", &self.password)])
                .send()
                .await;

            if let Ok(resp) = result
                && resp.status().is_success()
                && let Ok(body) = resp.text().await
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
            {
                let token = json
                    .get("data")
                    .and_then(|d| d.get("accessToken"))
                    .or_else(|| json.get("accessToken"))
                    .and_then(|t| t.as_str());

                if let Some(token_str) = token {
                    *self.token.write() = Some(token_str.to_string());
                    if let Some(ttl) = json
                        .get("data")
                        .and_then(|d| d.get("tokenTtl"))
                        .or_else(|| json.get("tokenTtl"))
                        .and_then(|t| t.as_i64())
                    {
                        *self.token_ttl.write() = ttl;
                    }
                    *self.last_login.write() = std::time::Instant::now();
                    return true;
                }
            }
        }
        false
    }

    fn get_identity_context(&self, _resource: &RequestResource) -> IdentityContext {
        let mut ctx = IdentityContext::new();
        if let Some(token) = self.token.read().as_ref() {
            ctx.headers.insert("accessToken".to_string(), token.clone());
        }
        ctx
    }

    fn is_enabled(&self) -> bool {
        !self.username.is_empty()
    }
}

/// AccessKey/SecretKey auth provider — HMAC-based authentication.
pub struct AccessKeyAuthProvider {
    access_key: String,
    secret_key: String,
}

impl AccessKeyAuthProvider {
    pub fn new(access_key: &str, secret_key: &str) -> Self {
        Self {
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ClientAuthService for AccessKeyAuthProvider {
    fn name(&self) -> &str {
        "accesskey"
    }

    async fn login(&self, _server_list: &[String]) -> bool {
        // AK/SK doesn't need login — signs each request independently
        true
    }

    fn get_identity_context(&self, resource: &RequestResource) -> IdentityContext {
        let headers =
            crate::signing::sign_request(&self.access_key, &self.secret_key, &resource.namespace);
        IdentityContext { headers }
    }

    fn is_enabled(&self) -> bool {
        !self.access_key.is_empty() && !self.secret_key.is_empty()
    }
}

/// SecurityProxy orchestrates multiple auth providers.
///
/// Merges identity headers from all enabled providers.
pub struct SecurityProxy {
    providers: Vec<Arc<dyn ClientAuthService>>,
}

impl SecurityProxy {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add an auth provider
    pub fn add_provider(&mut self, provider: Arc<dyn ClientAuthService>) {
        self.providers.push(provider);
    }

    /// Login with all enabled providers
    pub async fn login(&self, server_list: &[String]) -> bool {
        let mut any_success = false;
        for provider in &self.providers {
            if provider.is_enabled() && provider.login(server_list).await {
                any_success = true;
            }
        }
        any_success
    }

    /// Get merged identity context from all providers
    pub fn get_identity_context(&self, resource: &RequestResource) -> IdentityContext {
        let mut merged = IdentityContext::new();
        for provider in &self.providers {
            if provider.is_enabled() {
                let ctx = provider.get_identity_context(resource);
                merged.merge(&ctx);
            }
        }
        merged
    }

    /// Check if any provider is enabled
    pub fn has_enabled_provider(&self) -> bool {
        self.providers.iter().any(|p| p.is_enabled())
    }

    /// Get number of registered providers
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

impl Default for SecurityProxy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_context() {
        let ctx = IdentityContext::new()
            .with_header("accessToken", "abc123")
            .with_header("ak", "mykey");
        assert_eq!(ctx.headers.len(), 2);
        assert_eq!(ctx.headers.get("accessToken").unwrap(), "abc123");
    }

    #[test]
    fn test_identity_context_merge() {
        let mut ctx1 = IdentityContext::new().with_header("key1", "val1");
        let ctx2 = IdentityContext::new().with_header("key2", "val2");
        ctx1.merge(&ctx2);
        assert_eq!(ctx1.headers.len(), 2);
    }

    #[test]
    fn test_jwt_provider_disabled_when_empty() {
        let provider = JwtAuthProvider::new("", "", "/nacos");
        assert!(!provider.is_enabled());
    }

    #[test]
    fn test_jwt_provider_enabled() {
        let provider = JwtAuthProvider::new("admin", "pass", "/nacos");
        assert!(provider.is_enabled());
        assert_eq!(provider.name(), "jwt");
    }

    #[test]
    fn test_ak_provider_identity() {
        let provider = AccessKeyAuthProvider::new("myak", "mysk");
        assert!(provider.is_enabled());
        assert_eq!(provider.name(), "accesskey");

        let ctx = provider.get_identity_context(&RequestResource {
            namespace: "public".to_string(),
            ..Default::default()
        });
        assert!(ctx.headers.contains_key("ak"));
        assert!(ctx.headers.contains_key("sign"));
    }

    #[test]
    fn test_ak_provider_disabled_when_empty() {
        let provider = AccessKeyAuthProvider::new("", "");
        assert!(!provider.is_enabled());
    }

    #[test]
    fn test_security_proxy_merge() {
        let mut proxy = SecurityProxy::new();

        struct MockProvider;
        #[async_trait::async_trait]
        impl ClientAuthService for MockProvider {
            fn name(&self) -> &str {
                "mock"
            }
            async fn login(&self, _: &[String]) -> bool {
                true
            }
            fn get_identity_context(&self, _: &RequestResource) -> IdentityContext {
                IdentityContext::new().with_header("mock-key", "mock-val")
            }
        }

        proxy.add_provider(Arc::new(MockProvider));
        assert_eq!(proxy.provider_count(), 1);
        assert!(proxy.has_enabled_provider());

        let ctx = proxy.get_identity_context(&RequestResource::default());
        assert_eq!(ctx.headers.get("mock-key").unwrap(), "mock-val");
    }

    #[tokio::test]
    async fn test_security_proxy_login() {
        let mut proxy = SecurityProxy::new();

        struct AlwaysSuccessProvider;
        #[async_trait::async_trait]
        impl ClientAuthService for AlwaysSuccessProvider {
            fn name(&self) -> &str {
                "always-ok"
            }
            async fn login(&self, _: &[String]) -> bool {
                true
            }
            fn get_identity_context(&self, _: &RequestResource) -> IdentityContext {
                IdentityContext::default()
            }
        }

        proxy.add_provider(Arc::new(AlwaysSuccessProvider));
        assert!(proxy.login(&["http://127.0.0.1:8848".to_string()]).await);
    }

    #[test]
    fn test_request_resource_default() {
        let res = RequestResource::default();
        assert!(res.namespace.is_empty());
        assert!(res.group.is_empty());
    }
}
