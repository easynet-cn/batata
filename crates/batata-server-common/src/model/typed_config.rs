use serde::{Deserialize, Deserializer};

// ============================================================================
// Module-level default functions
// ============================================================================

/// Deserialize a nullable string: YAML null → None, string → Some(s).
/// This allows `batata.console.context_path:` (with no value) to deserialize
/// as None instead of causing a type error.
fn deserialize_null_to_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    Ok(opt)
}

// --- Common bool default ---
fn default_true() -> bool {
    true
}

// --- i64 defaults ---
fn default_server_port() -> i64 {
    8849
}
fn default_http_keep_alive() -> i64 {
    75
}
fn default_http_max_payload_size() -> i64 {
    10_485_760
}
fn default_http_max_json_size() -> i64 {
    5_242_880
}
fn default_http_client_request_timeout() -> i64 {
    60
}
fn default_access_log_max_days() -> i64 {
    30
}
fn default_compression_min_size() -> i64 {
    256
}
fn default_shutdown_drain_timeout() -> i64 {
    30
}
fn default_shutdown_db_close_timeout() -> i64 {
    10
}
fn default_grpc_tcp_keepalive() -> i64 {
    30
}
fn default_grpc_http2_keepalive_interval() -> i64 {
    30
}
fn default_grpc_http2_keepalive_timeout() -> i64 {
    10
}
fn default_grpc_concurrency_limit() -> i64 {
    256
}
fn default_grpc_connection_stale_ms() -> i64 {
    60_000
}
fn default_grpc_push_message_timeout() -> i64 {
    5000
}
fn default_grpc_bistream_channel_capacity() -> i64 {
    128
}
fn default_grpc_max_push_timeouts() -> i64 {
    5
}
fn default_grpc_max_concurrent_streams() -> i64 {
    200
}
fn default_grpc_max_connections() -> i64 {
    10_000
}
fn default_grpc_initial_connection_window_size() -> i64 {
    1_048_576
}
fn default_grpc_initial_stream_window_size() -> i64 {
    524_288
}
fn default_grpc_max_frame_size() -> i64 {
    16_384
}
fn default_console_port() -> i64 {
    8081
}
fn default_console_remote_refresh_interval_secs() -> i64 {
    30
}
fn default_console_remote_initial_delay_secs() -> i64 {
    5
}
fn default_console_remote_connect_timeout_ms() -> i64 {
    5000
}
fn default_console_remote_read_timeout_ms() -> i64 {
    30_000
}
fn default_console_http_keep_alive() -> i64 {
    30
}
fn default_db_pool_max_connections() -> i64 {
    200
}
fn default_db_pool_min_connections() -> i64 {
    5
}
fn default_db_pool_connect_timeout() -> i64 {
    10
}
fn default_db_pool_acquire_timeout() -> i64 {
    10
}
fn default_db_pool_idle_timeout() -> i64 {
    300
}
fn default_db_pool_max_lifetime() -> i64 {
    1800
}
fn default_token_expire_seconds() -> i64 {
    18_000
}
fn default_ldap_timeout() -> i64 {
    5000
}
fn default_oauth_discovery_ttl_secs() -> i64 {
    3600
}
fn default_oauth_discovery_capacity() -> i64 {
    100
}
fn default_oauth_state_ttl_secs() -> i64 {
    600
}
fn default_oauth_state_capacity() -> i64 {
    10_000
}
fn default_oauth_http_timeout_secs() -> i64 {
    30
}
fn default_auth_token_capacity() -> i64 {
    50_000
}
fn default_auth_token_ttl_secs() -> i64 {
    60
}
fn default_auth_roles_capacity() -> i64 {
    50_000
}
fn default_auth_permissions_capacity() -> i64 {
    20_000
}
fn default_auth_blacklist_capacity() -> i64 {
    100_000
}
fn default_auth_blacklist_ttl_secs() -> i64 {
    86_400
}
fn default_grpc_permission_capacity() -> i64 {
    10_000
}
fn default_grpc_permission_ttl_secs() -> i64 {
    60
}
fn default_address_server_retry() -> i64 {
    5
}
fn default_address_server_port() -> i64 {
    8080
}
fn default_ratelimit_max_requests() -> i64 {
    100
}
fn default_ratelimit_window_seconds() -> i64 {
    60
}
fn default_ratelimit_auth_max_attempts() -> i64 {
    5
}
fn default_ratelimit_auth_window_seconds() -> i64 {
    60
}
fn default_ratelimit_auth_lockout_seconds() -> i64 {
    300
}
fn default_ratelimit_max_tracked_ips() -> i64 {
    100_000
}
fn default_ratelimit_cleanup_interval_secs() -> i64 {
    300
}
fn default_control_default_tps() -> i64 {
    10_000
}
fn default_control_max_connections() -> i64 {
    50_000
}
fn default_consul_port() -> i64 {
    8500
}
fn default_consul_check_reap_interval() -> i64 {
    30
}
fn default_consul_connect_timeout_secs() -> i64 {
    5
}
fn default_consul_read_timeout_secs() -> i64 {
    30
}
fn default_apollo_port() -> i64 {
    8080
}
fn default_apollo_version() -> String {
    // Current latest Apollo release: https://github.com/apolloconfig/apollo/releases
    "2.5.1".to_string()
}
fn default_webhook_default_timeout_secs() -> i64 {
    30
}
fn default_config_retention_days() -> i64 {
    30
}
fn default_config_gray_max_count() -> i64 {
    10
}
fn default_config_push_max_retry_time() -> i64 {
    50
}
fn default_config_webhook_content_max_capacity() -> i64 {
    102_400
}
fn default_config_read_cache_max_entries() -> i64 {
    10_000
}

// --- Config: Notify / Health Check / Capacity defaults ---
fn default_notify_connect_timeout() -> i64 {
    100
}
fn default_notify_socket_timeout() -> i64 {
    200
}
fn default_max_health_check_fail_count() -> i64 {
    12
}
fn default_max_content() -> i64 {
    10 * 1024 * 1024
}
fn default_capacity_default_cluster_quota() -> i64 {
    100_000
}
fn default_capacity_default_group_quota() -> i64 {
    200
}
fn default_capacity_default_max_size() -> i64 {
    100 * 1024
}
fn default_capacity_default_max_aggr_count() -> i64 {
    10_000
}
fn default_capacity_default_max_aggr_size() -> i64 {
    1024
}
fn default_naming_heartbeat_interval_secs() -> i64 {
    5
}
fn default_naming_ttl_monitor_interval_secs() -> i64 {
    5
}
fn default_naming_deregister_monitor_interval_secs() -> i64 {
    10
}
fn default_naming_clean_initial_delay_ms() -> i64 {
    50_000
}
fn default_naming_clean_period_time_ms() -> i64 {
    30_000
}
fn default_otel_export_timeout_secs() -> i64 {
    10
}
fn default_mesh_xds_port() -> i64 {
    15_010
}
fn default_mesh_xds_sync_interval_ms() -> i64 {
    5000
}
fn default_mesh_xds_default_listener_port() -> i64 {
    15_001
}
fn default_raft_election_timeout_ms() -> i64 {
    5000
}
fn default_raft_heartbeat_interval_ms() -> i64 {
    1000
}
fn default_raft_rpc_timeout_ms() -> i64 {
    5000
}
fn default_raft_snapshot_threshold() -> i64 {
    10_000
}
fn default_raft_snapshot_transfer_timeout_ms() -> i64 {
    30_000
}
fn default_raft_forward_max_retries() -> i64 {
    3
}
fn default_raft_forward_initial_delay_ms() -> i64 {
    200
}
fn default_raft_peer_connect_timeout_secs() -> i64 {
    30
}
fn default_raft_peer_connect_retry_interval_ms() -> i64 {
    500
}
fn default_raft_grpc_tcp_keepalive() -> i64 {
    10
}
fn default_raft_grpc_http2_keepalive_interval() -> i64 {
    10
}
fn default_raft_grpc_http2_keepalive_timeout() -> i64 {
    5
}
fn default_remote_max_inbound_message_size() -> i64 {
    10_485_760
}
fn default_remote_keep_alive_time() -> i64 {
    7_200_000
}
fn default_remote_keep_alive_timeout() -> i64 {
    20_000
}
fn default_remote_permit_keep_alive_time() -> i64 {
    300_000
}
fn default_remote_cluster_connect_timeout() -> i64 {
    5000
}
fn default_remote_cluster_request_timeout() -> i64 {
    5000
}
fn default_remote_cluster_max_retries() -> i64 {
    3
}
fn default_remote_cluster_retry_delay() -> i64 {
    500
}
fn default_remote_cluster_idle_timeout() -> i64 {
    300_000
}
fn default_metrics_system_stats_interval_secs() -> i64 {
    15
}
fn default_rocksdb_write_buffer_mb() -> i64 {
    128
}
fn default_rocksdb_max_write_buffers() -> i64 {
    4
}
fn default_rocksdb_max_background_jobs() -> i64 {
    4
}
fn default_rocksdb_block_cache_mb() -> i64 {
    256
}
fn default_cluster_event_queue_size() -> i64 {
    1024
}
fn default_cluster_circuit_failure_threshold() -> i64 {
    5
}
fn default_cluster_circuit_reset_timeout_ms() -> i64 {
    30_000
}
fn default_cluster_circuit_success_threshold() -> i64 {
    3
}
fn default_cluster_circuit_failure_window_ms() -> i64 {
    60_000
}
fn default_cluster_distro_sync_delay_ms() -> i64 {
    1000
}
fn default_cluster_distro_sync_timeout_ms() -> i64 {
    3000
}
fn default_cluster_distro_sync_retry_delay_ms() -> i64 {
    3000
}
fn default_cluster_distro_verify_interval_ms() -> i64 {
    5000
}
fn default_cluster_distro_verify_timeout_ms() -> i64 {
    3000
}
fn default_cluster_distro_load_retry_delay_ms() -> i64 {
    30_000
}
fn default_cluster_distro_load_max_retries() -> i64 {
    5
}
fn default_cluster_health_check_interval_ms() -> i64 {
    5000
}
fn default_cluster_health_check_timeout_ms() -> i64 {
    3000
}
fn default_cluster_health_check_max_fail_count() -> i64 {
    3
}
fn default_cluster_health_check_suspicious_threshold() -> i64 {
    1
}
fn default_cluster_member_report_interval_ms() -> i64 {
    5000
}
fn default_ai_mcp_registry_port() -> i64 {
    9080
}
fn default_ai_registry_port() -> i64 {
    9080
}
fn default_cmdb_dump_task_interval() -> i64 {
    3600
}
fn default_cmdb_event_task_interval() -> i64 {
    10
}
fn default_cmdb_label_task_interval() -> i64 {
    300
}

// --- f64 defaults ---
fn default_otel_sampling_ratio() -> f64 {
    1.0
}
fn default_rocksdb_bloom_filter_bits() -> f64 {
    10.0
}
fn default_rocksdb_data_block_hash_ratio() -> f64 {
    0.75
}

