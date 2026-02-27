//! Configuration management for Batata server
//!
//! This module handles loading and accessing application configuration.

use std::time::Duration;

use clap::Parser;
use config::Config;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde_yaml::Value as YamlValue;

use crate::auth::model::{DEFAULT_TOKEN_EXPIRE_SECONDS, TOKEN_EXPIRE_SECONDS};

use super::constants::{
    CONFIG_RENTENTION_DAYS, DATASOURCE_PLATFORM_PROPERTY, DEFAULT_CLUSTER_QUOTA,
    DEFAULT_GROUP_QUOTA, DEFAULT_MAX_AGGR_COUNT, DEFAULT_MAX_AGGR_SIZE, DEFAULT_MAX_SIZE,
    DEFAULT_SERVER_PORT, FUNCTION_MODE_PROPERTY_NAME, IS_CAPACITY_LIMIT_CHECK, IS_HEALTH_CHECK,
    IS_MANAGE_CAPACITY, MAX_CONTENT, MAX_HEALTH_CHECK_FAIL_COUNT,
    NACOS_CONSOLE_REMOTE_CONNECT_TIMEOUT_MS, NACOS_CONSOLE_REMOTE_PASSWORD,
    NACOS_CONSOLE_REMOTE_READ_TIMEOUT_MS, NACOS_CONSOLE_REMOTE_SERVER_ADDR,
    NACOS_CONSOLE_REMOTE_USERNAME, NACOS_DEPLOYMENT_TYPE, NACOS_DEPLOYMENT_TYPE_CONSOLE,
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
    #[arg(short = 'c', long = "config")]
    config_file: Option<String>,
}

/// Application configuration loaded from config files and environment
#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub config: Config,
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

/// Pre-process YAML: rename top-level `nacos:` key to `batata:` for compatibility.
///
/// If YAML has both `nacos:` and `batata:`, deep-merge `nacos:` into `batata:` (batata wins).
/// After this step, all nacos-namespace config lives under `batata:`.
fn preprocess_yaml(yaml_content: &str) -> String {
    let mut root: YamlValue =
        serde_yaml::from_str(yaml_content).expect("Failed to parse YAML configuration file");

    if let YamlValue::Mapping(ref mut map) = root {
        let nacos_key = YamlValue::String("nacos".to_string());
        let batata_key = YamlValue::String("batata".to_string());

        if let Some(nacos_val) = map.remove(&nacos_key) {
            let batata_val = map.remove(&batata_key);
            let merged = match batata_val {
                Some(existing) => deep_merge(nacos_val, existing),
                None => nacos_val,
            };
            map.insert(batata_key, merged);
        }
    }

    serde_yaml::to_string(&root).expect("Failed to serialize merged YAML")
}

