//! Configuration management for Batata server
//!
//! This module handles loading and accessing application configuration.

use std::time::Duration;

use clap::Parser;
use config::{Config, Environment};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use crate::auth::model::{DEFAULT_TOKEN_EXPIRE_SECONDS, TOKEN_EXPIRE_SECONDS};

use super::constants::{
    CONFIG_RENTENTION_DAYS, DATASOURCE_PLATFORM_PROPERTY, DEFAULT_CLUSTER_QUOTA,
    DEFAULT_GROUP_QUOTA, DEFAULT_MAX_AGGR_COUNT, DEFAULT_MAX_AGGR_SIZE, DEFAULT_MAX_SIZE,
    DEFAULT_SERVER_PORT, FUNCTION_MODE_PROPERTY_NAME, IS_CAPACITY_LIMIT_CHECK, IS_HEALTH_CHECK,
    IS_MANAGE_CAPACITY, MAX_CONTENT, MAX_HEALTH_CHECK_FAIL_COUNT, NACOS_CONSOLE_MODE,
    NACOS_CONSOLE_MODE_LOCAL, NACOS_CONSOLE_MODE_REMOTE, NACOS_CONSOLE_REMOTE_CONNECT_TIMEOUT_MS,
    NACOS_CONSOLE_REMOTE_PASSWORD, NACOS_CONSOLE_REMOTE_READ_TIMEOUT_MS,
    NACOS_CONSOLE_REMOTE_SERVER_ADDR, NACOS_CONSOLE_REMOTE_USERNAME, NACOS_DEPLOYMENT_TYPE,
    NACOS_DEPLOYMENT_TYPE_MERGED, NACOS_PLUGIN_DATASOURCE_LOG, NOTIFY_CONNECT_TIMEOUT,
    NOTIFY_SOCKET_TIMEOUT, SERVER_PORT_PROPERTY, STANDALONE_MODE_PROPERTY_NAME,
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
}

/// Application configuration loaded from config files and environment
#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub config: Config,
}

impl Configuration {
    pub fn new() -> Self {
        let args = Cli::parse();
        let mut config_builder = Config::builder()
            .add_source(
                Environment::with_prefix("nacos")
                    .separator(".")
                    .try_parsing(true),
            )
            .add_source(config::File::with_name("conf/application.yml"));

        config_builder = config_builder.add_source(config::File::with_name("conf/application.yml"));

        if let Some(v) = args.mode {
            config_builder = config_builder
                .set_override("nacos.standalone", v == "standalone")
                .expect("Failed to set standalone mode override");
        }
        if let Some(v) = args.function_mode {
            config_builder = config_builder
                .set_override("nacos.function.mode", v)
                .expect("Failed to set function mode override");
        }
        if let Some(v) = args.deployment {
            config_builder = config_builder
                .set_override(NACOS_DEPLOYMENT_TYPE, v)
                .expect("Failed to set deployment type override");
        }

        let app_config = config_builder
            .build()
            .expect("Failed to build configuration - check conf/application.yml");

        Configuration { config: app_config }
    }

    // ========================================================================
    // Deployment Configuration
    // ========================================================================

    pub fn deployment_type(&self) -> String {
        self.config
            .get_string(NACOS_DEPLOYMENT_TYPE)
            .unwrap_or(NACOS_DEPLOYMENT_TYPE_MERGED.to_string())
    }

    pub fn is_standalone(&self) -> bool {
        self.config
            .get_bool(STANDALONE_MODE_PROPERTY_NAME)
            .unwrap_or(false)
    }

    pub fn startup_mode(&self) -> String {
        if self.is_standalone() {
            "standalone".to_string()
        } else {
            "cluster".to_string()
        }
    }

    pub fn function_mode(&self) -> Option<String> {
        self.config.get_string(FUNCTION_MODE_PROPERTY_NAME).ok()
    }

    pub fn version(&self) -> String {
        self.config
            .get_string("nacos.version")
            .unwrap_or("".to_string())
    }

    // ========================================================================
    // Server Configuration
    // ========================================================================

    pub fn server_address(&self) -> String {
        self.config
            .get_string("server.address")
            .unwrap_or("0.0.0.0".to_string())
    }

    pub fn server_main_port(&self) -> u16 {
        self.config
            .get_int(SERVER_PORT_PROPERTY)
            .unwrap_or(DEFAULT_SERVER_PORT.into()) as u16
    }

    pub fn server_context_path(&self) -> String {
        self.config
            .get_string("nacos.server.contextPath")
            .unwrap_or("nacos".to_string())
    }

    pub fn sdk_server_port(&self) -> u16 {
        self.server_main_port() + SDK_GRPC_PORT_DEFAULT_OFFSET
    }

    pub fn cluster_server_port(&self) -> u16 {
        self.server_main_port() + CLUSTER_GRPC_PORT_DEFAULT_OFFSET
    }

    // ========================================================================
    // Console Configuration
    // ========================================================================

