//! Unified client configuration.
//!
//! `ClientConfig` is the single entry point for configuring all aspects of
//! the Batata client SDK: server addresses, authentication, proxy, timeouts,
//! TLS, server list management, and gRPC-specific settings.
//!
//! # Example
//! ```no_run
//! use batata_client::ClientConfig;
//!
//! let config = ClientConfig::new("127.0.0.1:8848")
//!     .with_auth("nacos", "nacos")
//!     .with_context_path("/nacos");
//! ```

use std::collections::HashMap;
use std::time::Duration;

/// Proxy configuration for HTTP/gRPC connections.
#[derive(Clone, Debug, Default)]
pub enum ProxyConfig {
    /// No proxy — direct connection. Best for local/intranet.
    #[default]
    NoProxy,
    /// Use system proxy settings (reads HTTP_PROXY, HTTPS_PROXY, macOS settings, etc.)
    SystemProxy,
    /// Custom proxy URL (e.g., "http://proxy.corp:8080" or "socks5://proxy:1080")
    Custom(String),
}

/// Unified client configuration for Batata SDK.
///
/// Configures server addresses, authentication, network settings, and
/// behavior for all client components (gRPC, HTTP, server list management).
#[derive(Clone, Debug)]
pub struct ClientConfig {
    // ===== Server Addresses =====
    /// Server addresses (ip:port or host:port). Supports multiple for failover.
    /// Example: `["192.168.1.10:8848", "192.168.1.11:8848"]`
    pub server_addrs: Vec<String>,

    /// Context path for HTTP API requests. Default: `/nacos`.
    /// This is prepended to all HTTP API paths.
    pub context_path: String,

    // ===== Authentication =====
    /// Auth endpoint path (relative to context_path or absolute).
    /// Default: `/v3/auth/user/login` (V3 API).
    /// Use `/v1/auth/login` for legacy Go SDK compatibility.
    pub auth_endpoint: String,

    /// Username for JWT authentication. Empty = disabled.
    pub username: String,

    /// Password for JWT authentication.
    pub password: String,

    /// AccessKey for HMAC-based authentication. Empty = disabled.
    pub access_key: String,

    /// SecretKey for HMAC-based authentication.
    pub secret_key: String,

    // ===== Network =====
    /// Proxy configuration. Default: `NoProxy`.
    pub proxy: ProxyConfig,

    /// Connection timeout. Default: 5 seconds.
    pub connect_timeout: Duration,

    /// Request/read timeout. Default: 30 seconds.
    pub request_timeout: Duration,

    // ===== gRPC Specific =====
    /// gRPC port offset from the main HTTP port. Default: 1000.
    /// The gRPC server listens on `main_port + grpc_port_offset`.
    pub grpc_port_offset: u16,

    /// Module identifier sent to the server during connection setup.
    /// Typically "config" or "naming". Default: empty.
    pub module: String,

    /// Namespace/tenant ID. Default: empty (public namespace).
    pub namespace: String,

    /// Client labels attached to gRPC connections.
    pub labels: HashMap<String, String>,

    // ===== Server List Management =====
    /// Address server URL for dynamic server list refresh.
    /// When set, the server list is periodically fetched from this URL.
    pub address_server_url: Option<String>,

    /// Server list refresh interval. Default: 30 seconds. Set to 0 to disable.
    pub server_list_refresh_interval: Duration,

    /// Max consecutive failures before marking a server unhealthy. Default: 3.
    pub server_max_failures: u32,

    // ===== TLS =====
    /// Enable TLS for gRPC connections. Default: false.
    pub tls_enabled: bool,

    /// Path to CA certificate file (PEM format) for TLS verification.
    pub tls_ca_path: Option<String>,
}

impl ClientConfig {
    /// Create a new config with a single server address.
    ///
    /// Uses sensible defaults: `/nacos` context path, V3 auth endpoint,
    /// no proxy, 5s connect timeout, 30s request timeout.
    pub fn new(server_addr: &str) -> Self {
        Self {
            server_addrs: vec![server_addr.to_string()],
            context_path: "/nacos".to_string(),
            auth_endpoint: "/v3/auth/user/login".to_string(),
            username: String::new(),
            password: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            proxy: ProxyConfig::default(),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            grpc_port_offset: 1000,
            module: String::new(),
            namespace: String::new(),
            labels: HashMap::new(),
            address_server_url: None,
            server_list_refresh_interval: Duration::from_secs(30),
            server_max_failures: 3,
            tls_enabled: false,
            tls_ca_path: None,
        }
    }

    /// Create with multiple server addresses for failover.
    pub fn with_server_addrs(addrs: Vec<String>) -> Self {
        let mut config = Self::new(
            addrs
                .first()
                .map(|s| s.as_str())
                .unwrap_or("127.0.0.1:8848"),
        );
        config.server_addrs = addrs;
        config
    }