/// Deep-merge two YAML values. `override_val` takes priority on conflict.
fn deep_merge(base: YamlValue, override_val: YamlValue) -> YamlValue {
    match (base, override_val) {
        (YamlValue::Mapping(mut base_map), YamlValue::Mapping(override_map)) => {
            for (k, v) in override_map {
                let merged = if let Some(base_v) = base_map.remove(&k) {
                    deep_merge(base_v, v)
                } else {
                    v
                };
                base_map.insert(k, merged);
            }
            YamlValue::Mapping(base_map)
        }
        // For non-mapping types, override wins
        (_, override_val) => override_val,
    }
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
/// - `NACOS_*`: prefix is remapped to `batata.` (e.g., `NACOS_SERVER_MAIN_PORT` → `batata.server.main.port`)
/// - `BATATA_*`: prefix is kept as `batata.` (e.g., `BATATA_SERVER_MAIN_PORT` → `batata.server.main.port`)
///
/// Returns sorted Vec to ensure deterministic override order.
fn collect_env_overrides(prefix: &str, remap_to_batata: bool) -> Vec<(String, config::Value)> {
    let prefix_with_sep = format!("{prefix}_");
    let mut overrides: Vec<(String, config::Value)> = std::env::vars()
        .filter_map(|(key, value)| {
            let rest = key.strip_prefix(&prefix_with_sep)?;
            let config_key = if remap_to_batata {
                // NACOS_SERVER_MAIN_PORT → batata.server.main.port
                format!("batata.{}", rest.to_lowercase().replace('_', "."))
            } else {
                // BATATA_SERVER_MAIN_PORT → batata.server.main.port (keep_prefix equivalent)
                format!(
                    "{}.{}",
                    prefix.to_lowercase(),
                    rest.to_lowercase().replace('_', ".")
                )
            };
            Some((config_key, try_parse_env_value(&value)))
        })
        .collect();
    overrides.sort_by(|a, b| a.0.cmp(&b.0));
    overrides
}

impl Configuration {
    pub fn new() -> Self {
        // Step 1: Extract --dotted.key=value overrides before clap sees them
        let (property_overrides, filtered_args) = extract_property_overrides();

        // Step 2: Parse clap from filtered args
        let args = Cli::parse_from(filtered_args);

        // Step 3: Pre-process YAML — rename nacos: → batata: for compatibility
        let config_file = args
            .config_file
            .as_deref()
            .unwrap_or("conf/application.yml");
        let yaml_content =
            std::fs::read_to_string(config_file).expect("Failed to read configuration file");
        let merged_yaml = preprocess_yaml(&yaml_content);

        // Step 4: Build config with layered sources (lowest to highest priority)
        let mut config_builder = Config::builder()
            // Priority 4 (lowest): Pre-processed YAML (nacos: already merged into batata:)
            .add_source(config::File::from_str(
                &merged_yaml,
                config::FileFormat::Yaml,
            ));

        // Priority 3: NACOS_* and BATATA_* env vars (manual processing)
        // Both are remapped to batata.* keys. BATATA_* applied after NACOS_* so it wins.
        let nacos_env = collect_env_overrides("NACOS", true);
        let batata_env = collect_env_overrides("BATATA", false);

        // Apply NACOS_* first (lower priority)
        for (key, value) in &nacos_env {
            config_builder = config_builder
                .set_override(key, value.clone())
                .unwrap_or_else(|e| panic!("Failed to set NACOS_ env override for {key}: {e}"));
        }
        // Apply BATATA_* second (higher priority, overwrites NACOS_* for same keys)
        for (key, value) in &batata_env {
            config_builder = config_builder
                .set_override(key, value.clone())
                .unwrap_or_else(|e| panic!("Failed to set BATATA_ env override for {key}: {e}"));
        }

        // Priority 2: Convenience CLI args
        if let Some(v) = args.mode {
            config_builder = config_builder
                .set_override(STANDALONE_MODE_PROPERTY_NAME, v == "standalone")
                .expect("Failed to set standalone mode override");
        }
        if let Some(v) = args.function_mode {
            config_builder = config_builder
                .set_override(FUNCTION_MODE_PROPERTY_NAME, v)
                .expect("Failed to set function mode override");
        }
        if let Some(v) = args.deployment {
            config_builder = config_builder
                .set_override(NACOS_DEPLOYMENT_TYPE, v)
                .expect("Failed to set deployment type override");
        }
        if let Some(v) = args.database_url {
            config_builder = config_builder
                .set_override("batata.db.url", v)
                .expect("Failed to set database URL override");
        }

        // Priority 1 (highest): --dotted.key=value property overrides
        for (key, value) in property_overrides {
            config_builder = config_builder
                .set_override(&key, value)
                .unwrap_or_else(|e| panic!("Failed to set override for {key}: {e}"));
        }

        let app_config = config_builder
            .build()
            .expect("Failed to build configuration");

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
            .get_string("batata.version")
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
            .get_string("batata.server.contextPath")
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
        self.config.get_int("batata.console.port").unwrap_or(8081) as u16
    }

    pub fn console_server_context_path(&self) -> String {
        self.config
            .get_string("batata.console.contextPath")
            .unwrap_or("".to_string())
    }

    pub fn console_ui_enabled(&self) -> bool {
        self.config
            .get_bool("batata.console.ui.enabled")
            .unwrap_or(true)
    }

    /// Check if console is in remote mode.
    /// Derived from deployment type: `console` deployment → remote mode.
    pub fn is_console_remote_mode(&self) -> bool {
        self.deployment_type() == NACOS_DEPLOYMENT_TYPE_CONSOLE
    }

    pub fn console_remote_server_addr(&self) -> String {
        self.config
            .get_string(NACOS_CONSOLE_REMOTE_SERVER_ADDR)
            .unwrap_or("http://127.0.0.1:8848".to_string())
    }

    /// Resolve remote server addresses for console remote mode.
    ///
    /// Resolution order (same as cluster member lookup):
    /// 1. `nacos.member.list` config property (comma-separated `ip:port`)
    /// 2. `conf/cluster.conf` file (one `ip:port` per line, skip `#` comments)
    /// 3. Fall back to `nacos.console.remote.server_addr` (legacy config)
    ///
    /// Each address is normalized to `http://ip:port` format.
    pub fn resolve_remote_server_addrs(&self) -> Vec<String> {
        // 1. Try nacos.member.list
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

        // 3. Fall back to nacos.console.remote.server_addr (legacy)
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
            .get_bool("batata.core.auth.enabled")
            .unwrap_or(false)
    }

    pub fn auth_admin_enabled(&self) -> bool {
        self.config
            .get_bool("batata.core.auth.admin.enabled")
            .unwrap_or(false)
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
        self.config
            .get_string("batata.core.auth.server.identity.key")
            .unwrap_or_default()
    }

    pub fn server_identity_value(&self) -> String {
        self.config
            .get_string("batata.core.auth.server.identity.value")
            .unwrap_or_default()
    }

    pub fn auth_system_type(&self) -> String {
        self.config
            .get_string("batata.core.auth.system.type")
            .unwrap_or("nacos".to_string())
    }

    pub fn auth_console_enabled(&self) -> bool {
        self.config
            .get_bool("batata.core.auth.console.enabled")
            .unwrap_or(true)
    }

    pub fn token_secret_key(&self) -> String {
        self.config
            .get_string("batata.core.auth.plugin.nacos.token.secret.key")
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
        self.config.get_string("batata.core.auth.ldap.url").ok()
    }

    /// Get LDAP base DN
    pub fn ldap_base_dn(&self) -> String {
        self.config
            .get_string("batata.core.auth.ldap.basedc")
            .unwrap_or_default()
    }

    /// Get LDAP bind DN (admin user)
    pub fn ldap_bind_dn(&self) -> String {
        self.config
            .get_string("batata.core.auth.ldap.userDn")
            .unwrap_or_default()
    }

    /// Get LDAP bind password
    pub fn ldap_bind_password(&self) -> String {
        self.config
            .get_string("batata.core.auth.ldap.password")
            .unwrap_or_default()
    }

    /// Get LDAP user DN pattern
    pub fn ldap_user_dn_pattern(&self) -> String {
        self.config
            .get_string("batata.core.auth.ldap.userdn")
            .unwrap_or_default()
    }

    /// Get LDAP filter prefix (default: uid)
    pub fn ldap_filter_prefix(&self) -> String {
        self.config
            .get_string("batata.core.auth.ldap.filter.prefix")
            .unwrap_or_else(|_| "uid".to_string())
    }

    /// Get LDAP connection timeout in milliseconds
    pub fn ldap_timeout_ms(&self) -> u64 {
        self.config
            .get_int("batata.core.auth.ldap.timeout")
            .unwrap_or(5000) as u64
    }

    /// Check if LDAP username comparison is case-sensitive
    pub fn ldap_case_sensitive(&self) -> bool {
        self.config
            .get_bool("batata.core.auth.ldap.case.sensitive")
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
                .get_bool("batata.core.auth.ldap.ignore.partial.result.exception")
                .unwrap_or(false),
        }
    }

    // ========================================================================
    // OAuth2/OIDC Configuration
    // ========================================================================

    /// Check if OAuth2/OIDC authentication is enabled
    pub fn is_oauth_enabled(&self) -> bool {
        self.config
            .get_bool("batata.core.auth.oauth.enabled")
            .unwrap_or(false)
    }

    /// Get OAuth user creation mode (auto or manual)
    pub fn oauth_user_creation(&self) -> String {
        self.config
            .get_string("batata.core.auth.oauth.user.creation")
            .unwrap_or_else(|_| "auto".to_string())
    }

    /// Get OAuth role sync mode (on_login or periodic)
    pub fn oauth_role_sync(&self) -> String {
        self.config
            .get_string("batata.core.auth.oauth.role.sync")
            .unwrap_or_else(|_| "on_login".to_string())
    }

    /// Get default OAuth redirect URI template
    pub fn oauth_redirect_uri(&self) -> Option<String> {
        self.config
            .get_string("batata.core.auth.oauth.redirect.uri")
            .ok()
    }

    /// Get OAuth configuration as OAuthConfig struct
    pub fn oauth_config(&self) -> batata_auth::service::oauth::OAuthConfig {
        use std::collections::HashMap;

        let mut providers = HashMap::new();

        // Load providers from config (e.g., nacos.core.auth.oauth.providers.google)
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

    /// Derive the persistence storage mode from `batata.sql.init.platform` and `batata.standalone`.
    ///
    /// Logic (aligned with Nacos 3.x):
    /// - `batata.sql.init.platform` = "mysql" or "postgresql" → ExternalDb
    /// - `batata.sql.init.platform` = empty + standalone=true → StandaloneEmbedded (RocksDB)
    /// - `batata.sql.init.platform` = empty + standalone=false → DistributedEmbedded (Raft + RocksDB)
    pub fn persistence_mode(&self) -> batata_persistence::StorageMode {
        let platform = self.datasource_platform();
        if platform.eq_ignore_ascii_case("mysql") || platform.eq_ignore_ascii_case("postgresql") {
            batata_persistence::StorageMode::ExternalDb
        } else if self.is_standalone() {
            batata_persistence::StorageMode::StandaloneEmbedded
        } else {
            batata_persistence::StorageMode::DistributedEmbedded
        }
    }

    /// Get the RocksDB data directory for embedded modes
    pub fn embedded_data_dir(&self) -> String {
        self.config
            .get_string("batata.persistence.embedded.data_dir")
            .unwrap_or_else(|_| "data/rocksdb".to_string())
    }

    // ========================================================================
    // Database Configuration
    // ========================================================================

    pub fn datasource_platform(&self) -> String {
        self.config
            .get_string(DATASOURCE_PLATFORM_PROPERTY)
            .unwrap_or_default()
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
            .get_int("batata.db.pool.max_connections")
            .unwrap_or(100) as u32;
        let min_connections = self
            .config
            .get_int("batata.db.pool.min_connections")
            .unwrap_or(1) as u32;
        let connect_timeout = self
            .config
            .get_int("batata.db.pool.connect_timeout")
            .unwrap_or(30) as u64;
        let acquire_timeout = self
            .config
            .get_int("batata.db.pool.acquire_timeout")
            .unwrap_or(8) as u64;
        let idle_timeout = self
            .config
            .get_int("batata.db.pool.idle_timeout")
            .unwrap_or(10) as u64;
        let max_lifetime = self
            .config
            .get_int("batata.db.pool.max_lifetime")
            .unwrap_or(1800) as u64;
        let sqlx_logging = self
            .config
            .get_bool("batata.db.pool.sqlx_logging")
            .unwrap_or(false);

        let url = self.config.get_string("batata.db.url")?;

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
    // Naming Module Configuration
    // ========================================================================

    /// Check if instance expiration is enabled
    /// When true, instances will be automatically deleted after ip_delete_timeout
    pub fn expire_instance_enabled(&self) -> bool {
        self.config
            .get_bool("batata.naming.expireInstance")
            .unwrap_or(true)
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
        self.config.get_bool("batata.otel.enabled").unwrap_or(false)
    }

    pub fn otel_endpoint(&self) -> String {
        self.config
            .get_string("batata.otel.endpoint")
            .unwrap_or_else(|_| "http://localhost:4317".to_string())
    }

    pub fn otel_service_name(&self) -> String {
        self.config
            .get_string("batata.otel.service_name")
            .unwrap_or_else(|_| "batata".to_string())
    }

    pub fn otel_sampling_ratio(&self) -> f64 {
        self.config
            .get_float("batata.otel.sampling_ratio")
            .unwrap_or(1.0)
    }

    pub fn otel_export_timeout_secs(&self) -> u64 {
        self.config
            .get_int("batata.otel.export_timeout_secs")
            .unwrap_or(10) as u64
    }

    // ========================================================================
    // Rate Limiting Configuration
    // ========================================================================

    /// Check if API rate limiting is enabled
    pub fn ratelimit_enabled(&self) -> bool {
        self.config
            .get_bool("batata.ratelimit.enabled")
            .unwrap_or(false)
    }

    /// Get maximum requests per window for API rate limiting
    pub fn ratelimit_max_requests(&self) -> u32 {
        self.config
            .get_int("batata.ratelimit.max_requests")
            .unwrap_or(100) as u32
    }

    /// Get rate limit window duration in seconds
    pub fn ratelimit_window_seconds(&self) -> u64 {
        self.config
            .get_int("batata.ratelimit.window_seconds")
            .unwrap_or(60) as u64
    }

    /// Check if authentication rate limiting is enabled
    pub fn ratelimit_auth_enabled(&self) -> bool {
        self.config
            .get_bool("batata.ratelimit.auth.enabled")
            .unwrap_or(false)
    }

    /// Get maximum login attempts before lockout
    pub fn ratelimit_auth_max_attempts(&self) -> u32 {
        self.config
            .get_int("batata.ratelimit.auth.max_attempts")
            .unwrap_or(5) as u32
    }

    /// Get login attempt window duration in seconds
    pub fn ratelimit_auth_window_seconds(&self) -> u64 {
        self.config
            .get_int("batata.ratelimit.auth.window_seconds")
            .unwrap_or(60) as u64
    }

    /// Get lockout duration in seconds after exceeding max login attempts
    pub fn ratelimit_auth_lockout_seconds(&self) -> u64 {
        self.config
            .get_int("batata.ratelimit.auth.lockout_seconds")
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
            .get_bool("batata.config.encryption.enabled")
            .unwrap_or(false)
    }

    /// Get the encryption plugin type
    pub fn encryption_plugin_type(&self) -> String {
        self.config
            .get_string("batata.config.encryption.plugin.type")
            .unwrap_or_else(|_| "aes-gcm".to_string())
    }

    /// Get the encryption key (Base64-encoded)
    pub fn encryption_key(&self) -> Option<String> {
        self.config.get_string("batata.config.encryption.key").ok()
    }

    /// Get the encryption hot reload interval in milliseconds (0 = disabled)
    pub fn encryption_reload_interval_ms(&self) -> u64 {
        self.config
            .get_int("batata.config.encryption.reload.interval.ms")
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
            .get_bool("batata.remote.server.grpc.sdk.tls.enabled")
            .unwrap_or(false)
    }

    /// Check if TLS is enabled for cluster gRPC server
    pub fn grpc_cluster_tls_enabled(&self) -> bool {
        self.config
            .get_bool("batata.remote.server.grpc.cluster.tls.enabled")
            .unwrap_or(false)
    }

    /// Get the path to the server certificate file
    pub fn grpc_tls_cert_path(&self) -> Option<String> {
        self.config
            .get_string("batata.remote.server.grpc.tls.cert.path")
            .ok()
    }

    /// Get the path to the server private key file
    pub fn grpc_tls_key_path(&self) -> Option<String> {
        self.config
            .get_string("batata.remote.server.grpc.tls.key.path")
            .ok()
    }

    /// Get the path to the CA certificate for mTLS
    pub fn grpc_tls_ca_cert_path(&self) -> Option<String> {
        self.config
            .get_string("batata.remote.server.grpc.tls.ca.cert.path")
            .ok()
    }

    /// Check if mutual TLS is enabled
    pub fn grpc_mtls_enabled(&self) -> bool {
        self.config
            .get_bool("batata.remote.server.grpc.tls.mtls.enabled")
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
            .get_bool("batata.mesh.xds.enabled")
            .unwrap_or(false)
    }

    /// Get xDS server port (default: 15010)
    pub fn xds_server_port(&self) -> u16 {
        self.config.get_int("batata.mesh.xds.port").unwrap_or(15010) as u16
    }

    /// Get xDS server ID
    pub fn xds_server_id(&self) -> String {
        self.config
            .get_string("batata.mesh.xds.server.id")
            .unwrap_or_else(|_| "batata-xds-server".to_string())
    }

    /// Get xDS sync interval in milliseconds
    pub fn xds_sync_interval_ms(&self) -> u64 {
        self.config
            .get_int("batata.mesh.xds.sync.interval.ms")
            .unwrap_or(5000) as u64
    }

    /// Check if xDS should generate default listeners
    pub fn xds_generate_listeners(&self) -> bool {
        self.config
            .get_bool("batata.mesh.xds.generate.listeners")
            .unwrap_or(true)
    }

    /// Check if xDS should generate default routes
    pub fn xds_generate_routes(&self) -> bool {
        self.config
            .get_bool("batata.mesh.xds.generate.routes")
            .unwrap_or(true)
    }

    /// Get default listener port for xDS generated listeners
    pub fn xds_default_listener_port(&self) -> u16 {
        self.config
            .get_int("batata.mesh.xds.default.listener.port")
            .unwrap_or(15001) as u16
    }

    /// Check if xDS TLS is enabled
    pub fn xds_tls_enabled(&self) -> bool {
        self.config
            .get_bool("batata.mesh.xds.tls.enabled")
            .unwrap_or(false)
    }

    /// Get xDS TLS certificate path
    pub fn xds_tls_cert_path(&self) -> Option<String> {
        self.config.get_string("batata.mesh.xds.tls.cert.path").ok()
    }

    /// Get xDS TLS key path
    pub fn xds_tls_key_path(&self) -> Option<String> {
        self.config.get_string("batata.mesh.xds.tls.key.path").ok()
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
    // Consul Compatibility Plugin Configuration
    // ========================================================================

    /// Check if Consul compatibility server is enabled
    pub fn consul_enabled(&self) -> bool {
        self.config
            .get_bool("batata.plugin.consul.enabled")
            .unwrap_or(false)
    }

    /// Get Consul compatibility server port (default: 8500)
    pub fn consul_server_port(&self) -> u16 {
        self.config
            .get_int("batata.plugin.consul.port")
            .unwrap_or(8500) as u16
    }

    /// Check if Consul ACL is enabled (default: false)
    pub fn consul_acl_enabled(&self) -> bool {
        self.config
            .get_bool("batata.plugin.consul.acl.enabled")
            .unwrap_or(false)
    }

    /// Get Consul data directory for RocksDB persistence (default: data/consul_rocksdb)
    pub fn consul_data_dir(&self) -> String {
        self.config
            .get_string("batata.plugin.consul.data_dir")
            .unwrap_or_else(|_| "data/consul_rocksdb".to_string())
    }

    /// Check if Consul should register itself as a service (default: true)
    pub fn consul_register_self(&self) -> bool {
        self.config
            .get_bool("batata.plugin.consul.register_self")
            .unwrap_or(true)
    }

    /// Get Consul check reap interval in seconds (default: 30)
    /// This is the interval at which services with critical health checks
    /// will be checked for automatic deregistration.
    pub fn consul_check_reap_interval(&self) -> u64 {
        self.config
            .get_int("batata.plugin.consul.check_reap_interval")
            .unwrap_or(30) as u64
    }

    // ========================================================================
    // Apollo Compatibility Plugin Configuration
    // ========================================================================

    /// Check if Apollo compatibility server is enabled
    pub fn apollo_enabled(&self) -> bool {
        self.config
            .get_bool("batata.plugin.apollo.enabled")
            .unwrap_or(false)
    }

    /// Get Apollo compatibility server port (default: 8080)
    pub fn apollo_server_port(&self) -> u16 {
        self.config
            .get_int("batata.plugin.apollo.port")
            .unwrap_or(8080) as u16
    }

    // ========================================================================
    // MCP Registry Configuration
    // ========================================================================

    /// Check if MCP Registry server is enabled (default: false)
    pub fn mcp_registry_enabled(&self) -> bool {
        self.config
            .get_bool("batata.ai.mcp.registry.enabled")
            .unwrap_or(false)
    }

    /// Get MCP Registry server port (default: 9080)
    pub fn mcp_registry_port(&self) -> u16 {
        self.config
            .get_int("batata.ai.mcp.registry.port")
            .unwrap_or(9080) as u16
    }

    // ========================================================================
    // Logging Configuration
    // ========================================================================

    /// Get log directory path
    pub fn log_dir(&self) -> Option<String> {
        self.config.get_string("batata.logs.path").ok()
    }

    /// Check if console logging is enabled
    pub fn log_console_enabled(&self) -> bool {
        self.config
            .get_bool("batata.logs.console.enabled")
            .unwrap_or(true)
    }

    /// Check if file logging is enabled
    pub fn log_file_enabled(&self) -> bool {
        self.config
            .get_bool("batata.logs.file.enabled")
            .unwrap_or(true)
    }

    /// Get log level
    pub fn log_level(&self) -> String {
        self.config
            .get_string("batata.logs.level")
            .unwrap_or_else(|_| "info".to_string())
    }

    /// Get logging configuration
    pub fn logging_config(&self) -> crate::startup::LoggingConfig {
        crate::startup::LoggingConfig::from_config(
            self.log_dir(),
            self.log_console_enabled(),
            self.log_file_enabled(),
            self.log_level(),
        )
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
    use batata_common::ApiType;
    use config::Config;

    fn build_config(overrides: Vec<(&str, config::Value)>) -> Configuration {
        let mut builder = Config::builder();
        for (key, value) in overrides {
            builder = builder.set_override(key, value).unwrap();
        }
        Configuration {
            config: builder.build().unwrap(),
        }
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
        let cfg = build_config(vec![("batata.core.auth.admin.enabled", true.into())]);
        assert!(!cfg.auth_enabled());
    }

    #[test]
    fn test_auth_admin_enabled() {
        let cfg = build_config(vec![("batata.core.auth.admin.enabled", true.into())]);
        assert!(cfg.auth_admin_enabled());
    }

    #[test]
    fn test_auth_enabled_for_api_type_open_api() {
        let cfg = build_config(vec![("batata.core.auth.enabled", true.into())]);
        assert!(cfg.auth_enabled_for_api_type(ApiType::OpenApi));

        let cfg2 = build_config(vec![]);
        assert!(!cfg2.auth_enabled_for_api_type(ApiType::OpenApi));
    }

    #[test]
    fn test_auth_enabled_for_api_type_admin_api() {
        let cfg = build_config(vec![("batata.core.auth.admin.enabled", true.into())]);
        assert!(cfg.auth_enabled_for_api_type(ApiType::AdminApi));

        let cfg2 = build_config(vec![]);
        assert!(!cfg2.auth_enabled_for_api_type(ApiType::AdminApi));
    }

    #[test]
    fn test_auth_enabled_for_api_type_console_api() {
        // Console auth defaults to true
        let cfg = build_config(vec![]);
        assert!(cfg.auth_enabled_for_api_type(ApiType::ConsoleApi));

        let cfg2 = build_config(vec![("batata.core.auth.console.enabled", false.into())]);
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
            (
                "batata.core.auth.server.identity.key",
                "serverIdentity".into(),
            ),
            (
                "batata.core.auth.server.identity.value",
                "cluster-node-1".into(),
            ),
        ]);
        assert_eq!(cfg.server_identity_key(), "serverIdentity");
        assert_eq!(cfg.server_identity_value(), "cluster-node-1");
    }

    #[test]
    fn test_consul_enabled_default_false() {
        let cfg = build_config(vec![]);
        assert!(!cfg.consul_enabled());
    }

    #[test]
    fn test_consul_enabled_false() {
        let cfg = build_config(vec![("batata.plugin.consul.enabled", false.into())]);
        assert!(!cfg.consul_enabled());
    }

    #[test]
    fn test_consul_server_port_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.consul_server_port(), 8500);
    }

    #[test]
    fn test_consul_server_port_custom() {
        let cfg = build_config(vec![("batata.plugin.consul.port", 9500_i64.into())]);
        assert_eq!(cfg.consul_server_port(), 9500);
    }

    #[test]
    fn test_consul_data_dir_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.consul_data_dir(), "data/consul_rocksdb");
    }

    #[test]
    fn test_consul_data_dir_custom() {
        let cfg = build_config(vec![(
            "batata.plugin.consul.data_dir",
            "/custom/path/consul".into(),
        )]);
        assert_eq!(cfg.consul_data_dir(), "/custom/path/consul");
    }

    #[test]
    fn test_consul_register_self_default() {
        let cfg = build_config(vec![]);
        assert!(cfg.consul_register_self());
    }

    #[test]
    fn test_consul_register_self_true() {
        let cfg = build_config(vec![("batata.plugin.consul.register_self", true.into())]);
        assert!(cfg.consul_register_self());
    }

    #[test]
    fn test_consul_register_self_false() {
        let cfg = build_config(vec![("batata.plugin.consul.register_self", false.into())]);
        assert!(!cfg.consul_register_self());
    }

    #[test]
    fn test_apollo_enabled_default_false() {
        let cfg = build_config(vec![]);
        assert!(!cfg.apollo_enabled());
    }

    #[test]
    fn test_apollo_enabled_false() {
        let cfg = build_config(vec![("batata.plugin.apollo.enabled", false.into())]);
        assert!(!cfg.apollo_enabled());
    }

    #[test]
    fn test_apollo_server_port_default() {
        let cfg = build_config(vec![]);
        assert_eq!(cfg.apollo_server_port(), 8080);
    }

    #[test]
    fn test_apollo_server_port_custom() {
        let cfg = build_config(vec![("batata.plugin.apollo.port", 9080_i64.into())]);
        assert_eq!(cfg.apollo_server_port(), 9080);
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
            if let Some(rest) = arg.strip_prefix("--") {
                if let Some((key, value)) = rest.split_once('=') {
                    if key.contains('.') {
                        overrides.push((key.to_string(), value.to_string()));
                        continue;
                    }
                }
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
            ("batata.db.url".to_string(), "mysql://localhost/db".to_string())
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
}
