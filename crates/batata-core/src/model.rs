// Core data models and structures
// This file defines fundamental data structures used throughout the application

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use tokio::sync::mpsc::Sender;
use tonic::Status;

use batata_api::{
    grpc::Payload,
    model::{APP_CONN_PREFIX, APPNAME, CLIENT_VERSION_KEY},
};

// Label source constants (originally from remote::model)
pub const LABEL_SOURCE: &str = "source";
pub const LABEL_SOURCE_SDK: &str = "sdk";
pub const LABEL_SOURCE_CLUSTER: &str = "cluster";

// Pagination parameters for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageParam {
    #[serde(default = "PageParam::default_page_no")]
    pub page_no: u64,
    #[serde(default = "PageParam::default_page_size")]
    pub page_size: u64,
}

impl PageParam {
    pub fn start(&self) -> u64 {
        (self.page_no - 1) * self.page_size
    }

    fn default_page_no() -> u64 {
        1
    }

    fn default_page_size() -> u64 {
        100
    }
}

/// Connection meta infomation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionMeta {
    /// ConnectionType.
    pub connect_type: String,

    /// Client IP Address.
    pub client_ip: String,

    /// Remote IP Address.
    pub remote_ip: String,

    /// Remote IP Port.
    pub remote_port: u16,

    /// Local Ip Port.
    pub local_port: u16,

    /// Client version.
    pub version: String,

    /// Identify Unique connectionId.
    pub connection_id: String,

    /// Create time.
    pub create_time: i64,

    /// Last active time.
    pub last_active_time: i64,

    /// App name.
    pub app_name: String,

    /// Namespace id.
    pub namespace_id: String,

    /// Labels.
    pub labels: HashMap<String, String>,

    /// Tls protected.
    pub tls_protected: bool,

    first_push_queue_block_time: i64,
    last_push_queue_block_time: i64,
}

impl ConnectionMeta {
    pub fn is_sdk_source(&self) -> bool {
        self.labels
            .get(LABEL_SOURCE)
            .is_some_and(|e| e.to_lowercase() == LABEL_SOURCE_SDK.to_lowercase())
    }

    pub fn is_cluster_source(&self) -> bool {
        self.labels
            .get(LABEL_SOURCE)
            .is_some_and(|e| e.to_lowercase() == LABEL_SOURCE_CLUSTER.to_lowercase())
    }

    pub fn get_app_labels(&self) -> HashMap<String, String> {
        let mut map = HashMap::<String, String>::new();

        map.insert(
            APPNAME.to_string(),
            self.labels
                .get(APPNAME)
                .map_or(String::default(), |e| e.to_string()),
        );
        map.insert(CLIENT_VERSION_KEY.to_string(), self.version.clone());

        for (k, v) in self.labels.iter() {
            if k.starts_with(APP_CONN_PREFIX) && k.len() > APP_CONN_PREFIX.len() && !v.is_empty() {
                map.insert(k[APP_CONN_PREFIX.len()..].to_string(), v.to_string());
            }
        }

        map
    }

    pub fn record_push_queue_block_times(&mut self) {
        if self.first_push_queue_block_time == 0 {
            self.first_push_queue_block_time = chrono::Utc::now().timestamp_millis();
        } else {
            self.last_push_queue_block_time = chrono::Utc::now().timestamp_millis();
        }
    }

    pub fn push_queue_block_times_last_over(&self, time_mills_seconds: i64) -> bool {
        self.last_push_queue_block_time - self.first_push_queue_block_time > time_mills_seconds
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub traced: bool,

    pub ability_table: HashMap<String, bool>,

    #[serde(flatten)]
    pub meta_info: ConnectionMeta,
}

impl Connection {}

#[derive(Clone)]
pub struct GrpcClient {
    pub connection: Connection,
    pub tx: Sender<Result<Payload, Status>>,
}

impl GrpcClient {
    pub fn new(connection: Connection, tx: Sender<Result<Payload, Status>>) -> Self {
        Self { connection, tx }
    }
}

/// Application configuration wrapper
/// Provides access to configuration values for cluster and connection management
#[derive(Clone, Debug)]
pub struct Configuration {
    pub config: config::Config,
}

impl Configuration {
    /// Create a new configuration from a Config instance
    pub fn from_config(config: config::Config) -> Self {
        Self { config }
    }

    /// Get the main server port
    pub fn server_main_port(&self) -> u16 {
        self.config
            .get_int("batata.server.main.port")
            .unwrap_or(8848) as u16
    }