    pub fn console_server_port(&self) -> u16 {
        self.config.get_int("nacos.console.port").unwrap_or(8081) as u16
    }

    pub fn console_server_context_path(&self) -> String {
        self.config
            .get_string("nacos.console.contextPath")
            .unwrap_or("".to_string())
    }

    pub fn console_ui_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.console.ui.enabled")
            .unwrap_or(true)
    }

    pub fn console_mode(&self) -> String {
        self.config
            .get_string(NACOS_CONSOLE_MODE)
            .unwrap_or(NACOS_CONSOLE_MODE_LOCAL.to_string())
    }

    pub fn is_console_remote_mode(&self) -> bool {
        self.console_mode() == NACOS_CONSOLE_MODE_REMOTE
    }

    pub fn console_remote_server_addr(&self) -> String {
        self.config
            .get_string(NACOS_CONSOLE_REMOTE_SERVER_ADDR)
            .unwrap_or("http://127.0.0.1:8848".to_string())
    }

    pub fn console_remote_username(&self) -> String {
        self.config
            .get_string(NACOS_CONSOLE_REMOTE_USERNAME)
            .unwrap_or("nacos".to_string())
    }

    pub fn console_remote_password(&self) -> String {
        self.config
            .get_string(NACOS_CONSOLE_REMOTE_PASSWORD)
            .unwrap_or("nacos".to_string())
    }

    pub fn console_remote_connect_timeout_ms(&self) -> u64 {
        self.config
            .get_int(NACOS_CONSOLE_REMOTE_CONNECT_TIMEOUT_MS)
            .unwrap_or(5000) as u64
    }

    pub fn console_remote_read_timeout_ms(&self) -> u64 {
        self.config
            .get_int(NACOS_CONSOLE_REMOTE_READ_TIMEOUT_MS)
            .unwrap_or(30000) as u64
    }

    // ========================================================================
    // Authentication Configuration
    // ========================================================================

    pub fn auth_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.core.auth.enabled")
            .unwrap_or(false)
            || self
                .config
                .get_bool("nacos.core.auth.admin.enabled")
                .unwrap_or(false)
    }

    pub fn auth_system_type(&self) -> String {
        self.config
            .get_string("nacos.core.auth.system.type")
            .unwrap_or("nacos".to_string())
    }

    pub fn auth_console_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.core.auth.console.enabled")
            .unwrap_or(true)
    }

    pub fn token_secret_key(&self) -> String {
        self.config
            .get_string("nacos.core.auth.plugin.nacos.token.secret.key")
            .unwrap_or_default()
    }

    pub fn auth_token_expire_seconds(&self) -> i64 {
        self.config
            .get_int(TOKEN_EXPIRE_SECONDS)
            .unwrap_or(DEFAULT_TOKEN_EXPIRE_SECONDS)
    }

    // ========================================================================
    // Database Configuration
    // ========================================================================

    pub fn datasource_platform(&self) -> String {
        self.config
            .get_string(DATASOURCE_PLATFORM_PROPERTY)
            .unwrap_or("false".to_string())
    }

    pub fn plugin_datasource_log(&self) -> bool {
        self.config
            .get_bool(NACOS_PLUGIN_DATASOURCE_LOG)
            .unwrap_or(false)
    }

    pub async fn database_connection(
        &self,
    ) -> std::result::Result<DatabaseConnection, Box<dyn std::error::Error>> {
        let max_connections = self
            .config
            .get_int("db.pool.config.maximumPoolSize")
            .unwrap_or(100) as u32;
        let min_connections = self
            .config
            .get_int("db.pool.config.minimumPoolSize")
            .unwrap_or(1) as u32;
        let connect_timeout = self
            .config
            .get_int("db.pool.config.connectionTimeout")
            .unwrap_or(30) as u64;
        let acquire_timeout = self
            .config
            .get_int("db.pool.config.initializationFailTimeout")
            .unwrap_or(8) as u64;
        let idle_timeout = self
            .config
            .get_int("db.pool.config.idleTimeout")
            .unwrap_or(10) as u64;
        let max_lifetime = self
            .config
            .get_int("db.pool.config.maxLifetime")
            .unwrap_or(1800) as u64;
        let sqlx_logging = self
            .config
            .get_bool("db.pool.config.sqlxLogging")
            .unwrap_or(false);

        let url = self.config.get_string("db.url")?;

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

    // ========================================================================
    // Capacity & Health Configuration
    // ========================================================================

    pub fn notify_connect_timeout(&self) -> i32 {
        self.config.get_int(NOTIFY_CONNECT_TIMEOUT).unwrap_or(100) as i32
    }

    pub fn notify_socket_timeout(&self) -> i32 {
        self.config.get_int(NOTIFY_SOCKET_TIMEOUT).unwrap_or(200) as i32
    }

    pub fn is_health_check(&self) -> bool {
        self.config.get_bool(IS_HEALTH_CHECK).unwrap_or(true)
    }

    pub fn max_health_check_fail_count(&self) -> i32 {
        self.config
            .get_int(MAX_HEALTH_CHECK_FAIL_COUNT)
            .unwrap_or(12) as i32
    }

    pub fn max_content(&self) -> i32 {
        self.config.get_int(MAX_CONTENT).unwrap_or(10 * 1024 * 1024) as i32
    }

    pub fn is_manage_capacity(&self) -> bool {
        self.config.get_bool(IS_MANAGE_CAPACITY).unwrap_or(true)
    }

    pub fn is_capacity_limit_check(&self) -> bool {
        self.config
            .get_bool(IS_CAPACITY_LIMIT_CHECK)
            .unwrap_or(false)
    }

    pub fn default_cluster_quota(&self) -> i32 {
        self.config.get_int(DEFAULT_CLUSTER_QUOTA).unwrap_or(100000) as i32
    }

    pub fn default_group_quota(&self) -> i32 {
        self.config.get_int(DEFAULT_GROUP_QUOTA).unwrap_or(200) as i32
    }

    pub fn default_max_size(&self) -> i32 {
        self.config.get_int(DEFAULT_MAX_SIZE).unwrap_or(100 * 1024) as i32
    }

    pub fn default_max_aggr_count(&self) -> i32 {
        self.config.get_int(DEFAULT_MAX_AGGR_COUNT).unwrap_or(10000) as i32
    }

    pub fn default_max_aggr_size(&self) -> i32 {
        self.config.get_int(DEFAULT_MAX_AGGR_SIZE).unwrap_or(1024) as i32
    }

    pub fn config_rentention_days(&self) -> i32 {
        self.config.get_int(CONFIG_RENTENTION_DAYS).unwrap_or(30) as i32
    }

    // ========================================================================
    // OpenTelemetry Configuration
    // ========================================================================

    pub fn otel_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.otel.enabled")
            .unwrap_or(false)
    }

    pub fn otel_endpoint(&self) -> String {
        self.config
            .get_string("nacos.otel.endpoint")
            .unwrap_or_else(|_| "http://localhost:4317".to_string())
    }

    pub fn otel_service_name(&self) -> String {
        self.config
            .get_string("nacos.otel.service_name")
            .unwrap_or_else(|_| "batata".to_string())
    }

    pub fn otel_sampling_ratio(&self) -> f64 {
        self.config
            .get_float("nacos.otel.sampling_ratio")
            .unwrap_or(1.0)
    }

    pub fn otel_export_timeout_secs(&self) -> u64 {
        self.config
            .get_int("nacos.otel.export_timeout_secs")
            .unwrap_or(10) as u64
    }

    // ========================================================================
    // Rate Limiting Configuration
    // ========================================================================

    /// Check if API rate limiting is enabled
    pub fn ratelimit_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.ratelimit.enabled")
            .unwrap_or(true)
    }

    /// Get maximum requests per window for API rate limiting
    pub fn ratelimit_max_requests(&self) -> u32 {
        self.config
            .get_int("nacos.ratelimit.max_requests")
            .unwrap_or(100) as u32
    }

    /// Get rate limit window duration in seconds
    pub fn ratelimit_window_seconds(&self) -> u64 {
        self.config
            .get_int("nacos.ratelimit.window_seconds")
            .unwrap_or(60) as u64
    }

    /// Check if authentication rate limiting is enabled
    pub fn ratelimit_auth_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.ratelimit.auth.enabled")
            .unwrap_or(true)
    }

    /// Get maximum login attempts before lockout
    pub fn ratelimit_auth_max_attempts(&self) -> u32 {
        self.config
            .get_int("nacos.ratelimit.auth.max_attempts")
            .unwrap_or(5) as u32
    }

    /// Get login attempt window duration in seconds
    pub fn ratelimit_auth_window_seconds(&self) -> u64 {
        self.config
            .get_int("nacos.ratelimit.auth.window_seconds")
            .unwrap_or(60) as u64
    }

    /// Get lockout duration in seconds after exceeding max login attempts
    pub fn ratelimit_auth_lockout_seconds(&self) -> u64 {
        self.config
            .get_int("nacos.ratelimit.auth.lockout_seconds")
            .unwrap_or(300) as u64
    }

    /// Create RateLimitConfig from configuration
    pub fn rate_limit_config(&self) -> crate::middleware::rate_limit::RateLimitConfig {
        crate::middleware::rate_limit::RateLimitConfig {
            max_requests: self.ratelimit_max_requests(),
            window_duration: std::time::Duration::from_secs(self.ratelimit_window_seconds()),
            enabled: self.ratelimit_enabled(),
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
    // Core Config Conversion
    // ========================================================================

    /// Convert to batata_core Configuration for use with ServerMemberManager
    pub fn to_core_config(&self) -> batata_core::model::Configuration {
        batata_core::model::Configuration::from_config(self.config.clone())
    }
}
