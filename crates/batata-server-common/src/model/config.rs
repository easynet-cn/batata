//! Configuration management for Batata server
//!
//! This module handles loading and accessing application configuration.

use std::time::Duration;

use clap::Parser;
use config::Config;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use super::typed_config;
use super::constants::{
    DEPLOYMENT_TYPE, DEPLOYMENT_TYPE_CONSOLE, FUNCTION_MODE_PROPERTY_NAME,
    STANDALONE_MODE_PROPERTY_NAME,
};

use batata_api::model::{CLUSTER_GRPC_PORT_DEFAULT_OFFSET, SDK_GRPC_PORT_DEFAULT_OFFSET};

/// Command line arguments for the server
#[derive(Debug, Parser)]
#[command()]
struct Cli {
    #[arg(short = 'm', long = "mode")]
    mode: Option<String>,
    #[arg(short = 'f', long = "function_mode")]
    function_mode: Option<String>,
    #[arg(short = 'd', long = "deployment")]
    deployment: Option<String>,
    #[arg(long = "db-url", env = "DATABASE_URL")]
    database_url: Option<String>,
    #[arg(short = 'c', long = "config")]
    config_file: Option<String>,
}

/// Application configuration loaded from config files and environment
#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub config: Config,
    pub typed: typed_config::BatataTypedConfig,
}

/// Extract `--dotted.key=value` property overrides from command-line arguments.
///
/// Any argument matching `--<key>=<value>` where `<key>` contains a `.` is treated
/// as a property override and removed from the args list so clap won't reject it.
///
/// Returns `(property_overrides, filtered_args)`.
fn extract_property_overrides() -> (Vec<(String, String)>, Vec<String>) {
    let mut overrides = Vec::new();
    let mut filtered_args = Vec::new();

    for arg in std::env::args() {
        if let Some(rest) = arg.strip_prefix("--")
            && let Some((key, value)) = rest.split_once('=')
            && key.contains('.')
        {
            overrides.push((key.to_string(), value.to_string()));
            continue;
        }
        filtered_args.push(arg);
    }

    (overrides, filtered_args)
}

/// Try to parse a string value as bool, int, or float (for env var type coercion).
fn try_parse_env_value(s: &str) -> config::Value {
    if s.eq_ignore_ascii_case("true") {
        return true.into();
    }
    if s.eq_ignore_ascii_case("false") {
        return false.into();
    }
    if let Ok(i) = s.parse::<i64>() {
        return i.into();
    }
    if let Ok(f) = s.parse::<f64>() {
        return f.into();
    }
    s.into()
}

/// Collect environment variable overrides for a given prefix, mapped to config keys.
///
/// `BATATA_*`: prefix is mapped to `batata.` (e.g., `BATATA_SERVER_MAIN_PORT` → `batata.server.main.port`)
///
/// Returns sorted Vec to ensure deterministic override order.
fn collect_env_overrides(prefix: &str) -> Vec<(String, config::Value)> {
    let prefix_with_sep = format!("{prefix}_");
    let mut overrides: Vec<(String, config::Value)> = std::env::vars()
        .filter_map(|(key, value)| {
            let rest = key.strip_prefix(&prefix_with_sep)?;
            let config_key = format!(
                "{}.{}",
                prefix.to_lowercase(),
                rest.to_lowercase().replace('_', ".")
            );
            Some((config_key, try_parse_env_value(&value)))
        })
        .collect();
    overrides.sort_by(|a, b| a.0.cmp(&b.0));
    overrides
}

impl Configuration {
    pub fn new() -> anyhow::Result<Self> {
        // Step 1: Extract --dotted.key=value overrides before clap sees them
        let (property_overrides, filtered_args) = extract_property_overrides();

        // Step 2: Parse clap from filtered args
        let args = Cli::parse_from(filtered_args);

        // Step 3: Load YAML config file
        let config_file = args
            .config_file
            .as_deref()
            .unwrap_or("conf/application.yml");

        // Step 4: Build config with layered sources (lowest to highest priority)
        let mut config_builder = Config::builder()
            // Priority 4 (lowest): YAML config file
            .add_source(config::File::with_name(config_file));

        // Priority 3: BATATA_* env vars (manual processing)
        let batata_env = collect_env_overrides("BATATA");

        for (key, value) in &batata_env {
            config_builder = config_builder
                .set_override(key, value.clone())
                .map_err(|e| {
                    anyhow::anyhow!("Failed to set BATATA_ env override for {key}: {e}")
                })?;
        }

        // Priority 2: Convenience CLI args
        if let Some(v) = args.mode {
            config_builder = config_builder
                .set_override(STANDALONE_MODE_PROPERTY_NAME, v == "standalone")
                .map_err(|e| anyhow::anyhow!("Failed to set standalone mode override: {e}"))?;
        }
        if let Some(v) = args.function_mode {
            config_builder = config_builder
                .set_override(FUNCTION_MODE_PROPERTY_NAME, v)
                .map_err(|e| anyhow::anyhow!("Failed to set function mode override: {e}"))?;
        }
        if let Some(v) = args.deployment {
            config_builder = config_builder
                .set_override(DEPLOYMENT_TYPE, v)
                .map_err(|e| anyhow::anyhow!("Failed to set deployment type override: {e}"))?;
        }
        if let Some(v) = args.database_url {
            config_builder = config_builder
                .set_override("batata.db.url", v)
                .map_err(|e| anyhow::anyhow!("Failed to set database URL override: {e}"))?;
        }

        // Priority 1 (highest): --dotted.key=value property overrides
        for (key, value) in property_overrides {
            config_builder = config_builder
                .set_override(&key, value)
                .map_err(|e| anyhow::anyhow!("Failed to set override for {key}: {e}"))?;
        }

        let app_config = config_builder
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build configuration: {e}"))?;

        let typed: typed_config::BatataTypedConfig = app_config
            .get("batata")
            .unwrap_or_default();

        Ok(Configuration { config: app_config, typed })
    }

    // ========================================================================
    // Deployment Configuration
    // ========================================================================

    pub fn deployment_type(&self) -> String {
        self.typed.deployment.type_.clone()
    }

    pub fn is_standalone(&self) -> bool {
        self.typed.standalone
    }

    pub fn startup_mode(&self) -> String {
        if self.is_standalone() {
            "standalone".to_string()
        } else {
            "cluster".to_string()
        }
    }