    /// Check if running in standalone mode
    pub fn is_standalone(&self) -> bool {
        self.config.get_bool("batata.standalone").unwrap_or(true)
    }

    /// Get the server version
    pub fn version(&self) -> String {
        self.config
            .get_string("batata.version")
            .unwrap_or_else(|_| "1.0.0".to_string())
    }

    /// Check if running in console remote mode
    pub fn is_console_remote_mode(&self) -> bool {
        self.config
            .get_bool("batata.console.remote.mode")
            .unwrap_or(false)
    }

    /// Get console remote server addresses
    pub fn console_remote_servers(&self) -> Vec<String> {
        self.config
            .get_string("batata.console.remote.servers")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default()
    }

    // ===================== Cluster Client Configuration =====================

    /// Get cluster client connect timeout in milliseconds (default: 5000ms)
    pub fn cluster_connect_timeout_ms(&self) -> u64 {
        self.config
            .get_int("batata.remote.server.grpc.cluster.connect-timeout")
            .unwrap_or(5000) as u64
    }

    /// Get cluster client request timeout in milliseconds (default: 5000ms)
    pub fn cluster_request_timeout_ms(&self) -> u64 {
        self.config
            .get_int("batata.remote.server.grpc.cluster.request-timeout")
            .unwrap_or(5000) as u64
    }

    /// Get cluster client max retry count (default: 3)
    pub fn cluster_max_retries(&self) -> u32 {
        self.config
            .get_int("batata.remote.server.grpc.cluster.max-retries")
            .unwrap_or(3) as u32
    }

    /// Get cluster client retry delay in milliseconds (default: 500ms)
    pub fn cluster_retry_delay_ms(&self) -> u64 {
        self.config
            .get_int("batata.remote.server.grpc.cluster.retry-delay")
            .unwrap_or(500) as u64
    }

    /// Get cluster client idle connection timeout in milliseconds (default: 300000ms = 5 minutes)
    pub fn cluster_idle_timeout_ms(&self) -> u64 {
        self.config
            .get_int("batata.remote.server.grpc.cluster.idle-timeout")
            .unwrap_or(300000) as u64
    }

    // ===================== SDK gRPC Configuration =====================

    /// Get SDK gRPC max inbound message size in bytes (default: 10MB)
    pub fn sdk_grpc_max_inbound_message_size(&self) -> usize {
        self.config
            .get_int("batata.remote.server.grpc.sdk.max-inbound-message-size")
            .unwrap_or(10485760) as usize
    }

    /// Get SDK gRPC keep-alive time in milliseconds (default: 7200000ms = 2 hours)
    pub fn sdk_grpc_keep_alive_time_ms(&self) -> u64 {
        self.config
            .get_int("batata.remote.server.grpc.sdk.keep-alive-time")
            .unwrap_or(7200000) as u64
    }

    /// Get SDK gRPC keep-alive timeout in milliseconds (default: 20000ms = 20 seconds)
    pub fn sdk_grpc_keep_alive_timeout_ms(&self) -> u64 {
        self.config
            .get_int("batata.remote.server.grpc.sdk.keep-alive-timeout")
            .unwrap_or(20000) as u64
    }

    // ===================== Cluster Client TLS Configuration =====================

    /// Check if TLS is enabled for cluster client connections
    pub fn cluster_client_tls_enabled(&self) -> bool {
        self.config
            .get_bool("batata.remote.client.grpc.cluster.tls.enabled")
            .unwrap_or(false)
    }

    /// Get cluster client TLS certificate path (for mTLS)
    pub fn cluster_client_tls_cert_path(&self) -> Option<String> {
        self.config
            .get_string("batata.remote.client.grpc.cluster.tls.cert.path")
            .ok()
    }

    /// Get cluster client TLS private key path (for mTLS)
    pub fn cluster_client_tls_key_path(&self) -> Option<String> {
        self.config
            .get_string("batata.remote.client.grpc.cluster.tls.key.path")
            .ok()
    }

    /// Get cluster client TLS CA certificate path for server verification
    pub fn cluster_client_tls_ca_cert_path(&self) -> Option<String> {
        self.config
            .get_string("batata.remote.client.grpc.cluster.tls.ca.cert.path")
            .ok()
    }

    /// Get cluster client TLS domain name for SNI verification
    pub fn cluster_client_tls_domain(&self) -> Option<String> {
        self.config
            .get_string("batata.remote.client.grpc.cluster.tls.domain")
            .ok()
    }
}
