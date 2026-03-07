use std::time::Duration;

/// Configuration for the Consul client
#[derive(Clone, Debug)]
pub struct ConsulClientConfig {
    /// Consul HTTP address (e.g., "http://127.0.0.1:8500")
    pub address: String,
    /// ACL token for authentication (sent as X-Consul-Token header)
    pub token: String,
    /// Datacenter to use (empty = agent's default datacenter)
    pub datacenter: String,
    /// Default wait time for blocking queries
    pub wait_time: Option<Duration>,
    /// HTTP connection timeout
    pub connect_timeout: Duration,
    /// HTTP read timeout
    pub read_timeout: Duration,
    /// Namespace (Consul Enterprise)
    pub namespace: String,
    /// Partition (Consul Enterprise)
    pub partition: String,
}

impl Default for ConsulClientConfig {
    fn default() -> Self {
        Self {
            address: "http://127.0.0.1:8500".to_string(),
            token: String::new(),
            datacenter: String::new(),
            wait_time: None,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            namespace: String::new(),
            partition: String::new(),
        }
    }
}

impl ConsulClientConfig {
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            ..Default::default()
        }
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = token.to_string();
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
}