    pub fn function_mode(&self) -> Option<String> {
        self.typed.function_mode.clone()
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn compat_version(&self) -> String {
        self.config.get_string("nacos.version").unwrap_or_default()
    }

    pub fn batata_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    // ========================================================================
    // Server Configuration
    // ========================================================================

    pub fn server_address(&self) -> String {
        self.typed.server.address.clone()
    }

    pub fn server_main_port(&self) -> u16 {
        self.typed.server.main.port as u16
    }

    pub fn server_context_path(&self) -> String {
        self.typed.server.context_path.clone()
    }

    pub fn sdk_server_port(&self) -> u16 {
        self.server_main_port() + SDK_GRPC_PORT_DEFAULT_OFFSET
    }

    pub fn cluster_server_port(&self) -> u16 {
        self.server_main_port() + CLUSTER_GRPC_PORT_DEFAULT_OFFSET
    }

    pub fn raft_port(&self) -> u16 {
        self.server_main_port() - batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT
    }

    /// Read raw cluster member addresses from config or cluster.conf.
    /// Returns addresses in `ip:port` format (with optional `?raft_port=xxx` params).
    pub fn cluster_member_addresses(&self) -> Vec<String> {
        // 1. Try batata.member.list
        let addresses: Vec<String> = self
            .config
            .get_string("batata.member.list")
            .ok()
            .map(|list| {
                list.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        if !addresses.is_empty() {
            return addresses;
        }

        // 2. Fall back to conf/cluster.conf
        let path = std::path::Path::new("conf/cluster.conf");
        if let Ok(content) = std::fs::read_to_string(path) {
            return content
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .collect();
        }

        Vec::new()
    }

    // ========================================================================
    // Console Configuration
    // ========================================================================

    pub fn console_server_port(&self) -> u16 {
        self.typed.console.port as u16
    }

    pub fn console_server_context_path(&self) -> String {
        self.typed.console.context_path.clone()
    }

    pub fn console_ui_enabled(&self) -> bool {
        self.typed.console.ui.enabled
    }

    /// Check if console is in remote mode.
    /// Derived from deployment type: `console` deployment → remote mode.
    pub fn is_console_remote_mode(&self) -> bool {
        self.deployment_type() == DEPLOYMENT_TYPE_CONSOLE
    }

    pub fn console_remote_server_addr(&self) -> String {
        self.typed.console.remote.server_addr.clone()
    }

    /// Resolve remote server addresses for console remote mode.
    ///
    /// Resolution order (same as cluster member lookup):
    /// 1. `batata.member.list` config property (comma-separated `ip:port`)
    /// 2. `conf/cluster.conf` file (one `ip:port` per line, skip `#` comments)
    /// 3. Fall back to `batata.console.remote.server_addr`
    ///
    /// Each address is normalized to `http://ip:port` format.
    pub fn resolve_remote_server_addrs(&self) -> Vec<String> {
        // 1. Try batata.member.list
        let mut addresses: Vec<String> = self
            .config
            .get_string("batata.member.list")
            .ok()
            .map(|list| {
                list.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // 2. Fall back to conf/cluster.conf
        if addresses.is_empty() {
            let path = std::path::Path::new("conf/cluster.conf");
            if let Ok(content) = std::fs::read_to_string(path) {
                addresses = content
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                    .collect();
            }
        }

        // 3. Fall back to batata.console.remote.server_addr
        if addresses.is_empty() {
            let server_addr = self.console_remote_server_addr();
            return server_addr
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // Strip query parameters (e.g., ?raft_port=xxx) and convert ip:port → http://ip:port
        addresses
            .into_iter()
            .map(|addr| {
                let addr = addr.split('?').next().unwrap_or(&addr).to_string();
                if addr.starts_with("http://") || addr.starts_with("https://") {
                    addr
                } else {
                    format!("http://{}", addr)
                }
            })
            .collect()
    }

    pub fn console_remote_username(&self) -> String {
        self.typed.console.remote.username.clone()
    }

    pub fn console_remote_password(&self) -> String {
        self.typed.console.remote.password.clone()
    }

    pub fn console_remote_connect_timeout_ms(&self) -> u64 {
        self.typed.console.remote.connect_timeout_ms as u64
    }

    pub fn console_remote_read_timeout_ms(&self) -> u64 {
        self.typed.console.remote.read_timeout_ms as u64
    }

    // ========================================================================
    // Authentication Configuration
    // ========================================================================

    pub fn auth_enabled(&self) -> bool {
        self.typed.core.auth.enabled
    }

    pub fn auth_admin_enabled(&self) -> bool {
        self.typed.core.auth.admin.enabled
    }

    pub fn auth_enabled_for_api_type(&self, api_type: batata_common::ApiType) -> bool {
        match api_type {
            batata_common::ApiType::OpenApi => self.auth_enabled(),
            batata_common::ApiType::AdminApi => self.auth_admin_enabled(),
            batata_common::ApiType::ConsoleApi => self.auth_console_enabled(),
            batata_common::ApiType::InnerApi => true,
        }
    }

    pub fn server_identity_key(&self) -> String {
        self.typed.core.auth.server.identity.key.clone()
    }

    pub fn server_identity_value(&self) -> String {
        self.typed.core.auth.server.identity.value.clone()
    }

    pub fn auth_system_type(&self) -> String {
        self.typed.core.auth.system.type_.clone()
    }

    pub fn auth_console_enabled(&self) -> bool {
        self.typed.core.auth.console.enabled
    }

    pub fn token_secret_key(&self) -> String {
        self.typed.core.auth.plugin.default.token.secret.key.clone()
    }

    pub fn auth_token_expire_seconds(&self) -> i64 {
        self.typed.core.auth.plugin.default.token.expire.seconds
    }

    /// Check if LDAP authentication is enabled
    pub fn is_ldap_auth_enabled(&self) -> bool {
        self.auth_system_type().to_lowercase() == "ldap"
    }

    /// Get LDAP URL
    pub fn ldap_url(&self) -> Option<String> {
        self.typed.core.auth.ldap.url.clone()
    }

    /// Get LDAP base DN
    pub fn ldap_base_dn(&self) -> String {
        self.typed.core.auth.ldap.base_dc.clone()
    }

    /// Get LDAP bind DN (admin user)
    pub fn ldap_bind_dn(&self) -> String {
        self.typed.core.auth.ldap.bind_dn.clone()
    }

    /// Get LDAP bind password
    pub fn ldap_bind_password(&self) -> String {
        self.typed.core.auth.ldap.password.clone()
    }

    /// Get LDAP user DN pattern
    pub fn ldap_user_dn_pattern(&self) -> String {
        self.typed.core.auth.ldap.user_dn_pattern.clone()
    }

    /// Get LDAP filter prefix (default: uid)
    pub fn ldap_filter_prefix(&self) -> String {
        self.typed.core.auth.ldap.filter.prefix.clone()
    }

    /// Get LDAP connection timeout in milliseconds
    pub fn ldap_timeout_ms(&self) -> u64 {
        self.typed.core.auth.ldap.timeout as u64
    }

    /// Check if LDAP username comparison is case-sensitive
    pub fn ldap_case_sensitive(&self) -> bool {
        self.typed.core.auth.ldap.case.sensitive
    }

    /// Check if LDAP should ignore partial result exceptions
    pub fn ldap_ignore_partial_result_exception(&self) -> bool {
        self.typed.core.auth.ldap.ignore.partial.result.exception
    }

    /// Get LDAP configuration as LdapConfig struct
    pub fn ldap_config(&self) -> batata_auth::LdapConfig {
        batata_auth::LdapConfig {
            url: self.ldap_url().unwrap_or_default(),
            base_dn: self.ldap_base_dn(),
            bind_dn: self.ldap_bind_dn(),
            bind_password: self.ldap_bind_password(),
            user_dn_pattern: self.ldap_user_dn_pattern(),
            filter_prefix: self.ldap_filter_prefix(),
            timeout_ms: self.ldap_timeout_ms(),
            case_sensitive: self.ldap_case_sensitive(),
            ignore_partial_result_exception: self.ldap_ignore_partial_result_exception(),
        }
    }

    // ========================================================================
    // OAuth2/OIDC Configuration
    // ========================================================================

    /// Check if OAuth2/OIDC authentication is enabled
    pub fn is_oauth_enabled(&self) -> bool {
        self.typed.core.auth.oauth.enabled
    }

    /// Get OAuth user creation mode (auto or manual)
    pub fn oauth_user_creation(&self) -> String {
        self.typed.core.auth.oauth.user.creation.clone()
    }

    /// Get OAuth role sync mode (on_login or periodic)
    pub fn oauth_role_sync(&self) -> String {
        self.typed.core.auth.oauth.role.sync.clone()
    }

    /// Get default OAuth redirect URI template
    pub fn oauth_redirect_uri(&self) -> Option<String> {
        self.typed.core.auth.oauth.redirect.uri.clone()
    }

    /// Get OAuth configuration as OAuthConfig struct
    pub fn oauth_config(&self) -> batata_auth::service::oauth::OAuthConfig {
        use std::collections::HashMap;

        let mut providers = HashMap::new();

        // Load providers from config (e.g., batata.core.auth.oauth.providers.google)
        // This is a simplified version - actual implementation would iterate over providers
        if let Ok(provider_config) = self.config.get_table("batata.core.auth.oauth.providers") {
            for (name, value) in provider_config {
                if let Ok(provider) = value
                    .clone()
                    .try_deserialize::<batata_auth::service::oauth::OAuthProviderConfig>()
                {
                    providers.insert(name, provider);
                }
            }
        }

        batata_auth::service::oauth::OAuthConfig {
            enabled: self.is_oauth_enabled(),
            providers,
            user_creation: self.oauth_user_creation(),
            role_sync: self.oauth_role_sync(),
            redirect_uri: self.oauth_redirect_uri(),
        }
    }

    // ========================================================================
    // Persistence Mode Configuration
    // ========================================================================

    /// Get the storage backend type.
    ///
    /// - `batata.sql.init.platform` = "mysql" or "postgresql" → ExternalDb
    /// - Otherwise → Embedded (RocksDB)
    pub fn storage_backend(&self) -> batata_persistence::StorageBackend {
        let platform = self.datasource_platform();
        if platform.eq_ignore_ascii_case("mysql") || platform.eq_ignore_ascii_case("postgresql") {
            batata_persistence::StorageBackend::ExternalDb
        } else {
            batata_persistence::StorageBackend::Embedded
        }
    }

    /// Get the deploy topology.
    ///
    /// - `batata.standalone` = true → Standalone
    /// - `batata.standalone` = false → Cluster (Raft)
    pub fn deploy_topology(&self) -> batata_persistence::DeployTopology {
        if self.is_standalone() {
            batata_persistence::DeployTopology::Standalone
        } else {
            batata_persistence::DeployTopology::Cluster
        }
    }

    /// Derive the persistence storage mode from the two independent dimensions.
    pub fn persistence_mode(&self) -> batata_persistence::StorageMode {
        batata_persistence::StorageMode::from_dimensions(
            self.storage_backend(),
            self.deploy_topology(),
        )
    }

    /// Get the base data directory for embedded modes.
    /// This is the root directory for all persistent data (RocksDB, node-id, etc.)
    pub fn embedded_data_dir(&self) -> String {
        self.typed.persistence.embedded.data_dir.clone()
    }

    /// Get the RocksDB database name (subdirectory under data_dir).
    pub fn embedded_db_name(&self) -> String {
        self.typed.persistence.embedded.db_name.clone()
    }

    /// Get the full RocksDB storage path: {data_dir}/{db_name}
    pub fn embedded_rocksdb_dir(&self) -> String {
        let data_dir = self.embedded_data_dir();
        let db_name = self.embedded_db_name();
        format!("{}/{}", data_dir, db_name)
    }

    // ========================================================================
    // Database Configuration
    // ========================================================================

    pub fn datasource_platform(&self) -> String {
        self.typed.sql.init.platform.clone()
    }

    pub fn plugin_datasource_log(&self) -> bool {
        self.typed.plugin.datasource.log.enabled
    }

    pub async fn database_connection(
        &self,
    ) -> std::result::Result<DatabaseConnection, Box<dyn std::error::Error>> {
        let max_connections = self.typed.db.pool.max_connections as u32;
        let min_connections = self.typed.db.pool.min_connections as u32;
        let connect_timeout = self.typed.db.pool.connect_timeout as u64;
        let acquire_timeout = self.typed.db.pool.acquire_timeout as u64;
        let idle_timeout = self.typed.db.pool.idle_timeout as u64;
        let max_lifetime = self.typed.db.pool.max_lifetime as u64;
        let sqlx_logging = self.typed.db.pool.sqlx_logging;

        let url = self
            .typed
            .db
            .url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database URL not configured (batata.db.url)"))?;

        let mut opt = ConnectOptions::new(url);

        opt.max_connections(max_connections)
            .min_connections(min_connections)
            .connect_timeout(Duration::from_secs(connect_timeout))
            .acquire_timeout(Duration::from_secs(acquire_timeout))
            .idle_timeout(Duration::from_secs(idle_timeout))
            .max_lifetime(Duration::from_secs(max_lifetime))
            .sqlx_logging(sqlx_logging)
            .sqlx_logging_level(tracing::log::LevelFilter::Debug);

        tracing::info!(
            max_connections = max_connections,
            min_connections = min_connections,
            connect_timeout = connect_timeout,
            idle_timeout = idle_timeout,
            max_lifetime = max_lifetime,
            sqlx_logging = sqlx_logging,
            "Database connection pool configured"
        );

        let database_connection: DatabaseConnection = Database::connect(opt).await?;

        Ok(database_connection)
    }

    /// Check if database migration is enabled on startup.
    /// When true, pending SeaORM migrations will be applied automatically.
    pub fn db_migration_enabled(&self) -> bool {
        self.typed.db.migration.enabled
    }

    // ========================================================================
    // Naming Module Configuration
    // ========================================================================

    /// Check if instance expiration is enabled
    /// When true, instances will be automatically deleted after ip_delete_timeout
    pub fn expire_instance_enabled(&self) -> bool {
        self.typed.naming.expire_instance
    }

    /// Check if data warmup is enabled for naming service.
    /// When true, the server stays in STARTING state until subsystems are ready.
    /// When false (default), the server transitions to UP immediately after startup.
    pub fn data_warmup(&self) -> bool {
        self.typed.naming.data.warmup
    }

    // ========================================================================
    // Shutdown Configuration
    // ========================================================================

    /// Graceful shutdown drain timeout in seconds.
    /// During this period, the server stops accepting new connections and waits
    /// for in-flight requests to complete before proceeding with cleanup.
    pub fn shutdown_drain_timeout_secs(&self) -> u64 {
        self.typed.server.shutdown.drain_timeout as u64
    }

    /// Database connection close timeout in seconds during graceful shutdown.
    /// If pending transactions or cleanup take longer, the shutdown proceeds anyway.
    pub fn shutdown_db_close_timeout_secs(&self) -> u64 {
        self.typed.server.shutdown.db_close_timeout as u64
    }

    // ========================================================================
    // Capacity & Health Configuration
    // ========================================================================

    pub fn notify_connect_timeout(&self) -> i32 {
        self.typed.config.notify.connect_timeout as i32
    }

    pub fn notify_socket_timeout(&self) -> i32 {
        self.typed.config.notify.socket_timeout as i32
    }

    pub fn is_health_check(&self) -> bool {
        self.typed.config.health_check.enabled
    }

    pub fn max_health_check_fail_count(&self) -> i32 {
        self.typed.config.health_check.max_fail_count as i32
    }

    pub fn max_content(&self) -> i32 {
        self.typed.config.max_content as i32
    }

    pub fn is_manage_capacity(&self) -> bool {
        self.typed.config.capacity.manage_enabled
    }

    pub fn is_capacity_limit_check(&self) -> bool {
        self.typed.config.capacity.limit_check
    }

    pub fn default_cluster_quota(&self) -> i32 {
        self.typed.config.capacity.default_cluster_quota as i32
    }

    pub fn default_group_quota(&self) -> i32 {
        self.typed.config.capacity.default_group_quota as i32
    }

    pub fn default_max_size(&self) -> i32 {
        self.typed.config.capacity.default_max_size as i32
    }

    pub fn default_max_aggr_count(&self) -> i32 {
        self.typed.config.capacity.default_max_aggr_count as i32
    }

    pub fn default_max_aggr_size(&self) -> i32 {
        self.typed.config.capacity.default_max_aggr_size as i32
    }

    pub fn config_rentention_days(&self) -> i32 {
        self.typed.config.retention.days as i32
    }

    // ========================================================================
    // OpenTelemetry Configuration
    // ========================================================================

    pub fn otel_enabled(&self) -> bool {
        self.typed.otel.enabled
    }

    pub fn otel_endpoint(&self) -> String {
        self.typed.otel.endpoint.clone()
    }

    pub fn otel_service_name(&self) -> String {
        self.typed.otel.service_name.clone()
    }

    pub fn otel_sampling_ratio(&self) -> f64 {
        self.typed.otel.sampling_ratio
    }

    pub fn otel_export_timeout_secs(&self) -> u64 {
        self.typed.otel.export_timeout_secs as u64
    }

    // ========================================================================
    // Rate Limiting Configuration
    // ========================================================================

    /// Check if API rate limiting is enabled
    pub fn ratelimit_enabled(&self) -> bool {
        self.typed.ratelimit.enabled
    }

    /// Get maximum requests per window for API rate limiting
    pub fn ratelimit_max_requests(&self) -> u32 {
        self.typed.ratelimit.max_requests as u32
    }

    /// Get rate limit window duration in seconds
    pub fn ratelimit_window_seconds(&self) -> u64 {
        self.typed.ratelimit.window_seconds as u64
    }

    /// Check if authentication rate limiting is enabled
    pub fn ratelimit_auth_enabled(&self) -> bool {
        self.typed.ratelimit.auth.enabled
    }

    /// Get maximum login attempts before lockout
    pub fn ratelimit_auth_max_attempts(&self) -> u32 {
        self.typed.ratelimit.auth.max_attempts as u32
    }

    /// Get login attempt window duration in seconds
    pub fn ratelimit_auth_window_seconds(&self) -> u64 {
        self.typed.ratelimit.auth.window_seconds as u64
    }

    /// Get lockout duration in seconds after exceeding max login attempts
    pub fn ratelimit_auth_lockout_seconds(&self) -> u64 {
        self.typed.ratelimit.auth.lockout_seconds as u64
    }

    // ========================================================================
    // Control Plugin Configuration
    // ========================================================================

    /// Check if the control plugin (TPS + connection limiting) is enabled
    /// Whether HTTP access logging is enabled (default: true).
    /// Disable for better performance in production.
    pub fn http_access_log_enabled(&self) -> bool {
        self.typed.server.http.access_log.enabled
    }

    pub fn control_plugin_enabled(&self) -> bool {
        self.typed.plugin.control.enabled
    }

    /// Get default TPS limit per control point
    pub fn control_plugin_default_tps(&self) -> u32 {
        self.typed.plugin.control.default_tps as u32
    }

    /// Get maximum concurrent gRPC connections
    pub fn control_plugin_max_connections(&self) -> u32 {
        self.typed.plugin.control.max_connections as u32
    }

    /// Create ControlPluginConfig from configuration
    pub fn control_plugin_config(&self) -> batata_plugin::ControlPluginConfig {
        batata_plugin::ControlPluginConfig {
            enabled: self.control_plugin_enabled(),
            default_tps: self.control_plugin_default_tps(),
            default_max_connections: self.control_plugin_max_connections(),
            ..Default::default()
        }
    }

    /// Create RateLimitConfig from configuration
    pub fn rate_limit_config(&self) -> crate::middleware::rate_limit::RateLimitConfig {
        crate::middleware::rate_limit::RateLimitConfig {
            max_requests: self.ratelimit_max_requests(),
            window_duration: std::time::Duration::from_secs(self.ratelimit_window_seconds()),
            enabled: self.ratelimit_enabled(),
            max_tracked_ips: self.rate_limit_max_tracked_ips(),
        }
    }

    /// Create AuthRateLimitConfig from configuration
    pub fn auth_rate_limit_config(&self) -> crate::middleware::rate_limit::AuthRateLimitConfig {
        crate::middleware::rate_limit::AuthRateLimitConfig {
            max_attempts: self.ratelimit_auth_max_attempts(),
            window_duration: std::time::Duration::from_secs(self.ratelimit_auth_window_seconds()),
            lockout_duration: std::time::Duration::from_secs(self.ratelimit_auth_lockout_seconds()),
            enabled: self.ratelimit_auth_enabled(),
        }
    }

    // ========================================================================
    // Encryption Configuration
    // ========================================================================

    /// Check if configuration encryption is enabled
    pub fn encryption_enabled(&self) -> bool {
        self.typed.config.encryption.enabled
    }

    /// Get the encryption plugin type
    pub fn encryption_plugin_type(&self) -> String {
        self.typed.config.encryption.plugin.type_.clone()
    }

    /// Get the encryption key (Base64-encoded)
    pub fn encryption_key(&self) -> Option<String> {
        self.typed.config.encryption.key.clone()
    }

    /// Get the encryption hot reload interval in milliseconds (0 = disabled)
    pub fn encryption_reload_interval_ms(&self) -> u64 {
        self.typed.config.encryption.reload.interval.ms as u64
    }

    /// Check if encryption hot reload is enabled
    pub fn encryption_hot_reload_enabled(&self) -> bool {
        self.encryption_reload_interval_ms() > 0
    }

    // ========================================================================
    // gRPC TLS Configuration
    // ========================================================================

    /// Check if TLS is enabled for SDK gRPC server
    pub fn grpc_sdk_tls_enabled(&self) -> bool {
        self.typed.remote.server.grpc.sdk.tls.enabled
    }

    /// Check if TLS is enabled for cluster gRPC server
    pub fn grpc_cluster_tls_enabled(&self) -> bool {
        self.typed.remote.server.grpc.cluster.tls.enabled
    }

    /// Get the path to the server certificate file
    pub fn grpc_tls_cert_path(&self) -> Option<String> {
        self.typed.remote.server.grpc.tls.cert.path.clone()
    }

    /// Get the path to the server private key file
    pub fn grpc_tls_key_path(&self) -> Option<String> {
        self.typed.remote.server.grpc.tls.key.path.clone()
    }

    /// Get the path to the CA certificate for mTLS
    pub fn grpc_tls_ca_cert_path(&self) -> Option<String> {
        self.typed.remote.server.grpc.tls.ca.cert.path.clone()
    }

    /// Check if mutual TLS is enabled
    pub fn grpc_mtls_enabled(&self) -> bool {
        self.typed.remote.server.grpc.tls.mtls.enabled
    }

    /// Get gRPC TLS configuration
    pub fn grpc_tls_config(&self) -> super::tls::GrpcTlsConfig {
        super::tls::GrpcTlsConfig {
            sdk_enabled: self.grpc_sdk_tls_enabled(),
            cluster_enabled: self.grpc_cluster_tls_enabled(),
            cert_path: self.grpc_tls_cert_path().map(std::path::PathBuf::from),
            key_path: self.grpc_tls_key_path().map(std::path::PathBuf::from),
            ca_cert_path: self.grpc_tls_ca_cert_path().map(std::path::PathBuf::from),
            mtls_enabled: self.grpc_mtls_enabled(),
            alpn_protocols: vec!["h2".to_string()],
        }
    }

    // ========================================================================
    // Core Config Conversion
    // ========================================================================

    /// Convert to batata_core Configuration for use with ServerMemberManager
    pub fn to_core_config(&self) -> batata_core::model::Configuration {
        batata_core::model::Configuration::from_config(self.config.clone())
    }

    // ========================================================================
    // xDS Server Configuration (Service Mesh Support)
    // ========================================================================

    /// Check if xDS server is enabled
    pub fn xds_enabled(&self) -> bool {
        self.typed.mesh.xds.enabled
    }

    /// Get xDS server port (default: 15010)
    pub fn xds_server_port(&self) -> u16 {
        self.typed.mesh.xds.port as u16
    }

    /// Get xDS server ID
    pub fn xds_server_id(&self) -> String {
        self.typed.mesh.xds.server.id.clone()
    }

    /// Get xDS sync interval in milliseconds
    pub fn xds_sync_interval_ms(&self) -> u64 {
        self.typed.mesh.xds.sync.interval.ms as u64
    }

    /// Check if xDS should generate default listeners
    pub fn xds_generate_listeners(&self) -> bool {
        self.typed.mesh.xds.generate.listeners
    }

    /// Check if xDS should generate default routes
    pub fn xds_generate_routes(&self) -> bool {
        self.typed.mesh.xds.generate.routes
    }

    /// Get default listener port for xDS generated listeners
    pub fn xds_default_listener_port(&self) -> u16 {
        self.typed.mesh.xds.default.listener.port as u16
    }

    /// Check if xDS TLS is enabled
    pub fn xds_tls_enabled(&self) -> bool {
        self.typed.mesh.xds.tls.enabled
    }

    /// Get xDS TLS certificate path
    pub fn xds_tls_cert_path(&self) -> Option<String> {
        self.typed.mesh.xds.tls.cert.path.clone()
    }

    /// Get xDS TLS key path
    pub fn xds_tls_key_path(&self) -> Option<String> {
        self.typed.mesh.xds.tls.key.path.clone()
    }

    /// Get xDS configuration
    pub fn xds_config(&self) -> XdsConfig {
        XdsConfig {
            enabled: self.xds_enabled(),
            port: self.xds_server_port(),
            server_id: self.xds_server_id(),
            sync_interval_ms: self.xds_sync_interval_ms(),
            generate_listeners: self.xds_generate_listeners(),
            generate_routes: self.xds_generate_routes(),
            default_listener_port: self.xds_default_listener_port(),
            tls_enabled: self.xds_tls_enabled(),
            tls_cert_path: self.xds_tls_cert_path().map(std::path::PathBuf::from),
            tls_key_path: self.xds_tls_key_path().map(std::path::PathBuf::from),
        }
    }

    // ========================================================================
    // MCP Registry Configuration
    // ========================================================================

    /// Check if MCP Registry server is enabled (default: false)
    pub fn mcp_registry_enabled(&self) -> bool {
        self.typed.ai.mcp.registry.enabled
    }

    /// Get MCP Registry server port (default: 9080)
    pub fn mcp_registry_port(&self) -> u16 {
        self.typed.ai.mcp.registry.port as u16
    }

    // ========================================================================
    // Logging Configuration
    // ========================================================================

    /// Get log directory path
    pub fn log_dir(&self) -> Option<String> {
        self.typed.logs.path.clone()
    }

    /// Check if console logging is enabled
    pub fn log_console_enabled(&self) -> bool {
        self.typed.logs.console.enabled
    }

    /// Check if file logging is enabled
    pub fn log_file_enabled(&self) -> bool {
        self.typed.logs.file.enabled
    }

    /// Get log level
    pub fn log_level(&self) -> String {
        self.typed.logs.level.clone()
    }

    // NOTE: logging_config() is provided as an extension in batata-server/src/startup/logging.rs
    // because LoggingConfig lives in the startup module which is server-specific.

    // ========================================================================
    // Performance tuning configuration
    // ========================================================================

    /// HTTP server worker threads (0 = auto-detect based on CPU cores)
    pub fn http_workers(&self) -> usize {
        let v = self.typed.server.http.workers as usize;
        if v == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        } else {
            v
        }
    }

    /// HTTP keep-alive timeout in seconds (main server)
    pub fn http_keep_alive_secs(&self) -> u64 {
        self.typed.server.http.keep_alive as u64
    }

    /// Console HTTP keep-alive timeout in seconds
    pub fn console_keep_alive_secs(&self) -> u64 {
        self.typed.console.http.keep_alive as u64
    }

    /// Maximum request payload size in bytes (default 10MB)
    pub fn max_payload_size(&self) -> usize {
        self.typed.server.http.max_payload_size as usize
    }

    /// Maximum JSON body size in bytes (default 5MB)
    pub fn max_json_size(&self) -> usize {
        self.typed.server.http.max_json_size as usize
    }

    /// gRPC TCP keep-alive interval in seconds
    pub fn grpc_tcp_keepalive_secs(&self) -> u64 {
        self.typed.server.grpc.tcp_keepalive as u64
    }

    /// gRPC HTTP/2 keep-alive interval in seconds
    pub fn grpc_http2_keepalive_interval_secs(&self) -> u64 {
        self.typed.server.grpc.http2_keepalive_interval as u64
    }

    /// gRPC HTTP/2 keep-alive timeout in seconds
    pub fn grpc_http2_keepalive_timeout_secs(&self) -> u64 {
        self.typed.server.grpc.http2_keepalive_timeout as u64
    }

    /// gRPC max concurrent streams per connection
    pub fn grpc_concurrency_limit(&self) -> usize {
        self.typed.server.grpc.concurrency_limit as usize
    }

    /// Auth token cache max capacity
    pub fn auth_token_cache_capacity(&self) -> u64 {
        self.typed.core.auth.cache.token_capacity as u64
    }

    /// Auth roles cache max capacity
    pub fn auth_roles_cache_capacity(&self) -> u64 {
        self.typed.core.auth.cache.roles_capacity as u64
    }

    /// Auth permissions cache max capacity
    pub fn auth_permissions_cache_capacity(&self) -> u64 {
        self.typed.core.auth.cache.permissions_capacity as u64
    }

    /// RocksDB write buffer size in MB
    pub fn rocksdb_write_buffer_mb(&self) -> usize {
        self.typed.rocksdb.write_buffer_mb as usize
    }

    /// RocksDB max write buffer number
    pub fn rocksdb_max_write_buffers(&self) -> i32 {
        self.typed.rocksdb.max_write_buffers as i32
    }

    /// RocksDB max background jobs
    pub fn rocksdb_max_background_jobs(&self) -> i32 {
        self.typed.rocksdb.max_background_jobs as i32
    }

    /// RocksDB block cache size in MB
    pub fn rocksdb_block_cache_mb(&self) -> usize {
        self.typed.rocksdb.block_cache_mb as usize
    }

    /// Connection stale threshold in milliseconds
    pub fn grpc_connection_stale_ms(&self) -> u64 {
        self.typed.server.grpc.connection_stale_ms as u64
    }

    // ========================================================================
    // HTTP Compression
    // ========================================================================

    /// Whether HTTP response compression is enabled
    pub fn http_compression_enabled(&self) -> bool {
        self.typed.server.http.compression.enabled
    }

    /// Minimum response size in bytes to trigger compression (default 256)
    pub fn http_compression_min_size(&self) -> usize {
        self.typed.server.http.compression.min_size as usize
    }

    /// HTTP client request timeout in seconds
    pub fn http_client_request_timeout_secs(&self) -> u64 {
        self.typed.server.http.client_request_timeout as u64
    }

    // ========================================================================
    // gRPC Advanced Tuning
    // ========================================================================

    /// Enable TCP_NODELAY for gRPC (disable Nagle's algorithm)
    pub fn grpc_tcp_nodelay(&self) -> bool {
        self.typed.server.grpc.tcp_nodelay
    }

    /// gRPC HTTP/2 initial connection window size in bytes (default 1MB)
    pub fn grpc_initial_connection_window_size(&self) -> u32 {
        self.typed.server.grpc.initial_connection_window_size as u32
    }

    /// gRPC HTTP/2 initial stream window size in bytes (default 512KB)
    pub fn grpc_initial_stream_window_size(&self) -> u32 {
        self.typed.server.grpc.initial_stream_window_size as u32
    }

    /// gRPC HTTP/2 max frame size in bytes (default 16KB)
    pub fn grpc_max_frame_size(&self) -> u32 {
        self.typed.server.grpc.max_frame_size as u32
    }

    // ========================================================================
    // Raft gRPC Tuning
    // ========================================================================

    /// Raft gRPC TCP keep-alive interval in seconds (default 10s)
    pub fn raft_grpc_tcp_keepalive_secs(&self) -> u64 {
        self.typed.raft.grpc.tcp_keepalive as u64
    }

    /// Raft gRPC TCP_NODELAY (default true)
    pub fn raft_grpc_tcp_nodelay(&self) -> bool {
        self.typed.raft.grpc.tcp_nodelay
    }

    /// Raft gRPC HTTP/2 keep-alive interval in seconds (default 10s)
    pub fn raft_grpc_http2_keepalive_interval_secs(&self) -> u64 {
        self.typed.raft.grpc.http2_keepalive_interval as u64
    }

    /// Raft gRPC HTTP/2 keep-alive timeout in seconds (default 5s)
    pub fn raft_grpc_http2_keepalive_timeout_secs(&self) -> u64 {
        self.typed.raft.grpc.http2_keepalive_timeout as u64
    }

    // ========================================================================
    // Push Message Tuning
    // ========================================================================

    /// Timeout in milliseconds for pushing a message to a client connection (default 5000ms)
    pub fn grpc_push_message_timeout_ms(&self) -> u64 {
        self.typed.server.grpc.push_message_timeout as u64
    }

    /// Buffer size for bi-directional streaming channel (default 128)
    pub fn grpc_bistream_channel_capacity(&self) -> usize {
        self.typed.server.grpc.bistream_channel_capacity as usize
    }

    /// Timeout in milliseconds for subscriber notification (default 3000ms).
    /// 0 means fire-and-forget (non-blocking).
    pub fn notify_subscriber_timeout_ms(&self) -> u64 {
        self.typed.server.grpc.notify_subscriber_timeout as u64
    }

    /// Config read cache TTL in seconds (default 0 = disabled).
    /// When enabled, caches config_find_one results to reduce RocksDB reads in distributed mode.
    /// Recommended: 5-30 for high-read workloads.
    pub fn config_read_cache_ttl_secs(&self) -> u64 {
        self.typed.config.read_cache_ttl as u64
    }

    /// Config read cache max entries (default 10000).
    pub fn config_read_cache_max_entries(&self) -> u64 {
        self.typed.config.read_cache_max_entries as u64
    }

    /// Max consecutive push timeouts before closing a slow client connection (default 5).
    /// 0 = disabled (never close connections due to push timeouts).
    pub fn grpc_max_push_timeouts(&self) -> u32 {
        self.typed.server.grpc.max_push_timeouts as u32
    }

    /// Maximum concurrent HTTP/2 streams per gRPC connection (default 200).
    /// Limits the number of concurrent RPCs a single client can make.
    /// Protects against resource exhaustion from misbehaving clients.
    pub fn grpc_max_concurrent_streams(&self) -> u32 {
        self.typed.server.grpc.max_concurrent_streams as u32
    }

    /// Maximum number of SDK gRPC connections (default 10000).
    /// 0 means unlimited.
    pub fn grpc_max_connections(&self) -> usize {
        self.typed.server.grpc.max_connections as usize
    }

    // ========================================================================
    // RocksDB Advanced Tuning
    // ========================================================================

    /// RocksDB bloom filter bits per key (0 = disabled)
    pub fn rocksdb_bloom_filter_bits(&self) -> f64 {
        self.typed.rocksdb.bloom_filter_bits
    }

    /// Whether to enable dynamic level compaction
    pub fn rocksdb_level_compaction_dynamic(&self) -> bool {
        self.typed.rocksdb.level_compaction_dynamic
    }

    /// RocksDB bottommost level compression type (zstd or lz4)
    pub fn rocksdb_bottommost_compression(&self) -> String {
        self.typed.rocksdb.bottommost_compression.clone()
    }

    /// RocksDB default compression type (lz4, zstd, snappy, none)
    pub fn rocksdb_compression(&self) -> String {
        self.typed.rocksdb.compression.clone()
    }

    /// Whether RocksDB internal statistics are enabled
    pub fn rocksdb_enable_statistics(&self) -> bool {
        self.typed.rocksdb.enable_statistics
    }

    /// Whether whole-key filtering is enabled in bloom filter
    pub fn rocksdb_whole_key_filtering(&self) -> bool {
        self.typed.rocksdb.whole_key_filtering
    }

    /// Hash ratio for binary-and-hash data block index (0.0 = disabled)
    pub fn rocksdb_data_block_hash_ratio(&self) -> f64 {
        self.typed.rocksdb.data_block_hash_ratio
    }

    /// Whether to fsync WAL on every state-machine write (default: false)
    pub fn rocksdb_sm_sync(&self) -> bool {
        self.typed.rocksdb.sm_sync
    }

    /// Whether to disable WAL for state-machine writes (default: false)
    pub fn rocksdb_sm_disable_wal(&self) -> bool {
        self.typed.rocksdb.sm_disable_wal
    }

    /// Write buffer size in MB for history column family (default: 0 = same as write_buffer_mb)
    pub fn rocksdb_history_write_buffer_mb(&self) -> usize {
        self.typed.rocksdb.history_write_buffer_mb as usize
    }

    /// Whether to enable bloom filter for history CF (default: false)
    pub fn rocksdb_history_bloom_filter(&self) -> bool {
        self.typed.rocksdb.history_bloom_filter
    }

    // ========================================================================
    // Rate Limiting Advanced
    // ========================================================================

    /// Maximum number of tracked IPs for rate limiting (prevents memory exhaustion)
    pub fn rate_limit_max_tracked_ips(&self) -> usize {
        self.typed.ratelimit.max_tracked_ips as usize
    }

    /// Cleanup interval for rate limiter entries in seconds
    pub fn rate_limit_cleanup_interval_secs(&self) -> u64 {
        self.typed.ratelimit.cleanup_interval_secs as u64
    }

    // ========================================================================
    // Auth Cache Advanced
    // ========================================================================

    /// Token cache TTL in seconds (default: 60s for cluster-safe operation)
    pub fn auth_token_cache_ttl_secs(&self) -> u64 {
        self.typed.core.auth.cache.token_ttl_secs as u64
    }

    /// Token blacklist max capacity
    pub fn auth_blacklist_capacity(&self) -> u64 {
        self.typed.core.auth.cache.blacklist_capacity as u64
    }

    /// Token blacklist TTL in seconds
    pub fn auth_blacklist_ttl_secs(&self) -> u64 {
        self.typed.core.auth.cache.blacklist_ttl_secs as u64
    }

    // ---- Raft consensus tuning ----

    /// Raft election timeout in milliseconds (default: 5000).
    /// If a follower doesn't hear from the leader within this time, it starts an election.
    /// Increase for WAN clusters or high-latency networks.
    pub fn raft_election_timeout_ms(&self) -> u64 {
        self.typed.raft.election_timeout_ms as u64
    }

    /// Raft heartbeat interval in milliseconds (default: 1000).
    /// Leader sends heartbeats at this interval. Should be < election_timeout / 3.
    pub fn raft_heartbeat_interval_ms(&self) -> u64 {
        self.typed.raft.heartbeat_interval_ms as u64
    }

    /// Raft RPC request timeout in milliseconds (default: 5000).
    /// Timeout for individual Raft RPCs (AppendEntries, Vote).
    pub fn raft_rpc_timeout_ms(&self) -> u64 {
        self.typed.raft.rpc_timeout_ms as u64
    }

    /// Raft snapshot threshold — log entries before triggering snapshot (default: 10000).
    pub fn raft_snapshot_threshold(&self) -> u64 {
        self.typed.raft.snapshot_threshold as u64
    }

    /// Raft snapshot transfer timeout in milliseconds (default: 30000).
    /// Timeout for full snapshot transfer between nodes. Should be larger than rpc_timeout.
    pub fn raft_snapshot_transfer_timeout_ms(&self) -> u64 {
        self.typed.raft.snapshot_transfer_timeout_ms as u64
    }

    /// Max retries when forwarding write to Raft leader (default: 3)
    pub fn raft_forward_max_retries(&self) -> u32 {
        self.typed.raft.forward.max_retries as u32
    }

    /// Initial retry delay in ms for leader forwarding (default: 200)
    /// Doubles each attempt with 25% jitter.
    pub fn raft_forward_initial_delay_ms(&self) -> u64 {
        self.typed.raft.forward.initial_delay_ms as u64
    }

    /// Timeout in seconds for waiting for Raft peer gRPC servers to become reachable
    /// during cluster initialization (default: 30).
    pub fn raft_peer_connect_timeout_secs(&self) -> u64 {
        self.typed.raft.peer_connect_timeout_secs as u64
    }

    /// Retry interval in milliseconds when probing Raft peer readiness (default: 500).
    pub fn raft_peer_connect_retry_interval_ms(&self) -> u64 {
        self.typed.raft.peer_connect_retry_interval_ms as u64
    }

    // ========================================================================
    // Config Gray Version Management
    // ========================================================================

    /// Maximum number of gray versions per config (default: 10).
    /// Matches batata.config.gray.version.max.count property.
    pub fn config_gray_max_version_count(&self) -> usize {
        self.typed.config.gray.version.max_count as usize
    }

    // ========================================================================
    // Console Remote Polling Configuration
    // ========================================================================

    /// Console remote data source refresh interval in seconds (default: 30)
    pub fn console_remote_refresh_interval_secs(&self) -> u64 {
        self.typed.console.remote.refresh_interval_secs as u64
    }

    /// Console remote data source initial delay in seconds (default: 5)
    pub fn console_remote_initial_delay_secs(&self) -> u64 {
        self.typed.console.remote.initial_delay_secs as u64
    }

    // ========================================================================
    // Webhook Configuration
    // ========================================================================

    /// Webhook HTTP client default timeout in seconds (default: 30)
    pub fn webhook_default_timeout_secs(&self) -> u64 {
        self.typed.plugin.webhook.default_timeout_secs as u64
    }

    // ========================================================================
    // Naming Health Check Intervals
    // ========================================================================

    /// Naming heartbeat check interval in seconds (default: 5)
    pub fn naming_heartbeat_check_interval_secs(&self) -> u64 {
        self.typed.naming.healthcheck.heartbeat_interval_secs as u64
    }

    /// Naming TTL monitor interval in seconds (default: 5)
    pub fn naming_ttl_monitor_interval_secs(&self) -> u64 {
        self.typed.naming.healthcheck.ttl_monitor_interval_secs as u64
    }

    /// Naming deregister monitor interval in seconds (default: 10)
    pub fn naming_deregister_monitor_interval_secs(&self) -> u64 {
        self.typed.naming.healthcheck.deregister_monitor_interval_secs as u64
    }

    // ========================================================================
    // OAuth Cache Configuration
    // ========================================================================

    /// OAuth provider discovery cache TTL in seconds (default: 3600)
    pub fn oauth_discovery_cache_ttl_secs(&self) -> u64 {
        self.typed.core.auth.oauth.cache.discovery_ttl_secs as u64
    }

    /// OAuth provider discovery cache max capacity (default: 100)
    pub fn oauth_discovery_cache_capacity(&self) -> u64 {
        self.typed.core.auth.oauth.cache.discovery_capacity as u64
    }

    /// OAuth state cache TTL in seconds (default: 600)
    pub fn oauth_state_cache_ttl_secs(&self) -> u64 {
        self.typed.core.auth.oauth.cache.state_ttl_secs as u64
    }

    /// OAuth state cache max capacity (default: 10000)
    pub fn oauth_state_cache_capacity(&self) -> u64 {
        self.typed.core.auth.oauth.cache.state_capacity as u64
    }

    /// OAuth HTTP client timeout in seconds (default: 30)
    pub fn oauth_http_timeout_secs(&self) -> u64 {
        self.typed.core.auth.oauth.http_timeout_secs as u64
    }

    // ========================================================================
    // gRPC Auth Cache Configuration
    // ========================================================================

    /// gRPC auth permission check cache max capacity (default: 10000)
    pub fn grpc_auth_cache_capacity(&self) -> u64 {
        self.typed.core.auth.cache.grpc_permission_capacity as u64
    }

    /// gRPC auth permission check cache TTL in seconds (default: 60s for cluster-safe operation)
    pub fn grpc_auth_cache_ttl_secs(&self) -> u64 {
        self.typed.core.auth.cache.grpc_permission_ttl_secs as u64
    }

    // ========================================================================
    // Metrics Configuration
    // ========================================================================

    /// Check if system stats reporter is enabled (default: true)
    /// When enabled, periodically collects CPU/memory metrics and exports via Prometheus
    pub fn metrics_system_stats_enabled(&self) -> bool {
        self.typed.metrics.system_stats.enabled
    }

    /// System stats reporter collection interval in seconds (default: 15)
    /// Only effective when metrics_system_stats_enabled is true
    pub fn metrics_system_stats_interval_secs(&self) -> u64 {
        self.typed.metrics.system_stats.interval_secs as u64
    }

    /// Build a RocksDB configuration struct from the current config values
    pub fn rocksdb_config(&self) -> RocksDbConfig {
        RocksDbConfig {
            write_buffer_mb: self.rocksdb_write_buffer_mb(),
            max_write_buffers: self.rocksdb_max_write_buffers(),
            max_background_jobs: self.rocksdb_max_background_jobs(),
            block_cache_mb: self.rocksdb_block_cache_mb(),
            bloom_filter_bits: self.rocksdb_bloom_filter_bits(),
            level_compaction_dynamic: self.rocksdb_level_compaction_dynamic(),
            bottommost_compression: self.rocksdb_bottommost_compression(),
            compression: self.rocksdb_compression(),
            enable_statistics: self.rocksdb_enable_statistics(),
            whole_key_filtering: self.rocksdb_whole_key_filtering(),
            data_block_hash_ratio: self.rocksdb_data_block_hash_ratio(),
            sm_sync: self.rocksdb_sm_sync(),
            sm_disable_wal: self.rocksdb_sm_disable_wal(),
            history_write_buffer_mb: self.rocksdb_history_write_buffer_mb(),
            history_bloom_filter: self.rocksdb_history_bloom_filter(),
        }
    }
}

/// RocksDB tuning configuration bundle
#[derive(Debug, Clone)]
pub struct RocksDbConfig {
    pub write_buffer_mb: usize,
    pub max_write_buffers: i32,
    pub max_background_jobs: i32,
    pub block_cache_mb: usize,
    pub bloom_filter_bits: f64,
    pub level_compaction_dynamic: bool,
    pub bottommost_compression: String,
    pub compression: String,
    pub enable_statistics: bool,
    /// Enable whole-key filtering in bloom filter for better point lookups
    pub whole_key_filtering: bool,
    /// Hash ratio for binary-and-hash data block index (0.0 = disabled, 0.75 = recommended)
    pub data_block_hash_ratio: f64,
    /// Whether to fsync WAL on every state-machine write (default: false).
    /// When false, WAL is still written but not fsynced per write (OS page cache provides durability).
    /// For Raft mode, Raft log already provides durability so sync is unnecessary.
    pub sm_sync: bool,
    /// Whether to disable WAL for state-machine writes (default: false).
    /// In Raft mode, the Raft log provides durability, so disabling WAL is safe and faster.
    /// In standalone embedded mode, keep WAL enabled (false) for crash safety.
    pub sm_disable_wal: bool,
    /// Write buffer size in MB for history column family (default: same as write_buffer_mb).
    /// History CF is append-only with infrequent reads, so a larger buffer reduces compaction.
    pub history_write_buffer_mb: usize,
    /// Whether to enable bloom filter for history column family (default: false).
    /// History CF is mostly scanned by prefix, not point-queried, so bloom filter adds overhead.
    pub history_bloom_filter: bool,
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            write_buffer_mb: 128,
            max_write_buffers: 4,
            max_background_jobs: 4,
            block_cache_mb: 256,
            bloom_filter_bits: 10.0,
            level_compaction_dynamic: true,
            bottommost_compression: "zstd".to_string(),
            compression: "lz4".to_string(),
            enable_statistics: false,
            whole_key_filtering: true,
            data_block_hash_ratio: 0.75,
            sm_sync: false,
            sm_disable_wal: false,
            history_write_buffer_mb: 0, // 0 = use same as write_buffer_mb
            history_bloom_filter: false,
        }
    }
}

impl RocksDbConfig {
    /// Parse compression type string to RocksDB DBCompressionType
    pub fn parse_compression(name: &str) -> rocksdb::DBCompressionType {
        match name.to_lowercase().as_str() {
            "zstd" => rocksdb::DBCompressionType::Zstd,
            "lz4" => rocksdb::DBCompressionType::Lz4,
            "snappy" => rocksdb::DBCompressionType::Snappy,
            "none" => rocksdb::DBCompressionType::None,
            _ => rocksdb::DBCompressionType::Lz4,
        }
    }

    /// Create RocksDB Options configured with these settings
    pub fn to_db_options(&self) -> rocksdb::Options {
        let mut db_opts = rocksdb::Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_write_buffer_size(self.write_buffer_mb * 1024 * 1024);
        db_opts.set_max_write_buffer_number(self.max_write_buffers);
        db_opts.set_compression_type(Self::parse_compression(&self.compression));
        db_opts
            .set_bottommost_compression_type(Self::parse_compression(&self.bottommost_compression));
        db_opts.set_max_background_jobs(self.max_background_jobs);
        if self.level_compaction_dynamic {
            db_opts.set_level_compaction_dynamic_level_bytes(true);
        }
        db_opts.increase_parallelism(std::cmp::max(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4) as i32
                / 2,
            2,
        ));
        if self.enable_statistics {
            db_opts.enable_statistics();
        }
        db_opts
    }

    /// Create WriteOptions for state-machine writes.
    pub fn to_write_options(&self) -> rocksdb::WriteOptions {
        let mut opts = rocksdb::WriteOptions::default();
        opts.set_sync(self.sm_sync);
        opts.disable_wal(self.sm_disable_wal);
        opts
    }

    /// Create column family Options optimized for history (append-only, infrequent reads).
    /// Accepts a shared block cache to avoid allocating a separate 256MB cache.
    pub fn to_history_cf_options(&self, shared_cache: &rocksdb::Cache) -> rocksdb::Options {
        let mut cf_opts = rocksdb::Options::default();
        let buf_mb = if self.history_write_buffer_mb > 0 {
            self.history_write_buffer_mb
        } else {
            self.write_buffer_mb
        };
        cf_opts.set_write_buffer_size(buf_mb * 1024 * 1024);
        // History is append-only: use zstd for all levels (better compression ratio)
        cf_opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
        cf_opts
            .set_bottommost_compression_type(Self::parse_compression(&self.bottommost_compression));
        if self.level_compaction_dynamic {
            cf_opts.set_level_compaction_dynamic_level_bytes(true);
        }

        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_block_cache(shared_cache);
        // Only add bloom filter for history if explicitly enabled (default off)
        if self.history_bloom_filter && self.bloom_filter_bits > 0.0 {
            block_opts.set_bloom_filter(self.bloom_filter_bits, false);
        }
        cf_opts.set_block_based_table_factory(&block_opts);
        cf_opts
    }

    /// Create a shared LRU block cache for all column families.
    pub fn create_shared_block_cache(&self) -> rocksdb::Cache {
        rocksdb::Cache::new_lru_cache(self.block_cache_mb * 1024 * 1024)
    }

    /// Create column family Options with block cache, bloom filter, and optimizations.
    /// Uses a shared block cache to avoid multiple 256MB allocations across CFs.
    pub fn to_cf_options_with_cache(&self, shared_cache: &rocksdb::Cache) -> rocksdb::Options {
        let mut cf_opts = rocksdb::Options::default();
        cf_opts.set_write_buffer_size(self.write_buffer_mb * 1024 * 1024);
        cf_opts.set_compression_type(Self::parse_compression(&self.compression));
        cf_opts
            .set_bottommost_compression_type(Self::parse_compression(&self.bottommost_compression));

        if self.level_compaction_dynamic {
            cf_opts.set_level_compaction_dynamic_level_bytes(true);
        }

        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_block_cache(shared_cache);
        if self.bloom_filter_bits > 0.0 {
            block_opts.set_bloom_filter(self.bloom_filter_bits, false);
            block_opts.set_whole_key_filtering(self.whole_key_filtering);
        }
        if self.data_block_hash_ratio > 0.0 {
            block_opts.set_data_block_index_type(rocksdb::DataBlockIndexType::BinaryAndHash);
            block_opts.set_data_block_hash_ratio(self.data_block_hash_ratio);
        }
        cf_opts.set_block_based_table_factory(&block_opts);
        cf_opts
    }

    /// Create column family Options (convenience: creates its own block cache).
    /// Use `to_cf_options_with_cache` when multiple CFs should share one cache.
    pub fn to_cf_options(&self) -> rocksdb::Options {
        let cache = self.create_shared_block_cache();
        self.to_cf_options_with_cache(&cache)
    }
}

/// xDS server configuration
#[derive(Debug, Clone)]
pub struct XdsConfig {
    /// Whether xDS server is enabled
    pub enabled: bool,
    /// xDS server port
    pub port: u16,
    /// xDS server ID
    pub server_id: String,
    /// Sync interval in milliseconds
    pub sync_interval_ms: u64,
    /// Generate default listeners
    pub generate_listeners: bool,
    /// Generate default routes
    pub generate_routes: bool,
    /// Default listener port
    pub default_listener_port: u16,
    /// TLS enabled
    pub tls_enabled: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<std::path::PathBuf>,
    /// TLS key path
    pub tls_key_path: Option<std::path::PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_auth::model::{
        AUTH_ADMIN_ENABLED_KEY, AUTH_CONSOLE_ENABLED_KEY, AUTH_ENABLED_KEY,
        AUTH_SERVER_IDENTITY_KEY_PROP,
    };
    use batata_common::ApiType;
    use config::Config;

    fn build_config(overrides: Vec<(&str, config::Value)>) -> Configuration {
        let mut builder = Config::builder();
        for (key, value) in overrides {
            builder = builder.set_override(key, value).unwrap();
        }
        let config = builder.build().unwrap();
        let typed = config.get("batata").unwrap_or_default();
        Configuration { config, typed }
    }

    #[test]
    fn test_auth_enabled_default_false() {
        let cfg = build_config(vec![]);
        assert!(!cfg.auth_enabled());
    }

    #[test]
    fn test_auth_enabled_only_core_auth() {
        let cfg = build_config(vec![("batata.core.auth.enabled", true.into())]);
        assert!(cfg.auth_enabled());
    }

    #[test]
    fn test_auth_enabled_does_not_include_admin() {
        // Setting admin.enabled=true should NOT make auth_enabled() return true
        let cfg = build_config(vec![(AUTH_ADMIN_ENABLED_KEY, true.into())]);
        assert!(!cfg.auth_enabled());
    }

    #[test]
    fn test_auth_admin_enabled() {
        let cfg = build_config(vec![(AUTH_ADMIN_ENABLED_KEY, true.into())]);
        assert!(cfg.auth_admin_enabled());
    }

    #[test]
    fn test_auth_enabled_for_api_type_open_api() {
        let cfg = build_config(vec![(AUTH_ENABLED_KEY, true.into())]);
        assert!(cfg.auth_enabled_for_api_type(ApiType::OpenApi));

        let cfg2 = build_config(vec![]);
        assert!(!cfg2.auth_enabled_for_api_type(ApiType::OpenApi));
    }

    #[test]
    fn test_auth_enabled_for_api_type_admin_api() {
        let cfg = build_config(vec![(AUTH_ADMIN_ENABLED_KEY, true.into())]);
        assert!(cfg.auth_enabled_for_api_type(ApiType::AdminApi));

        let cfg2 = build_config(vec![]);
        assert!(!cfg2.auth_enabled_for_api_type(ApiType::AdminApi));
    }

    #[test]
    fn test_auth_enabled_for_api_type_console_api() {
        // Console auth defaults to true
        let cfg = build_config(vec![]);
        assert!(cfg.auth_enabled_for_api_type(ApiType::ConsoleApi));

        let cfg2 = build_config(vec![(AUTH_CONSOLE_ENABLED_KEY, false.into())]);
        assert!(!cfg2.auth_enabled_for_api_type(ApiType::ConsoleApi));
    }

    #[test]
    fn test_auth_enabled_for_api_type_inner_api() {
        // InnerApi always returns true (uses server identity instead)
        let cfg = build_config(vec![]);
        assert!(cfg.auth_enabled_for_api_type(ApiType::InnerApi));
    }

    #[test]
    fn test_server_identity_key_default() {
        let cfg = build_config(vec![]);
        assert!(cfg.server_identity_key().is_empty());
    }

    #[test]
    fn test_server_identity_key_value() {
        let cfg = build_config(vec![
            (AUTH_SERVER_IDENTITY_KEY_PROP, "serverIdentity".into()),
            (
                "batata.core.auth.server.identity.value",
                "cluster-node-1".into(),
            ),
        ]);
        assert_eq!(cfg.server_identity_key(), "serverIdentity");
        assert_eq!(cfg.server_identity_value(), "cluster-node-1");
    }

    // Rate Limit Configuration Tests
    #[test]
    fn test_ratelimit_enabled_default_false() {
        let cfg = build_config(vec![]);
        assert!(!cfg.ratelimit_enabled());
    }

    #[test]
    fn test_ratelimit_enabled_true() {
        let cfg = build_config(vec![("batata.ratelimit.enabled", true.into())]);
        assert!(cfg.ratelimit_enabled());
    }

    #[test]
    fn test_ratelimit_max_requests_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.ratelimit_max_requests(), 100);
    }

