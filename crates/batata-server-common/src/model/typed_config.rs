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
pub struct BatataTypedConfig {
    #[serde(default)]
    pub standalone: bool,
    #[serde(default)]
    pub function_mode: Option<String>,
    #[serde(default)]
    pub deployment: DeploymentConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub console: ConsoleConfig,
    #[serde(default)]
    pub db: DbConfig,
    #[serde(default)]
    pub sql: SqlConfig,
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub ratelimit: RateLimitConfig,
    #[serde(default)]
    pub plugin: PluginConfig,
    #[serde(default)]
    pub config: ConfigSection,
    #[serde(default)]
    pub naming: NamingConfig,
    #[serde(default)]
    pub otel: OtelConfig,
    #[serde(default)]
    pub logs: LogsConfig,
    #[serde(default)]
    pub mesh: MeshConfig,
    #[serde(default)]
    pub raft: RaftConfig,
    #[serde(default)]
    pub remote: RemoteConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub persistence: PersistenceConfig,
    #[serde(default)]
    pub rocksdb: RocksdbConfig,
    #[serde(default)]
    pub cluster: ClusterConfig,
    #[serde(default)]
    pub inetutils: InetutilsConfig,
    #[serde(default)]
    pub member: MemberConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub cmdb: CmdbConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub extension: ExtensionConfig,
    #[serde(default)]
    pub prometheus: PrometheusConfig,
    #[serde(default)]
    pub istio: IstioConfig,
    #[serde(default)]
    pub k8s: K8sConfig,
}