// ============================================================================
// Top-level typed configuration (deserialized from "batata" key)
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `BatataTypedConfig`.
pub struct BatataTypedConfig {
    #[serde(default)]
    /// Whether the server runs in standalone mode.
    pub standalone: bool,
    #[serde(default)]
    /// The enabled function mode (`config` or `naming`).
    pub function_mode: Option<String>,
    #[serde(default)]
    /// Deployment topology settings.
    pub deployment: DeploymentConfig,
    #[serde(default)]
    /// HTTP and gRPC server settings.
    pub server: ServerConfig,
    #[serde(default)]
    /// Management console settings.
    pub console: ConsoleConfig,
    #[serde(default)]
    /// Database settings.
    pub db: DbConfig,
    #[serde(default)]
    /// SQL datasource settings.
    pub sql: SqlConfig,
    #[serde(default)]
    /// Core runtime settings.
    pub core: CoreConfig,
    #[serde(default)]
    /// Rate limiting settings.
    pub ratelimit: RateLimitConfig,
    #[serde(default)]
    /// Plugin settings.
    pub plugin: PluginConfig,
    #[serde(default)]
    /// Config management settings.
    pub config: ConfigSection,
    #[serde(default)]
    /// Service discovery settings.
    pub naming: NamingConfig,
    #[serde(default)]
    /// OpenTelemetry settings.
    pub otel: OtelConfig,
    #[serde(default)]
    /// Logging settings.
    pub logs: LogsConfig,
    #[serde(default)]
    /// Service mesh settings.
    pub mesh: MeshConfig,
    #[serde(default)]
    /// Raft consensus settings.
    pub raft: RaftConfig,
    #[serde(default)]
    /// Remote (gRPC client) settings.
    pub remote: RemoteConfig,
    #[serde(default)]
    /// Metrics settings.
    pub metrics: MetricsConfig,
    #[serde(default)]
    /// Persistence settings.
    pub persistence: PersistenceConfig,
    #[serde(default)]
    /// RocksDB tuning settings.
    pub rocksdb: RocksdbConfig,
    #[serde(default)]
    /// Cluster membership settings.
    pub cluster: ClusterConfig,
    #[serde(default)]
    /// Network interface selection settings.
    pub inetutils: InetutilsConfig,
    #[serde(default)]
    /// Cluster member list settings.
    pub member: MemberConfig,
    #[serde(default)]
    /// AI module settings.
    pub ai: AiConfig,
    #[serde(default)]
    /// CMDB integration settings.
    pub cmdb: CmdbConfig,
    #[serde(default)]
    /// Security settings.
    pub security: SecurityConfig,
    #[serde(default)]
    /// Extension settings.
    pub extension: ExtensionConfig,
    #[serde(default)]
    /// Prometheus integration settings.
    pub prometheus: PrometheusConfig,
    #[serde(default)]
    /// Istio integration settings.
    pub istio: IstioConfig,
    #[serde(default)]
    /// Kubernetes integration settings.
    pub k8s: K8sConfig,
}

// ============================================================================
// Deployment
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `DeploymentConfig`.
pub struct DeploymentConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
    /// The deployment type (e.g. `merged`, `server`, `console`).
    pub type_: Option<String>,
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            type_: None,
        }
    }
}

// ============================================================================
// Server
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ServerConfig`.
pub struct ServerConfig {
    #[serde(default)]
    /// Main server port settings.
    pub main: ServerMainConfig,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The servlet context path.
    pub context_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The bind address of the server.
    pub address: Option<String>,
    #[serde(default)]
    /// The preferred server IP.
    pub ip: Option<String>,
    #[serde(default)]
    /// HTTP server settings.
    pub http: ServerHttpConfig,
    #[serde(default)]
    /// Graceful shutdown settings.
    pub shutdown: ServerShutdownConfig,
    #[serde(default)]
    /// gRPC server settings.
    pub grpc: ServerGrpcConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            main: ServerMainConfig::default(),
            context_path: None,
            address: None,
            ip: None,
            http: ServerHttpConfig::default(),
            shutdown: ServerShutdownConfig::default(),
            grpc: ServerGrpcConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ServerMainConfig`.
pub struct ServerMainConfig {
    #[serde(default = "default_server_port")]
    /// The port the main server listens on.
    pub port: i64,
}

