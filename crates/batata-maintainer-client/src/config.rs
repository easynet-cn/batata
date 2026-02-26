// Configuration for MaintainerClient

/// Configuration for the maintainer HTTP client
#[derive(Clone, Debug)]
pub struct MaintainerClientConfig {
    /// Server addresses (e.g. ["http://127.0.0.1:8848"])
    pub server_addrs: Vec<String>,
    /// Username for authentication (JWT fallback)
    pub username: String,
    /// Password for authentication (JWT fallback)
    pub password: String,
    /// Connection timeout in milliseconds (default: 5000)
    pub connect_timeout_ms: u64,
    /// Read timeout in milliseconds (default: 30000)
    pub read_timeout_ms: u64,
    /// Context path (default: "nacos")
    pub context_path: String,
    /// Server identity header key (primary auth, bypasses JWT login)
    pub server_identity_key: String,
    /// Server identity header value
    pub server_identity_value: String,
}

impl Default for MaintainerClientConfig {
    fn default() -> Self {
        Self {
            server_addrs: vec!["http://127.0.0.1:8848".to_string()],
            username: String::new(),
            password: String::new(),
            connect_timeout_ms: 5000,
            read_timeout_ms: 30000,
            context_path: "nacos".to_string(),
            server_identity_key: String::new(),
            server_identity_value: String::new(),
        }
    }
}