// ============================================================================
// Deployment
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
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
pub struct ServerConfig {
    #[serde(default)]
    pub main: ServerMainConfig,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub context_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub address: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub http: ServerHttpConfig,
    #[serde(default)]
    pub shutdown: ServerShutdownConfig,
    #[serde(default)]
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
pub struct ServerMainConfig {
    #[serde(default = "default_server_port")]
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
pub struct ServerHttpConfig {
    #[serde(default)]
    pub workers: i64,
    #[serde(default = "default_http_keep_alive")]
    pub keep_alive: i64,
    #[serde(default = "default_http_max_payload_size")]
    pub max_payload_size: i64,
    #[serde(default = "default_http_max_json_size")]
    pub max_json_size: i64,
    #[serde(default = "default_http_client_request_timeout")]
    pub client_request_timeout: i64,
    #[serde(default)]
    pub access_log: ServerHttpAccessLogConfig,
    #[serde(default)]
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
pub struct ServerHttpAccessLogConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_access_log_max_days")]
    pub max_days: i64,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
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
pub struct ServerHttpCompressionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_compression_min_size")]
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
pub struct ServerShutdownConfig {
    #[serde(default = "default_shutdown_drain_timeout")]
    pub drain_timeout: i64,
    #[serde(default = "default_shutdown_db_close_timeout")]
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
pub struct ServerGrpcConfig {
    #[serde(default = "default_grpc_tcp_keepalive")]
    pub tcp_keepalive: i64,
    #[serde(default = "default_true")]
    pub tcp_nodelay: bool,
    #[serde(default = "default_grpc_http2_keepalive_interval")]
    pub http2_keepalive_interval: i64,
    #[serde(default = "default_grpc_http2_keepalive_timeout")]
    pub http2_keepalive_timeout: i64,
    #[serde(default = "default_grpc_concurrency_limit")]
    pub concurrency_limit: i64,
    #[serde(default = "default_grpc_connection_stale_ms")]
    pub connection_stale_ms: i64,
    #[serde(default = "default_grpc_push_message_timeout")]
    pub push_message_timeout: i64,
    #[serde(default = "default_grpc_bistream_channel_capacity")]
    pub bistream_channel_capacity: i64,
    #[serde(default)]
    pub notify_subscriber_timeout: i64,
    #[serde(default = "default_grpc_max_push_timeouts")]
    pub max_push_timeouts: i64,
    #[serde(default = "default_grpc_max_concurrent_streams")]
    pub max_concurrent_streams: i64,
    #[serde(default = "default_grpc_max_connections")]
    pub max_connections: i64,
    #[serde(default = "default_grpc_initial_connection_window_size")]
    pub initial_connection_window_size: i64,
    #[serde(default = "default_grpc_initial_stream_window_size")]
    pub initial_stream_window_size: i64,
    #[serde(default = "default_grpc_max_frame_size")]
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
pub struct ConsoleConfig {
    #[serde(default = "default_console_port")]
    pub port: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub context_path: Option<String>,
    #[serde(default)]
    pub ui: ConsoleUiConfig,
    #[serde(default)]
    pub remote: ConsoleRemoteConfig,
    #[serde(default)]
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
pub struct ConsoleUiConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
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
pub struct ConsoleRemoteConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub server_addr: Option<String>,
    #[serde(default)]
    pub server_context_path: Option<String>,
    #[serde(default = "default_console_remote_refresh_interval_secs")]
    pub refresh_interval_secs: i64,
    #[serde(default = "default_console_remote_initial_delay_secs")]
    pub initial_delay_secs: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub username: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub password: Option<String>,
    #[serde(default = "default_console_remote_connect_timeout_ms")]
    pub connect_timeout_ms: i64,
    #[serde(default = "default_console_remote_read_timeout_ms")]
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
pub struct ConsoleHttpConfig {
    #[serde(default = "default_console_http_keep_alive")]
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
pub struct DbConfig {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub pool: DbPoolConfig,
    #[serde(default)]
    pub migration: DbMigrationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbPoolConfig {
    #[serde(default = "default_db_pool_max_connections")]
    pub max_connections: i64,
    #[serde(default = "default_db_pool_min_connections")]
    pub min_connections: i64,
    #[serde(default = "default_db_pool_connect_timeout")]
    pub connect_timeout: i64,
    #[serde(default = "default_db_pool_acquire_timeout")]
    pub acquire_timeout: i64,
    #[serde(default = "default_db_pool_idle_timeout")]
    pub idle_timeout: i64,
    #[serde(default = "default_db_pool_max_lifetime")]
    pub max_lifetime: i64,
    #[serde(default)]
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
pub struct DbMigrationConfig {
    #[serde(default = "default_true")]
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
pub struct SqlConfig {
    #[serde(default)]
    pub init: SqlInitConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SqlInitConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub platform: Option<String>,
}

// ============================================================================
// Core
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreConfig {
    #[serde(default)]
    pub auth: CoreAuthConfig,
    #[serde(default)]
    pub snowflake: CoreSnowflakeConfig,
    #[serde(default)]
    pub member: CoreMemberConfig,
    #[serde(default)]
    pub address_server: CoreAddressServerConfig,
    #[serde(default)]
    pub api: CoreApiConfig,
}

// --- Core: Auth ---

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub admin: CoreAuthAdminConfig,
    #[serde(default)]
    pub console: CoreAuthConsoleConfig,
    #[serde(default)]
    pub caching: CoreAuthCachingConfig,
    #[serde(default)]
    pub system: CoreAuthSystemConfig,
    #[serde(default)]
    pub server: CoreAuthServerConfig,
    #[serde(default)]
    pub plugin: CoreAuthPluginConfig,
    #[serde(default)]
    pub ldap: CoreAuthLdapConfig,
    #[serde(default)]
    pub oauth: CoreAuthOauthConfig,
    #[serde(default)]
    pub cache: CoreAuthCacheConfig,
    #[serde(default)]
    pub default: CoreAuthDefaultConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthAdminConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAuthConsoleConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for CoreAuthConsoleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAuthCachingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for CoreAuthCachingConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAuthSystemConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
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
pub struct CoreAuthServerConfig {
    #[serde(default)]
    pub identity: CoreAuthServerIdentityConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthServerIdentityConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthPluginConfig {
    #[serde(default)]
    pub default: CoreAuthPluginDefaultConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthPluginDefaultConfig {
    #[serde(default)]
    pub token: CoreAuthPluginDefaultTokenConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthPluginDefaultTokenConfig {
    #[serde(default)]
    pub expire: CoreAuthPluginDefaultTokenExpireConfig,
    #[serde(default)]
    pub secret: CoreAuthPluginDefaultTokenSecretConfig,
    #[serde(default)]
    pub cache: CoreAuthPluginDefaultTokenCacheConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAuthPluginDefaultTokenExpireConfig {
    #[serde(default = "default_token_expire_seconds")]
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
pub struct CoreAuthPluginDefaultTokenSecretConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthPluginDefaultTokenCacheConfig {
    #[serde(default)]
    pub enable: bool,
}

// --- Core: Auth LDAP ---

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAuthLdapConfig {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub base_dc: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub bind_dn: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub password: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub user_dn_pattern: Option<String>,
    #[serde(default)]
    pub filter: CoreAuthLdapFilterConfig,
    #[serde(default = "default_ldap_timeout")]
    pub timeout: i64,
    #[serde(default)]
    pub case: CoreAuthLdapCaseConfig,
    #[serde(default)]
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
pub struct CoreAuthLdapFilterConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
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
pub struct CoreAuthLdapCaseConfig {
    #[serde(default = "default_true")]
    pub sensitive: bool,
}

impl Default for CoreAuthLdapCaseConfig {
    fn default() -> Self {
        Self { sensitive: true }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthLdapIgnoreConfig {
    #[serde(default)]
    pub partial: CoreAuthLdapIgnorePartialConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthLdapIgnorePartialConfig {
    #[serde(default)]
    pub result: CoreAuthLdapIgnorePartialResultConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthLdapIgnorePartialResultConfig {
    #[serde(default)]
    pub exception: bool,
}

// --- Core: Auth OAuth ---

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAuthOauthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub user: CoreAuthOauthUserConfig,
    #[serde(default)]
    pub role: CoreAuthOauthRoleConfig,
    #[serde(default)]
    pub redirect: CoreAuthOauthRedirectConfig,
    #[serde(default)]
    pub cache: CoreAuthOauthCacheConfig,
    #[serde(default = "default_oauth_http_timeout_secs")]
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
pub struct CoreAuthOauthUserConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
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
pub struct CoreAuthOauthRoleConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
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
pub struct CoreAuthOauthRedirectConfig {
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAuthOauthCacheConfig {
    #[serde(default = "default_oauth_discovery_ttl_secs")]
    pub discovery_ttl_secs: i64,
    #[serde(default = "default_oauth_discovery_capacity")]
    pub discovery_capacity: i64,
    #[serde(default = "default_oauth_state_ttl_secs")]
    pub state_ttl_secs: i64,
    #[serde(default = "default_oauth_state_capacity")]
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
pub struct CoreAuthCacheConfig {
    #[serde(default = "default_auth_token_capacity")]
    pub token_capacity: i64,
    #[serde(default = "default_auth_token_ttl_secs")]
    pub token_ttl_secs: i64,
    #[serde(default = "default_auth_roles_capacity")]
    pub roles_capacity: i64,
    #[serde(default = "default_auth_permissions_capacity")]
    pub permissions_capacity: i64,
    #[serde(default = "default_auth_blacklist_capacity")]
    pub blacklist_capacity: i64,
    #[serde(default = "default_auth_blacklist_ttl_secs")]
    pub blacklist_ttl_secs: i64,
    #[serde(default = "default_grpc_permission_capacity")]
    pub grpc_permission_capacity: i64,
    #[serde(default = "default_grpc_permission_ttl_secs")]
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
pub struct CoreAuthDefaultConfig {
    #[serde(default)]
    pub anonymous: CoreAuthDefaultAnonymousConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthDefaultAnonymousConfig {
    #[serde(default)]
    pub ai: CoreAuthDefaultAnonymousAiConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreAuthDefaultAnonymousAiConfig {
    #[serde(default)]
    pub enabled: bool,
}

// --- Core: Snowflake ---

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreSnowflakeConfig {
    #[serde(default)]
    pub worker_id: Option<i64>,
}

// --- Core: Member ---

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreMemberConfig {
    #[serde(default)]
    pub lookup: CoreMemberLookupConfig,
    #[serde(default)]
    pub meta: CoreMemberMetaConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreMemberLookupConfig {
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreMemberMetaConfig {
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub adweight: Option<String>,
    #[serde(default)]
    pub weight: Option<String>,
}

// --- Core: Address Server ---

#[derive(Debug, Clone, Deserialize)]
pub struct CoreAddressServerConfig {
    #[serde(default = "default_address_server_retry")]
    pub retry: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub domain: Option<String>,
    #[serde(default = "default_address_server_port")]
    pub port: i64,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
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
pub struct CoreApiConfig {
    #[serde(default)]
    pub compatibility: CoreApiCompatibilityConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreApiCompatibilityConfig {
    #[serde(default)]
    pub client: CoreApiCompatibilityClientConfig,
    #[serde(default)]
    pub admin: CoreApiCompatibilityAdminConfig,
    #[serde(default)]
    pub console: CoreApiCompatibilityConsoleConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreApiCompatibilityClientConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for CoreApiCompatibilityClientConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreApiCompatibilityAdminConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreApiCompatibilityConsoleConfig {
    #[serde(default)]
    pub enabled: bool,
}

// ============================================================================
// Rate Limiting
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ratelimit_max_requests")]
    pub max_requests: i64,
    #[serde(default = "default_ratelimit_window_seconds")]
    pub window_seconds: i64,
    #[serde(default)]
    pub auth: RateLimitAuthConfig,
    #[serde(default = "default_ratelimit_max_tracked_ips")]
    pub max_tracked_ips: i64,
    #[serde(default = "default_ratelimit_cleanup_interval_secs")]
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
pub struct RateLimitAuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ratelimit_auth_max_attempts")]
    pub max_attempts: i64,
    #[serde(default = "default_ratelimit_auth_window_seconds")]
    pub window_seconds: i64,
    #[serde(default = "default_ratelimit_auth_lockout_seconds")]
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
pub struct PluginConfig {
    #[serde(default)]
    pub control: PluginControlConfig,
    #[serde(default)]
    pub consul: PluginConsulConfig,
    #[serde(default)]
    pub apollo: PluginApolloConfig,
    #[serde(default)]
    pub visibility: PluginVisibilityConfig,
    #[serde(default)]
    pub datasource: PluginDatasourceConfig,
    #[serde(default)]
    pub webhook: PluginWebhookConfig,
}

// --- Plugin: Control ---

#[derive(Debug, Clone, Deserialize)]
pub struct PluginControlConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_control_default_tps")]
    pub default_tps: i64,
    #[serde(default = "default_control_max_connections")]
    pub max_connections: i64,
    #[serde(default)]
    pub manager: PluginControlManagerConfig,
    #[serde(default)]
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
pub struct PluginControlManagerConfig {
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginControlRuleConfig {
    #[serde(default)]
    pub local: PluginControlRuleLocalConfig,
    #[serde(default)]
    pub external: PluginControlRuleExternalConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginControlRuleLocalConfig {
    #[serde(default)]
    pub basedir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginControlRuleExternalConfig {
    #[serde(default)]
    pub storage: Option<String>,
}

// --- Plugin: Consul ---

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConsulConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_consul_port")]
    pub port: i64,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub datacenter: Option<String>,
    #[serde(default)]
    pub primary_datacenter: Option<String>,
    #[serde(default)]
    pub node_name: Option<String>,
    #[serde(default)]
    pub register_self: bool,
    #[serde(default)]
    pub acl: PluginConsulAclConfig,
    #[serde(default = "default_consul_check_reap_interval")]
    pub check_reap_interval: i64,
    #[serde(default)]
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
pub struct PluginConsulAclConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub tokens: PluginConsulAclTokensConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginConsulAclTokensConfig {
    #[serde(default)]
    pub initial_management: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConsulClientConfig {
    #[serde(default = "default_consul_connect_timeout_secs")]
    pub connect_timeout_secs: i64,
    #[serde(default = "default_consul_read_timeout_secs")]
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
pub struct PluginApolloConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_apollo_port")]
    pub port: i64,
    #[serde(default)]
    pub http: PluginApolloHttpConfig,
}

impl Default for PluginApolloConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_apollo_port(),
            http: PluginApolloHttpConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginApolloHttpConfig {
    #[serde(default)]
    pub workers: i64,
}

// --- Plugin: Visibility ---

#[derive(Debug, Clone, Deserialize)]
pub struct PluginVisibilityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
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
pub struct PluginDatasourceConfig {
    #[serde(default)]
    pub log: PluginDatasourceLogConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginDatasourceLogConfig {
    #[serde(default)]
    pub enabled: bool,
}

// --- Plugin: Webhook ---

#[derive(Debug, Clone, Deserialize)]
pub struct PluginWebhookConfig {
    #[serde(default = "default_webhook_default_timeout_secs")]
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
pub struct ConfigSection {
    #[serde(default)]
    pub retention: ConfigRetentionConfig,
    #[serde(default)]
    pub gray: ConfigGrayConfig,
    #[serde(default)]
    pub encryption: ConfigEncryptionConfig,
    #[serde(default)]
    pub push: ConfigPushConfig,
    #[serde(default)]
    pub plugin: ConfigPluginConfig,
    #[serde(default)]
    pub read_cache_ttl: i64,
    #[serde(default = "default_config_read_cache_max_entries")]
    pub read_cache_max_entries: i64,
    #[serde(default)]
    pub notify: ConfigNotifyConfig,
    #[serde(default)]
    pub health_check: ConfigHealthCheckConfig,
    #[serde(default = "default_max_content")]
    pub max_content: i64,
    #[serde(default)]
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
pub struct ConfigNotifyConfig {
    #[serde(default = "default_notify_connect_timeout")]
    pub connect_timeout: i64,
    #[serde(default = "default_notify_socket_timeout")]
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
pub struct ConfigHealthCheckConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_health_check_fail_count")]
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
pub struct ConfigCapacityConfig {
    #[serde(default = "default_true")]
    pub manage_enabled: bool,
    #[serde(default)]
    pub limit_check: bool,
    #[serde(default = "default_capacity_default_cluster_quota")]
    pub default_cluster_quota: i64,
    #[serde(default = "default_capacity_default_group_quota")]
    pub default_group_quota: i64,
    #[serde(default)]
    pub default_tenant_quota: Option<i64>,
    #[serde(default = "default_capacity_default_max_size")]
    pub default_max_size: i64,
    #[serde(default = "default_capacity_default_max_aggr_count")]
    pub default_max_aggr_count: i64,
    #[serde(default = "default_capacity_default_max_aggr_size")]
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
pub struct ConfigRetentionConfig {
    #[serde(default = "default_config_retention_days")]
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
pub struct ConfigGrayConfig {
    #[serde(default)]
    pub version: ConfigGrayVersionConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigGrayVersionConfig {
    #[serde(default = "default_config_gray_max_count")]
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
pub struct ConfigEncryptionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub plugin: ConfigEncryptionPluginConfig,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub reload: ConfigEncryptionReloadConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigEncryptionPluginConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none", rename = "type")]
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
pub struct ConfigEncryptionReloadConfig {
    #[serde(default)]
    pub interval: ConfigEncryptionReloadIntervalConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConfigEncryptionReloadIntervalConfig {
    #[serde(default)]
    pub ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigPushConfig {
    #[serde(default = "default_config_push_max_retry_time")]
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
pub struct ConfigPluginConfig {
    #[serde(default)]
    pub webhook: ConfigPluginWebhookConfig,
    #[serde(default)]
    pub whitelist: ConfigPluginWhitelistConfig,
    #[serde(default)]
    pub fileformatcheck: ConfigPluginFileformatcheckConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigPluginWebhookConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_config_webhook_content_max_capacity")]
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
pub struct ConfigPluginWhitelistConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub suffixes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConfigPluginFileformatcheckConfig {
    #[serde(default)]
    pub enabled: bool,
}

// ============================================================================
// Naming
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct NamingConfig {
    #[serde(default = "default_true")]
    pub expire_instance: bool,
    #[serde(default)]
    pub data: NamingDataConfig,
    #[serde(default)]
    pub healthcheck: NamingHealthcheckConfig,
    #[serde(default)]
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
pub struct NamingDataConfig {
    #[serde(default)]
    pub warmup: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamingHealthcheckConfig {
    #[serde(default = "default_naming_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: i64,
    #[serde(default = "default_naming_ttl_monitor_interval_secs")]
    pub ttl_monitor_interval_secs: i64,
    #[serde(default = "default_naming_deregister_monitor_interval_secs")]
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
pub struct NamingEmptyServiceConfig {
    #[serde(default = "default_true")]
    pub auto_clean: bool,
    #[serde(default)]
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
pub struct NamingEmptyServiceCleanConfig {
    #[serde(default = "default_naming_clean_initial_delay_ms")]
    pub initial_delay_ms: i64,
    #[serde(default = "default_naming_clean_period_time_ms")]
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
pub struct OtelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub endpoint: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub service_name: Option<String>,
    #[serde(default = "default_otel_sampling_ratio")]
    pub sampling_ratio: f64,
    #[serde(default = "default_otel_export_timeout_secs")]
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
pub struct LogsConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub console: LogsConsoleConfig,
    #[serde(default)]
    pub file: LogsFileConfig,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
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
pub struct LogsConsoleConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for LogsConsoleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogsFileConfig {
    #[serde(default = "default_true")]
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
pub struct MeshConfig {
    #[serde(default)]
    pub xds: MeshXdsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeshXdsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mesh_xds_port")]
    pub port: i64,
    #[serde(default)]
    pub server: MeshXdsServerConfig,
    #[serde(default)]
    pub sync: MeshXdsSyncConfig,
    #[serde(default)]
    pub generate: MeshXdsGenerateConfig,
    #[serde(default)]
    pub default: MeshXdsDefaultConfig,
    #[serde(default)]
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
pub struct MeshXdsServerConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
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
pub struct MeshXdsSyncConfig {
    #[serde(default)]
    pub interval: MeshXdsSyncIntervalConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeshXdsSyncIntervalConfig {
    #[serde(default = "default_mesh_xds_sync_interval_ms")]
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
pub struct MeshXdsGenerateConfig {
    #[serde(default = "default_true")]
    pub listeners: bool,
    #[serde(default = "default_true")]
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
pub struct MeshXdsDefaultConfig {
    #[serde(default)]
    pub listener: MeshXdsDefaultListenerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeshXdsDefaultListenerConfig {
    #[serde(default = "default_mesh_xds_default_listener_port")]
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
pub struct MeshXdsTlsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cert: MeshXdsTlsCertConfig,
    #[serde(default)]
    pub key: MeshXdsTlsKeyConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MeshXdsTlsCertConfig {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MeshXdsTlsKeyConfig {
    #[serde(default)]
    pub path: Option<String>,
}

// ============================================================================
// Raft
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct RaftConfig {
    #[serde(default = "default_raft_election_timeout_ms")]
    pub election_timeout_ms: i64,
    #[serde(default = "default_raft_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: i64,
    #[serde(default = "default_raft_rpc_timeout_ms")]
    pub rpc_timeout_ms: i64,
    #[serde(default = "default_raft_snapshot_threshold")]
    pub snapshot_threshold: i64,
    #[serde(default = "default_raft_snapshot_transfer_timeout_ms")]
    pub snapshot_transfer_timeout_ms: i64,
    #[serde(default)]
    pub forward: RaftForwardConfig,
    #[serde(default = "default_raft_peer_connect_timeout_secs")]
    pub peer_connect_timeout_secs: i64,
    #[serde(default = "default_raft_peer_connect_retry_interval_ms")]
    pub peer_connect_retry_interval_ms: i64,
    #[serde(default)]
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
pub struct RaftForwardConfig {
    #[serde(default = "default_raft_forward_max_retries")]
    pub max_retries: i64,
    #[serde(default = "default_raft_forward_initial_delay_ms")]
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
pub struct RaftGrpcConfig {
    #[serde(default = "default_raft_grpc_tcp_keepalive")]
    pub tcp_keepalive: i64,
    #[serde(default = "default_true")]
    pub tcp_nodelay: bool,
    #[serde(default = "default_raft_grpc_http2_keepalive_interval")]
    pub http2_keepalive_interval: i64,
    #[serde(default = "default_raft_grpc_http2_keepalive_timeout")]
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
pub struct RemoteConfig {
    #[serde(default)]
    pub server: RemoteServerConfig,
    #[serde(default)]
    pub client: RemoteClientConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerConfig {
    #[serde(default)]
    pub grpc: RemoteServerGrpcConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerGrpcConfig {
    #[serde(default)]
    pub sdk: RemoteServerGrpcSdkConfig,
    #[serde(default)]
    pub cluster: RemoteServerGrpcClusterConfig,
    #[serde(default)]
    pub tls: RemoteServerGrpcTlsConfig,
}

// --- Remote: Server gRPC SDK ---

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteServerGrpcSdkConfig {
    #[serde(default)]
    pub tls: RemoteServerGrpcSdkTlsConfig,
    #[serde(default = "default_remote_max_inbound_message_size")]
    pub max_inbound_message_size: i64,
    #[serde(default = "default_remote_keep_alive_time")]
    pub keep_alive_time: i64,
    #[serde(default = "default_remote_keep_alive_timeout")]
    pub keep_alive_timeout: i64,
    #[serde(default = "default_remote_permit_keep_alive_time")]
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
pub struct RemoteServerGrpcSdkTlsConfig {
    #[serde(default)]
    pub enabled: bool,
}

// --- Remote: Server gRPC Cluster ---

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteServerGrpcClusterConfig {
    #[serde(default)]
    pub tls: RemoteServerGrpcClusterTlsConfig,
    #[serde(default = "default_remote_cluster_connect_timeout")]
    pub connect_timeout: i64,
    #[serde(default = "default_remote_cluster_request_timeout")]
    pub request_timeout: i64,
    #[serde(default = "default_remote_cluster_max_retries")]
    pub max_retries: i64,
    #[serde(default = "default_remote_cluster_retry_delay")]
    pub retry_delay: i64,
    #[serde(default = "default_remote_cluster_idle_timeout")]
    pub idle_timeout: i64,
    #[serde(default = "default_remote_max_inbound_message_size")]
    pub max_inbound_message_size: i64,
    #[serde(default = "default_remote_keep_alive_time")]
    pub keep_alive_time: i64,
    #[serde(default = "default_remote_keep_alive_timeout")]
    pub keep_alive_timeout: i64,
    #[serde(default = "default_remote_permit_keep_alive_time")]
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
pub struct RemoteServerGrpcClusterTlsConfig {
    #[serde(default)]
    pub enabled: bool,
}

// --- Remote: Server gRPC TLS ---

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerGrpcTlsConfig {
    #[serde(default)]
    pub cert: RemoteServerGrpcTlsCertConfig,
    #[serde(default)]
    pub key: RemoteServerGrpcTlsKeyConfig,
    #[serde(default)]
    pub ca: RemoteServerGrpcTlsCaConfig,
    #[serde(default)]
    pub mtls: RemoteServerGrpcTlsMtlsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerGrpcTlsCertConfig {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerGrpcTlsKeyConfig {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerGrpcTlsCaConfig {
    #[serde(default)]
    pub cert: RemoteServerGrpcTlsCaCertConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerGrpcTlsCaCertConfig {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteServerGrpcTlsMtlsConfig {
    #[serde(default)]
    pub enabled: bool,
}

// --- Remote: Client ---

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientConfig {
    #[serde(default)]
    pub grpc: RemoteClientGrpcConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientGrpcConfig {
    #[serde(default)]
    pub cluster: RemoteClientGrpcClusterConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientGrpcClusterConfig {
    #[serde(default)]
    pub tls: RemoteClientGrpcClusterTlsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientGrpcClusterTlsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cert: RemoteClientGrpcClusterTlsCertConfig,
    #[serde(default)]
    pub key: RemoteClientGrpcClusterTlsKeyConfig,
    #[serde(default)]
    pub ca: RemoteClientGrpcClusterTlsCaConfig,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientGrpcClusterTlsCertConfig {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientGrpcClusterTlsKeyConfig {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientGrpcClusterTlsCaConfig {
    #[serde(default)]
    pub cert: RemoteClientGrpcClusterTlsCaCertConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteClientGrpcClusterTlsCaCertConfig {
    #[serde(default)]
    pub path: Option<String>,
}

// ============================================================================
// Metrics
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MetricsConfig {
    #[serde(default)]
    pub system_stats: MetricsSystemStatsConfig,
    #[serde(default)]
    pub export: MetricsExportConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsSystemStatsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_metrics_system_stats_interval_secs")]
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
pub struct MetricsExportConfig {
    #[serde(default)]
    pub elastic: MetricsExportElasticConfig,
    #[serde(default)]
    pub influx: MetricsExportInfluxConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MetricsExportElasticConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub host: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsExportInfluxConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub db: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default = "default_true")]
    pub auto_create_db: bool,
    #[serde(default)]
    pub consistency: Option<String>,
    #[serde(default = "default_true")]
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
pub struct PersistenceConfig {
    #[serde(default)]
    pub embedded: PersistenceEmbeddedConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersistenceEmbeddedConfig {
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub data_dir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
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
pub struct RocksdbConfig {
    #[serde(default = "default_rocksdb_write_buffer_mb")]
    pub write_buffer_mb: i64,
    #[serde(default = "default_rocksdb_max_write_buffers")]
    pub max_write_buffers: i64,
    #[serde(default = "default_rocksdb_max_background_jobs")]
    pub max_background_jobs: i64,
    #[serde(default = "default_rocksdb_block_cache_mb")]
    pub block_cache_mb: i64,
    #[serde(default = "default_rocksdb_bloom_filter_bits")]
    pub bloom_filter_bits: f64,
    #[serde(default = "default_true")]
    pub level_compaction_dynamic: bool,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub bottommost_compression: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_none")]
    pub compression: Option<String>,
    #[serde(default)]
    pub enable_statistics: bool,
    #[serde(default = "default_true")]
    pub whole_key_filtering: bool,
    #[serde(default = "default_rocksdb_data_block_hash_ratio")]
    pub data_block_hash_ratio: f64,
    #[serde(default)]
    pub sm_sync: bool,
    #[serde(default)]
    pub sm_disable_wal: bool,
    #[serde(default)]
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
pub struct ClusterConfig {
    #[serde(default)]
    pub circuit_breaker: ClusterCircuitBreakerConfig,
    #[serde(default)]
    pub distro: ClusterDistroConfig,
    #[serde(default)]
    pub health_check: ClusterHealthCheckConfig,
    #[serde(default = "default_cluster_event_queue_size")]
    pub event_queue_size: i64,
    #[serde(default)]
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
pub struct ClusterCircuitBreakerConfig {
    #[serde(default = "default_cluster_circuit_failure_threshold")]
    pub failure_threshold: i64,
    #[serde(default = "default_cluster_circuit_reset_timeout_ms")]
    pub reset_timeout_ms: i64,
    #[serde(default = "default_cluster_circuit_success_threshold")]
    pub success_threshold: i64,
    #[serde(default = "default_cluster_circuit_failure_window_ms")]
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
pub struct ClusterDistroConfig {
    #[serde(default = "default_cluster_distro_sync_delay_ms")]
    pub sync_delay_ms: i64,
    #[serde(default = "default_cluster_distro_sync_timeout_ms")]
    pub sync_timeout_ms: i64,
    #[serde(default = "default_cluster_distro_sync_retry_delay_ms")]
    pub sync_retry_delay_ms: i64,
    #[serde(default = "default_cluster_distro_verify_interval_ms")]
    pub verify_interval_ms: i64,
    #[serde(default = "default_cluster_distro_verify_timeout_ms")]
    pub verify_timeout_ms: i64,
    #[serde(default = "default_cluster_distro_load_retry_delay_ms")]
    pub load_retry_delay_ms: i64,
    #[serde(default = "default_cluster_distro_load_max_retries")]
    pub load_max_retries: i64,
    #[serde(default)]
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
pub struct ClusterHealthCheckConfig {
    #[serde(default = "default_cluster_health_check_interval_ms")]
    pub interval_ms: i64,
    #[serde(default = "default_cluster_health_check_timeout_ms")]
    pub timeout_ms: i64,
    #[serde(default = "default_cluster_health_check_max_fail_count")]
    pub max_fail_count: i64,
    #[serde(default = "default_cluster_health_check_suspicious_threshold")]
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
pub struct ClusterMemberReportConfig {
    #[serde(default = "default_cluster_member_report_interval_ms")]
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
pub struct InetutilsConfig {
    #[serde(default)]
    pub prefer_hostname_over_ip: bool,
    #[serde(default)]
    pub ip_address: Option<String>,
}

// ============================================================================
// Member
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MemberConfig {
    #[serde(default)]
    pub list: Option<String>,
}

// ============================================================================
// AI
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub mcp: AiMcpConfig,
    #[serde(default)]
    pub registry: AiRegistryConfig,
    #[serde(default)]
    pub skill: AiSkillConfig,
    #[serde(default)]
    pub resource: AiResourceConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiMcpConfig {
    #[serde(default)]
    pub registry: AiMcpRegistryConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiMcpRegistryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ai_mcp_registry_port")]
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
pub struct AiRegistryConfig {
    #[serde(default = "default_ai_registry_port")]
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
pub struct AiSkillConfig {
    #[serde(default)]
    pub registry: AiSkillRegistryConfig,
    #[serde(default)]
    pub auto_publish_after_review: AiSkillAutoPublishConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiSkillRegistryConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiSkillAutoPublishConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiResourceConfig {
    #[serde(default)]
    pub import: AiResourceImportConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiResourceImportConfig {
    #[serde(default)]
    pub legacy_mcp_api_enabled: bool,
    #[serde(default)]
    pub allow_user_url: bool,
}

// ============================================================================
// CMDB
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct CmdbConfig {
    #[serde(default = "default_cmdb_dump_task_interval")]
    pub dump_task_interval: i64,
    #[serde(default = "default_cmdb_event_task_interval")]
    pub event_task_interval: i64,
    #[serde(default = "default_cmdb_label_task_interval")]
    pub label_task_interval: i64,
    #[serde(default)]
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
pub struct SecurityConfig {
    #[serde(default)]
    pub ignore: SecurityIgnoreConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SecurityIgnoreConfig {
    #[serde(default)]
    pub urls: Option<String>,
}

// ============================================================================
// Extension
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtensionConfig {
    #[serde(default)]
    pub ai: ExtensionAiConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionAiConfig {
    #[serde(default = "default_true")]
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
pub struct PrometheusConfig {
    #[serde(default)]
    pub metrics: PrometheusMetricsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrometheusMetricsConfig {
    #[serde(default = "default_true")]
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
pub struct IstioConfig {
    #[serde(default)]
    pub mcp: IstioMcpConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IstioMcpConfig {
    #[serde(default)]
    pub server: IstioMcpServerConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IstioMcpServerConfig {
    #[serde(default)]
    pub enabled: bool,
}

// ============================================================================
// Kubernetes
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct K8sConfig {
    #[serde(default)]
    pub sync: K8sSyncConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct K8sSyncConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub outside_cluster: bool,
    #[serde(default)]
    pub kube_config: Option<String>,
}
