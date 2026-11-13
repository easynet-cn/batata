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
//!     .context_path("/nacos")
//!     .namespace("dev")
//!     .max_retries(5)
//!     .naming_push_empty_protection(true);
//! ```

use std::collections::HashMap;
use std::time::Duration;

use crate::oidc::OidcConfig;

/// GA `PropertyKeyConst.CONFIG_SNAPSHOT_ENABLED` client property key.
///
/// Nacos 3.3.0 GA reads this once when `SnapShotSwitch` is initialized; the
/// default is `true`. Batata additionally accepts `BATATA_CONFIG_SNAPSHOT_ENABLED`
/// as an environment-variable alias, and `ClientConfig::config_snapshot_enabled`
/// as the explicit per-client override.
pub const CONFIG_SNAPSHOT_ENABLED_PROPERTY: &str = "configSnapshotEnabled";

/// Environment alias for [`CONFIG_SNAPSHOT_ENABLED_PROPERTY`].
pub const BATATA_CONFIG_SNAPSHOT_ENABLED_ENV: &str = "BATATA_CONFIG_SNAPSHOT_ENABLED";

/// Parse a GA-style boolean property (`true`/`false`, `1`/`0`, case-insensitive).
///
/// Unknown values keep `default`, matching `NacosClientProperties.getBoolean`
/// semantics rather than silently disabling a feature on a typo.
pub(crate) fn parse_client_property_bool(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => true,
        "false" | "0" => false,
        _ => default,
    }
}

/// Proxy configuration for HTTP/gRPC connections.
#[derive(Clone, Debug, Default)]
pub enum ProxyConfig {
    /// No proxy — direct connection. Best for local/intranet.
    #[default]
    NoProxy,
    /// Use system proxy settings (reads HTTP_PROXY, HTTPS_PROXY, macOS settings, etc.)
    SystemProxy,
    /// Custom proxy URL (e.g., <http://proxy.corp:8080> or <socks5://proxy:1080>)
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

    /// OIDC Client Credentials configuration (GA `nacos.client.auth.oidc.*`).
    /// `None` keeps the provider disabled; when configured it takes precedence
    /// over username/password login on both the HTTP and gRPC transports.
    pub oidc: Option<OidcConfig>,

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

    // ===== Retry & Resilience =====
    /// Maximum number of retry attempts for failed operations. Default: 3.
    pub max_retries: u32,

    /// Initial delay before first retry. Default: 500ms.
    pub retry_initial_delay: Duration,

    /// Redo delay for replaying registrations/subscriptions after reconnect. Default: 3s.
    pub redo_delay: Duration,

    // ===== Config Service =====
    /// Whether the client persists remote config snapshots locally.
    ///
    /// Mirrors GA `PropertyKeyConst.CONFIG_SNAPSHOT_ENABLED` / `SnapShotSwitch`:
    /// default `true`; when `false` the client neither reads nor writes
    /// snapshots and cleans existing snapshot files at startup.
    pub config_snapshot_enabled: bool,

    /// Config long polling timeout. Default: 30s. Minimum: 10s.
    /// Controls how long the client waits for config change notifications.
    pub config_long_poll_timeout: Duration,

    /// Config request retry interval on failure. Default: 2s.
    pub config_retry_interval: Duration,

    // ===== Naming Service =====
    /// Whether to load cached service info from disk at startup. Default: false.
    pub naming_load_cache_at_start: bool,

    /// Directory for caching service information. Default: empty (uses system temp).
    pub naming_cache_dir: String,

    /// Reject empty service instance pushes (protect against accidental deregistration). Default: false.
    pub naming_push_empty_protection: bool,

    // ===== TLS =====
    /// Enable TLS for gRPC connections. Default: false.
    pub tls_enabled: bool,

    /// Path to CA certificate file (PEM format) for TLS verification.
    /// When TLS is enabled but this is `None`, native root certificates are used.
    pub tls_ca_path: Option<String>,

    /// Path to the client certificate PEM (mTLS).
    pub tls_client_cert_path: Option<String>,

    /// Path to the client private key PEM (mTLS).
    pub tls_client_key_path: Option<String>,