    // ===== Builder methods =====

    /// Set the context path (e.g., "/nacos"). Default: "/nacos".
    pub fn context_path(mut self, path: &str) -> Self {
        self.context_path = path.to_string();
        self
    }

    /// Set the authentication endpoint path. Default: "/v3/auth/user/login".
    pub fn auth_endpoint(mut self, endpoint: &str) -> Self {
        self.auth_endpoint = endpoint.to_string();
        self
    }

    /// Set JWT username and password.
    pub fn with_auth(mut self, username: &str, password: &str) -> Self {
        self.username = username.to_string();
        self.password = password.to_string();
        self
    }

    /// Set AccessKey/SecretKey for HMAC authentication.
    pub fn with_access_key(mut self, access_key: &str, secret_key: &str) -> Self {
        self.access_key = access_key.to_string();
        self.secret_key = secret_key.to_string();
        self
    }

    /// Set proxy configuration.
    pub fn proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = proxy;
        self
    }

    /// Set connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set request/read timeout.
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set gRPC port offset. Default: 1000.
    pub fn grpc_port_offset(mut self, offset: u16) -> Self {
        self.grpc_port_offset = offset;
        self
    }

    /// Set the module name (e.g., "config", "naming").
    pub fn module(mut self, module: &str) -> Self {
        self.module = module.to_string();
        self
    }

    /// Set namespace/tenant ID.
    pub fn namespace(mut self, ns: &str) -> Self {
        self.namespace = ns.to_string();
        self
    }

    /// Add a client label.
    pub fn label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the address server URL for dynamic server list refresh.
    pub fn address_server(mut self, url: &str) -> Self {
        self.address_server_url = Some(url.to_string());
        self
    }

    /// Set server list refresh interval. Default: 30s. Set to Duration::ZERO to disable.
    pub fn server_list_refresh_interval(mut self, interval: Duration) -> Self {
        self.server_list_refresh_interval = interval;
        self
    }

    /// Set max failures before marking server unhealthy. Default: 3.
    pub fn server_max_failures(mut self, max: u32) -> Self {
        self.server_max_failures = max;
        self
    }

    /// Enable TLS with optional CA certificate path.
    pub fn with_tls(mut self, ca_path: Option<&str>) -> Self {
        self.tls_enabled = true;
        self.tls_ca_path = ca_path.map(|s| s.to_string());
        self
    }

    // ===== Derived helpers =====

    /// Build the full auth endpoint URL for a given server address.
    pub fn full_auth_url(&self, server_addr: &str) -> String {
        let base = if server_addr.starts_with("http") {
            server_addr.to_string()
        } else {
            format!("http://{}", server_addr)
        };
        let base = base.trim_end_matches('/');

        if self.auth_endpoint.starts_with('/') {
            // Absolute path — might already include context_path
            if self.auth_endpoint.starts_with(&self.context_path) {
                format!("{}{}", base, self.auth_endpoint)
            } else {
                format!("{}{}{}", base, self.context_path, self.auth_endpoint)
            }
        } else {
            format!("{}{}/{}", base, self.context_path, self.auth_endpoint)
        }
    }

    /// Build the full HTTP URL for an API path on a given server.
    pub fn full_http_url(&self, server_addr: &str, path: &str) -> String {
        let base = if server_addr.starts_with("http") {
            server_addr.to_string()
        } else {
            format!("http://{}", server_addr)
        };
        let base = base.trim_end_matches('/');
        format!("{}{}{}", base, self.context_path, path)
    }

    /// Build the gRPC endpoint address for a given server address.
    pub fn grpc_endpoint(&self, server_addr: &str) -> String {
        let addr = server_addr
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        if let Some(colon_pos) = addr.rfind(':')
            && let Ok(port) = addr[colon_pos + 1..].parse::<u16>()
        {
            let host = &addr[..colon_pos];
            return format!("http://{}:{}", host, port + self.grpc_port_offset);
        }
        format!("http://{}:{}", addr, 8848 + self.grpc_port_offset)
    }

    /// Whether JWT auth is configured.
    pub fn has_jwt_auth(&self) -> bool {
        !self.username.is_empty()
    }

    /// Whether AK/SK auth is configured.
    pub fn has_aksk_auth(&self) -> bool {
        !self.access_key.is_empty() && !self.secret_key.is_empty()
    }

    /// Build a reqwest Client with the configured proxy and timeout settings.
    pub fn build_reqwest_client(&self) -> reqwest::Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder()
            .connect_timeout(self.connect_timeout)
            .timeout(self.request_timeout);

        match &self.proxy {
            ProxyConfig::NoProxy => {
                builder = builder.no_proxy();
            }
            ProxyConfig::SystemProxy => {
                // Default reqwest behavior — reads system proxy
            }
            ProxyConfig::Custom(url) => {
                if let Ok(proxy) = reqwest::Proxy::all(url) {
                    builder = builder.proxy(proxy);
                }
            }
        }

        builder.build()
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self::new("127.0.0.1:8848")
    }
}

