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
    #[arg(long = "db-url", env = "DATABASE_URL")]
    database_url: Option<String>,
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
        if let Some(v) = args.database_url {
            config_builder = config_builder
                .set_override("db.url", v)
                .expect("Failed to set database URL override");
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

    /// Check if LDAP authentication is enabled
    pub fn is_ldap_auth_enabled(&self) -> bool {
        self.auth_system_type().to_lowercase() == "ldap"
    }

    /// Get LDAP URL
    pub fn ldap_url(&self) -> Option<String> {
        self.config.get_string("nacos.core.auth.ldap.url").ok()
    }

    /// Get LDAP base DN
    pub fn ldap_base_dn(&self) -> String {
        self.config
            .get_string("nacos.core.auth.ldap.basedc")
            .unwrap_or_default()
    }

    /// Get LDAP bind DN (admin user)
    pub fn ldap_bind_dn(&self) -> String {
        self.config
            .get_string("nacos.core.auth.ldap.userDn")
            .unwrap_or_default()
    }

    /// Get LDAP bind password
    pub fn ldap_bind_password(&self) -> String {
        self.config
            .get_string("nacos.core.auth.ldap.password")
            .unwrap_or_default()
    }

    /// Get LDAP user DN pattern
    pub fn ldap_user_dn_pattern(&self) -> String {
        self.config
            .get_string("nacos.core.auth.ldap.userdn")
            .unwrap_or_default()
    }

    /// Get LDAP filter prefix (default: uid)
    pub fn ldap_filter_prefix(&self) -> String {
        self.config
            .get_string("nacos.core.auth.ldap.filter.prefix")
            .unwrap_or_else(|_| "uid".to_string())
    }

    /// Get LDAP connection timeout in milliseconds
    pub fn ldap_timeout_ms(&self) -> u64 {
        self.config
            .get_int("nacos.core.auth.ldap.timeout")
            .unwrap_or(5000) as u64
    }

    /// Check if LDAP username comparison is case-sensitive
    pub fn ldap_case_sensitive(&self) -> bool {
        self.config
            .get_bool("nacos.core.auth.ldap.case.sensitive")
            .unwrap_or(true)
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
            ignore_partial_result_exception: self
                .config
                .get_bool("nacos.core.auth.ldap.ignore.partial.result.exception")
                .unwrap_or(false),
        }
    }

    // ========================================================================
    // OAuth2/OIDC Configuration
    // ========================================================================

    /// Check if OAuth2/OIDC authentication is enabled
    pub fn is_oauth_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.core.auth.oauth.enabled")
            .unwrap_or(false)
    }

    /// Get OAuth user creation mode (auto or manual)
    pub fn oauth_user_creation(&self) -> String {
        self.config
            .get_string("nacos.core.auth.oauth.user.creation")
            .unwrap_or_else(|_| "auto".to_string())
    }

    /// Get OAuth role sync mode (on_login or periodic)
    pub fn oauth_role_sync(&self) -> String {
        self.config
            .get_string("nacos.core.auth.oauth.role.sync")
            .unwrap_or_else(|_| "on_login".to_string())
    }

    /// Get default OAuth redirect URI template
    pub fn oauth_redirect_uri(&self) -> Option<String> {
        self.config
            .get_string("nacos.core.auth.oauth.redirect.uri")
            .ok()
    }

    /// Get OAuth configuration as OAuthConfig struct
    pub fn oauth_config(&self) -> batata_auth::service::oauth::OAuthConfig {
        use std::collections::HashMap;

        let mut providers = HashMap::new();

        // Load providers from config (e.g., nacos.core.auth.oauth.providers.google)
        // This is a simplified version - actual implementation would iterate over providers
        if let Ok(provider_config) = self.config.get_table("nacos.core.auth.oauth.providers") {
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
        self.config.get_bool("nacos.otel.enabled").unwrap_or(false)
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
    // Encryption Configuration
    // ========================================================================

    /// Check if configuration encryption is enabled
    pub fn encryption_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.config.encryption.enabled")
            .unwrap_or(false)
    }

    /// Get the encryption plugin type
    pub fn encryption_plugin_type(&self) -> String {
        self.config
            .get_string("nacos.config.encryption.plugin.type")
            .unwrap_or_else(|_| "aes-gcm".to_string())
    }

    /// Get the encryption key (Base64-encoded)
    pub fn encryption_key(&self) -> Option<String> {
        self.config.get_string("nacos.config.encryption.key").ok()
    }

    /// Get the encryption hot reload interval in milliseconds (0 = disabled)
    pub fn encryption_reload_interval_ms(&self) -> u64 {
        self.config
            .get_int("nacos.config.encryption.reload.interval.ms")
            .unwrap_or(0) as u64
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
        self.config
            .get_bool("nacos.remote.server.grpc.sdk.tls.enabled")
            .unwrap_or(false)
    }

    /// Check if TLS is enabled for cluster gRPC server
    pub fn grpc_cluster_tls_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.remote.server.grpc.cluster.tls.enabled")
            .unwrap_or(false)
    }

    /// Get the path to the server certificate file
    pub fn grpc_tls_cert_path(&self) -> Option<String> {
        self.config
            .get_string("nacos.remote.server.grpc.tls.cert.path")
            .ok()
    }

    /// Get the path to the server private key file
    pub fn grpc_tls_key_path(&self) -> Option<String> {
        self.config
            .get_string("nacos.remote.server.grpc.tls.key.path")
            .ok()
    }

    /// Get the path to the CA certificate for mTLS
    pub fn grpc_tls_ca_cert_path(&self) -> Option<String> {
        self.config
            .get_string("nacos.remote.server.grpc.tls.ca.cert.path")
            .ok()
    }

    /// Check if mutual TLS is enabled
    pub fn grpc_mtls_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.remote.server.grpc.tls.mtls.enabled")
            .unwrap_or(false)
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
        self.config
            .get_bool("nacos.mesh.xds.enabled")
            .unwrap_or(false)
    }

    /// Get xDS server port (default: 15010)
    pub fn xds_server_port(&self) -> u16 {
        self.config.get_int("nacos.mesh.xds.port").unwrap_or(15010) as u16
    }

    /// Get xDS server ID
    pub fn xds_server_id(&self) -> String {
        self.config
            .get_string("nacos.mesh.xds.server.id")
            .unwrap_or_else(|_| "batata-xds-server".to_string())
    }

    /// Get xDS sync interval in milliseconds
    pub fn xds_sync_interval_ms(&self) -> u64 {
        self.config
            .get_int("nacos.mesh.xds.sync.interval.ms")
            .unwrap_or(5000) as u64
    }

    /// Check if xDS should generate default listeners
    pub fn xds_generate_listeners(&self) -> bool {
        self.config
            .get_bool("nacos.mesh.xds.generate.listeners")
            .unwrap_or(true)
    }

    /// Check if xDS should generate default routes
    pub fn xds_generate_routes(&self) -> bool {
        self.config
            .get_bool("nacos.mesh.xds.generate.routes")
            .unwrap_or(true)
    }

    /// Get default listener port for xDS generated listeners
    pub fn xds_default_listener_port(&self) -> u16 {
        self.config
            .get_int("nacos.mesh.xds.default.listener.port")
            .unwrap_or(15001) as u16
    }

    /// Check if xDS TLS is enabled
    pub fn xds_tls_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.mesh.xds.tls.enabled")
            .unwrap_or(false)
    }

    /// Get xDS TLS certificate path
    pub fn xds_tls_cert_path(&self) -> Option<String> {
        self.config.get_string("nacos.mesh.xds.tls.cert.path").ok()
    }

    /// Get xDS TLS key path
    pub fn xds_tls_key_path(&self) -> Option<String> {
        self.config.get_string("nacos.mesh.xds.tls.key.path").ok()
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