    /// Domain name used for gRPC SNI / certificate verification.
    /// Needed when the server address is an IP literal but the certificate
    /// is issued for a DNS name.
    pub tls_domain: Option<String>,
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
            oidc: None,
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
            max_retries: 3,
            retry_initial_delay: Duration::from_millis(500),
            redo_delay: Duration::from_secs(3),
            config_snapshot_enabled: true,
            config_long_poll_timeout: Duration::from_secs(30),
            config_retry_interval: Duration::from_secs(2),
            naming_load_cache_at_start: false,
            naming_cache_dir: String::new(),
            naming_push_empty_protection: false,
            tls_enabled: false,
            tls_ca_path: None,
            tls_client_cert_path: None,
            tls_client_key_path: None,
            tls_domain: None,
        }
    }

    /// Create with multiple server addresses for failover.
    pub fn with_server_addrs(addrs: Vec<String>) -> Self {
        Self {
            server_addrs: addrs,
            ..Self::default()
        }
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

    /// Enable GA OIDC Client Credentials authentication.
    ///
    /// The provider is only active when [`OidcConfig::is_configured`] holds;
    /// an incomplete config leaves the existing auth paths untouched.
    pub fn with_oidc(mut self, oidc: OidcConfig) -> Self {
        self.oidc = Some(oidc);
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

    /// Set maximum retry attempts. Default: 3.
    pub fn max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Set initial retry delay. Default: 500ms.
    pub fn retry_initial_delay(mut self, delay: Duration) -> Self {
        self.retry_initial_delay = delay;
        self
    }

    /// Set redo delay for reconnection replay. Default: 3s.
    pub fn redo_delay(mut self, delay: Duration) -> Self {
        self.redo_delay = delay;
        self
    }

    /// Enable or disable local config snapshots (GA `configSnapshotEnabled`).
    ///
    /// Default: `true`. Disabling keeps the in-memory cache intact but stops
    /// all snapshot reads/writes and removes previously persisted snapshots
    /// when the config service is created.
    pub fn config_snapshot_enabled(mut self, enabled: bool) -> Self {
        self.config_snapshot_enabled = enabled;
        self
    }

    /// Apply GA-style client properties (a `Properties` map) to this config.
    ///
    /// Recognizes `configSnapshotEnabled` and the five
    /// `nacos.client.auth.oidc.*` keys. Explicit builder values are kept for
    /// keys that are absent from the map.
    pub fn with_client_properties(mut self, properties: &HashMap<String, String>) -> Self {
        if let Some(value) = properties.get(CONFIG_SNAPSHOT_ENABLED_PROPERTY) {
            self.config_snapshot_enabled =
                parse_client_property_bool(value, self.config_snapshot_enabled);
        }
        if let Some(oidc) = OidcConfig::from_properties(properties) {
            self.oidc = Some(oidc);
        }
        self
    }

    /// Set config long polling timeout. Default: 30s. Minimum: 10s.
    pub fn config_long_poll_timeout(mut self, timeout: Duration) -> Self {
        self.config_long_poll_timeout = timeout.max(Duration::from_secs(10));
        self
    }

    /// Set config retry interval on failure. Default: 2s.
    pub fn config_retry_interval(mut self, interval: Duration) -> Self {
        self.config_retry_interval = interval;
        self
    }

    /// Enable loading cached service info from disk at startup.
    pub fn naming_load_cache_at_start(mut self, enable: bool) -> Self {
        self.naming_load_cache_at_start = enable;
        self
    }

    /// Set naming cache directory. Default: system temp.
    pub fn naming_cache_dir(mut self, dir: &str) -> Self {
        self.naming_cache_dir = dir.to_string();
        self
    }

    /// Enable empty push protection for naming service. Default: false.
    pub fn naming_push_empty_protection(mut self, enable: bool) -> Self {
        self.naming_push_empty_protection = enable;
        self
    }

    /// Enable TLS for the gRPC channel with an optional CA certificate path.
    ///
    /// `None` uses the platform's native root certificates. The CA and any
    /// client identity are also applied to HTTP requests, which use the
    /// scheme given in `server_addrs` (`https://...` for TLS).
    pub fn with_tls(mut self, ca_path: Option<&str>) -> Self {
        self.tls_enabled = true;
        self.tls_ca_path = ca_path.map(|s| s.to_string());
        self
    }

    /// Enable mutual TLS for the gRPC channel with a client certificate/key.
    ///
    /// `ca_path` may be `None` to verify the server against native roots.
    pub fn with_mtls(
        mut self,
        ca_path: Option<&str>,
        client_cert_path: &str,
        client_key_path: &str,
    ) -> Self {
        self.tls_enabled = true;
        self.tls_ca_path = ca_path.map(|s| s.to_string());
        self.tls_client_cert_path = Some(client_cert_path.to_string());
        self.tls_client_key_path = Some(client_key_path.to_string());
        self
    }

    /// Override the domain used for gRPC SNI / certificate verification.
    pub fn tls_domain(mut self, domain: &str) -> Self {
        self.tls_domain = Some(domain.to_string());
        self
    }

    /// Normalized TLS settings shared by the gRPC and HTTP transports.
    pub fn tls_options(&self) -> crate::TlsOptions {
        let mut tls = crate::TlsOptions {
            enabled: self.tls_enabled,
            ca_cert_path: self.tls_ca_path.clone(),
            client_cert_path: self.tls_client_cert_path.clone(),
            client_key_path: self.tls_client_key_path.clone(),
            domain: None,
        };
        if let Some(domain) = &self.tls_domain {
            tls = tls.with_domain(domain);
        }
        tls
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
    ///
    /// Uses `https://` when TLS is enabled, matching the channel built by
    /// `GrpcConnection::connect_with_tls`.
    pub fn grpc_endpoint(&self, server_addr: &str) -> String {
        let scheme = self.tls_options().scheme();
        let addr = server_addr
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        if let Some(colon_pos) = addr.rfind(':')
            && let Ok(port) = addr[colon_pos + 1..].parse::<u16>()
        {
            let host = &addr[..colon_pos];
            return format!("{}://{}:{}", scheme, host, port + self.grpc_port_offset);
        }
        format!("{}://{}:{}", scheme, addr, 8848 + self.grpc_port_offset)
    }

    /// Whether JWT auth is configured.
    pub fn has_jwt_auth(&self) -> bool {
        !self.username.is_empty()
    }

    /// Whether AK/SK auth is configured.
    pub fn has_aksk_auth(&self) -> bool {
        !self.access_key.is_empty() && !self.secret_key.is_empty()
    }

    /// Whether a fully configured OIDC provider is present.
    pub fn has_oidc_auth(&self) -> bool {
        self.oidc.as_ref().is_some_and(|oidc| oidc.is_configured())
    }

    /// Build a reqwest Client with the configured proxy, timeout and TLS
    /// settings (custom CA / mTLS identity).
    ///
    /// Returns an error when a configured PEM file cannot be read or parsed,
    /// instead of silently building a client that would fail to verify the
    /// server.
    pub fn build_reqwest_client(&self) -> anyhow::Result<reqwest::Client> {
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

        let builder = self.tls_options().apply_to_reqwest(builder)?;
        Ok(builder.build()?)
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
            tls_enabled: grpc.tls_enabled,
            tls_ca_path: grpc.tls_ca_cert.clone(),
            tls_client_cert_path: grpc.tls_client_cert.clone(),
            tls_client_key_path: grpc.tls_client_key.clone(),
            tls_domain: grpc.tls_domain.clone(),
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
    fn test_config_snapshot_enabled_defaults_to_true() {
        let config = ClientConfig::new("localhost:8848");
        assert!(config.config_snapshot_enabled);
        assert!(
            !config
                .config_snapshot_enabled(false)
                .config_snapshot_enabled
        );
    }

    #[test]
    fn test_config_snapshot_enabled_property() {
        let mut props = HashMap::new();
        props.insert(
            CONFIG_SNAPSHOT_ENABLED_PROPERTY.to_string(),
            "false".to_string(),
        );
        assert!(
            !ClientConfig::new("localhost:8848")
                .with_client_properties(&props)
                .config_snapshot_enabled
        );

        props.insert(
            CONFIG_SNAPSHOT_ENABLED_PROPERTY.to_string(),
            "TRUE".to_string(),
        );
        assert!(
            ClientConfig::new("localhost:8848")
                .with_client_properties(&props)
                .config_snapshot_enabled
        );

        // A typo must not silently disable snapshots (GA getBoolean default).
        props.insert(
            CONFIG_SNAPSHOT_ENABLED_PROPERTY.to_string(),
            "flase".to_string(),
        );
        assert!(
            ClientConfig::new("localhost:8848")
                .with_client_properties(&props)
                .config_snapshot_enabled
        );
        assert!(
            !ClientConfig::new("localhost:8848")
                .config_snapshot_enabled(false)
                .with_client_properties(&props)
                .config_snapshot_enabled
        );
    }

    #[test]
    fn test_oidc_properties_and_builder() {
        let config = ClientConfig::new("localhost:8848");
        assert!(!config.has_oidc_auth());

        let config =
            config.with_oidc(OidcConfig::new("sdk", "secret").with_issuer("https://idp.test"));
        assert!(config.has_oidc_auth());
        assert_eq!(config.oidc.as_ref().unwrap().scope, "openid");

        let mut props = HashMap::new();
        props.insert(
            crate::oidc::OIDC_CLIENT_ID_PROPERTY.to_string(),
            "from-props".to_string(),
        );
        props.insert(
            crate::oidc::OIDC_CLIENT_SECRET_PROPERTY.to_string(),
            "s3cret".to_string(),
        );
        props.insert(
            crate::oidc::OIDC_TOKEN_ENDPOINT_PROPERTY.to_string(),
            "https://idp.test/token".to_string(),
        );
        let config = ClientConfig::new("localhost:8848").with_client_properties(&props);
        assert!(config.has_oidc_auth());
        assert_eq!(config.oidc.as_ref().unwrap().client_id, "from-props");

        // A partial property map must not silently enable the provider.
        let mut partial = HashMap::new();
        partial.insert(
            crate::oidc::OIDC_ISSUER_URI_PROPERTY.to_string(),
            "https://idp.test".to_string(),
        );
        assert!(
            !ClientConfig::new("localhost:8848")
                .with_client_properties(&partial)
                .has_oidc_auth()
        );
    }

    #[test]
    fn test_parse_client_property_bool() {
        assert!(parse_client_property_bool("1", false));
        assert!(!parse_client_property_bool("0", true));
        assert!(parse_client_property_bool(" true ", false));
        assert!(parse_client_property_bool("nonsense", true));
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
    fn test_tls_builders_and_options() {
        let config = ClientConfig::new("localhost:8848")
            .with_mtls(Some("ca.pem"), "client.pem", "client.key")
            .tls_domain("batata.example.com");

        assert_eq!(
            config.tls_options(),
            crate::TlsOptions::mtls(Some("ca.pem"), "client.pem", "client.key")
                .with_domain("batata.example.com")
        );
        assert_eq!(
            config.grpc_endpoint("10.0.0.1:8848"),
            "https://10.0.0.1:9848"
        );

        let native = ClientConfig::new("localhost:8848").with_tls(None);
        assert_eq!(native.tls_options(), crate::TlsOptions::enabled(None));
        assert_eq!(
            ClientConfig::new("localhost:8848").tls_options(),
            crate::TlsOptions::default()
        );
    }

    #[test]
    fn test_build_reqwest_rejects_bad_tls_ca() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.pem");
        std::fs::write(&bad, "not a pem").unwrap();

        let config = ClientConfig::new("localhost:8848").with_tls(Some(bad.to_str().unwrap()));
        let err = config.build_reqwest_client().unwrap_err();
        assert!(err.to_string().contains("certificate") || err.to_string().contains("CA"));
    }

    #[test]
    fn test_from_grpc_config_preserves_tls() {
        let grpc = crate::grpc::GrpcClientConfig::default()
            .with_mtls("ca.pem", "client.pem", "client.key")
            .with_tls_domain("batata.example.com");
        let config = ClientConfig::from(&grpc);
        assert_eq!(
            config.tls_options(),
            crate::TlsOptions::mtls(Some("ca.pem"), "client.pem", "client.key")
                .with_domain("batata.example.com")
        );
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