// ===== Backward-compatible conversions =====

impl From<&crate::grpc::GrpcClientConfig> for ClientConfig {
    fn from(grpc: &crate::grpc::GrpcClientConfig) -> Self {
        Self {
            server_addrs: grpc.server_addrs.clone(),
            context_path: grpc.context_path.clone(),
            username: grpc.username.clone(),
            password: grpc.password.clone(),
            module: grpc.module.clone(),
            namespace: grpc.tenant.clone(),
            labels: grpc.labels.clone(),
            ..Self::default()
        }
    }
}

impl From<&crate::http::HttpClientConfig> for ClientConfig {
    fn from(http: &crate::http::HttpClientConfig) -> Self {
        Self {
            server_addrs: http.server_addrs.clone(),
            context_path: http.context_path.clone(),
            auth_endpoint: http.auth_endpoint.clone(),
            username: http.username.clone(),
            password: http.password.clone(),
            connect_timeout: Duration::from_millis(http.connect_timeout_ms),
            request_timeout: Duration::from_millis(http.read_timeout_ms),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClientConfig::new("10.0.0.1:8848");
        assert_eq!(config.server_addrs, vec!["10.0.0.1:8848"]);
        assert_eq!(config.context_path, "/nacos");
        assert_eq!(config.auth_endpoint, "/v3/auth/user/login");
        assert_eq!(config.grpc_port_offset, 1000);
        assert!(matches!(config.proxy, ProxyConfig::NoProxy));
    }

    #[test]
    fn test_builder() {
        let config = ClientConfig::new("10.0.0.1:8848")
            .with_auth("admin", "pass")
            .context_path("/custom")
            .proxy(ProxyConfig::SystemProxy)
            .module("config")
            .namespace("dev")
            .label("env", "test");

        assert_eq!(config.username, "admin");
        assert_eq!(config.context_path, "/custom");
        assert_eq!(config.module, "config");
        assert_eq!(config.namespace, "dev");
        assert_eq!(config.labels.get("env").unwrap(), "test");
        assert!(matches!(config.proxy, ProxyConfig::SystemProxy));
    }

    #[test]
    fn test_multi_server() {
        let config =
            ClientConfig::with_server_addrs(vec!["10.0.0.1:8848".into(), "10.0.0.2:8848".into()]);
        assert_eq!(config.server_addrs.len(), 2);
    }

    #[test]
    fn test_full_auth_url() {
        let config = ClientConfig::new("10.0.0.1:8848");
        assert_eq!(
            config.full_auth_url("10.0.0.1:8848"),
            "http://10.0.0.1:8848/nacos/v3/auth/user/login"
        );

        let config2 = ClientConfig::new("10.0.0.1:8848").context_path("/custom");
        assert_eq!(
            config2.full_auth_url("http://10.0.0.1:8848"),
            "http://10.0.0.1:8848/custom/v3/auth/user/login"
        );
    }

    #[test]
    fn test_grpc_endpoint() {
        let config = ClientConfig::new("10.0.0.1:8848");
        assert_eq!(
            config.grpc_endpoint("10.0.0.1:8848"),
            "http://10.0.0.1:9848"
        );

        let config2 = ClientConfig::new("10.0.0.1:8848").grpc_port_offset(2000);
        assert_eq!(
            config2.grpc_endpoint("10.0.0.1:8848"),
            "http://10.0.0.1:10848"
        );
    }

    #[test]
    fn test_full_http_url() {
        let config = ClientConfig::new("10.0.0.1:8848");
        assert_eq!(
            config.full_http_url("10.0.0.1:8848", "/v3/admin/core/state"),
            "http://10.0.0.1:8848/nacos/v3/admin/core/state"
        );
    }

    #[test]
    fn test_has_auth() {
        let config = ClientConfig::new("localhost:8848").with_auth("u", "p");
        assert!(config.has_jwt_auth());
        assert!(!config.has_aksk_auth());

        let config2 = ClientConfig::new("localhost:8848").with_access_key("ak", "sk");
        assert!(!config2.has_jwt_auth());
        assert!(config2.has_aksk_auth());
    }

    #[test]
    fn test_build_reqwest_no_proxy() {
        let config = ClientConfig::new("localhost:8848");
        let client = config.build_reqwest_client();
        assert!(client.is_ok());
    }

    #[test]
    fn test_from_grpc_config() {
        let grpc = crate::grpc::GrpcClientConfig {
            server_addrs: vec!["10.0.0.1:8848".into()],
            username: "user".into(),
            password: "pass".into(),
            module: "config".into(),
            ..Default::default()
        };
        let config = ClientConfig::from(&grpc);
        assert_eq!(config.username, "user");
        assert_eq!(config.module, "config");
    }
}
