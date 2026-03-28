use std::path::PathBuf;
use std::time::Duration;

/// Configuration for the Consul client
#[derive(Clone, Debug)]
pub struct ConsulClientConfig {
    /// Consul HTTP address(es). First is primary; others are failover.
    pub addresses: Vec<String>,
    /// ACL token for authentication (sent as X-Consul-Token header)
    pub token: String,
    /// Path to a file containing the ACL token (re-read on each request if set)
    pub token_file: Option<PathBuf>,
    /// Datacenter to use (empty = agent's default datacenter)
    pub datacenter: String,
    /// Default wait time for blocking queries
    pub wait_time: Option<Duration>,
    /// HTTP connection timeout. Default: 5s.
    pub connect_timeout: Duration,
    /// HTTP read timeout. Default: 30s.
    pub read_timeout: Duration,
    /// Namespace (Consul Enterprise)
    pub namespace: String,
    /// Partition (Consul Enterprise)
    pub partition: String,

    // --- TLS Configuration ---
    /// CA certificate file path (PEM). Enables TLS verification.
    pub tls_ca_cert: Option<String>,
    /// Client certificate file path (PEM). For mTLS.
    pub tls_client_cert: Option<String>,
    /// Client key file path (PEM). For mTLS.
    pub tls_client_key: Option<String>,
    /// Skip TLS certificate verification (insecure, for development only)
    pub tls_insecure_skip_verify: bool,

    // --- Retry Configuration ---
    /// Maximum number of retries for transient errors. Default: 2.
    pub max_retries: u32,
    /// Base delay for exponential backoff in ms. Default: 100.
    pub retry_backoff_base_ms: u64,
    /// Maximum delay for exponential backoff in ms. Default: 5000.
    pub retry_backoff_max_ms: u64,

    // --- Backward compatibility ---
    /// Single address (deprecated: use `addresses` instead).
    /// If set, takes precedence over `addresses`.
    pub address: String,
}

impl Default for ConsulClientConfig {
    fn default() -> Self {
        Self {
            addresses: vec!["http://127.0.0.1:8500".to_string()],
            address: "http://127.0.0.1:8500".to_string(),
            token: String::new(),
            token_file: None,
            datacenter: String::new(),
            wait_time: None,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            namespace: String::new(),
            partition: String::new(),
            tls_ca_cert: None,
            tls_client_cert: None,
            tls_client_key: None,
            tls_insecure_skip_verify: false,
            max_retries: 2,
            retry_backoff_base_ms: 100,
            retry_backoff_max_ms: 5000,
        }
    }
}

impl ConsulClientConfig {
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            addresses: vec![address.to_string()],
            ..Default::default()
        }
    }

    /// Create config with multiple addresses for failover
    pub fn with_addresses(mut self, addresses: Vec<String>) -> Self {
        self.addresses = addresses;
        if let Some(first) = self.addresses.first() {
            self.address = first.clone();
        }
        self
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = token.to_string();
        self
    }

    pub fn with_token_file(mut self, path: PathBuf) -> Self {
        self.token_file = Some(path);
        self
    }

    pub fn with_datacenter(mut self, dc: &str) -> Self {
        self.datacenter = dc.to_string();
        self
    }

    pub fn with_wait_time(mut self, wait: Duration) -> Self {
        self.wait_time = Some(wait);
        self
    }

    pub fn with_timeouts(mut self, connect: Duration, read: Duration) -> Self {
        self.connect_timeout = connect;
        self.read_timeout = read;
        self
    }

    pub fn with_namespace(mut self, ns: &str) -> Self {
        self.namespace = ns.to_string();
        self
    }

    pub fn with_partition(mut self, partition: &str) -> Self {
        self.partition = partition.to_string();
        self
    }

    pub fn with_tls(mut self, ca_cert: &str) -> Self {
        self.tls_ca_cert = Some(ca_cert.to_string());
        self
    }

    pub fn with_mtls(mut self, ca_cert: &str, client_cert: &str, client_key: &str) -> Self {
        self.tls_ca_cert = Some(ca_cert.to_string());
        self.tls_client_cert = Some(client_cert.to_string());
        self.tls_client_key = Some(client_key.to_string());
        self
    }

    pub fn with_retry(mut self, max_retries: u32, backoff_base_ms: u64) -> Self {
        self.max_retries = max_retries;
        self.retry_backoff_base_ms = backoff_base_ms;
        self
    }

    /// Get the effective primary address
    pub fn effective_address(&self) -> &str {
        if !self.address.is_empty() {
            &self.address
        } else {
            self.addresses
                .first()
                .map(|s| s.as_str())
                .unwrap_or("http://127.0.0.1:8500")
        }
    }

    /// Get the effective token (from token_file if set, otherwise from token field)
    pub fn effective_token(&self) -> String {
        if let Some(path) = &self.token_file {
            std::fs::read_to_string(path)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| self.token.clone())
        } else {
            self.token.clone()
        }
    }
}