    #[test]
    fn test_ratelimit_max_requests_custom() {
        let cfg = build_config(vec![("batata.ratelimit.max_requests", 5000_i64.into())]);
        assert_eq!(cfg.ratelimit_max_requests(), 5000);
    }

    #[test]
    fn test_ratelimit_window_seconds_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.ratelimit_window_seconds(), 60);
    }

    #[test]
    fn test_ratelimit_window_seconds_custom() {
        let cfg = build_config(vec![("batata.ratelimit.window_seconds", 120_i64.into())]);
        assert_eq!(cfg.ratelimit_window_seconds(), 120);
    }

    #[test]
    fn test_ratelimit_auth_enabled_default_false() {
        let cfg = build_config(vec![]);
        assert!(!cfg.ratelimit_auth_enabled());
    }

    #[test]
    fn test_ratelimit_auth_enabled_true() {
        let cfg = build_config(vec![("batata.ratelimit.auth.enabled", true.into())]);
        assert!(cfg.ratelimit_auth_enabled());
    }

    #[test]
    fn test_ratelimit_auth_max_attempts_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.ratelimit_auth_max_attempts(), 5);
    }

    #[test]
    fn test_ratelimit_auth_max_attempts_custom() {
        let cfg = build_config(vec![("batata.ratelimit.auth.max_attempts", 10_i64.into())]);
        assert_eq!(cfg.ratelimit_auth_max_attempts(), 10);
    }

    #[test]
    fn test_rate_limit_config() {
        let cfg = build_config(vec![
            ("batata.ratelimit.enabled", true.into()),
            ("batata.ratelimit.max_requests", 1000_i64.into()),
            ("batata.ratelimit.window_seconds", 30_i64.into()),
        ]);
        let rate_limit_cfg = cfg.rate_limit_config();
        assert!(rate_limit_cfg.enabled);
        assert_eq!(rate_limit_cfg.max_requests, 1000);
        assert_eq!(rate_limit_cfg.window_duration.as_secs(), 30);
    }

    #[test]
    fn test_auth_rate_limit_config() {
        let cfg = build_config(vec![
            ("batata.ratelimit.auth.enabled", true.into()),
            ("batata.ratelimit.auth.max_attempts", 3_i64.into()),
            ("batata.ratelimit.auth.window_seconds", 120_i64.into()),
            ("batata.ratelimit.auth.lockout_seconds", 600_i64.into()),
        ]);
        let auth_rate_limit_cfg = cfg.auth_rate_limit_config();
        assert!(auth_rate_limit_cfg.enabled);
        assert_eq!(auth_rate_limit_cfg.max_attempts, 3);
        assert_eq!(auth_rate_limit_cfg.window_duration.as_secs(), 120);
        assert_eq!(auth_rate_limit_cfg.lockout_duration.as_secs(), 600);
    }

    // ========================================================================
    // resolve_remote_server_addrs tests
    // ========================================================================

    #[test]
    fn test_resolve_remote_server_addrs_from_member_list() {
        let cfg = build_config(vec![(
            "batata.member.list",
            "192.168.1.10:8848,192.168.1.11:8848".into(),
        )]);
        let addrs = cfg.resolve_remote_server_addrs();
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0], "http://192.168.1.10:8848");
        assert_eq!(addrs[1], "http://192.168.1.11:8848");
    }

    #[test]
    fn test_resolve_remote_server_addrs_strips_query_params() {
        let cfg = build_config(vec![(
            "batata.member.list",
            "192.168.1.10:8848?raft_port=8807,192.168.1.11:8848?raft_port=8808".into(),
        )]);
        let addrs = cfg.resolve_remote_server_addrs();
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0], "http://192.168.1.10:8848");
        assert_eq!(addrs[1], "http://192.168.1.11:8848");
    }

    #[test]
    fn test_resolve_remote_server_addrs_preserves_http_prefix() {
        let cfg = build_config(vec![(
            "batata.member.list",
            "http://10.0.0.1:8848,https://10.0.0.2:8848".into(),
        )]);
        let addrs = cfg.resolve_remote_server_addrs();
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0], "http://10.0.0.1:8848");
        assert_eq!(addrs[1], "https://10.0.0.2:8848");
    }

    #[test]
    fn test_resolve_remote_server_addrs_fallback_to_server_addr() {
        // No member.list and no cluster.conf → falls back to console.remote.server_addr
        let cfg = build_config(vec![(
            "batata.console.remote.server_addr",
            "http://my-server:8848".into(),
        )]);
        let addrs = cfg.resolve_remote_server_addrs();
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "http://my-server:8848");
    }

    #[test]
    fn test_resolve_remote_server_addrs_default_fallback() {
        // No member.list, no cluster.conf, no server_addr → default
        let cfg = build_config(vec![]);
        let addrs = cfg.resolve_remote_server_addrs();
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "http://127.0.0.1:8848");
    }

    // ========================================================================
    // Property Override Extraction Tests
    // ========================================================================

    #[test]
    fn test_extract_overrides_from_args() {
        // Simulate extracting property overrides from a list of args
        let args = vec![
            "batata-server".to_string(),
            "--batata.server.main.port=9090".to_string(),
            "-m".to_string(),
            "standalone".to_string(),
            "--batata.db.url=mysql://localhost/db".to_string(),
            "--db-url".to_string(),
            "postgres://other".to_string(),
        ];

        let mut overrides = Vec::new();
        let mut filtered = Vec::new();
        for arg in args {
            if let Some(rest) = arg.strip_prefix("--")
                && let Some((key, value)) = rest.split_once('=')
                && key.contains('.')
            {
                overrides.push((key.to_string(), value.to_string()));
                continue;
            }
            filtered.push(arg);
        }

        assert_eq!(overrides.len(), 2);
        assert_eq!(
            overrides[0],
            ("batata.server.main.port".to_string(), "9090".to_string())
        );
        assert_eq!(
            overrides[1],
            (
                "batata.db.url".to_string(),
                "mysql://localhost/db".to_string()
            )
        );

        // Filtered args should NOT contain the property overrides
        assert_eq!(filtered.len(), 5);
        assert_eq!(filtered[0], "batata-server");
        assert_eq!(filtered[1], "-m");
        assert_eq!(filtered[2], "standalone");
        assert_eq!(filtered[3], "--db-url");
        assert_eq!(filtered[4], "postgres://other");
    }

    #[test]
    fn test_extract_overrides_no_dot_is_not_property() {
        // --db-url=value has no dot in key, should NOT be extracted as property
        let arg = "--db-url=value";
        let rest = arg.strip_prefix("--").unwrap();
        let (key, _value) = rest.split_once('=').unwrap();
        assert!(!key.contains('.'));
    }

    #[test]
    fn test_extract_overrides_short_flag_ignored() {
        // Short flags like -m should never be treated as overrides
        let arg = "-m";
        assert!(arg.strip_prefix("--").is_none());
    }

    #[test]
    fn test_env_source_nacos_prefix() {
        // Verify NACOS_ env vars produce the correct config keys
        let config = Config::builder()
            .set_override("batata.server.main.port", 8848)
            .unwrap()
            .add_source(
                config::Environment::with_prefix("NACOS")
                    .keep_prefix(true)
                    .separator("_")
                    .try_parsing(true),
            )
            .build()
            .unwrap();

        // Default from set_override should be there
        assert_eq!(config.get_int("batata.server.main.port").unwrap(), 8848);
    }

    #[test]
    fn test_env_source_batata_prefix() {
        // Verify BATATA_ env vars produce the correct config keys
        let config = Config::builder()
            .set_override("batata.db.url", "default://url")
            .unwrap()
            .add_source(
                config::Environment::with_prefix("BATATA")
                    .separator("_")
                    .try_parsing(true),
            )
            .build()
            .unwrap();

        // Default from set_override should be there
        assert_eq!(config.get_string("batata.db.url").unwrap(), "default://url");
    }

    #[test]
    fn test_property_override_highest_priority() {
        // --dotted.key=value overrides should take highest priority
        let config = Config::builder()
            .set_override("batata.server.main.port", 8848)
            .unwrap()
            // Simulate property override applied last (highest priority)
            .set_override("batata.server.main.port", 9090)
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(config.get_int("batata.server.main.port").unwrap(), 9090);
    }

    #[test]
    fn test_config_file_path_default() {
        // When no -c flag, should use default path
        let default_path = None::<String>;
        let resolved = default_path.as_deref().unwrap_or("conf/application.yml");
        assert_eq!(resolved, "conf/application.yml");
    }

    #[test]
    fn test_config_file_path_custom() {
        // When -c is provided, should use custom path
        let custom_path = Some("/etc/batata/app.yml".to_string());
        let resolved = custom_path.as_deref().unwrap_or("conf/application.yml");
        assert_eq!(resolved, "/etc/batata/app.yml");
    }

    // ========================================================================
    // Typed Config: Server defaults and overrides
    // ========================================================================

    #[test]
    fn test_typed_server_port_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.server_main_port(), 8849);
    }

    #[test]
    fn test_typed_server_context_path_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.server_context_path(), "nacos");
    }

    #[test]
    fn test_typed_http_workers_default_zero() {
        let cfg = build_config(vec![]);
        // Default is 0, which means auto-detect
        assert!(cfg.http_workers() > 0); // auto-detected
    }

    #[test]
    fn test_typed_http_workers_custom() {
        let cfg = build_config(vec![("batata.server.http.workers", 8_i64.into())]);
        assert_eq!(cfg.http_workers(), 8);
    }

    #[test]
    fn test_typed_http_keep_alive_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.http_keep_alive_secs(), 75);
    }

    #[test]
    fn test_typed_max_payload_size_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.max_payload_size(), 10_485_760);
    }

    #[test]
    fn test_typed_max_json_size_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.max_json_size(), 5_242_880);
    }

    #[test]
    fn test_typed_http_compression_default() {
        let cfg = build_config(vec![]);
        assert!(cfg.http_compression_enabled());
        assert_eq!(cfg.http_compression_min_size(), 256);
    }

    #[test]
    fn test_typed_shutdown_timeouts_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.shutdown_drain_timeout_secs(), 30);
        assert_eq!(cfg.shutdown_db_close_timeout_secs(), 10);
    }

    #[test]
    fn test_typed_grpc_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.grpc_tcp_keepalive_secs(), 30);
        assert!(cfg.grpc_tcp_nodelay());
        assert_eq!(cfg.grpc_http2_keepalive_interval_secs(), 30);
        assert_eq!(cfg.grpc_http2_keepalive_timeout_secs(), 10);
        assert_eq!(cfg.grpc_concurrency_limit(), 256);
        assert_eq!(cfg.grpc_connection_stale_ms(), 60000);
        assert_eq!(cfg.grpc_push_message_timeout_ms(), 5000);
        assert_eq!(cfg.grpc_bistream_channel_capacity(), 128);
        assert_eq!(cfg.grpc_max_push_timeouts(), 5);
        assert_eq!(cfg.grpc_max_concurrent_streams(), 200);
        assert_eq!(cfg.grpc_max_connections(), 10000);
    }

    // ========================================================================
    // Typed Config: Standalone & Deployment
    // ========================================================================

    #[test]
    fn test_typed_standalone_default_false() {
        let cfg = build_config(vec![]);
        assert!(!cfg.is_standalone());
    }

    #[test]
    fn test_typed_deployment_type_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.deployment_type(), "merged");
    }

    #[test]
    fn test_typed_deployment_type_custom() {
        let cfg = build_config(vec![("batata.deployment.type", "console".into())]);
        assert_eq!(cfg.deployment_type(), "console");
    }

    #[test]
    fn test_typed_function_mode_default_none() {
        let cfg = build_config(vec![]);
        assert!(cfg.function_mode().is_none());
    }

    #[test]
    fn test_typed_function_mode_custom() {
        let cfg = build_config(vec![("batata.function_mode", "config".into())]);
        assert_eq!(cfg.function_mode(), Some("config".to_string()));
    }

    // ========================================================================
    // Typed Config: Console
    // ========================================================================

    #[test]
    fn test_typed_console_port_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.console_server_port(), 8081);
    }

    #[test]
    fn test_typed_console_context_path_default_empty() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.console_server_context_path(), "");
    }

    #[test]
    fn test_typed_console_ui_enabled_default() {
        let cfg = build_config(vec![]);
        assert!(cfg.console_ui_enabled());
    }

    #[test]
    fn test_typed_console_remote_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.console_remote_server_addr(), "http://127.0.0.1:8848");
        assert_eq!(cfg.console_remote_username(), "batata");
        assert_eq!(cfg.console_remote_password(), "batata");
        assert_eq!(cfg.console_remote_connect_timeout_ms(), 5000);
        assert_eq!(cfg.console_remote_read_timeout_ms(), 30000);
        assert_eq!(cfg.console_remote_refresh_interval_secs(), 30);
        assert_eq!(cfg.console_remote_initial_delay_secs(), 5);
    }

    #[test]
    fn test_typed_console_remote_overrides() {
        let cfg = build_config(vec![
            ("batata.console.remote.server_addr", "http://remote:9999".into()),
            ("batata.console.remote.username", "admin".into()),
            ("batata.console.remote.password", "secret".into()),
        ]);
        assert_eq!(cfg.console_remote_server_addr(), "http://remote:9999");
        assert_eq!(cfg.console_remote_username(), "admin");
        assert_eq!(cfg.console_remote_password(), "secret");
    }

    #[test]
    fn test_typed_console_keep_alive_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.console_keep_alive_secs(), 30);
    }

    // ========================================================================
    // Typed Config: Database
    // ========================================================================

    #[test]
    fn test_typed_db_pool_defaults() {
        let cfg = build_config(vec![]);
        // These are the code defaults, NOT the YAML values
        // YAML has 100/10/30/30/600 but code defaults are 200/5/10/10/300
        assert_eq!(cfg.typed.db.pool.max_connections, 200);
        assert_eq!(cfg.typed.db.pool.min_connections, 5);
        assert_eq!(cfg.typed.db.pool.connect_timeout, 10);
        assert_eq!(cfg.typed.db.pool.acquire_timeout, 10);
        assert_eq!(cfg.typed.db.pool.idle_timeout, 300);
        assert_eq!(cfg.typed.db.pool.max_lifetime, 1800);
        assert!(!cfg.typed.db.pool.sqlx_logging);
    }

    #[test]
    fn test_typed_db_pool_overrides() {
        let cfg = build_config(vec![
            ("batata.db.pool.max_connections", 50_i64.into()),
            ("batata.db.pool.min_connections", 2_i64.into()),
            ("batata.db.pool.sqlx_logging", true.into()),
        ]);
        assert_eq!(cfg.typed.db.pool.max_connections, 50);
        assert_eq!(cfg.typed.db.pool.min_connections, 2);
        assert!(cfg.typed.db.pool.sqlx_logging);
    }

    #[test]
    fn test_typed_db_migration_default() {
        let cfg = build_config(vec![]);
        assert!(cfg.db_migration_enabled());
    }

    #[test]
    fn test_typed_db_migration_disabled() {
        let cfg = build_config(vec![("batata.db.migration.enabled", false.into())]);
        assert!(!cfg.db_migration_enabled());
    }

    #[test]
    fn test_typed_datasource_platform_default_empty() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.datasource_platform(), "");
    }

    #[test]
    fn test_typed_datasource_platform_custom() {
        let cfg = build_config(vec![("batata.sql.init.platform", "postgresql".into())]);
        assert_eq!(cfg.datasource_platform(), "postgresql");
    }

    // ========================================================================
    // Typed Config: Authentication
    // ========================================================================

    #[test]
    fn test_typed_auth_system_type_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.auth_system_type(), "default");
    }

    #[test]
    fn test_typed_auth_system_type_ldap() {
        let cfg = build_config(vec![("batata.core.auth.system.type", "ldap".into())]);
        assert_eq!(cfg.auth_system_type(), "ldap");
        assert!(cfg.is_ldap_auth_enabled());
    }

    #[test]
    fn test_typed_auth_console_enabled_default_true() {
        let cfg = build_config(vec![]);
        assert!(cfg.auth_console_enabled());
    }

    #[test]
    fn test_typed_auth_caching_enabled_default_true() {
        let cfg = build_config(vec![]);
        assert!(cfg.typed.core.auth.caching.enabled);
    }

    #[test]
    fn test_typed_token_expire_seconds_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.auth_token_expire_seconds(), 18000);
    }

    #[test]
    fn test_typed_token_secret_key_default_empty() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.token_secret_key(), "");
    }

    #[test]
    fn test_typed_token_secret_key_override() {
        let cfg = build_config(vec![
            ("batata.core.auth.plugin.default.token.secret.key", "secret123".into()),
        ]);
        assert_eq!(cfg.token_secret_key(), "secret123");
    }

    #[test]
    fn test_typed_server_identity_defaults_empty() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.server_identity_key(), "");
        assert_eq!(cfg.server_identity_value(), "");
    }

    #[test]
    fn test_typed_server_identity_overrides() {
        let cfg = build_config(vec![
            ("batata.core.auth.server.identity.key", "my-key".into()),
            ("batata.core.auth.server.identity.value", "my-value".into()),
        ]);
        assert_eq!(cfg.server_identity_key(), "my-key");
        assert_eq!(cfg.server_identity_value(), "my-value");
    }

    // ========================================================================
    // Typed Config: LDAP
    // ========================================================================

    #[test]
    fn test_typed_ldap_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.ldap_url().is_none());
        assert_eq!(cfg.ldap_base_dn(), "");
        assert_eq!(cfg.ldap_bind_dn(), "");
        assert_eq!(cfg.ldap_bind_password(), "");
        assert_eq!(cfg.ldap_user_dn_pattern(), "");
        assert_eq!(cfg.ldap_filter_prefix(), "uid");
        assert_eq!(cfg.ldap_timeout_ms(), 5000);
        assert!(cfg.ldap_case_sensitive());
        assert!(!cfg.ldap_ignore_partial_result_exception());
    }

    #[test]
    fn test_typed_ldap_overrides() {
        let cfg = build_config(vec![
            ("batata.core.auth.ldap.url", "ldap://localhost:389".into()),
            ("batata.core.auth.ldap.base_dc", "dc=example,dc=org".into()),
            ("batata.core.auth.ldap.bind_dn", "cn=admin,dc=example,dc=org".into()),
            ("batata.core.auth.ldap.filter.prefix", "cn".into()),
            ("batata.core.auth.ldap.timeout", 10000_i64.into()),
            ("batata.core.auth.ldap.case.sensitive", false.into()),
            (
                "batata.core.auth.ldap.ignore.partial.result.exception",
                true.into(),
            ),
        ]);
        assert_eq!(cfg.ldap_url(), Some("ldap://localhost:389".to_string()));
        assert_eq!(cfg.ldap_base_dn(), "dc=example,dc=org");
        assert_eq!(cfg.ldap_bind_dn(), "cn=admin,dc=example,dc=org");
        assert_eq!(cfg.ldap_filter_prefix(), "cn");
        assert_eq!(cfg.ldap_timeout_ms(), 10000);
        assert!(!cfg.ldap_case_sensitive());
        assert!(cfg.ldap_ignore_partial_result_exception());
    }

    // ========================================================================
    // Typed Config: Auth Cache
    // ========================================================================

    #[test]
    fn test_typed_auth_cache_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.auth_token_cache_capacity(), 50000);
        assert_eq!(cfg.auth_token_cache_ttl_secs(), 60);
        assert_eq!(cfg.auth_roles_cache_capacity(), 50000);
        assert_eq!(cfg.auth_permissions_cache_capacity(), 20000);
        assert_eq!(cfg.auth_blacklist_capacity(), 100000);
        assert_eq!(cfg.auth_blacklist_ttl_secs(), 86400);
        assert_eq!(cfg.grpc_auth_cache_capacity(), 10000);
        assert_eq!(cfg.grpc_auth_cache_ttl_secs(), 60);
    }

    // ========================================================================
    // Typed Config: OAuth
    // ========================================================================

    #[test]
    fn test_typed_oauth_defaults() {
        let cfg = build_config(vec![]);
        assert!(!cfg.is_oauth_enabled());
        assert_eq!(cfg.oauth_user_creation(), "auto");
        assert_eq!(cfg.oauth_role_sync(), "on_login");
        assert!(cfg.oauth_redirect_uri().is_none());
        assert_eq!(cfg.oauth_discovery_cache_ttl_secs(), 3600);
        assert_eq!(cfg.oauth_discovery_cache_capacity(), 100);
        assert_eq!(cfg.oauth_state_cache_ttl_secs(), 600);
        assert_eq!(cfg.oauth_state_cache_capacity(), 10000);
        assert_eq!(cfg.oauth_http_timeout_secs(), 30);
    }

    // ========================================================================
    // Typed Config: Plugin
    // ========================================================================

    #[test]
    fn test_typed_control_plugin_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.control_plugin_enabled());
        assert_eq!(cfg.control_plugin_default_tps(), 10000);
        assert_eq!(cfg.control_plugin_max_connections(), 50000);
    }

    #[test]
    fn test_typed_control_plugin_overrides() {
        let cfg = build_config(vec![
            ("batata.plugin.control.enabled", false.into()),
            ("batata.plugin.control.default_tps", 5000_i64.into()),
        ]);
        assert!(!cfg.control_plugin_enabled());
        assert_eq!(cfg.control_plugin_default_tps(), 5000);
    }

    #[test]
    fn test_typed_consul_plugin_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.typed.plugin.consul.enabled);
        assert_eq!(cfg.typed.plugin.consul.port, 8500);
    }

    #[test]
    fn test_typed_apollo_plugin_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.typed.plugin.apollo.enabled);
        assert_eq!(cfg.typed.plugin.apollo.port, 8080);
    }

    #[test]
    fn test_typed_visibility_plugin_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.typed.plugin.visibility.enabled);
        assert_eq!(cfg.typed.plugin.visibility.type_, "nacos");
    }

    #[test]
    fn test_typed_webhook_timeout_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.webhook_default_timeout_secs(), 30);
    }

    // ========================================================================
    // Typed Config: Encryption
    // ========================================================================

    #[test]
    fn test_typed_encryption_defaults() {
        let cfg = build_config(vec![]);
        assert!(!cfg.encryption_enabled());
        assert_eq!(cfg.encryption_plugin_type(), "aes-gcm");
        assert!(cfg.encryption_key().is_none());
        assert_eq!(cfg.encryption_reload_interval_ms(), 0);
        assert!(!cfg.encryption_hot_reload_enabled());
    }

    #[test]
    fn test_typed_encryption_overrides() {
        let cfg = build_config(vec![
            ("batata.config.encryption.enabled", true.into()),
            ("batata.config.encryption.plugin.type", "aes-cbc".into()),
            ("batata.config.encryption.key", "base64key".into()),
            ("batata.config.encryption.reload.interval.ms", 5000_i64.into()),
        ]);
        assert!(cfg.encryption_enabled());
        assert_eq!(cfg.encryption_plugin_type(), "aes-cbc");
        assert_eq!(cfg.encryption_key(), Some("base64key".to_string()));
        assert_eq!(cfg.encryption_reload_interval_ms(), 5000);
        assert!(cfg.encryption_hot_reload_enabled());
    }

    // ========================================================================
    // Typed Config: OpenTelemetry
    // ========================================================================

    #[test]
    fn test_typed_otel_defaults() {
        let cfg = build_config(vec![]);
        assert!(!cfg.otel_enabled());
        assert_eq!(cfg.otel_endpoint(), "http://localhost:4317");
        assert_eq!(cfg.otel_service_name(), "batata");
        assert_eq!(cfg.otel_sampling_ratio(), 1.0);
        assert_eq!(cfg.otel_export_timeout_secs(), 10);
    }

    #[test]
    fn test_typed_otel_overrides() {
        let cfg = build_config(vec![
            ("batata.otel.enabled", true.into()),
            ("batata.otel.endpoint", "http://otel:4317".into()),
            ("batata.otel.service_name", "my-service".into()),
            ("batata.otel.sampling_ratio", 0.5_f64.into()),
            ("batata.otel.export_timeout_secs", 30_i64.into()),
        ]);
        assert!(cfg.otel_enabled());
        assert_eq!(cfg.otel_endpoint(), "http://otel:4317");
        assert_eq!(cfg.otel_service_name(), "my-service");
        assert_eq!(cfg.otel_sampling_ratio(), 0.5);
        assert_eq!(cfg.otel_export_timeout_secs(), 30);
    }

    // ========================================================================
    // Typed Config: Logging
    // ========================================================================

    #[test]
    fn test_typed_log_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.log_dir().is_none());
        assert!(cfg.log_console_enabled());
        assert!(cfg.log_file_enabled());
        assert_eq!(cfg.log_level(), "info");
    }

    #[test]
    fn test_typed_log_overrides() {
        let cfg = build_config(vec![
            ("batata.logs.path", "/var/log/batata".into()),
            ("batata.logs.console.enabled", false.into()),
            ("batata.logs.level", "debug".into()),
        ]);
        assert_eq!(cfg.log_dir(), Some("/var/log/batata".to_string()));
        assert!(!cfg.log_console_enabled());
        assert_eq!(cfg.log_level(), "debug");
    }

    // ========================================================================
    // Typed Config: xDS / Mesh
    // ========================================================================

    #[test]
    fn test_typed_xds_defaults() {
        let cfg = build_config(vec![]);
        assert!(!cfg.xds_enabled());
        assert_eq!(cfg.xds_server_port(), 15010);
        assert_eq!(cfg.xds_server_id(), "batata-xds-server");
        assert_eq!(cfg.xds_sync_interval_ms(), 5000);
        assert!(cfg.xds_generate_listeners());
        assert!(cfg.xds_generate_routes());
        assert_eq!(cfg.xds_default_listener_port(), 15001);
        assert!(!cfg.xds_tls_enabled());
    }

    #[test]
    fn test_typed_xds_overrides() {
        let cfg = build_config(vec![
            ("batata.mesh.xds.enabled", true.into()),
            ("batata.mesh.xds.port", 16010_i64.into()),
            ("batata.mesh.xds.server.id", "custom-xds".into()),
        ]);
        assert!(cfg.xds_enabled());
        assert_eq!(cfg.xds_server_port(), 16010);
        assert_eq!(cfg.xds_server_id(), "custom-xds");
    }

    // ========================================================================
    // Typed Config: MCP Registry
    // ========================================================================

    #[test]
    fn test_typed_mcp_registry_defaults() {
        let cfg = build_config(vec![]);
        assert!(!cfg.mcp_registry_enabled());
        assert_eq!(cfg.mcp_registry_port(), 9080);
    }

    #[test]
    fn test_typed_mcp_registry_override() {
        let cfg = build_config(vec![
            ("batata.ai.mcp.registry.enabled", true.into()),
            ("batata.ai.mcp.registry.port", 9081_i64.into()),
        ]);
        assert!(cfg.mcp_registry_enabled());
        assert_eq!(cfg.mcp_registry_port(), 9081);
    }

    // ========================================================================
    // Typed Config: Raft
    // ========================================================================

    #[test]
    fn test_typed_raft_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.raft_election_timeout_ms(), 5000);
        assert_eq!(cfg.raft_heartbeat_interval_ms(), 1000);
        assert_eq!(cfg.raft_rpc_timeout_ms(), 5000);
        assert_eq!(cfg.raft_snapshot_threshold(), 10000);
        assert_eq!(cfg.raft_snapshot_transfer_timeout_ms(), 30000);
        assert_eq!(cfg.raft_forward_max_retries(), 3);
        assert_eq!(cfg.raft_forward_initial_delay_ms(), 200);
        assert_eq!(cfg.raft_peer_connect_timeout_secs(), 30);
        assert_eq!(cfg.raft_peer_connect_retry_interval_ms(), 500);
    }

    #[test]
    fn test_typed_raft_grpc_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.raft_grpc_tcp_keepalive_secs(), 10);
        assert!(cfg.raft_grpc_tcp_nodelay());
        assert_eq!(cfg.raft_grpc_http2_keepalive_interval_secs(), 10);
        assert_eq!(cfg.raft_grpc_http2_keepalive_timeout_secs(), 5);
    }

    #[test]
    fn test_typed_raft_overrides() {
        let cfg = build_config(vec![
            ("batata.raft.election_timeout_ms", 10000_i64.into()),
            ("batata.raft.heartbeat_interval_ms", 2000_i64.into()),
        ]);
        assert_eq!(cfg.raft_election_timeout_ms(), 10000);
        assert_eq!(cfg.raft_heartbeat_interval_ms(), 2000);
    }

    // ========================================================================
    // Typed Config: RocksDB
    // ========================================================================

    #[test]
    fn test_typed_rocksdb_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.rocksdb_write_buffer_mb(), 128);
        assert_eq!(cfg.rocksdb_max_write_buffers(), 4);
        assert_eq!(cfg.rocksdb_max_background_jobs(), 4);
        assert_eq!(cfg.rocksdb_block_cache_mb(), 256);
        assert_eq!(cfg.rocksdb_bloom_filter_bits(), 10.0);
        assert!(cfg.rocksdb_level_compaction_dynamic());
        assert_eq!(cfg.rocksdb_bottommost_compression(), "zstd");
        assert_eq!(cfg.rocksdb_compression(), "lz4");
        assert!(!cfg.rocksdb_enable_statistics());
        assert!(cfg.rocksdb_whole_key_filtering());
        assert_eq!(cfg.rocksdb_data_block_hash_ratio(), 0.75);
        assert!(!cfg.rocksdb_sm_sync());
        assert!(!cfg.rocksdb_sm_disable_wal());
        assert_eq!(cfg.rocksdb_history_write_buffer_mb(), 0);
        assert!(!cfg.rocksdb_history_bloom_filter());
    }

    #[test]
    fn test_typed_rocksdb_overrides() {
        let cfg = build_config(vec![
            ("batata.rocksdb.write_buffer_mb", 256_i64.into()),
            ("batata.rocksdb.compression", "zstd".into()),
            ("batata.rocksdb.enable_statistics", true.into()),
        ]);
        assert_eq!(cfg.rocksdb_write_buffer_mb(), 256);
        assert_eq!(cfg.rocksdb_compression(), "zstd");
        assert!(cfg.rocksdb_enable_statistics());
    }

    // ========================================================================
    // Typed Config: Persistence / Embedded
    // ========================================================================

    #[test]
    fn test_typed_persistence_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.embedded_data_dir(), "data");
        assert_eq!(cfg.embedded_db_name(), "batata_rocksdb");
        assert_eq!(cfg.embedded_rocksdb_dir(), "data/batata_rocksdb");
    }

    #[test]
    fn test_typed_persistence_overrides() {
        let cfg = build_config(vec![
            ("batata.persistence.embedded.data_dir", "/var/data".into()),
            ("batata.persistence.embedded.db_name", "custom_db".into()),
        ]);
        assert_eq!(cfg.embedded_data_dir(), "/var/data");
        assert_eq!(cfg.embedded_db_name(), "custom_db");
        assert_eq!(cfg.embedded_rocksdb_dir(), "/var/data/custom_db");
    }

    // ========================================================================
    // Typed Config: Naming
    // ========================================================================

    #[test]
    fn test_typed_naming_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.expire_instance_enabled());
        assert!(!cfg.data_warmup());
        assert_eq!(cfg.naming_heartbeat_check_interval_secs(), 5);
        assert_eq!(cfg.naming_ttl_monitor_interval_secs(), 5);
        assert_eq!(cfg.naming_deregister_monitor_interval_secs(), 10);
    }

    #[test]
    fn test_typed_naming_overrides() {
        let cfg = build_config(vec![
            ("batata.naming.expire_instance", false.into()),
            ("batata.naming.data.warmup", true.into()),
            ("batata.naming.healthcheck.heartbeat_interval_secs", 10_i64.into()),
        ]);
        assert!(!cfg.expire_instance_enabled());
        assert!(cfg.data_warmup());
        assert_eq!(cfg.naming_heartbeat_check_interval_secs(), 10);
    }

    // ========================================================================
    // Typed Config: Config Module
    // ========================================================================

    #[test]
    fn test_typed_config_retention_days_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.config_rentention_days(), 30);
    }

    #[test]
    fn test_typed_config_gray_max_count_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.config_gray_max_version_count(), 10);
    }

    #[test]
    fn test_typed_config_read_cache_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.config_read_cache_ttl_secs(), 0);
        assert_eq!(cfg.config_read_cache_max_entries(), 10000);
    }

    // ========================================================================
    // Typed Config: Metrics
    // ========================================================================

    #[test]
    fn test_typed_metrics_defaults() {
        let cfg = build_config(vec![]);
        assert!(cfg.metrics_system_stats_enabled());
        assert_eq!(cfg.metrics_system_stats_interval_secs(), 15);
    }

    // ========================================================================
    // Typed Config: gRPC TLS
    // ========================================================================

    #[test]
    fn test_typed_grpc_tls_defaults() {
        let cfg = build_config(vec![]);
        assert!(!cfg.grpc_sdk_tls_enabled());
        assert!(!cfg.grpc_cluster_tls_enabled());
        assert!(cfg.grpc_tls_cert_path().is_none());
        assert!(cfg.grpc_tls_key_path().is_none());
        assert!(cfg.grpc_tls_ca_cert_path().is_none());
        assert!(!cfg.grpc_mtls_enabled());
    }

    #[test]
    fn test_typed_grpc_tls_overrides() {
        let cfg = build_config(vec![
            ("batata.remote.server.grpc.sdk.tls.enabled", true.into()),
            ("batata.remote.server.grpc.tls.cert.path", "/certs/server.crt".into()),
            ("batata.remote.server.grpc.tls.key.path", "/certs/server.key".into()),
            ("batata.remote.server.grpc.tls.mtls.enabled", true.into()),
        ]);
        assert!(cfg.grpc_sdk_tls_enabled());
        assert_eq!(cfg.grpc_tls_cert_path(), Some("/certs/server.crt".to_string()));
        assert_eq!(cfg.grpc_tls_key_path(), Some("/certs/server.key".to_string()));
        assert!(cfg.grpc_mtls_enabled());
    }

    // ========================================================================
    // Typed Config: HTTP Access Log
    // ========================================================================

    #[test]
    fn test_typed_http_access_log_default() {
        let cfg = build_config(vec![]);
        assert!(cfg.http_access_log_enabled());
    }

    #[test]
    fn test_typed_http_access_log_disabled() {
        let cfg = build_config(vec![("batata.server.http.access_log.enabled", false.into())]);
        assert!(!cfg.http_access_log_enabled());
    }

    // ========================================================================
    // Typed Config: gRPC Advanced Tuning
    // ========================================================================

    #[test]
    fn test_typed_grpc_advanced_defaults() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.grpc_initial_connection_window_size(), 1_048_576);
        assert_eq!(cfg.grpc_initial_stream_window_size(), 524_288);
        assert_eq!(cfg.grpc_max_frame_size(), 16_384);
        assert_eq!(cfg.notify_subscriber_timeout_ms(), 0);
    }

    // ========================================================================
    // Typed Config: Deeply Nested Paths
    // ========================================================================

    #[test]
    fn test_typed_deeply_nested_ldap_ignore() {
        // Test the deeply nested path:
        // batata.core.auth.ldap.ignore.partial.result.exception
        let cfg = build_config(vec![
            (
                "batata.core.auth.ldap.ignore.partial.result.exception",
                true.into(),
            ),
        ]);
        assert!(cfg.ldap_ignore_partial_result_exception());
    }

    #[test]
    fn test_typed_deeply_nested_token_cache() {
        // Test: batata.core.auth.plugin.default.token.cache.enable
        let cfg = build_config(vec![
            ("batata.core.auth.plugin.default.token.cache.enable", true.into()),
        ]);
        assert!(cfg.typed.core.auth.plugin.default.token.cache.enable);
    }

    #[test]
    fn test_typed_deeply_nested_anonymous_ai() {
        // Test: batata.core.auth.default.anonymous.ai.enabled
        let cfg = build_config(vec![
            ("batata.core.auth.default.anonymous.ai.enabled", true.into()),
        ]);
        assert!(cfg.typed.core.auth.default.anonymous.ai.enabled);
    }

    // ========================================================================
    // Typed Config: Compatibility with YAML file loading
    // ========================================================================

    #[test]
    fn test_typed_yaml_file_loading() {
        // Test that typed config works when loading from a YAML file
        // (not just from overrides)
        let yaml_content = r#"
batata.server.main.port: 9999
batata.standalone: false
batata.core.auth.enabled: true
batata.core.auth.system.type: ldap
batata.console.port: 7777
batata.db.pool.max_connections: 42
"#;
        let config = Config::builder()
            .add_source(config::File::from_str(yaml_content, config::FileFormat::Yaml))
            .build()
            .unwrap();
        let typed: typed_config::BatataTypedConfig =
            config.get("batata").unwrap_or_default();

        assert_eq!(typed.server.main.port, 9999);
        assert!(!typed.standalone);
        assert!(typed.core.auth.enabled);
        assert_eq!(typed.core.auth.system.type_, "ldap");
        assert_eq!(typed.console.port, 7777);
        assert_eq!(typed.db.pool.max_connections, 42);
    }

    #[test]
    fn test_typed_partial_yaml_loading() {
        // Test that partial YAML (missing keys) uses defaults
        let yaml_content = r#"
batata.server.main.port: 8888
"#;
        let config = Config::builder()
            .add_source(config::File::from_str(yaml_content, config::FileFormat::Yaml))
            .build()
            .unwrap();
        let typed: typed_config::BatataTypedConfig =
            config.get("batata").unwrap_or_default();

        // Set value
        assert_eq!(typed.server.main.port, 8888);
        // Default values for missing keys
        assert_eq!(typed.server.context_path, "nacos");
        assert!(!typed.standalone);
        assert_eq!(typed.deployment.type_, "merged");
        assert_eq!(typed.console.port, 8081);
    }

    // ========================================================================
    // Typed Config: Environment variable override simulation
    // ========================================================================

    #[test]
    fn test_typed_env_var_override_simulation() {
        // Simulate env var override: BATATA_SERVER_MAIN_PORT=9999
        // which maps to batata.server.main.port=9999
        let config = Config::builder()
            .set_override("batata.server.main.port", 9999_i64)
            .unwrap()
            .set_override("batata.core.auth.enabled", true)
            .unwrap()
            .build()
            .unwrap();
        let typed: typed_config::BatataTypedConfig =
            config.get("batata").unwrap_or_default();

        assert_eq!(typed.server.main.port, 9999);
        assert!(typed.core.auth.enabled);
    }

    // ========================================================================
    // Typed Config: All defaults via empty config
    // ========================================================================

    #[test]
    fn test_typed_all_defaults_from_empty_config() {
        // Build a config with no batata keys at all
        let config = Config::builder().build().unwrap();
        let typed: typed_config::BatataTypedConfig =
            config.get("batata").unwrap_or_default();

        // Server
        assert_eq!(typed.server.main.port, 8849);
        assert_eq!(typed.server.context_path, "nacos");
        assert_eq!(typed.server.address, "0.0.0.0");
        // Console
        assert_eq!(typed.console.port, 8081);
        assert_eq!(typed.console.context_path, "");
        assert!(typed.console.ui.enabled);
        // DB
        assert_eq!(typed.db.pool.max_connections, 200);
        assert!(typed.db.migration.enabled);
        // Auth
        assert!(!typed.core.auth.enabled);
        assert!(!typed.core.auth.admin.enabled);
        assert!(typed.core.auth.console.enabled);
        assert_eq!(typed.core.auth.system.type_, "default");
        // RateLimit
        assert!(!typed.ratelimit.enabled);
        assert_eq!(typed.ratelimit.max_requests, 100);
        // Plugin
        assert!(typed.plugin.control.enabled);
        assert!(typed.plugin.consul.enabled);
        assert!(typed.plugin.apollo.enabled);
        assert!(typed.plugin.visibility.enabled);
        // Otel
        assert!(!typed.otel.enabled);
        assert_eq!(typed.otel.service_name, "batata");
        // Logs
        assert!(typed.logs.console.enabled);
        assert_eq!(typed.logs.level, "info");
        // RocksDB
        assert_eq!(typed.rocksdb.write_buffer_mb, 128);
        assert_eq!(typed.rocksdb.compression, "lz4");
    }

    // ========================================================================
    // Server Address (typed config)
    // ========================================================================

    #[test]
    fn test_server_address_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.server_address(), "0.0.0.0");
    }

    #[test]
    fn test_server_address_override() {
        let cfg = build_config(vec![("batata.server.address", "192.168.1.1".into())]);
        assert_eq!(cfg.server_address(), "192.168.1.1");
    }

    // ========================================================================
    // Derived port methods
    // ========================================================================

    #[test]
    fn test_sdk_server_port() {
        let cfg = build_config(vec![("batata.server.main.port", 8848_i64.into())]);
        assert_eq!(cfg.sdk_server_port(), 8848 + SDK_GRPC_PORT_DEFAULT_OFFSET);
    }

    #[test]
    fn test_cluster_server_port() {
        let cfg = build_config(vec![("batata.server.main.port", 8848_i64.into())]);
        assert_eq!(
            cfg.cluster_server_port(),
            8848 + CLUSTER_GRPC_PORT_DEFAULT_OFFSET
        );
    }

    #[test]
    fn test_raft_port() {
        let cfg = build_config(vec![("batata.server.main.port", 8848_i64.into())]);
        assert_eq!(
            cfg.raft_port(),
            8848 - batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT
        );
    }

    // ========================================================================
    // Startup mode and version
    // ========================================================================

    #[test]
    fn test_startup_mode_standalone() {
        let cfg = build_config(vec![("batata.standalone", true.into())]);
        assert_eq!(cfg.startup_mode(), "standalone");
    }

    #[test]
    fn test_startup_mode_cluster() {
        let cfg = build_config(vec![("batata.standalone", false.into())]);
        assert_eq!(cfg.startup_mode(), "cluster");
    }

    #[test]
    fn test_version_matches_cargo() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(cfg.batata_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_compat_version_default_empty() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.compat_version(), "");
    }

    #[test]
    fn test_compat_version_override() {
        let cfg = build_config(vec![("nacos.version", "3.2.3".into())]);
        assert_eq!(cfg.compat_version(), "3.2.3");
    }

    // ========================================================================
    // Storage backend and deploy topology (derived)
    // ========================================================================

    #[test]
    fn test_storage_backend_mysql() {
        let cfg = build_config(vec![("batata.sql.init.platform", "mysql".into())]);
        assert_eq!(cfg.storage_backend(), batata_persistence::StorageBackend::ExternalDb);
    }

    #[test]
    fn test_storage_backend_postgresql() {
        let cfg = build_config(vec![("batata.sql.init.platform", "postgresql".into())]);
        assert_eq!(cfg.storage_backend(), batata_persistence::StorageBackend::ExternalDb);
    }

    #[test]
    fn test_storage_backend_embedded() {
        let cfg = build_config(vec![("batata.sql.init.platform", "".into())]);
        assert_eq!(cfg.storage_backend(), batata_persistence::StorageBackend::Embedded);
    }

    #[test]
    fn test_deploy_topology_standalone() {
        let cfg = build_config(vec![("batata.standalone", true.into())]);
        assert_eq!(cfg.deploy_topology(), batata_persistence::DeployTopology::Standalone);
    }

    #[test]
    fn test_deploy_topology_cluster() {
        let cfg = build_config(vec![("batata.standalone", false.into())]);
        assert_eq!(cfg.deploy_topology(), batata_persistence::DeployTopology::Cluster);
    }

    // ========================================================================
    // Shutdown timeouts (override tests; defaults in test_typed_shutdown_timeouts_default)
    // ========================================================================

    #[test]
    fn test_shutdown_drain_timeout_override() {
        let cfg = build_config(vec![("batata.server.shutdown.drain_timeout", 60_i64.into())]);
        assert_eq!(cfg.shutdown_drain_timeout_secs(), 60);
    }

    #[test]
    fn test_shutdown_db_close_timeout_override() {
        let cfg = build_config(vec![("batata.server.shutdown.db_close_timeout", 30_i64.into())]);
        assert_eq!(cfg.shutdown_db_close_timeout_secs(), 30);
    }

    // ========================================================================
    // Datasource log (unique; platform tests in test_typed_datasource_platform_*)
    // ========================================================================

    #[test]
    fn test_plugin_datasource_log_default() {
        let cfg = build_config(vec![]);
        assert!(!cfg.plugin_datasource_log());
    }

    #[test]
    fn test_plugin_datasource_log_enabled() {
        let cfg = build_config(vec![("batata.plugin.datasource.log.enabled", true.into())]);
        assert!(cfg.plugin_datasource_log());
    }

    // ========================================================================
    // DB URL (typed config, Option<String>)
    // ========================================================================

    #[test]
    fn test_typed_db_url_default_none() {
        let cfg = build_config(vec![]);
        assert!(cfg.typed.db.url.is_none());
    }

    #[test]
    fn test_typed_db_url_override() {
        let cfg = build_config(vec![(
            "batata.db.url",
            "mysql://user:pass@localhost:3306/batata".into(),
        )]);
        assert_eq!(
            cfg.typed.db.url.as_deref(),
            Some("mysql://user:pass@localhost:3306/batata")
        );
    }

    // ========================================================================
    // ========================================================================
    // Typed config vs accessor consistency (override values)
    // ========================================================================

    #[test]
    fn test_consistency_server_port() {
        let cfg = build_config(vec![("batata.server.main.port", 9090_i64.into())]);
        assert_eq!(cfg.typed.server.main.port, 9090);
        assert_eq!(cfg.server_main_port(), 9090);
    }

    #[test]
    fn test_consistency_server_context_path() {
        let cfg = build_config(vec![("batata.server.context_path", "/custom".into())]);
        assert_eq!(cfg.typed.server.context_path, "/custom");
        assert_eq!(cfg.server_context_path(), "/custom");
    }

    #[test]
    fn test_consistency_auth_enabled() {
        let cfg = build_config(vec![("batata.core.auth.enabled", true.into())]);
        assert!(cfg.typed.core.auth.enabled);
        assert!(cfg.auth_enabled());
    }

    #[test]
    fn test_consistency_console_port() {
        let cfg = build_config(vec![("batata.console.port", 3000_i64.into())]);
        assert_eq!(cfg.typed.console.port, 3000);
        assert_eq!(cfg.console_server_port(), 3000);
    }

    #[test]
    fn test_consistency_standalone() {
        let cfg = build_config(vec![("batata.standalone", true.into())]);
        assert!(cfg.typed.standalone);
        assert!(cfg.is_standalone());
    }

    #[test]
    fn test_consistency_deployment_type() {
        let cfg = build_config(vec![("batata.deployment.type", "console".into())]);
        assert_eq!(cfg.typed.deployment.type_, "console");
        assert_eq!(cfg.deployment_type(), "console");
    }

    #[test]
    fn test_consistency_auth_system_type() {
        let cfg = build_config(vec![("batata.core.auth.system.type", "ldap".into())]);
        assert_eq!(cfg.typed.core.auth.system.type_, "ldap");
        assert_eq!(cfg.auth_system_type(), "ldap");
    }

    #[test]
    fn test_consistency_token_expire_seconds() {
        let cfg = build_config(vec![
            ("batata.core.auth.plugin.default.token.expire.seconds", 3600_i64.into()),
        ]);
        assert_eq!(
            cfg.typed.core.auth.plugin.default.token.expire.seconds,
            3600
        );
        assert_eq!(cfg.auth_token_expire_seconds(), 3600);
    }

    #[test]
    fn test_consistency_db_pool_max_connections() {
        let cfg = build_config(vec![("batata.db.pool.max_connections", 50_i64.into())]);
        assert_eq!(cfg.typed.db.pool.max_connections, 50);
    }

    #[test]
    fn test_consistency_ratelimit_enabled() {
        let cfg = build_config(vec![("batata.ratelimit.enabled", true.into())]);
        assert!(cfg.typed.ratelimit.enabled);
        assert!(cfg.ratelimit_enabled());
    }

    #[test]
    fn test_consistency_encryption_enabled() {
        let cfg = build_config(vec![("batata.config.encryption.enabled", true.into())]);
        assert!(cfg.typed.config.encryption.enabled);
        assert!(cfg.encryption_enabled());
    }

    #[test]
    fn test_consistency_otel_enabled() {
        let cfg = build_config(vec![("batata.otel.enabled", true.into())]);
        assert!(cfg.typed.otel.enabled);
        assert!(cfg.otel_enabled());
    }

    #[test]
    fn test_consistency_log_level() {
        let cfg = build_config(vec![("batata.logs.level", "debug".into())]);
        assert_eq!(cfg.typed.logs.level, "debug");
        assert_eq!(cfg.log_level(), "debug");
    }

    #[test]
    fn test_consistency_xds_enabled() {
        let cfg = build_config(vec![("batata.mesh.xds.enabled", true.into())]);
        assert!(cfg.typed.mesh.xds.enabled);
        assert!(cfg.xds_enabled());
    }

    #[test]
    fn test_consistency_server_address() {
        let cfg = build_config(vec![("batata.server.address", "10.0.0.1".into())]);
        assert_eq!(cfg.typed.server.address, "10.0.0.1");
        assert_eq!(cfg.server_address(), "10.0.0.1");
    }

    // ========================================================================
    // Default value consistency: all defaults from empty config
    // ========================================================================

    #[test]
    fn test_default_consistency_all_empty_config() {
        let cfg = build_config(vec![]);

        // Server
        assert_eq!(cfg.server_main_port(), 8849); // typed default
        assert_eq!(cfg.server_address(), "0.0.0.0");
        assert_eq!(cfg.server_context_path(), "nacos");

        // Console
        assert_eq!(cfg.console_server_port(), 8081);
        assert_eq!(cfg.console_server_context_path(), "");
        assert!(cfg.console_ui_enabled());

        // Auth
        assert!(!cfg.auth_enabled());
        assert!(!cfg.auth_admin_enabled());
        assert!(cfg.auth_console_enabled());
        assert_eq!(cfg.auth_system_type(), "default");

        // DB pool
        assert_eq!(cfg.typed.db.pool.max_connections, 200);
        assert_eq!(cfg.typed.db.pool.min_connections, 5);

        // RateLimit
        assert!(!cfg.ratelimit_enabled());
        assert_eq!(cfg.ratelimit_max_requests(), 100);

        // Otel
        assert!(!cfg.otel_enabled());
        assert_eq!(cfg.otel_service_name(), "batata");

        // Logs
        assert!(cfg.log_console_enabled());
        assert_eq!(cfg.log_level(), "info");

        // Capacity & Health (now typed config, default values)
        assert_eq!(cfg.notify_connect_timeout(), 100);
        assert_eq!(cfg.notify_socket_timeout(), 200);
        assert!(cfg.is_health_check());
        assert_eq!(cfg.max_health_check_fail_count(), 12);
        assert_eq!(cfg.max_content(), 10 * 1024 * 1024);
        assert!(cfg.is_manage_capacity());
        assert!(!cfg.is_capacity_limit_check());
        assert_eq!(cfg.default_cluster_quota(), 100_000);
        assert_eq!(cfg.default_group_quota(), 200);
        assert_eq!(cfg.default_max_size(), 100 * 1024);
        assert_eq!(cfg.default_max_aggr_count(), 10_000);
        assert_eq!(cfg.default_max_aggr_size(), 1024);
        assert_eq!(cfg.config_rentention_days(), 30);
    }

    // ========================================================================
    // Behavior consistency: typed config vs accessor for config.* keys
    // ========================================================================

    #[test]
    fn test_consistency_notify_connect_timeout() {
        let cfg = build_config(vec![("batata.config.notify.connect_timeout", 300_i64.into())]);
        assert_eq!(cfg.typed.config.notify.connect_timeout, 300);
        assert_eq!(cfg.notify_connect_timeout(), 300);
    }

    #[test]
    fn test_consistency_notify_socket_timeout() {
        let cfg = build_config(vec![("batata.config.notify.socket_timeout", 600_i64.into())]);
        assert_eq!(cfg.typed.config.notify.socket_timeout, 600);
        assert_eq!(cfg.notify_socket_timeout(), 600);
    }

    #[test]
    fn test_consistency_is_health_check() {
        let cfg = build_config(vec![("batata.config.health_check.enabled", false.into())]);
        assert!(!cfg.typed.config.health_check.enabled);
        assert!(!cfg.is_health_check());
    }

    #[test]
    fn test_consistency_max_health_check_fail_count() {
        let cfg = build_config(vec![("batata.config.health_check.max_fail_count", 24_i64.into())]);
        assert_eq!(cfg.typed.config.health_check.max_fail_count, 24);
        assert_eq!(cfg.max_health_check_fail_count(), 24);
    }

    #[test]
    fn test_consistency_max_content() {
        let cfg = build_config(vec![("batata.config.max_content", 999_i64.into())]);
        assert_eq!(cfg.typed.config.max_content, 999);
        assert_eq!(cfg.max_content(), 999);
    }

    #[test]
    fn test_consistency_manage_capacity() {
        let cfg = build_config(vec![("batata.config.capacity.manage_enabled", false.into())]);
        assert!(!cfg.typed.config.capacity.manage_enabled);
        assert!(!cfg.is_manage_capacity());
    }

    #[test]
    fn test_consistency_capacity_limit_check() {
        let cfg = build_config(vec![("batata.config.capacity.limit_check", true.into())]);
        assert!(cfg.typed.config.capacity.limit_check);
        assert!(cfg.is_capacity_limit_check());
    }

    #[test]
    fn test_consistency_default_cluster_quota() {
        let cfg = build_config(vec![("batata.config.capacity.default_cluster_quota", 42_i64.into())]);
        assert_eq!(cfg.typed.config.capacity.default_cluster_quota, 42);
        assert_eq!(cfg.default_cluster_quota(), 42);
    }

    #[test]
    fn test_consistency_default_group_quota() {
        let cfg = build_config(vec![("batata.config.capacity.default_group_quota", 500_i64.into())]);
        assert_eq!(cfg.typed.config.capacity.default_group_quota, 500);
        assert_eq!(cfg.default_group_quota(), 500);
    }

    #[test]
    fn test_consistency_default_max_size() {
        let cfg = build_config(vec![("batata.config.capacity.default_max_size", 200_000_i64.into())]);
        assert_eq!(cfg.typed.config.capacity.default_max_size, 200_000);
        assert_eq!(cfg.default_max_size(), 200_000);
    }

    #[test]
    fn test_consistency_default_max_aggr_count() {
        let cfg = build_config(vec![("batata.config.capacity.default_max_aggr_count", 5_000_i64.into())]);
        assert_eq!(cfg.typed.config.capacity.default_max_aggr_count, 5_000);
        assert_eq!(cfg.default_max_aggr_count(), 5_000);
    }

    #[test]
    fn test_consistency_default_max_aggr_size() {
        let cfg = build_config(vec![("batata.config.capacity.default_max_aggr_size", 2048_i64.into())]);
        assert_eq!(cfg.typed.config.capacity.default_max_aggr_size, 2048);
        assert_eq!(cfg.default_max_aggr_size(), 2048);
    }

    #[test]
    fn test_consistency_config_retention_days() {
        let cfg = build_config(vec![("batata.config.retention.days", 90_i64.into())]);
        assert_eq!(cfg.typed.config.retention.days, 90);
        assert_eq!(cfg.config_rentention_days(), 90);
    }
}