impl Default for ServerMainConfig {
    fn default() -> Self {
        Self {
            port: default_server_port(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ServerHttpConfig`.
pub struct ServerHttpConfig {
    #[serde(default)]
    /// Number of HTTP worker threads; 0 means auto-detect.
    pub workers: i64,
    #[serde(default = "default_http_keep_alive")]
    /// Keep-alive timeout in seconds.
    pub keep_alive: i64,
    #[serde(default = "default_http_max_payload_size")]
    /// Maximum request payload size in bytes.
    pub max_payload_size: i64,
    #[serde(default = "default_http_max_json_size")]
    /// Maximum JSON body size in bytes.
    pub max_json_size: i64,
    #[serde(default = "default_http_client_request_timeout")]
    /// Client request timeout in milliseconds.
    pub client_request_timeout: i64,
    #[serde(default)]
    /// HTTP access log settings.
    pub access_log: ServerHttpAccessLogConfig,
    #[serde(default)]
    /// HTTP response compression settings.
    pub compression: ServerHttpCompressionConfig,
}

impl Default for ServerHttpConfig {
    fn default() -> Self {
        Self {
            workers: 0,
            keep_alive: default_http_keep_alive(),
            max_payload_size: default_http_max_payload_size(),
            max_json_size: default_http_max_json_size(),
            client_request_timeout: default_http_client_request_timeout(),
            access_log: ServerHttpAccessLogConfig::default(),
            compression: ServerHttpCompressionConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ServerHttpAccessLogConfig`.
pub struct ServerHttpAccessLogConfig {
    #[serde(default = "default_true")]
    /// Whether access logging is enabled.
    pub enabled: bool,
    #[serde(default = "default_access_log_max_days")]
    /// Number of days access logs are retained.
    pub max_days: i64,
    #[serde(default)]
    /// The access log format pattern.
    pub pattern: Option<String>,
    #[serde(default)]
    /// The base directory for access logs.
    pub basedir: Option<String>,
}

impl Default for ServerHttpAccessLogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_days: default_access_log_max_days(),
            pattern: None,
            basedir: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ServerHttpCompressionConfig`.
pub struct ServerHttpCompressionConfig {
    #[serde(default = "default_true")]
    /// Whether response compression is enabled.
    pub enabled: bool,
    #[serde(default = "default_compression_min_size")]
    /// Minimum response size in bytes before compression applies.
    pub min_size: i64,
}

impl Default for ServerHttpCompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_size: default_compression_min_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ServerShutdownConfig`.
pub struct ServerShutdownConfig {
    #[serde(default = "default_shutdown_drain_timeout")]
    /// How long to wait for in-flight requests to drain, in milliseconds.
    pub drain_timeout: i64,
    #[serde(default = "default_shutdown_db_close_timeout")]
    /// How long to wait for the database to close, in milliseconds.
    pub db_close_timeout: i64,
}

impl Default for ServerShutdownConfig {
    fn default() -> Self {
        Self {
            drain_timeout: default_shutdown_drain_timeout(),
            db_close_timeout: default_shutdown_db_close_timeout(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ServerGrpcConfig`.
pub struct ServerGrpcConfig {
    #[serde(default = "default_grpc_tcp_keepalive")]
    /// TCP keep-alive interval in seconds.
    pub tcp_keepalive: i64,
    #[serde(default = "default_true")]
    /// Whether Nagle's algorithm is disabled.
    pub tcp_nodelay: bool,
    #[serde(default = "default_grpc_http2_keepalive_interval")]
    /// HTTP/2 keep-alive ping interval in seconds.
    pub http2_keepalive_interval: i64,
    #[serde(default = "default_grpc_http2_keepalive_timeout")]
    /// HTTP/2 keep-alive ping timeout in seconds.
    pub http2_keepalive_timeout: i64,
    #[serde(default = "default_grpc_concurrency_limit")]
    /// Maximum number of concurrent requests per connection.
    pub concurrency_limit: i64,
    #[serde(default = "default_grpc_connection_stale_ms")]
    /// How long a connection may stay idle before it is considered stale.
    pub connection_stale_ms: i64,
    #[serde(default = "default_grpc_push_message_timeout")]
    /// Timeout for pushing a message to a client, in milliseconds.
    pub push_message_timeout: i64,
    #[serde(default = "default_grpc_bistream_channel_capacity")]
    /// Capacity of the bi-directional stream channel.
    pub bistream_channel_capacity: i64,
    #[serde(default)]
    /// Timeout for notifying subscribers, in milliseconds.
    pub notify_subscriber_timeout: i64,
    #[serde(default = "default_grpc_max_push_timeouts")]
    /// Number of push timeouts tolerated before a connection is closed.
    pub max_push_timeouts: i64,
    #[serde(default = "default_grpc_max_concurrent_streams")]
    /// Maximum number of concurrent HTTP/2 streams.
    pub max_concurrent_streams: i64,
    #[serde(default = "default_grpc_max_connections")]
    /// Maximum number of concurrent connections.
    pub max_connections: i64,
    #[serde(default = "default_grpc_initial_connection_window_size")]
    /// Initial HTTP/2 connection-level flow control window size.
    pub initial_connection_window_size: i64,
    #[serde(default = "default_grpc_initial_stream_window_size")]
    /// Initial HTTP/2 stream-level flow control window size.
    pub initial_stream_window_size: i64,
    #[serde(default = "default_grpc_max_frame_size")]
    /// Maximum HTTP/2 frame size.
    pub max_frame_size: i64,
}

impl Default for ServerGrpcConfig {
    fn default() -> Self {
        Self {
            tcp_keepalive: default_grpc_tcp_keepalive(),
            tcp_nodelay: true,
            http2_keepalive_interval: default_grpc_http2_keepalive_interval(),
            http2_keepalive_timeout: default_grpc_http2_keepalive_timeout(),
            concurrency_limit: default_grpc_concurrency_limit(),
            connection_stale_ms: default_grpc_connection_stale_ms(),
            push_message_timeout: default_grpc_push_message_timeout(),
            bistream_channel_capacity: default_grpc_bistream_channel_capacity(),
            notify_subscriber_timeout: 0,
            max_push_timeouts: default_grpc_max_push_timeouts(),
            max_concurrent_streams: default_grpc_max_concurrent_streams(),
            max_connections: default_grpc_max_connections(),
            initial_connection_window_size: default_grpc_initial_connection_window_size(),
            initial_stream_window_size: default_grpc_initial_stream_window_size(),
            max_frame_size: default_grpc_max_frame_size(),
        }
    }
}

// ============================================================================
// Console
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConsoleConfig`.
pub struct ConsoleConfig {
    #[serde(default = "default_console_port")]
    /// The port the console listens on.
    pub port: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The console context path.
    pub context_path: Option<String>,
    #[serde(default)]
    /// Console UI settings.
    pub ui: ConsoleUiConfig,
    #[serde(default)]
    /// Remote server settings used in console-only deployments.
    pub remote: ConsoleRemoteConfig,
    #[serde(default)]
    /// Console HTTP settings.
    pub http: ConsoleHttpConfig,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            port: default_console_port(),
            context_path: None,
            ui: ConsoleUiConfig::default(),
            remote: ConsoleRemoteConfig::default(),
            http: ConsoleHttpConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConsoleUiConfig`.
pub struct ConsoleUiConfig {
    #[serde(default = "default_true")]
    /// Whether the built-in console UI is served.
    pub enabled: bool,
    #[serde(default)]
    /// The default UI variant to serve.
    pub default: Option<String>,
    /// Directory containing the built frontend (batata-ui) static assets.
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub dir: Option<String>,
}

impl Default for ConsoleUiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default: None,
            dir: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConsoleRemoteConfig`.
pub struct ConsoleRemoteConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Address of the remote server.
    pub server_addr: Option<String>,
    #[serde(default)]
    /// Context path of the remote server.
    pub server_context_path: Option<String>,
    #[serde(default = "default_console_remote_refresh_interval_secs")]
    /// How often remote state is refreshed, in seconds.
    pub refresh_interval_secs: i64,
    #[serde(default = "default_console_remote_initial_delay_secs")]
    /// Delay before the first refresh, in seconds.
    pub initial_delay_secs: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Username for the remote server.
    pub username: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Password for the remote server.
    pub password: Option<String>,
    #[serde(default = "default_console_remote_connect_timeout_ms")]
    /// Connect timeout for remote calls, in milliseconds.
    pub connect_timeout_ms: i64,
    #[serde(default = "default_console_remote_read_timeout_ms")]
    /// Read timeout for remote calls, in milliseconds.
    pub read_timeout_ms: i64,
}

impl Default for ConsoleRemoteConfig {
    fn default() -> Self {
        Self {
            server_addr: None,
            server_context_path: None,
            refresh_interval_secs: default_console_remote_refresh_interval_secs(),
            initial_delay_secs: default_console_remote_initial_delay_secs(),
            username: None,
            password: None,
            connect_timeout_ms: default_console_remote_connect_timeout_ms(),
            read_timeout_ms: default_console_remote_read_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConsoleHttpConfig`.
pub struct ConsoleHttpConfig {
    #[serde(default = "default_console_http_keep_alive")]
    /// Keep-alive timeout in seconds.
    pub keep_alive: i64,
}

impl Default for ConsoleHttpConfig {
    fn default() -> Self {
        Self {
            keep_alive: default_console_http_keep_alive(),
        }
    }
}

// ============================================================================
// Database
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `DbConfig`.
pub struct DbConfig {
    #[serde(default)]
    /// The database connection URL.
    pub url: Option<String>,
    #[serde(default)]
    /// Connection pool settings.
    pub pool: DbPoolConfig,
    #[serde(default)]
    /// Schema migration settings.
    pub migration: DbMigrationConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `DbPoolConfig`.
pub struct DbPoolConfig {
    #[serde(default = "default_db_pool_max_connections")]
    /// Maximum number of pooled connections.
    pub max_connections: i64,
    #[serde(default = "default_db_pool_min_connections")]
    /// Minimum number of pooled connections.
    pub min_connections: i64,
    #[serde(default = "default_db_pool_connect_timeout")]
    /// Connect timeout in seconds.
    pub connect_timeout: i64,
    #[serde(default = "default_db_pool_acquire_timeout")]
    /// How long to wait for a pooled connection, in seconds.
    pub acquire_timeout: i64,
    #[serde(default = "default_db_pool_idle_timeout")]
    /// How long an idle connection is retained, in seconds.
    pub idle_timeout: i64,
    #[serde(default = "default_db_pool_max_lifetime")]
    /// Maximum lifetime of a pooled connection, in seconds.
    pub max_lifetime: i64,
    #[serde(default)]
    /// Whether SQLx query logging is enabled.
    pub sqlx_logging: bool,
}

impl Default for DbPoolConfig {
    fn default() -> Self {
        Self {
            max_connections: default_db_pool_max_connections(),
            min_connections: default_db_pool_min_connections(),
            connect_timeout: default_db_pool_connect_timeout(),
            acquire_timeout: default_db_pool_acquire_timeout(),
            idle_timeout: default_db_pool_idle_timeout(),
            max_lifetime: default_db_pool_max_lifetime(),
            sqlx_logging: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `DbMigrationConfig`.
pub struct DbMigrationConfig {
    #[serde(default = "default_true")]
    /// Whether schema migrations run automatically on startup.
    pub enabled: bool,
}

impl Default for DbMigrationConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

// ============================================================================
// SQL
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `SqlConfig`.
pub struct SqlConfig {
    #[serde(default)]
    /// SQL initialization settings.
    pub init: SqlInitConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `SqlInitConfig`.
pub struct SqlInitConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The SQL platform used to initialize the schema.
    pub platform: Option<String>,
}

// ============================================================================
// Core
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreConfig`.
pub struct CoreConfig {
    #[serde(default)]
    /// Authentication and authorization settings.
    pub auth: CoreAuthConfig,
    #[serde(default)]
    /// Snowflake ID generator settings.
    pub snowflake: CoreSnowflakeConfig,
    #[serde(default)]
    /// Cluster member lookup settings.
    pub member: CoreMemberConfig,
    #[serde(default)]
    /// Address server settings.
    pub address_server: CoreAddressServerConfig,
    #[serde(default)]
    /// Core API settings.
    pub api: CoreApiConfig,
}

// --- Core: Auth ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthConfig`.
pub struct CoreAuthConfig {
    #[serde(default)]
    /// Whether authentication is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Admin credential settings.
    pub admin: CoreAuthAdminConfig,
    #[serde(default)]
    /// Console authentication settings.
    pub console: CoreAuthConsoleConfig,
    #[serde(default)]
    /// Permission caching settings.
    pub caching: CoreAuthCachingConfig,
    #[serde(default)]
    /// Authentication system type settings.
    pub system: CoreAuthSystemConfig,
    #[serde(default)]
    /// Server identity settings.
    pub server: CoreAuthServerConfig,
    #[serde(default)]
    /// Auth plugin settings.
    pub plugin: CoreAuthPluginConfig,
    #[serde(default)]
    /// LDAP authentication settings.
    pub ldap: CoreAuthLdapConfig,
    #[serde(default)]
    /// OAuth2/OIDC authentication settings.
    pub oauth: CoreAuthOauthConfig,
    #[serde(default)]
    /// Token cache settings.
    pub cache: CoreAuthCacheConfig,
    #[serde(default)]
    /// Default user/role settings.
    pub default: CoreAuthDefaultConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthAdminConfig`.
pub struct CoreAuthAdminConfig {
    #[serde(default)]
    /// Whether the built-in admin account is enabled.
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthConsoleConfig`.
pub struct CoreAuthConsoleConfig {
    #[serde(default = "default_true")]
    /// Whether console login is required.
    pub enabled: bool,
}

impl Default for CoreAuthConsoleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthCachingConfig`.
pub struct CoreAuthCachingConfig {
    #[serde(default = "default_true")]
    /// Whether authentication results are cached.
    pub enabled: bool,
}

impl Default for CoreAuthCachingConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthSystemConfig`.
pub struct CoreAuthSystemConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
    /// The authentication system type (e.g. `nacos`, `ldap`).
    pub type_: Option<String>,
}

impl Default for CoreAuthSystemConfig {
    fn default() -> Self {
        Self {
            type_: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthServerConfig`.
pub struct CoreAuthServerConfig {
    #[serde(default)]
    /// Identity used by the server when authenticating to itself.
    pub identity: CoreAuthServerIdentityConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthServerIdentityConfig`.
pub struct CoreAuthServerIdentityConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The identity key presented by the server.
    pub key: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The identity value presented by the server.
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthPluginConfig`.
pub struct CoreAuthPluginConfig {
    #[serde(default)]
    /// Default auth plugin settings.
    pub default: CoreAuthPluginDefaultConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthPluginDefaultConfig`.
pub struct CoreAuthPluginDefaultConfig {
    #[serde(default)]
    /// Token settings for the default auth plugin.
    pub token: CoreAuthPluginDefaultTokenConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthPluginDefaultTokenConfig`.
pub struct CoreAuthPluginDefaultTokenConfig {
    #[serde(default)]
    /// Token expiration settings.
    pub expire: CoreAuthPluginDefaultTokenExpireConfig,
    #[serde(default)]
    /// Token signing secret settings.
    pub secret: CoreAuthPluginDefaultTokenSecretConfig,
    #[serde(default)]
    /// Token cache settings.
    pub cache: CoreAuthPluginDefaultTokenCacheConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthPluginDefaultTokenExpireConfig`.
pub struct CoreAuthPluginDefaultTokenExpireConfig {
    #[serde(default = "default_token_expire_seconds")]
    /// Token lifetime in seconds.
    pub seconds: i64,
}

impl Default for CoreAuthPluginDefaultTokenExpireConfig {
    fn default() -> Self {
        Self {
            seconds: default_token_expire_seconds(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthPluginDefaultTokenSecretConfig`.
pub struct CoreAuthPluginDefaultTokenSecretConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The secret key used to sign tokens.
    pub key: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthPluginDefaultTokenCacheConfig`.
pub struct CoreAuthPluginDefaultTokenCacheConfig {
    #[serde(default)]
    /// Whether the token cache is enabled.
    pub enable: bool,
}

// --- Core: Auth LDAP ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthLdapConfig`.
pub struct CoreAuthLdapConfig {
    #[serde(default)]
    /// The LDAP server URL.
    pub url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The base DN used for searches.
    pub base_dc: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The DN used to bind to the LDAP server.
    pub bind_dn: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The password used to bind to the LDAP server.
    pub password: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The DN pattern used to resolve users.
    pub user_dn_pattern: Option<String>,
    #[serde(default)]
    /// User search filter settings.
    pub filter: CoreAuthLdapFilterConfig,
    #[serde(default = "default_ldap_timeout")]
    /// LDAP operation timeout in milliseconds.
    pub timeout: i64,
    #[serde(default)]
    /// Username case-handling settings.
    pub case: CoreAuthLdapCaseConfig,
    #[serde(default)]
    /// Settings for ignoring partial LDAP results.
    pub ignore: CoreAuthLdapIgnoreConfig,
}

impl Default for CoreAuthLdapConfig {
    fn default() -> Self {
        Self {
            url: None,
            base_dc: None,
            bind_dn: None,
            password: None,
            user_dn_pattern: None,
            filter: CoreAuthLdapFilterConfig::default(),
            timeout: default_ldap_timeout(),
            case: CoreAuthLdapCaseConfig::default(),
            ignore: CoreAuthLdapIgnoreConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthLdapFilterConfig`.
pub struct CoreAuthLdapFilterConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The LDAP filter prefix used for user searches.
    pub prefix: Option<String>,
}

impl Default for CoreAuthLdapFilterConfig {
    fn default() -> Self {
        Self {
            prefix: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthLdapCaseConfig`.
pub struct CoreAuthLdapCaseConfig {
    #[serde(default = "default_true")]
    /// Whether username matching is case sensitive.
    pub sensitive: bool,
}

impl Default for CoreAuthLdapCaseConfig {
    fn default() -> Self {
        Self { sensitive: true }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthLdapIgnoreConfig`.
pub struct CoreAuthLdapIgnoreConfig {
    #[serde(default)]
    /// Settings for ignoring partial LDAP results.
    pub partial: CoreAuthLdapIgnorePartialConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthLdapIgnorePartialConfig`.
pub struct CoreAuthLdapIgnorePartialConfig {
    #[serde(default)]
    /// Settings for ignoring partial LDAP results.
    pub result: CoreAuthLdapIgnorePartialResultConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthLdapIgnorePartialResultConfig`.
pub struct CoreAuthLdapIgnorePartialResultConfig {
    #[serde(default)]
    /// Whether partial-result exceptions are ignored.
    pub exception: bool,
}

// --- Core: Auth OAuth ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthOauthConfig`.
pub struct CoreAuthOauthConfig {
    #[serde(default)]
    /// Whether OAuth2/OIDC login is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// User provisioning settings.
    pub user: CoreAuthOauthUserConfig,
    #[serde(default)]
    /// Role mapping settings.
    pub role: CoreAuthOauthRoleConfig,
    #[serde(default)]
    /// Redirect URI settings.
    pub redirect: CoreAuthOauthRedirectConfig,
    #[serde(default)]
    /// Discovery and state cache settings.
    pub cache: CoreAuthOauthCacheConfig,
    #[serde(default = "default_oauth_http_timeout_secs")]
    /// HTTP timeout in seconds for provider requests.
    pub http_timeout_secs: i64,
}

impl Default for CoreAuthOauthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            user: CoreAuthOauthUserConfig::default(),
            role: CoreAuthOauthRoleConfig::default(),
            redirect: CoreAuthOauthRedirectConfig::default(),
            cache: CoreAuthOauthCacheConfig::default(),
            http_timeout_secs: default_oauth_http_timeout_secs(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthOauthUserConfig`.
pub struct CoreAuthOauthUserConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Whether users are created automatically on first login.
    pub creation: Option<String>,
}

impl Default for CoreAuthOauthUserConfig {
    fn default() -> Self {
        Self {
            creation: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthOauthRoleConfig`.
pub struct CoreAuthOauthRoleConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Whether roles are synchronized from the provider.
    pub sync: Option<String>,
}

impl Default for CoreAuthOauthRoleConfig {
    fn default() -> Self {
        Self {
            sync: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthOauthRedirectConfig`.
pub struct CoreAuthOauthRedirectConfig {
    #[serde(default)]
    /// The redirect URI registered with the provider.
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthOauthCacheConfig`.
pub struct CoreAuthOauthCacheConfig {
    #[serde(default = "default_oauth_discovery_ttl_secs")]
    /// How long discovery documents are cached, in seconds.
    pub discovery_ttl_secs: i64,
    #[serde(default = "default_oauth_discovery_capacity")]
    /// Maximum number of cached discovery documents.
    pub discovery_capacity: i64,
    #[serde(default = "default_oauth_state_ttl_secs")]
    /// How long login state entries are retained, in seconds.
    pub state_ttl_secs: i64,
    #[serde(default = "default_oauth_state_capacity")]
    /// Maximum number of cached login state entries.
    pub state_capacity: i64,
}

impl Default for CoreAuthOauthCacheConfig {
    fn default() -> Self {
        Self {
            discovery_ttl_secs: default_oauth_discovery_ttl_secs(),
            discovery_capacity: default_oauth_discovery_capacity(),
            state_ttl_secs: default_oauth_state_ttl_secs(),
            state_capacity: default_oauth_state_capacity(),
        }
    }
}

// --- Core: Auth Cache ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAuthCacheConfig`.
pub struct CoreAuthCacheConfig {
    #[serde(default = "default_auth_token_capacity")]
    /// Maximum number of cached tokens.
    pub token_capacity: i64,
    #[serde(default = "default_auth_token_ttl_secs")]
    /// How long cached tokens remain valid, in seconds.
    pub token_ttl_secs: i64,
    #[serde(default = "default_auth_roles_capacity")]
    /// Maximum number of cached role entries.
    pub roles_capacity: i64,
    #[serde(default = "default_auth_permissions_capacity")]
    /// Maximum number of cached permission entries.
    pub permissions_capacity: i64,
    #[serde(default = "default_auth_blacklist_capacity")]
    /// Maximum number of blacklisted tokens.
    pub blacklist_capacity: i64,
    #[serde(default = "default_auth_blacklist_ttl_secs")]
    /// How long blacklisted tokens remain listed, in seconds.
    pub blacklist_ttl_secs: i64,
    #[serde(default = "default_grpc_permission_capacity")]
    /// Maximum number of cached gRPC permission entries.
    pub grpc_permission_capacity: i64,
    #[serde(default = "default_grpc_permission_ttl_secs")]
    /// How long cached gRPC permissions remain valid, in seconds.
    pub grpc_permission_ttl_secs: i64,
}

impl Default for CoreAuthCacheConfig {
    fn default() -> Self {
        Self {
            token_capacity: default_auth_token_capacity(),
            token_ttl_secs: default_auth_token_ttl_secs(),
            roles_capacity: default_auth_roles_capacity(),
            permissions_capacity: default_auth_permissions_capacity(),
            blacklist_capacity: default_auth_blacklist_capacity(),
            blacklist_ttl_secs: default_auth_blacklist_ttl_secs(),
            grpc_permission_capacity: default_grpc_permission_capacity(),
            grpc_permission_ttl_secs: default_grpc_permission_ttl_secs(),
        }
    }
}

// --- Core: Auth Default ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthDefaultConfig`.
pub struct CoreAuthDefaultConfig {
    #[serde(default)]
    /// Default settings for anonymous access.
    pub anonymous: CoreAuthDefaultAnonymousConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthDefaultAnonymousConfig`.
pub struct CoreAuthDefaultAnonymousConfig {
    #[serde(default)]
    /// Anonymous access settings for the AI module.
    pub ai: CoreAuthDefaultAnonymousAiConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreAuthDefaultAnonymousAiConfig`.
pub struct CoreAuthDefaultAnonymousAiConfig {
    #[serde(default)]
    /// Whether anonymous access to AI endpoints is allowed.
    pub enabled: bool,
}

// --- Core: Snowflake ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreSnowflakeConfig`.
pub struct CoreSnowflakeConfig {
    #[serde(default)]
    /// The worker ID used by the snowflake ID generator.
    pub worker_id: Option<i64>,
}

// --- Core: Member ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreMemberConfig`.
pub struct CoreMemberConfig {
    #[serde(default)]
    /// How cluster members are discovered.
    pub lookup: CoreMemberLookupConfig,
    #[serde(default)]
    /// Metadata advertised for this member.
    pub meta: CoreMemberMetaConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreMemberLookupConfig`.
pub struct CoreMemberLookupConfig {
    #[serde(default, rename = "type")]
    /// The member lookup type (e.g. `address-server`, `file`).
    pub type_: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreMemberMetaConfig`.
pub struct CoreMemberMetaConfig {
    #[serde(default)]
    /// The site this member belongs to.
    pub site: Option<String>,
    #[serde(default)]
    /// The advertised weight of this member.
    pub adweight: Option<String>,
    #[serde(default)]
    /// The weight of this member.
    pub weight: Option<String>,
}

// --- Core: Address Server ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreAddressServerConfig`.
pub struct CoreAddressServerConfig {
    #[serde(default = "default_address_server_retry")]
    /// How many times to retry the address server.
    pub retry: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The address server domain.
    pub domain: Option<String>,
    #[serde(default = "default_address_server_port")]
    /// The address server port.
    pub port: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The full address server URL, overriding domain and port.
    pub url: Option<String>,
}

impl Default for CoreAddressServerConfig {
    fn default() -> Self {
        Self {
            retry: default_address_server_retry(),
            domain: None,
            port: default_address_server_port(),
            url: None,
        }
    }
}

// --- Core: API Compatibility ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreApiConfig`.
pub struct CoreApiConfig {
    #[serde(default)]
    /// Settings for Nacos API compatibility layers.
    pub compatibility: CoreApiCompatibilityConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreApiCompatibilityConfig`.
pub struct CoreApiCompatibilityConfig {
    #[serde(default)]
    /// Nacos client API compatibility settings.
    pub client: CoreApiCompatibilityClientConfig,
    #[serde(default)]
    /// Nacos admin API compatibility settings.
    pub admin: CoreApiCompatibilityAdminConfig,
    #[serde(default)]
    /// Nacos console API compatibility settings.
    pub console: CoreApiCompatibilityConsoleConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CoreApiCompatibilityClientConfig`.
pub struct CoreApiCompatibilityClientConfig {
    #[serde(default = "default_true")]
    /// Whether the Nacos client API is exposed.
    pub enabled: bool,
}

impl Default for CoreApiCompatibilityClientConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreApiCompatibilityAdminConfig`.
pub struct CoreApiCompatibilityAdminConfig {
    #[serde(default)]
    /// Whether the Nacos admin API is exposed.
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `CoreApiCompatibilityConsoleConfig`.
pub struct CoreApiCompatibilityConsoleConfig {
    #[serde(default)]
    /// Whether the Nacos console API is exposed.
    pub enabled: bool,
}

// ============================================================================
// Rate Limiting
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RateLimitConfig`.
pub struct RateLimitConfig {
    #[serde(default)]
    /// Whether rate limiting is enabled.
    pub enabled: bool,
    #[serde(default = "default_ratelimit_max_requests")]
    /// Maximum number of requests allowed per window.
    pub max_requests: i64,
    #[serde(default = "default_ratelimit_window_seconds")]
    /// Length of the rate limit window in seconds.
    pub window_seconds: i64,
    #[serde(default)]
    /// Login rate limiting settings.
    pub auth: RateLimitAuthConfig,
    #[serde(default = "default_ratelimit_max_tracked_ips")]
    /// Maximum number of client IPs tracked for rate limiting.
    pub max_tracked_ips: i64,
    #[serde(default = "default_ratelimit_cleanup_interval_secs")]
    /// How often expired rate limit state is cleaned up, in seconds.
    pub cleanup_interval_secs: i64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_requests: default_ratelimit_max_requests(),
            window_seconds: default_ratelimit_window_seconds(),
            auth: RateLimitAuthConfig::default(),
            max_tracked_ips: default_ratelimit_max_tracked_ips(),
            cleanup_interval_secs: default_ratelimit_cleanup_interval_secs(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RateLimitAuthConfig`.
pub struct RateLimitAuthConfig {
    #[serde(default)]
    /// Whether login rate limiting is enabled.
    pub enabled: bool,
    #[serde(default = "default_ratelimit_auth_max_attempts")]
    /// Maximum number of failed login attempts per window.
    pub max_attempts: i64,
    #[serde(default = "default_ratelimit_auth_window_seconds")]
    /// Length of the login rate limit window in seconds.
    pub window_seconds: i64,
    #[serde(default = "default_ratelimit_auth_lockout_seconds")]
    /// How long an account stays locked out, in seconds.
    pub lockout_seconds: i64,
}

impl Default for RateLimitAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_attempts: default_ratelimit_auth_max_attempts(),
            window_seconds: default_ratelimit_auth_window_seconds(),
            lockout_seconds: default_ratelimit_auth_lockout_seconds(),
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginConfig`.
pub struct PluginConfig {
    #[serde(default)]
    /// Traffic control (TPS) plugin settings.
    pub control: PluginControlConfig,
    #[serde(default)]
    /// Consul protocol adapter plugin settings.
    pub consul: PluginConsulConfig,
    #[serde(default)]
    /// Apollo protocol adapter plugin settings.
    pub apollo: PluginApolloConfig,
    #[serde(default)]
    /// Plugin visibility settings.
    pub visibility: PluginVisibilityConfig,
    #[serde(default)]
    /// Datasource plugin settings.
    pub datasource: PluginDatasourceConfig,
    #[serde(default)]
    /// Webhook plugin settings.
    pub webhook: PluginWebhookConfig,
}

// --- Plugin: Control ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PluginControlConfig`.
pub struct PluginControlConfig {
    #[serde(default = "default_true")]
    /// Whether the traffic control plugin is enabled.
    pub enabled: bool,
    #[serde(default = "default_control_default_tps")]
    /// Default TPS limit applied when no rule matches.
    pub default_tps: i64,
    #[serde(default = "default_control_max_connections")]
    /// Maximum number of connections tracked for TPS control.
    pub max_connections: i64,
    #[serde(default)]
    /// TPS manager settings.
    pub manager: PluginControlManagerConfig,
    #[serde(default)]
    /// TPS rule storage settings.
    pub rule: PluginControlRuleConfig,
}

impl Default for PluginControlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_tps: default_control_default_tps(),
            max_connections: default_control_max_connections(),
            manager: PluginControlManagerConfig::default(),
            rule: PluginControlRuleConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginControlManagerConfig`.
pub struct PluginControlManagerConfig {
    #[serde(default, rename = "type")]
    /// The TPS manager implementation type.
    pub type_: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginControlRuleConfig`.
pub struct PluginControlRuleConfig {
    #[serde(default)]
    /// Locally stored TPS rule settings.
    pub local: PluginControlRuleLocalConfig,
    #[serde(default)]
    /// Externally stored TPS rule settings.
    pub external: PluginControlRuleExternalConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginControlRuleLocalConfig`.
pub struct PluginControlRuleLocalConfig {
    #[serde(default)]
    /// Base directory for locally stored TPS rules.
    pub basedir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginControlRuleExternalConfig`.
pub struct PluginControlRuleExternalConfig {
    #[serde(default)]
    /// The external storage used for TPS rules.
    pub storage: Option<String>,
}

// --- Plugin: Consul ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PluginConsulConfig`.
pub struct PluginConsulConfig {
    #[serde(default = "default_true")]
    /// Whether the Consul adapter is enabled.
    pub enabled: bool,
    #[serde(default = "default_consul_port")]
    /// The port the Consul API is served on.
    pub port: i64,
    #[serde(default)]
    /// The Consul version advertised to clients.
    pub version: Option<String>,
    #[serde(default)]
    /// The Consul datacenter name.
    pub datacenter: Option<String>,
    #[serde(default)]
    /// The primary Consul datacenter name.
    pub primary_datacenter: Option<String>,
    #[serde(default)]
    /// The node name advertised by the Consul adapter.
    pub node_name: Option<String>,
    #[serde(default)]
    /// Whether the server registers itself as a Consul node.
    pub register_self: bool,
    #[serde(default)]
    /// Consul ACL settings.
    pub acl: PluginConsulAclConfig,
    #[serde(default = "default_consul_check_reap_interval")]
    /// How often stale Consul checks are reaped, in seconds.
    pub check_reap_interval: i64,
    #[serde(default)]
    /// Consul HTTP client settings.
    pub client: PluginConsulClientConfig,
}

impl Default for PluginConsulConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_consul_port(),
            version: None,
            datacenter: None,
            primary_datacenter: None,
            node_name: None,
            register_self: false,
            acl: PluginConsulAclConfig::default(),
            check_reap_interval: default_consul_check_reap_interval(),
            client: PluginConsulClientConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginConsulAclConfig`.
pub struct PluginConsulAclConfig {
    #[serde(default)]
    /// Whether Consul ACL enforcement is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Consul ACL token settings.
    pub tokens: PluginConsulAclTokensConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginConsulAclTokensConfig`.
pub struct PluginConsulAclTokensConfig {
    #[serde(default)]
    /// The initial management token seeded on first start.
    pub initial_management: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PluginConsulClientConfig`.
pub struct PluginConsulClientConfig {
    #[serde(default = "default_consul_connect_timeout_secs")]
    /// Connect timeout for Consul client requests, in seconds.
    pub connect_timeout_secs: i64,
    #[serde(default = "default_consul_read_timeout_secs")]
    /// Read timeout for Consul client requests, in seconds.
    pub read_timeout_secs: i64,
}

impl Default for PluginConsulClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: default_consul_connect_timeout_secs(),
            read_timeout_secs: default_consul_read_timeout_secs(),
        }
    }
}

// --- Plugin: Apollo ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PluginApolloConfig`.
pub struct PluginApolloConfig {
    #[serde(default = "default_true")]
    /// Whether the Apollo adapter is enabled.
    pub enabled: bool,
    #[serde(default = "default_apollo_port")]
    /// The port the Apollo API is served on.
    pub port: i64,
    #[serde(default)]
    /// Apollo HTTP server settings.
    pub http: PluginApolloHttpConfig,
    #[serde(default = "default_apollo_version")]
    /// The supported Apollo version this adapter is compatible with.
    pub version: String,
}

impl Default for PluginApolloConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_apollo_port(),
            http: PluginApolloHttpConfig::default(),
            version: default_apollo_version(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginApolloHttpConfig`.
pub struct PluginApolloHttpConfig {
    #[serde(default)]
    /// Number of HTTP worker threads; 0 means auto-detect.
    pub workers: i64,
}

// --- Plugin: Visibility ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PluginVisibilityConfig`.
pub struct PluginVisibilityConfig {
    #[serde(default = "default_true")]
    /// Whether the visibility plugin is enabled.
    pub enabled: bool,
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
    /// The visibility implementation type.
    pub type_: Option<String>,
}

impl Default for PluginVisibilityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            type_: None,
        }
    }
}

// --- Plugin: Datasource ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginDatasourceConfig`.
pub struct PluginDatasourceConfig {
    #[serde(default)]
    /// Datasource logging settings.
    pub log: PluginDatasourceLogConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PluginDatasourceLogConfig`.
pub struct PluginDatasourceLogConfig {
    #[serde(default)]
    /// Whether datasource logging is enabled.
    pub enabled: bool,
}

// --- Plugin: Webhook ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PluginWebhookConfig`.
pub struct PluginWebhookConfig {
    #[serde(default = "default_webhook_default_timeout_secs")]
    /// Default timeout for webhook deliveries, in seconds.
    pub default_timeout_secs: i64,
}

impl Default for PluginWebhookConfig {
    fn default() -> Self {
        Self {
            default_timeout_secs: default_webhook_default_timeout_secs(),
        }
    }
}

// ============================================================================
// Config Section (batata.config)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigSection`.
pub struct ConfigSection {
    #[serde(default)]
    /// History retention settings.
    pub retention: ConfigRetentionConfig,
    #[serde(default)]
    /// Gray release settings.
    pub gray: ConfigGrayConfig,
    #[serde(default)]
    /// Config content encryption settings.
    pub encryption: ConfigEncryptionConfig,
    #[serde(default)]
    /// Config push settings.
    pub push: ConfigPushConfig,
    #[serde(default)]
    /// Config plugin settings.
    pub plugin: ConfigPluginConfig,
    #[serde(default)]
    /// TTL in milliseconds for the config read cache; 0 disables caching.
    pub read_cache_ttl: i64,
    #[serde(default = "default_config_read_cache_max_entries")]
    /// Maximum number of entries in the config read cache.
    pub read_cache_max_entries: i64,
    #[serde(default)]
    /// Settings for notifying clients of config changes.
    pub notify: ConfigNotifyConfig,
    #[serde(default)]
    /// Config health check settings.
    pub health_check: ConfigHealthCheckConfig,
    #[serde(default = "default_max_content")]
    /// Maximum allowed config content size in bytes.
    pub max_content: i64,
    #[serde(default)]
    /// Capacity management settings.
    pub capacity: ConfigCapacityConfig,
}

impl Default for ConfigSection {
    fn default() -> Self {
        Self {
            retention: ConfigRetentionConfig::default(),
            gray: ConfigGrayConfig::default(),
            encryption: ConfigEncryptionConfig::default(),
            push: ConfigPushConfig::default(),
            plugin: ConfigPluginConfig::default(),
            read_cache_ttl: 0,
            read_cache_max_entries: default_config_read_cache_max_entries(),
            notify: ConfigNotifyConfig::default(),
            health_check: ConfigHealthCheckConfig::default(),
            max_content: default_max_content(),
            capacity: ConfigCapacityConfig::default(),
        }
    }
}

// --- Config: Notify ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigNotifyConfig`.
pub struct ConfigNotifyConfig {
    #[serde(default = "default_notify_connect_timeout")]
    /// Connect timeout in milliseconds for config notify requests.
    pub connect_timeout: i64,
    #[serde(default = "default_notify_socket_timeout")]
    /// Socket timeout in milliseconds for config notify requests.
    pub socket_timeout: i64,
}

impl Default for ConfigNotifyConfig {
    fn default() -> Self {
        Self {
            connect_timeout: default_notify_connect_timeout(),
            socket_timeout: default_notify_socket_timeout(),
        }
    }
}

// --- Config: Health Check ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigHealthCheckConfig`.
pub struct ConfigHealthCheckConfig {
    #[serde(default = "default_true")]
    /// Whether config health checks are enabled.
    pub enabled: bool,
    #[serde(default = "default_max_health_check_fail_count")]
    /// Number of tolerated health check failures.
    pub max_fail_count: i64,
}

impl Default for ConfigHealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_fail_count: default_max_health_check_fail_count(),
        }
    }
}

// --- Config: Capacity ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigCapacityConfig`.
pub struct ConfigCapacityConfig {
    #[serde(default = "default_true")]
    /// Whether capacity management is enabled.
    pub manage_enabled: bool,
    #[serde(default)]
    /// Whether capacity limits are enforced on writes.
    pub limit_check: bool,
    #[serde(default = "default_capacity_default_cluster_quota")]
    /// Default number of configs allowed per cluster.
    pub default_cluster_quota: i64,
    #[serde(default = "default_capacity_default_group_quota")]
    /// Default number of configs allowed per group.
    pub default_group_quota: i64,
    #[serde(default)]
    /// Default number of configs allowed per tenant.
    pub default_tenant_quota: Option<i64>,
    #[serde(default = "default_capacity_default_max_size")]
    /// Default maximum config content size in bytes.
    pub default_max_size: i64,
    #[serde(default = "default_capacity_default_max_aggr_count")]
    /// Default maximum aggregated config count.
    pub default_max_aggr_count: i64,
    #[serde(default = "default_capacity_default_max_aggr_size")]
    /// Default maximum aggregated config size in bytes.
    pub default_max_aggr_size: i64,
}

impl Default for ConfigCapacityConfig {
    fn default() -> Self {
        Self {
            manage_enabled: true,
            limit_check: false,
            default_cluster_quota: default_capacity_default_cluster_quota(),
            default_group_quota: default_capacity_default_group_quota(),
            default_tenant_quota: None,
            default_max_size: default_capacity_default_max_size(),
            default_max_aggr_count: default_capacity_default_max_aggr_count(),
            default_max_aggr_size: default_capacity_default_max_aggr_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigRetentionConfig`.
pub struct ConfigRetentionConfig {
    #[serde(default = "default_config_retention_days")]
    /// Number of days config history is retained.
    pub days: i64,
}

impl Default for ConfigRetentionConfig {
    fn default() -> Self {
        Self {
            days: default_config_retention_days(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ConfigGrayConfig`.
pub struct ConfigGrayConfig {
    #[serde(default)]
    /// Gray version settings.
    pub version: ConfigGrayVersionConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigGrayVersionConfig`.
pub struct ConfigGrayVersionConfig {
    #[serde(default = "default_config_gray_max_count")]
    /// Maximum number of gray versions retained per config.
    pub max_count: i64,
}

impl Default for ConfigGrayVersionConfig {
    fn default() -> Self {
        Self {
            max_count: default_config_gray_max_count(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ConfigEncryptionConfig`.
pub struct ConfigEncryptionConfig {
    #[serde(default)]
    /// Whether config content encryption is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Encryption plugin settings.
    pub plugin: ConfigEncryptionPluginConfig,
    #[serde(default)]
    /// The default encryption data key.
    pub key: Option<String>,
    #[serde(default)]
    /// Encryption key reload settings.
    pub reload: ConfigEncryptionReloadConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigEncryptionPluginConfig`.
pub struct ConfigEncryptionPluginConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
    /// The encryption plugin implementation type.
    pub type_: Option<String>,
}

impl Default for ConfigEncryptionPluginConfig {
    fn default() -> Self {
        Self {
            type_: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ConfigEncryptionReloadConfig`.
pub struct ConfigEncryptionReloadConfig {
    #[serde(default)]
    /// How often encryption keys are reloaded.
    pub interval: ConfigEncryptionReloadIntervalConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ConfigEncryptionReloadIntervalConfig`.
pub struct ConfigEncryptionReloadIntervalConfig {
    #[serde(default)]
    /// The reload interval in milliseconds.
    pub ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigPushConfig`.
pub struct ConfigPushConfig {
    #[serde(default = "default_config_push_max_retry_time")]
    /// Maximum number of times a config push is retried.
    pub max_retry_time: i64,
}

impl Default for ConfigPushConfig {
    fn default() -> Self {
        Self {
            max_retry_time: default_config_push_max_retry_time(),
        }
    }
}

// --- Config: Plugin ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ConfigPluginConfig`.
pub struct ConfigPluginConfig {
    #[serde(default)]
    /// Webhook notification settings for config changes.
    pub webhook: ConfigPluginWebhookConfig,
    #[serde(default)]
    /// Config content whitelist settings.
    pub whitelist: ConfigPluginWhitelistConfig,
    #[serde(default)]
    /// Config file format check settings.
    pub fileformatcheck: ConfigPluginFileformatcheckConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ConfigPluginWebhookConfig`.
pub struct ConfigPluginWebhookConfig {
    #[serde(default)]
    /// Whether config change webhooks are enabled.
    pub enabled: bool,
    #[serde(default)]
    /// The webhook endpoint to notify.
    pub url: Option<String>,
    #[serde(default = "default_config_webhook_content_max_capacity")]
    /// Maximum webhook payload size in bytes.
    pub content_max_capacity: i64,
}

impl Default for ConfigPluginWebhookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: None,
            content_max_capacity: default_config_webhook_content_max_capacity(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ConfigPluginWhitelistConfig`.
pub struct ConfigPluginWhitelistConfig {
    #[serde(default)]
    /// Whether the config content whitelist is enforced.
    pub enabled: bool,
    #[serde(default)]
    /// Comma-separated list of allowed config suffixes.
    pub suffixes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ConfigPluginFileformatcheckConfig`.
pub struct ConfigPluginFileformatcheckConfig {
    #[serde(default)]
    /// Whether config file format validation is enabled.
    pub enabled: bool,
}

// ============================================================================
// Naming
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `NamingConfig`.
pub struct NamingConfig {
    #[serde(default = "default_true")]
    /// Whether instances expire automatically when heartbeats stop.
    pub expire_instance: bool,
    #[serde(default)]
    /// Naming data settings.
    pub data: NamingDataConfig,
    #[serde(default)]
    /// Health check settings.
    pub healthcheck: NamingHealthcheckConfig,
    #[serde(default)]
    /// Empty service cleanup settings.
    pub empty_service: NamingEmptyServiceConfig,
}

impl Default for NamingConfig {
    fn default() -> Self {
        Self {
            expire_instance: true,
            data: NamingDataConfig::default(),
            healthcheck: NamingHealthcheckConfig::default(),
            empty_service: NamingEmptyServiceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `NamingDataConfig`.
pub struct NamingDataConfig {
    #[serde(default)]
    /// Whether naming data is warmed up on startup.
    pub warmup: bool,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `NamingHealthcheckConfig`.
pub struct NamingHealthcheckConfig {
    #[serde(default = "default_naming_heartbeat_interval_secs")]
    /// How often clients are expected to send heartbeats, in seconds.
    pub heartbeat_interval_secs: i64,
    #[serde(default = "default_naming_ttl_monitor_interval_secs")]
    /// How often TTL-based instances are checked, in seconds.
    pub ttl_monitor_interval_secs: i64,
    #[serde(default = "default_naming_deregister_monitor_interval_secs")]
    /// How often instance deregistration is monitored, in seconds.
    pub deregister_monitor_interval_secs: i64,
}

impl Default for NamingHealthcheckConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: default_naming_heartbeat_interval_secs(),
            ttl_monitor_interval_secs: default_naming_ttl_monitor_interval_secs(),
            deregister_monitor_interval_secs: default_naming_deregister_monitor_interval_secs(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `NamingEmptyServiceConfig`.
pub struct NamingEmptyServiceConfig {
    #[serde(default = "default_true")]
    /// Whether empty services are removed automatically.
    pub auto_clean: bool,
    #[serde(default)]
    /// Cleanup task settings for empty services.
    pub clean: NamingEmptyServiceCleanConfig,
}

impl Default for NamingEmptyServiceConfig {
    fn default() -> Self {
        Self {
            auto_clean: true,
            clean: NamingEmptyServiceCleanConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `NamingEmptyServiceCleanConfig`.
pub struct NamingEmptyServiceCleanConfig {
    #[serde(default = "default_naming_clean_initial_delay_ms")]
    /// Delay before the first cleanup run, in milliseconds.
    pub initial_delay_ms: i64,
    #[serde(default = "default_naming_clean_period_time_ms")]
    /// Interval between cleanup runs, in milliseconds.
    pub period_time_ms: i64,
}

impl Default for NamingEmptyServiceCleanConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: default_naming_clean_initial_delay_ms(),
            period_time_ms: default_naming_clean_period_time_ms(),
        }
    }
}

// ============================================================================
// OpenTelemetry
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `OtelConfig`.
pub struct OtelConfig {
    #[serde(default)]
    /// Whether OpenTelemetry export is enabled.
    pub enabled: bool,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The OTLP collector endpoint.
    pub endpoint: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The service name reported to the collector.
    pub service_name: Option<String>,
    #[serde(default = "default_otel_sampling_ratio")]
    /// Fraction of traces sampled, between 0.0 and 1.0.
    pub sampling_ratio: f64,
    #[serde(default = "default_otel_export_timeout_secs")]
    /// Timeout for exporting telemetry, in seconds.
    pub export_timeout_secs: i64,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: None,
            sampling_ratio: default_otel_sampling_ratio(),
            export_timeout_secs: default_otel_export_timeout_secs(),
        }
    }
}

// ============================================================================
// Logs
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `LogsConfig`.
pub struct LogsConfig {
    #[serde(default)]
    /// Directory where log files are written.
    pub path: Option<String>,
    #[serde(default)]
    /// Console logging settings.
    pub console: LogsConsoleConfig,
    #[serde(default)]
    /// File logging settings.
    pub file: LogsFileConfig,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The global log level filter.
    pub level: Option<String>,
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            path: None,
            console: LogsConsoleConfig::default(),
            file: LogsFileConfig::default(),
            level: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `LogsConsoleConfig`.
pub struct LogsConsoleConfig {
    #[serde(default = "default_true")]
    /// Whether logs are written to the console.
    pub enabled: bool,
}

impl Default for LogsConsoleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `LogsFileConfig`.
pub struct LogsFileConfig {
    #[serde(default = "default_true")]
    /// Whether logs are written to files.
    pub enabled: bool,
}

impl Default for LogsFileConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

// ============================================================================
// Mesh
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MeshConfig`.
pub struct MeshConfig {
    #[serde(default)]
    /// xDS (service mesh) settings.
    pub xds: MeshXdsConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `MeshXdsConfig`.
pub struct MeshXdsConfig {
    #[serde(default)]
    /// Whether the xDS server is enabled.
    pub enabled: bool,
    #[serde(default = "default_mesh_xds_port")]
    /// The port the xDS server listens on.
    pub port: i64,
    #[serde(default)]
    /// xDS server identity settings.
    pub server: MeshXdsServerConfig,
    #[serde(default)]
    /// Settings for syncing xDS resources to Envoy.
    pub sync: MeshXdsSyncConfig,
    #[serde(default)]
    /// Settings controlling which xDS resources are generated.
    pub generate: MeshXdsGenerateConfig,
    #[serde(default)]
    /// Default xDS resource settings.
    pub default: MeshXdsDefaultConfig,
    #[serde(default)]
    /// TLS settings for the xDS server.
    pub tls: MeshXdsTlsConfig,
}

impl Default for MeshXdsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_mesh_xds_port(),
            server: MeshXdsServerConfig::default(),
            sync: MeshXdsSyncConfig::default(),
            generate: MeshXdsGenerateConfig::default(),
            default: MeshXdsDefaultConfig::default(),
            tls: MeshXdsTlsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `MeshXdsServerConfig`.
pub struct MeshXdsServerConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// The identifier advertised by this xDS server.
    pub id: Option<String>,
}

impl Default for MeshXdsServerConfig {
    fn default() -> Self {
        Self {
            id: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MeshXdsSyncConfig`.
pub struct MeshXdsSyncConfig {
    #[serde(default)]
    /// How often xDS resources are synced.
    pub interval: MeshXdsSyncIntervalConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `MeshXdsSyncIntervalConfig`.
pub struct MeshXdsSyncIntervalConfig {
    #[serde(default = "default_mesh_xds_sync_interval_ms")]
    /// The sync interval in milliseconds.
    pub ms: i64,
}

impl Default for MeshXdsSyncIntervalConfig {
    fn default() -> Self {
        Self {
            ms: default_mesh_xds_sync_interval_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `MeshXdsGenerateConfig`.
pub struct MeshXdsGenerateConfig {
    #[serde(default = "default_true")]
    /// Whether listener resources are generated.
    pub listeners: bool,
    #[serde(default = "default_true")]
    /// Whether route resources are generated.
    pub routes: bool,
}

impl Default for MeshXdsGenerateConfig {
    fn default() -> Self {
        Self {
            listeners: true,
            routes: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MeshXdsDefaultConfig`.
pub struct MeshXdsDefaultConfig {
    #[serde(default)]
    /// Default listener settings.
    pub listener: MeshXdsDefaultListenerConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `MeshXdsDefaultListenerConfig`.
pub struct MeshXdsDefaultListenerConfig {
    #[serde(default = "default_mesh_xds_default_listener_port")]
    /// The default listener port.
    pub port: i64,
}

impl Default for MeshXdsDefaultListenerConfig {
    fn default() -> Self {
        Self {
            port: default_mesh_xds_default_listener_port(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MeshXdsTlsConfig`.
pub struct MeshXdsTlsConfig {
    #[serde(default)]
    /// Whether TLS is enabled for the xDS server.
    pub enabled: bool,
    #[serde(default)]
    /// TLS certificate settings.
    pub cert: MeshXdsTlsCertConfig,
    #[serde(default)]
    /// TLS private key settings.
    pub key: MeshXdsTlsKeyConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MeshXdsTlsCertConfig`.
pub struct MeshXdsTlsCertConfig {
    #[serde(default)]
    /// Path to the TLS certificate file.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MeshXdsTlsKeyConfig`.
pub struct MeshXdsTlsKeyConfig {
    #[serde(default)]
    /// Path to the TLS private key file.
    pub path: Option<String>,
}

// ============================================================================
// Raft
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RaftConfig`.
pub struct RaftConfig {
    #[serde(default = "default_raft_election_timeout_ms")]
    /// Election timeout in milliseconds.
    pub election_timeout_ms: i64,
    #[serde(default = "default_raft_heartbeat_interval_ms")]
    /// Leader heartbeat interval in milliseconds.
    pub heartbeat_interval_ms: i64,
    #[serde(default = "default_raft_rpc_timeout_ms")]
    /// Timeout for Raft RPC calls, in milliseconds.
    pub rpc_timeout_ms: i64,
    #[serde(default = "default_raft_snapshot_threshold")]
    /// Number of log entries before a snapshot is taken.
    pub snapshot_threshold: i64,
    #[serde(default = "default_raft_snapshot_transfer_timeout_ms")]
    /// Timeout for transferring a snapshot, in milliseconds.
    pub snapshot_transfer_timeout_ms: i64,
    #[serde(default)]
    /// Settings for forwarding writes to the leader.
    pub forward: RaftForwardConfig,
    #[serde(default = "default_raft_peer_connect_timeout_secs")]
    /// Connect timeout for peer connections, in seconds.
    pub peer_connect_timeout_secs: i64,
    #[serde(default = "default_raft_peer_connect_retry_interval_ms")]
    /// How often peer connections are retried, in milliseconds.
    pub peer_connect_retry_interval_ms: i64,
    #[serde(default)]
    /// gRPC transport settings for Raft traffic.
    pub grpc: RaftGrpcConfig,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_ms: default_raft_election_timeout_ms(),
            heartbeat_interval_ms: default_raft_heartbeat_interval_ms(),
            rpc_timeout_ms: default_raft_rpc_timeout_ms(),
            snapshot_threshold: default_raft_snapshot_threshold(),
            snapshot_transfer_timeout_ms: default_raft_snapshot_transfer_timeout_ms(),
            forward: RaftForwardConfig::default(),
            peer_connect_timeout_secs: default_raft_peer_connect_timeout_secs(),
            peer_connect_retry_interval_ms: default_raft_peer_connect_retry_interval_ms(),
            grpc: RaftGrpcConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RaftForwardConfig`.
pub struct RaftForwardConfig {
    #[serde(default = "default_raft_forward_max_retries")]
    /// Maximum number of leader-forward retries.
    pub max_retries: i64,
    #[serde(default = "default_raft_forward_initial_delay_ms")]
    /// Initial delay before retrying, in milliseconds.
    pub initial_delay_ms: i64,
}

impl Default for RaftForwardConfig {
    fn default() -> Self {
        Self {
            max_retries: default_raft_forward_max_retries(),
            initial_delay_ms: default_raft_forward_initial_delay_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RaftGrpcConfig`.
pub struct RaftGrpcConfig {
    #[serde(default = "default_raft_grpc_tcp_keepalive")]
    /// TCP keep-alive interval in seconds.
    pub tcp_keepalive: i64,
    #[serde(default = "default_true")]
    /// Whether Nagle's algorithm is disabled.
    pub tcp_nodelay: bool,
    #[serde(default = "default_raft_grpc_http2_keepalive_interval")]
    /// HTTP/2 keep-alive ping interval in seconds.
    pub http2_keepalive_interval: i64,
    #[serde(default = "default_raft_grpc_http2_keepalive_timeout")]
    /// HTTP/2 keep-alive ping timeout in seconds.
    pub http2_keepalive_timeout: i64,
}

impl Default for RaftGrpcConfig {
    fn default() -> Self {
        Self {
            tcp_keepalive: default_raft_grpc_tcp_keepalive(),
            tcp_nodelay: true,
            http2_keepalive_interval: default_raft_grpc_http2_keepalive_interval(),
            http2_keepalive_timeout: default_raft_grpc_http2_keepalive_timeout(),
        }
    }
}

// ============================================================================
// Remote
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteConfig`.
pub struct RemoteConfig {
    #[serde(default)]
    /// Remote (gRPC) server settings.
    pub server: RemoteServerConfig,
    #[serde(default)]
    /// Remote (gRPC) client settings.
    pub client: RemoteClientConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerConfig`.
pub struct RemoteServerConfig {
    #[serde(default)]
    /// gRPC server settings.
    pub grpc: RemoteServerGrpcConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcConfig`.
pub struct RemoteServerGrpcConfig {
    #[serde(default)]
    /// Settings for the SDK-facing gRPC channel.
    pub sdk: RemoteServerGrpcSdkConfig,
    #[serde(default)]
    /// Settings for the cluster-facing gRPC channel.
    pub cluster: RemoteServerGrpcClusterConfig,
    #[serde(default)]
    /// TLS settings for the gRPC server.
    pub tls: RemoteServerGrpcTlsConfig,
}

// --- Remote: Server gRPC SDK ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RemoteServerGrpcSdkConfig`.
pub struct RemoteServerGrpcSdkConfig {
    #[serde(default)]
    /// TLS settings for the SDK channel.
    pub tls: RemoteServerGrpcSdkTlsConfig,
    #[serde(default = "default_remote_max_inbound_message_size")]
    /// Maximum inbound message size in bytes.
    pub max_inbound_message_size: i64,
    #[serde(default = "default_remote_keep_alive_time")]
    /// HTTP/2 keep-alive ping interval in seconds.
    pub keep_alive_time: i64,
    #[serde(default = "default_remote_keep_alive_timeout")]
    /// HTTP/2 keep-alive ping timeout in seconds.
    pub keep_alive_timeout: i64,
    #[serde(default = "default_remote_permit_keep_alive_time")]
    /// Minimum interval allowed between client keep-alive pings.
    pub permit_keep_alive_time: i64,
}

impl Default for RemoteServerGrpcSdkConfig {
    fn default() -> Self {
        Self {
            tls: RemoteServerGrpcSdkTlsConfig::default(),
            max_inbound_message_size: default_remote_max_inbound_message_size(),
            keep_alive_time: default_remote_keep_alive_time(),
            keep_alive_timeout: default_remote_keep_alive_timeout(),
            permit_keep_alive_time: default_remote_permit_keep_alive_time(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcSdkTlsConfig`.
pub struct RemoteServerGrpcSdkTlsConfig {
    #[serde(default)]
    /// Whether TLS is enabled on the SDK channel.
    pub enabled: bool,
}

// --- Remote: Server gRPC Cluster ---

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RemoteServerGrpcClusterConfig`.
pub struct RemoteServerGrpcClusterConfig {
    #[serde(default)]
    /// TLS settings for the cluster channel.
    pub tls: RemoteServerGrpcClusterTlsConfig,
    #[serde(default = "default_remote_cluster_connect_timeout")]
    /// Connect timeout in milliseconds for cluster calls.
    pub connect_timeout: i64,
    #[serde(default = "default_remote_cluster_request_timeout")]
    /// Request timeout in milliseconds for cluster calls.
    pub request_timeout: i64,
    #[serde(default = "default_remote_cluster_max_retries")]
    /// Maximum number of retries for cluster calls.
    pub max_retries: i64,
    #[serde(default = "default_remote_cluster_retry_delay")]
    /// Delay between retries in milliseconds.
    pub retry_delay: i64,
    #[serde(default = "default_remote_cluster_idle_timeout")]
    /// How long idle cluster connections are kept, in milliseconds.
    pub idle_timeout: i64,
    #[serde(default = "default_remote_max_inbound_message_size")]
    /// Maximum inbound message size in bytes.
    pub max_inbound_message_size: i64,
    #[serde(default = "default_remote_keep_alive_time")]
    /// HTTP/2 keep-alive ping interval in seconds.
    pub keep_alive_time: i64,
    #[serde(default = "default_remote_keep_alive_timeout")]
    /// HTTP/2 keep-alive ping timeout in seconds.
    pub keep_alive_timeout: i64,
    #[serde(default = "default_remote_permit_keep_alive_time")]
    /// Minimum interval allowed between keep-alive pings.
    pub permit_keep_alive_time: i64,
}

impl Default for RemoteServerGrpcClusterConfig {
    fn default() -> Self {
        Self {
            tls: RemoteServerGrpcClusterTlsConfig::default(),
            connect_timeout: default_remote_cluster_connect_timeout(),
            request_timeout: default_remote_cluster_request_timeout(),
            max_retries: default_remote_cluster_max_retries(),
            retry_delay: default_remote_cluster_retry_delay(),
            idle_timeout: default_remote_cluster_idle_timeout(),
            max_inbound_message_size: default_remote_max_inbound_message_size(),
            keep_alive_time: default_remote_keep_alive_time(),
            keep_alive_timeout: default_remote_keep_alive_timeout(),
            permit_keep_alive_time: default_remote_permit_keep_alive_time(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcClusterTlsConfig`.
pub struct RemoteServerGrpcClusterTlsConfig {
    #[serde(default)]
    /// Whether TLS is enabled on the cluster channel.
    pub enabled: bool,
}

// --- Remote: Server gRPC TLS ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcTlsConfig`.
pub struct RemoteServerGrpcTlsConfig {
    #[serde(default)]
    /// Server certificate settings.
    pub cert: RemoteServerGrpcTlsCertConfig,
    #[serde(default)]
    /// Server private key settings.
    pub key: RemoteServerGrpcTlsKeyConfig,
    #[serde(default)]
    /// Certificate authority settings.
    pub ca: RemoteServerGrpcTlsCaConfig,
    #[serde(default)]
    /// Mutual TLS settings.
    pub mtls: RemoteServerGrpcTlsMtlsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcTlsCertConfig`.
pub struct RemoteServerGrpcTlsCertConfig {
    #[serde(default)]
    /// Path to the server certificate file.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcTlsKeyConfig`.
pub struct RemoteServerGrpcTlsKeyConfig {
    #[serde(default)]
    /// Path to the server private key file.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcTlsCaConfig`.
pub struct RemoteServerGrpcTlsCaConfig {
    #[serde(default)]
    /// CA certificate settings.
    pub cert: RemoteServerGrpcTlsCaCertConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcTlsCaCertConfig`.
pub struct RemoteServerGrpcTlsCaCertConfig {
    #[serde(default)]
    /// Path to the CA certificate file.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteServerGrpcTlsMtlsConfig`.
pub struct RemoteServerGrpcTlsMtlsConfig {
    #[serde(default)]
    /// Whether mutual TLS is required.
    pub enabled: bool,
}

// --- Remote: Client ---

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientConfig`.
pub struct RemoteClientConfig {
    #[serde(default)]
    /// gRPC client settings.
    pub grpc: RemoteClientGrpcConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientGrpcConfig`.
pub struct RemoteClientGrpcConfig {
    #[serde(default)]
    /// Cluster channel settings.
    pub cluster: RemoteClientGrpcClusterConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientGrpcClusterConfig`.
pub struct RemoteClientGrpcClusterConfig {
    #[serde(default)]
    /// TLS settings for outbound cluster calls.
    pub tls: RemoteClientGrpcClusterTlsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientGrpcClusterTlsConfig`.
pub struct RemoteClientGrpcClusterTlsConfig {
    #[serde(default)]
    /// Whether TLS is used for outbound cluster calls.
    pub enabled: bool,
    #[serde(default)]
    /// Client certificate settings.
    pub cert: RemoteClientGrpcClusterTlsCertConfig,
    #[serde(default)]
    /// Client private key settings.
    pub key: RemoteClientGrpcClusterTlsKeyConfig,
    #[serde(default)]
    /// Certificate authority settings.
    pub ca: RemoteClientGrpcClusterTlsCaConfig,
    #[serde(default)]
    /// The domain name expected on the peer certificate.
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientGrpcClusterTlsCertConfig`.
pub struct RemoteClientGrpcClusterTlsCertConfig {
    #[serde(default)]
    /// Path to the client certificate file.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientGrpcClusterTlsKeyConfig`.
pub struct RemoteClientGrpcClusterTlsKeyConfig {
    #[serde(default)]
    /// Path to the client private key file.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientGrpcClusterTlsCaConfig`.
pub struct RemoteClientGrpcClusterTlsCaConfig {
    #[serde(default)]
    /// CA certificate settings.
    pub cert: RemoteClientGrpcClusterTlsCaCertConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `RemoteClientGrpcClusterTlsCaCertConfig`.
pub struct RemoteClientGrpcClusterTlsCaCertConfig {
    #[serde(default)]
    /// Path to the CA certificate file.
    pub path: Option<String>,
}

// ============================================================================
// Metrics
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MetricsConfig`.
pub struct MetricsConfig {
    #[serde(default)]
    /// System statistics collection settings.
    pub system_stats: MetricsSystemStatsConfig,
    #[serde(default)]
    /// Settings for exporting metrics to external systems.
    pub export: MetricsExportConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `MetricsSystemStatsConfig`.
pub struct MetricsSystemStatsConfig {
    #[serde(default = "default_true")]
    /// Whether system statistics are collected.
    pub enabled: bool,
    #[serde(default = "default_metrics_system_stats_interval_secs")]
    /// How often statistics are collected, in seconds.
    pub interval_secs: i64,
}

impl Default for MetricsSystemStatsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: default_metrics_system_stats_interval_secs(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MetricsExportConfig`.
pub struct MetricsExportConfig {
    #[serde(default)]
    /// Elasticsearch export settings.
    pub elastic: MetricsExportElasticConfig,
    #[serde(default)]
    /// InfluxDB export settings.
    pub influx: MetricsExportInfluxConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MetricsExportElasticConfig`.
pub struct MetricsExportElasticConfig {
    #[serde(default)]
    /// Whether Elasticsearch export is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// The Elasticsearch host to export to.
    pub host: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `MetricsExportInfluxConfig`.
pub struct MetricsExportInfluxConfig {
    #[serde(default)]
    /// Whether InfluxDB export is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// The InfluxDB database name.
    pub db: Option<String>,
    #[serde(default)]
    /// The InfluxDB connection URI.
    pub uri: Option<String>,
    #[serde(default = "default_true")]
    /// Whether the database is created automatically.
    pub auto_create_db: bool,
    #[serde(default)]
    /// The write consistency level.
    pub consistency: Option<String>,
    #[serde(default = "default_true")]
    /// Whether exported payloads are compressed.
    pub compressed: bool,
}

impl Default for MetricsExportInfluxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            db: None,
            uri: None,
            auto_create_db: true,
            consistency: None,
            compressed: true,
        }
    }
}

// ============================================================================
// Persistence
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PersistenceConfig`.
pub struct PersistenceConfig {
    #[serde(default)]
    /// Embedded storage settings.
    pub embedded: PersistenceEmbeddedConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PersistenceEmbeddedConfig`.
pub struct PersistenceEmbeddedConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Directory where embedded storage files are kept.
    pub data_dir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Name of the embedded database.
    pub db_name: Option<String>,
}

impl Default for PersistenceEmbeddedConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            db_name: None,
        }
    }
}

// ============================================================================
// RocksDB
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `RocksdbConfig`.
pub struct RocksdbConfig {
    #[serde(default = "default_rocksdb_write_buffer_mb")]
    /// Size of a single memtable write buffer in MiB.
    pub write_buffer_mb: i64,
    #[serde(default = "default_rocksdb_max_write_buffers")]
    /// Maximum number of memtables before writes are stalled.
    pub max_write_buffers: i64,
    #[serde(default = "default_rocksdb_max_background_jobs")]
    /// Number of background flush/compaction threads.
    pub max_background_jobs: i64,
    #[serde(default = "default_rocksdb_block_cache_mb")]
    /// Size of the shared block cache in MiB.
    pub block_cache_mb: i64,
    #[serde(default = "default_rocksdb_bloom_filter_bits")]
    /// Number of bits per key allocated to bloom filters.
    pub bloom_filter_bits: f64,
    #[serde(default = "default_true")]
    /// Whether level compaction is dynamically leveled.
    pub level_compaction_dynamic: bool,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Compression algorithm for the bottommost level.
    pub bottommost_compression: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    /// Compression algorithm for other levels.
    pub compression: Option<String>,
    #[serde(default)]
    /// Whether RocksDB statistics collection is enabled.
    pub enable_statistics: bool,
    #[serde(default = "default_true")]
    /// Whether bloom filters use whole-key filtering.
    pub whole_key_filtering: bool,
    #[serde(default = "default_rocksdb_data_block_hash_ratio")]
    /// Ratio of data blocks that use a hash index.
    pub data_block_hash_ratio: f64,
    #[serde(default)]
    /// Whether writes are flushed synchronously to storage.
    pub sm_sync: bool,
    #[serde(default)]
    /// Whether the write-ahead log is disabled.
    pub sm_disable_wal: bool,
    #[serde(default)]
    /// Size of the history memtable write buffer in MiB.
    pub history_write_buffer_mb: i64,
    #[serde(default)]
    pub history_bloom_filter: bool,
}

impl Default for RocksdbConfig {
    fn default() -> Self {
        Self {
            write_buffer_mb: default_rocksdb_write_buffer_mb(),
            max_write_buffers: default_rocksdb_max_write_buffers(),
            max_background_jobs: default_rocksdb_max_background_jobs(),
            block_cache_mb: default_rocksdb_block_cache_mb(),
            bloom_filter_bits: default_rocksdb_bloom_filter_bits(),
            level_compaction_dynamic: true,
            bottommost_compression: None,
            compression: None,
            enable_statistics: false,
            whole_key_filtering: true,
            data_block_hash_ratio: default_rocksdb_data_block_hash_ratio(),
            sm_sync: false,
            sm_disable_wal: false,
            history_write_buffer_mb: 0,
            history_bloom_filter: false,
        }
    }
}

// ============================================================================
// Cluster
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ClusterConfig`.
pub struct ClusterConfig {
    #[serde(default)]
    /// Circuit breaker settings for cluster calls.
    pub circuit_breaker: ClusterCircuitBreakerConfig,
    #[serde(default)]
    /// Distro (data synchronization) settings.
    pub distro: ClusterDistroConfig,
    #[serde(default)]
    /// Cluster health check settings.
    pub health_check: ClusterHealthCheckConfig,
    #[serde(default = "default_cluster_event_queue_size")]
    /// Maximum number of queued cluster events.
    pub event_queue_size: i64,
    #[serde(default)]
    /// Settings for reporting this member to the cluster.
    pub member_report: ClusterMemberReportConfig,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: ClusterCircuitBreakerConfig::default(),
            distro: ClusterDistroConfig::default(),
            health_check: ClusterHealthCheckConfig::default(),
            event_queue_size: default_cluster_event_queue_size(),
            member_report: ClusterMemberReportConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ClusterCircuitBreakerConfig`.
pub struct ClusterCircuitBreakerConfig {
    #[serde(default = "default_cluster_circuit_failure_threshold")]
    /// Number of failures before the circuit opens.
    pub failure_threshold: i64,
    #[serde(default = "default_cluster_circuit_reset_timeout_ms")]
    /// How long the circuit stays open, in milliseconds.
    pub reset_timeout_ms: i64,
    #[serde(default = "default_cluster_circuit_success_threshold")]
    /// Number of successes needed to close the circuit.
    pub success_threshold: i64,
    #[serde(default = "default_cluster_circuit_failure_window_ms")]
    /// Window in milliseconds used to count failures.
    pub failure_window_ms: i64,
}

impl Default for ClusterCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_cluster_circuit_failure_threshold(),
            reset_timeout_ms: default_cluster_circuit_reset_timeout_ms(),
            success_threshold: default_cluster_circuit_success_threshold(),
            failure_window_ms: default_cluster_circuit_failure_window_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ClusterDistroConfig`.
pub struct ClusterDistroConfig {
    #[serde(default = "default_cluster_distro_sync_delay_ms")]
    /// Delay before syncing data to peers, in milliseconds.
    pub sync_delay_ms: i64,
    #[serde(default = "default_cluster_distro_sync_timeout_ms")]
    /// Timeout for a sync operation, in milliseconds.
    pub sync_timeout_ms: i64,
    #[serde(default = "default_cluster_distro_sync_retry_delay_ms")]
    /// Delay before retrying a failed sync, in milliseconds.
    pub sync_retry_delay_ms: i64,
    #[serde(default = "default_cluster_distro_verify_interval_ms")]
    /// How often peer data is verified, in milliseconds.
    pub verify_interval_ms: i64,
    #[serde(default = "default_cluster_distro_verify_timeout_ms")]
    /// Timeout for a verify operation, in milliseconds.
    pub verify_timeout_ms: i64,
    #[serde(default = "default_cluster_distro_load_retry_delay_ms")]
    /// Delay before retrying a failed snapshot load, in milliseconds.
    pub load_retry_delay_ms: i64,
    #[serde(default = "default_cluster_distro_load_max_retries")]
    /// Maximum number of snapshot load retries.
    pub load_max_retries: i64,
    #[serde(default)]
    /// Whether startup blocks until the initial data load succeeds.
    pub require_initial_load: bool,
}

impl Default for ClusterDistroConfig {
    fn default() -> Self {
        Self {
            sync_delay_ms: default_cluster_distro_sync_delay_ms(),
            sync_timeout_ms: default_cluster_distro_sync_timeout_ms(),
            sync_retry_delay_ms: default_cluster_distro_sync_retry_delay_ms(),
            verify_interval_ms: default_cluster_distro_verify_interval_ms(),
            verify_timeout_ms: default_cluster_distro_verify_timeout_ms(),
            load_retry_delay_ms: default_cluster_distro_load_retry_delay_ms(),
            load_max_retries: default_cluster_distro_load_max_retries(),
            require_initial_load: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ClusterHealthCheckConfig`.
pub struct ClusterHealthCheckConfig {
    #[serde(default = "default_cluster_health_check_interval_ms")]
    /// How often members are health checked, in milliseconds.
    pub interval_ms: i64,
    #[serde(default = "default_cluster_health_check_timeout_ms")]
    /// Timeout for a health check, in milliseconds.
    pub timeout_ms: i64,
    #[serde(default = "default_cluster_health_check_max_fail_count")]
    /// Number of failures before a member is marked unhealthy.
    pub max_fail_count: i64,
    #[serde(default = "default_cluster_health_check_suspicious_threshold")]
    /// Number of failures before a member is marked suspicious.
    pub suspicious_threshold: i64,
}

impl Default for ClusterHealthCheckConfig {
    fn default() -> Self {
        Self {
            interval_ms: default_cluster_health_check_interval_ms(),
            timeout_ms: default_cluster_health_check_timeout_ms(),
            max_fail_count: default_cluster_health_check_max_fail_count(),
            suspicious_threshold: default_cluster_health_check_suspicious_threshold(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ClusterMemberReportConfig`.
pub struct ClusterMemberReportConfig {
    #[serde(default = "default_cluster_member_report_interval_ms")]
    /// How often this member reports itself, in milliseconds.
    pub interval_ms: i64,
}

impl Default for ClusterMemberReportConfig {
    fn default() -> Self {
        Self {
            interval_ms: default_cluster_member_report_interval_ms(),
        }
    }
}

// ============================================================================
// Inetutils
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `InetutilsConfig`.
pub struct InetutilsConfig {
    #[serde(default)]
    /// Whether the hostname is advertised instead of the IP address.
    pub prefer_hostname_over_ip: bool,
    #[serde(default)]
    /// The IP address to advertise.
    pub ip_address: Option<String>,
}

// ============================================================================
// Member
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `MemberConfig`.
pub struct MemberConfig {
    #[serde(default)]
    /// Comma-separated list of cluster members.
    pub list: Option<String>,
}

// ============================================================================
// AI
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `AiConfig`.
pub struct AiConfig {
    #[serde(default)]
    /// MCP (Model Context Protocol) settings.
    pub mcp: AiMcpConfig,
    #[serde(default)]
    /// AI registry settings.
    pub registry: AiRegistryConfig,
    #[serde(default)]
    /// Skill settings.
    pub skill: AiSkillConfig,
    #[serde(default)]
    /// AI resource settings.
    pub resource: AiResourceConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `AiMcpConfig`.
pub struct AiMcpConfig {
    #[serde(default)]
    /// MCP registry settings.
    pub registry: AiMcpRegistryConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `AiMcpRegistryConfig`.
pub struct AiMcpRegistryConfig {
    #[serde(default)]
    /// Whether the MCP registry is enabled.
    pub enabled: bool,
    #[serde(default = "default_ai_mcp_registry_port")]
    /// The port the MCP registry listens on.
    pub port: i64,
}

impl Default for AiMcpRegistryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_ai_mcp_registry_port(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `AiRegistryConfig`.
pub struct AiRegistryConfig {
    #[serde(default = "default_ai_registry_port")]
    /// The port the AI registry listens on.
    pub port: i64,
}

impl Default for AiRegistryConfig {
    fn default() -> Self {
        Self {
            port: default_ai_registry_port(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `AiSkillConfig`.
pub struct AiSkillConfig {
    #[serde(default)]
    /// Skill registry settings.
    pub registry: AiSkillRegistryConfig,
    #[serde(default)]
    /// Settings for publishing a skill automatically after review.
    pub auto_publish_after_review: AiSkillAutoPublishConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `AiSkillRegistryConfig`.
pub struct AiSkillRegistryConfig {
    #[serde(default)]
    /// Whether the skill registry is enabled.
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `AiSkillAutoPublishConfig`.
pub struct AiSkillAutoPublishConfig {
    #[serde(default)]
    /// Whether skills are published automatically once approved.
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `AiResourceConfig`.
pub struct AiResourceConfig {
    #[serde(default)]
    /// Resource import settings.
    pub import: AiResourceImportConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `AiResourceImportConfig`.
pub struct AiResourceImportConfig {
    #[serde(default)]
    /// Whether the legacy MCP API can be used for imports.
    pub legacy_mcp_api_enabled: bool,
    #[serde(default)]
    /// Whether user-supplied URLs are allowed for imports.
    pub allow_user_url: bool,
}

// ============================================================================
// CMDB
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `CmdbConfig`.
pub struct CmdbConfig {
    #[serde(default = "default_cmdb_dump_task_interval")]
    /// How often CMDB data is dumped, in milliseconds.
    pub dump_task_interval: i64,
    #[serde(default = "default_cmdb_event_task_interval")]
    /// How often CMDB events are processed, in milliseconds.
    pub event_task_interval: i64,
    #[serde(default = "default_cmdb_label_task_interval")]
    /// How often CMDB labels are refreshed, in milliseconds.
    pub label_task_interval: i64,
    #[serde(default)]
    /// Whether CMDB data is loaded during startup.
    pub load_data_at_start: bool,
}

impl Default for CmdbConfig {
    fn default() -> Self {
        Self {
            dump_task_interval: default_cmdb_dump_task_interval(),
            event_task_interval: default_cmdb_event_task_interval(),
            label_task_interval: default_cmdb_label_task_interval(),
            load_data_at_start: false,
        }
    }
}

// ============================================================================
// Security
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `SecurityConfig`.
pub struct SecurityConfig {
    #[serde(default)]
    /// URLs excluded from security checks.
    pub ignore: SecurityIgnoreConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `SecurityIgnoreConfig`.
pub struct SecurityIgnoreConfig {
    #[serde(default)]
    /// Comma-separated list of URLs exempt from security checks.
    pub urls: Option<String>,
}

// ============================================================================
// Extension
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `ExtensionConfig`.
pub struct ExtensionConfig {
    #[serde(default)]
    /// AI extension settings.
    pub ai: ExtensionAiConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `ExtensionAiConfig`.
pub struct ExtensionAiConfig {
    #[serde(default = "default_true")]
    /// Whether the AI extension is enabled.
    pub enabled: bool,
}

impl Default for ExtensionAiConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

// ============================================================================
// Prometheus
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `PrometheusConfig`.
pub struct PrometheusConfig {
    #[serde(default)]
    /// Prometheus metrics settings.
    pub metrics: PrometheusMetricsConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Configuration for `PrometheusMetricsConfig`.
pub struct PrometheusMetricsConfig {
    #[serde(default = "default_true")]
    /// Whether the Prometheus metrics endpoint is exposed.
    pub enabled: bool,
}

impl Default for PrometheusMetricsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

// ============================================================================
// Istio
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `IstioConfig`.
pub struct IstioConfig {
    #[serde(default)]
    /// Istio MCP settings.
    pub mcp: IstioMcpConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `IstioMcpConfig`.
pub struct IstioMcpConfig {
    #[serde(default)]
    /// Istio MCP server settings.
    pub server: IstioMcpServerConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `IstioMcpServerConfig`.
pub struct IstioMcpServerConfig {
    #[serde(default)]
    /// Whether the Istio MCP server is enabled.
    pub enabled: bool,
}

// ============================================================================
// Kubernetes
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `K8sConfig`.
pub struct K8sConfig {
    #[serde(default)]
    /// Kubernetes service synchronization settings.
    pub sync: K8sSyncConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// Configuration for `K8sSyncConfig`.
pub struct K8sSyncConfig {
    #[serde(default)]
    /// Whether Kubernetes synchronization is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Whether Batata runs outside the Kubernetes cluster.
    pub outside_cluster: bool,
    #[serde(default)]
    /// Path to the kubeconfig file used for out-of-cluster access.
    pub kube_config: Option<String>,
}
