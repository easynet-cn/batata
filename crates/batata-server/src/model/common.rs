// Common models and shared application structures
// This file defines application state, configuration, and common data structures

use std::{collections::HashMap, sync::Arc, time::Duration};

use actix_web::{HttpResponse, HttpResponseBuilder, http::StatusCode};
use clap::Parser;
use config::{Config, Environment};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::{Deserialize, Serialize};

use batata_core::cluster::ServerMemberManager;

use crate::auth::model::{DEFAULT_TOKEN_EXPIRE_SECONDS, TOKEN_EXPIRE_SECONDS};

// Re-export common constants from batata_api::model (97 constants)
pub use batata_api::model::{
    // Client constants
    CLIENT_VERSION, CLIENT_VERSION_KEY, CLIENT_IP, CLIENT_APPNAME_HEADER,
    CLIENT_REQUEST_TS_HEADER, CLIENT_REQUEST_TOKEN_HEADER, CLIENT_MODULE_TYPE,
    // Group/namespace constants
    DEFAULT_GROUP, DEFAULT_NAMESPACE_ID, DEFAULT_CLUSTER_NAME,
    // Port offsets
    SDK_GRPC_PORT_DEFAULT_OFFSET, CLUSTER_GRPC_PORT_DEFAULT_OFFSET,
    // Timeout constants
    ONCE_TIMEOUT, SO_TIMEOUT, CONFIG_LONG_POLL_TIMEOUT, MIN_CONFIG_LONG_POLL_TIMEOUT,
    CONFIG_RETRY_TIME, MAX_RETRY, RECV_WAIT_TIMEOUT, DEFAULT_HEART_BEAT_TIMEOUT,
    DEFAULT_IP_DELETE_TIMEOUT, DEFAULT_HEART_BEAT_INTERVAL,
    // Encoding and separators
    ENCODE, LINE_SEPARATOR, WORD_SEPARATOR, LONGPOLLING_LINE_SEPARATOR, SERVICE_INFO_SPLITER,
    // Header constants
    ACCEPT_ENCODING, CONTENT_ENCODING, LAST_MODIFIED, CONTENT_MD5, CONFIG_VERSION, CONFIG_TYPE,
    ENCRYPTED_DATA_KEY, IF_MODIFIED_SINCE, SPACING_INTERVAL,
    // Data identifiers
    DATA_ID, TENANT, GROUP, GROUP_NAME, NAMESPACE_ID, DATA_IN_BODY_VERSION, APPNAME,
    // Path constants
    BASE_PATH, CONFIG_CONTROLLER_PATH,
    // Token constants
    TOKEN, ACCESS_TOKEN, TOKEN_TTL, GLOBAL_ADMIN, USERNAME, TOKEN_REFRESH_WINDOW,
    // Misc constants
    UNKNOWN_APP, DEFAULT_DOMAINNAME, DAILY_DOMAINNAME, NULL, USE_ZIP,
    PROBE_MODIFY_REQUEST, PROBE_MODIFY_RESPONSE, PROBE_MODIFY_RESPONSE_NEW,
    ASYNC_UPDATE_ADDRESS_INTERVAL, POLLING_INTERVAL_TIME, FLOW_CONTROL_THRESHOLD,
    FLOW_CONTROL_SLOT, FLOW_CONTROL_INTERVAL, DEFAULT_PROTECT_THRESHOLD,
    ATOMIC_MAX_SIZE, NAMING_INSTANCE_ID_SPLITTER, NAMING_INSTANCE_ID_SEG_COUNT,
    NAMING_HTTP_HEADER_SPLITTER, DEFAULT_USE_CLOUD_NAMESPACE_PARSING,
    WRITE_REDIRECT_CODE, SERVICE_INFO_SPLIT_COUNT, NULL_STRING, NUMBER_PATTERN_STRING,
    ANY_PATTERN, DEFAULT_INSTANCE_ID_GENERATOR, SNOWFLAKE_INSTANCE_ID_GENERATOR,
    HTTP_PREFIX, ALL_PATTERN, COLON, LINE_BREAK, POUND, VIPSERVER_TAG, AMORY_TAG,
    LOCATION_TAG, CHARSET_KEY, CLUSTER_NAME_PATTERN_STRING,
    DEFAULT_REDO_DELAY_TIME, DEFAULT_REDO_THREAD_COUNT, APP_CONN_LABELS_KEY,
    DOT, WEIGHT, PROPERTIES_KEY, JVM_KEY, ENV_KEY, APP_CONN_LABELS_PREFERRED,
    APP_CONN_PREFIX, CONFIG_GRAY_LABEL, DEFAULT_USE_RAM_INFO_PARSING,
    // Page type
    Page,
};

// System env constants.
pub const SYS_MODULE: &str = "sys";

/**
 * Spring Profile : "standalone".
 */
pub const STANDALONE_SPRING_PROFILE: &str = "standalone";

/**
 * The System property name of  Standalone mode.
 */
pub const STANDALONE_MODE_PROPERTY_NAME: &str = "nacos.standalone";

pub const STARTUP_MODE_STATE: &str = "startup_mode";

/**
 * The System property name of  Function mode.
 */
pub const FUNCTION_MODE_PROPERTY_NAME: &str = "nacos.functionMode";

pub const FUNCTION_MODE_STATE: &str = "function_mode";

/**
 * The System property name of prefer hostname over ip.
 */
pub const PREFER_HOSTNAME_OVER_IP_PROPERTY_NAME: &str = "nacos.preferHostnameOverIp";

/**
 * the root context path.
 */
pub const ROOT_WEB_CONTEXT_PATH: &str = "/";
pub const NACOS_VERSION: &str = "version";
pub const NACOS_SERVER_IP: &str = "nacos.server.ip";
pub const NACOS_SERVER_IP_STATE: &str = "nacos_server_ip";
pub const SERVER_PORT_STATE: &str = "server_port";
pub const USE_ONLY_SITE_INTERFACES: &str = "nacos.inetutils.use-only-site-local-interfaces";
pub const PREFERRED_NETWORKS: &str = "nacos.inetutils.preferred-networks";
pub const IGNORED_INTERFACES: &str = "nacos.inetutils.ignored-interfaces";
pub const AUTO_REFRESH_TIME: &str = "nacos.core.inet.auto-refresh";
pub const IP_ADDRESS: &str = "nacos.inetutils.ip-address";
pub const PREFER_HOSTNAME_OVER_IP: &str = "nacos.inetutils.prefer-hostname-over-ip";
pub const SYSTEM_PREFER_HOSTNAME_OVER_IP: &str = "nacos.preferHostnameOverIp";
pub const WEB_CONTEXT_PATH: &str = "server.servlet.context-path";
pub const COMMA_DIVISION: &str = ",";
pub const NACOS_SERVER_HEADER: &str = "Nacos-Server";
pub const REQUEST_PATH_SEPARATOR: &str = "-->";
pub const AVAILABLE_PROCESSORS_BASIC: &str = "nacos.core.sys.basic.processors";
pub const NACOS_DEPLOYMENT_TYPE: &str = "nacos.deployment.type";
pub const NACOS_DEPLOYMENT_TYPE_MERGED: &str = "merged";
pub const NACOS_DEPLOYMENT_TYPE_SERVER: &str = "server";
pub const NACOS_DEPLOYMENT_TYPE_CONSOLE: &str = "console";
pub const NACOS_DEPLOYMENT_TYPE_SERVER_WITH_MCP: &str = "serverWithMcp";

// Console mode constants
pub const NACOS_CONSOLE_MODE: &str = "nacos.console.mode";
pub const NACOS_CONSOLE_MODE_LOCAL: &str = "local";
pub const NACOS_CONSOLE_MODE_REMOTE: &str = "remote";

// Console remote configuration keys
pub const NACOS_CONSOLE_REMOTE_SERVER_ADDR: &str = "nacos.console.remote.server_addr";
pub const NACOS_CONSOLE_REMOTE_USERNAME: &str = "nacos.console.remote.username";
pub const NACOS_CONSOLE_REMOTE_PASSWORD: &str = "nacos.console.remote.password";
pub const NACOS_CONSOLE_REMOTE_CONNECT_TIMEOUT_MS: &str = "nacos.console.remote.connect_timeout_ms";
pub const NACOS_CONSOLE_REMOTE_READ_TIMEOUT_MS: &str = "nacos.console.remote.read_timeout_ms";
pub const NACOS_DUPLICATE_BEAN_ENHANCEMENT_ENABLED: &str =
    "nacos.sys.duplicate.bean.enhancement.enabled";

// Persistence consts.
pub const DEFAULT_ENCODE: &str = "UTF-8";

/**
 * May be removed with the upgrade of springboot version.
 */
pub const DATASOURCE_PLATFORM_PROPERTY_OLD: &str = "spring.datasource.platform";

pub const DATASOURCE_PLATFORM_PROPERTY: &str = "spring.sql.init.platform";

pub const MYSQL: &str = "mysql";

pub const EMPTY_DATASOURCE_PLATFORM: &str = "";

pub const EMBEDDED_STORAGE: &str = "embeddedStorage";

/**
 * The derby base dir.
 */
pub const DERBY_BASE_DIR: &str = "derby-data";

/**
 * Specifies that reads wait without timeout.
 */
pub const EXTEND_NEED_READ_UNTIL_HAVE_DATA: &str = "00--0-read-join-0--00";

pub const CONFIG_MODEL_RAFT_GROUP: &str = "nacos_config";

// Datasource plugin common constants.
pub const NACOS_PLUGIN_DATASOURCE_LOG: &str = "nacos.plugin.datasource.log.enabled";

// Server constants.
pub const CLIENT_VERSION_HEADER: &str = "Client-Version";

pub const DATASOURCE_PLATFORM_PROPERTY_STATE: &str = "datasource_platform";

pub const CONFIG_RENTENTION_DAYS_PROPERTY_STATE: &str = "config_retention_days";

/**
 * Config file directory in server side.
 */
pub const BASE_DIR: &str = "config-data";

pub const DATAID: &str = "dataId";

/**
 * Unit: millisecond.
 */
pub const CONN_TIMEOUT: i32 = 2000;

pub const BASE_V2_PATH: &str = "/v2/cs";

pub const BASE_ADMIN_V3_PATH: &str = "/v3/admin/cs";

pub const OPS_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/ops";

pub const CAPACITY_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/capacity";

pub const CONFIG_CONTROLLER_V2_PATH: &str = "/v2/cs/config";

pub const CONFIG_ADMIN_V3_PATH: &str = "/v3/admin/cs/config";

pub const HISTORY_CONTROLLER_V2_PATH: &str = "/v2/cs/history";

pub const HISTORY_ADMIN_V3_PATH: &str = "/v3/admin/cs/history";

pub const LISTENER_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/listener";

pub const METRICS_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/metrics";

pub const CONFIG_V3_CLIENT_API_PATH: &str = "/v3/client/cs/config";

pub const ENCODE_GBK: &str = "GBK";

pub const ENCODE_UTF8: &str = "UTF-8";

pub const MAP_FILE: &str = "map-file.js";

pub const NACOS_LINE_SEPARATOR: &str = "\r\n";

/**
 * Total time of threshold value when getting data from network(unit: millisecond).
 */
pub const TOTALTIME_FROM_SERVER: i64 = 10000;

/**
 * Invalid total time of threshold value when getting data from network(unit: millisecond).
 */
pub const TOTALTIME_INVALID_THRESHOLD: i64 = 60000;

/**
 * When exception or error occurs.
 */
pub const BATCH_OP_ERROR: i32 = -1;

/**
 * State code of single data when batch operation.
 */
pub const BATCH_OP_ERROR_IO_MSG: &str = "get config dump error";

pub const BATCH_OP_ERROR_CONFLICT_MSG: &str = "config get conflicts";

/**
 * Batch query when data existent.
 */
pub const BATCH_QUERY_EXISTS: i32 = 1;

pub const BATCH_QUERY_EXISTS_MSG: &str = "config exits";

/**
 * Batch query when data non-existent.
 */
pub const BATCH_QUERY_NONEXISTS: i32 = 2;

pub const BATCH_QUERY_NONEEXISTS_MSG: &str = "config not exits";

/**
 * Batch adding successfully.
 */
pub const BATCH_ADD_SUCCESS: i32 = 3;

/**
 * Batch updating successfully.
 */
pub const BATCH_UPDATE_SUCCESS: i32 = 4;

pub const MAX_UPDATE_FAIL_COUNT: i32 = 5;

pub const MAX_UPDATEALL_FAIL_COUNT: i32 = 5;

pub const MAX_REMOVE_FAIL_COUNT: i32 = 5;

pub const MAX_REMOVEALL_FAIL_COUNT: i32 = 5;

pub const MAX_NOTIFY_COUNT: i32 = 5;

pub const MAX_ADDACK_COUNT: i32 = 5;

/**
 * First version of data.
 */
pub const FIRST_VERSION: i32 = 1;

/**
 * Poison version when data is deleted.
 */
pub const POISON_VERSION: i32 = -1;

/**
 * Temporary version when disk file is full.
 */
pub const TEMP_VERSION: i32 = 0;

/**
 * Plain sequence of getting data: backup file -> server -> local file.
 */
pub const GETCONFIG_LOCAL_SERVER_SNAPSHOT: i32 = 1;

/**
 * Plain sequence of getting data: backup file -> local file -> server.
 */
pub const GETCONFIG_LOCAL_SNAPSHOT_SERVER: i32 = 2;

/**
 * Client, identity for sdk request to server.
 */
pub const REQUEST_IDENTITY: &str = "Request-Identity";

/**
 * Forward to leader node.
 */
pub const FORWARD_LEADER: &str = "Forward-Leader";

/**
 * Acl result information.
 */
pub const ACL_RESPONSE: &str = "ACL-Response";

pub const CONFIG_EXPORT_ITEM_FILE_SEPARATOR: &str = "/";

pub const CONFIG_EXPORT_METADATA: &str = ".meta.yml";

pub const CONFIG_EXPORT_METADATA_NEW: &str = ".metadata.yml";

pub const LIMIT_ERROR_CODE: i32 = 429;

pub const NACOS_PLUGIN_DATASOURCE_LOG_STATE: &str = "plugin_datasource_log_enabled";

pub const CONFIG_SEARCH_BLUR: &str = "blur";

pub const CONFIG_SEARCH_ACCURATE: &str = "accurate";

/**
 * Gray rule.
 */
pub const GRAY_RULE_TYPE: &str = "type";

pub const GRAY_RULE_EXPR: &str = "expr";

pub const GRAY_RULE_VERSION: &str = "version";

pub const GRAY_RULE_PRIORITY: &str = "priority";

/**
 * default nacos encode.
 */
pub const DEFAULT_NACOS_ENCODE: &str = "UTF-8";

pub const NACOS_PERSIST_ENCODE_KEY: &str = "nacosPersistEncodingKey";

/**
 * config publish type.
 */
pub const FORMAL: &str = "formal";

pub const GRAY: &str = "gray";

/**
 * request source type.
 */
pub const HTTP: &str = "http";

pub const RPC: &str = "rpc";

// Property constants.
pub const NOTIFY_CONNECT_TIMEOUT: &str = "notifyConnectTimeout";

pub const NOTIFY_SOCKET_TIMEOUT: &str = "notifySocketTimeout";

pub const IS_HEALTH_CHECK: &str = "isHealthCheck";

pub const MAX_HEALTH_CHECK_FAIL_COUNT: &str = "maxHealthCheckFailCount";

pub const MAX_CONTENT: &str = "maxContent";

pub const IS_MANAGE_CAPACITY: &str = "isManageCapacity";

pub const IS_CAPACITY_LIMIT_CHECK: &str = "isCapacityLimitCheck";

pub const DEFAULT_CLUSTER_QUOTA: &str = "defaultClusterQuota";

pub const DEFAULT_GROUP_QUOTA: &str = "defaultGroupQuota";

pub const DEFAULT_TENANT_QUOTA: &str = "defaultTenantQuota";

pub const DEFAULT_MAX_SIZE: &str = "defaultMaxSize";

pub const DEFAULT_MAX_AGGR_COUNT: &str = "defaultMaxAggrCount";

pub const DEFAULT_MAX_AGGR_SIZE: &str = "defaultMaxAggrSize";

pub const CORRECT_USAGE_DELAY: &str = "correctUsageDelay";

pub const INITIAL_EXPANSION_PERCENT: &str = "initialExpansionPercent";

pub const SEARCH_MAX_CAPACITY: &str = "nacos.config.search.max_capacity";

pub const SEARCH_MAX_THREAD: &str = "nacos.config.search.max_thread";

pub const SEARCH_WAIT_TIMEOUT: &str = "nacos.config.search.wait_timeout";

pub const DUMP_CHANGE_ON: &str = "dumpChangeOn";

pub const DUMP_CHANGE_WORKER_INTERVAL: &str = "dumpChangeWorkerInterval";

pub const CONFIG_RENTENTION_DAYS: &str = "nacos.config.retention.days";

pub const GRAY_CAPATIBEL_MODEL: &str = "nacos.config.gray.compatible.model";

pub const NAMESPACE_COMPATIBLE_MODE: &str = "nacos.config.namespace.compatible.mode";

// Auth moudle state constants.
pub const AUTH_MODULE: &str = "auth";
pub const AUTH_ENABLED: &str = "auth_enabled";
pub const AUTH_SYSTEM_TYPE: &str = "auth_system_type";
pub const AUTH_ADMIN_REQUEST: &str = "auth_admin_request";

pub const STANDALONE_MODE_ALONE: &str = "standalone";

pub const STANDALONE_MODE_CLUSTER: &str = "cluster";

pub const FUNCTION_MODE_CONFIG: &str = "config";

pub const FUNCTION_MODE_NAMING: &str = "naming";

/**
 * The key of nacos home.
 */
pub const NACOS_HOME_KEY: &str = "nacos.home";

// Reserved configuration constants for Nacos compatibility
#[allow(dead_code)]
const FILE_PREFIX: &str = "file:";
#[allow(dead_code)]
const SERVER_PORT_PROPERTY: &str = "nacos.server.main.port";
#[allow(dead_code)]
const DEFAULT_SERVER_PORT: i32 = 8849;
#[allow(dead_code)]
const DEFAULT_WEB_CONTEXT_PATH: &str = "/nacos";
#[allow(dead_code)]
const MEMBER_LIST_PROPERTY: &str = "nacos.member.list";
#[allow(dead_code)]
const NACOS_HOME_PROPERTY: &str = "user.home";
#[allow(dead_code)]
const CUSTOM_CONFIG_LOCATION_PROPERTY: &str = "spring.config.additional-location";
#[allow(dead_code)]
const DEFAULT_CONFIG_LOCATION: &str = "application.properties";
#[allow(dead_code)]
const DEFAULT_RESOURCE_PATH: &str = "/application.properties";
#[allow(dead_code)]
const DEFAULT_ADDITIONAL_PATH: &str = "conf";
#[allow(dead_code)]
const DEFAULT_ADDITIONAL_FILE: &str = "cluster.conf";
#[allow(dead_code)]
const NACOS_HOME_ADDITIONAL_FILEPATH: &str = "nacos";
#[allow(dead_code)]
const NACOS_TEMP_DIR_1: &str = "data";
#[allow(dead_code)]
const NACOS_TEMP_DIR_2: &str = "tmp";
#[allow(dead_code)]
const NACOS_CUSTOM_ENVIRONMENT_ENABLED: &str = "nacos.custom.environment.enabled";
#[allow(dead_code)]
const NACOS_CUSTOM_CONFIG_NAME: &str = "customFirstNacosConfig";

pub const NACOS_SERVER_CONTEXT: &str = "/nacos";

pub const NACOS_SERVER_VERSION: &str = "/v1";

pub const NACOS_SERVER_VERSION_V2: &str = "/v2";

pub const NACOS_SERVER_VERSION_V3: &str = "/v3";

pub const DEFAULT_NACOS_CORE_CONTEXT: &str = "/v1/core";

pub const NACOS_CORE_CONTEXT: &str = "/v1/core";

pub const NACOS_CORE_CONTEXT_V2: &str = "/v2/core";

pub const NACOS_ADMIN_CORE_CONTEXT_V3: &str = "/v3/admin/core";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Result<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> Result<T> {
    pub fn new(code: i32, message: String, data: T) -> Self {
        Result::<T> {
            code,
            message,
            data,
        }
    }

    pub fn success(data: T) -> Result<T> {
        Result::<T> {
            code: 0,
            message: "success".to_string(),
            data,
        }
    }

    pub fn http_success(data: impl Serialize) -> HttpResponse {
        HttpResponse::Ok().json(Result::success(data))
    }

    pub fn http_response(
        status: u16,
        code: i32,
        message: String,
        data: impl Serialize,
    ) -> HttpResponse {
        HttpResponseBuilder::new(StatusCode::from_u16(status).unwrap_or_default())
            .json(Result::new(code, message, data))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub config: Config,
}

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

    pub fn deployment_type(&self) -> String {
        self.config
            .get_string(NACOS_DEPLOYMENT_TYPE)
            .unwrap_or(NACOS_DEPLOYMENT_TYPE_MERGED.to_string())
    }

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

    pub fn notify_connect_timeout(&self) -> i32 {
        self.config.get_int(NOTIFY_CONNECT_TIMEOUT).unwrap_or(100) as i32
    }

    pub fn notify_socket_timeout(&self) -> i32 {
        self.config.get_int(NOTIFY_SOCKET_TIMEOUT).unwrap_or(200) as i32
    }

    pub fn is_health_check(&self) -> bool {
        self.config.get_bool(NOTIFY_SOCKET_TIMEOUT).unwrap_or(true)
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

    pub fn console_ui_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.console.ui.enabled")
            .unwrap_or(true)
    }

    pub fn auth_console_enabled(&self) -> bool {
        self.config
            .get_bool("nacos.core.auth.console.enabled")
            .unwrap_or(true)
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
            .unwrap_or(30) as u64;

        let url = self.config.get_string("db.url")?;

        let mut opt = ConnectOptions::new(url);

        opt.max_connections(max_connections)
            .min_connections(min_connections)
            .connect_timeout(Duration::from_secs(connect_timeout))
            .acquire_timeout(Duration::from_secs(acquire_timeout))
            .idle_timeout(Duration::from_secs(idle_timeout))
            .max_lifetime(Duration::from_secs(max_lifetime));

        let database_connection: DatabaseConnection = Database::connect(opt).await?;

        Ok(database_connection)
    }

    pub fn server_address(&self) -> String {
        self.config
            .get_string("server.address")
            .unwrap_or("0.0.0.0".to_string())
    }

    pub fn console_server_port(&self) -> u16 {
        self.config.get_int("nacos.console.port").unwrap_or(8081) as u16
    }

    pub fn console_server_context_path(&self) -> String {
        self.config
            .get_string("nacos.console.contextPath")
            .unwrap_or("".to_string())
    }

    pub fn token_secret_key(&self) -> String {
        self.config
            .get_string("nacos.core.auth.plugin.nacos.token.secret.key")
            .unwrap_or_default()
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

    pub fn auth_token_expire_seconds(&self) -> i64 {
        self.config
            .get_int(TOKEN_EXPIRE_SECONDS)
            .unwrap_or(DEFAULT_TOKEN_EXPIRE_SECONDS)
    }

    pub fn sdk_server_port(&self) -> u16 {
        self.server_main_port() + SDK_GRPC_PORT_DEFAULT_OFFSET
    }

    pub fn cluster_server_port(&self) -> u16 {
        self.server_main_port() + CLUSTER_GRPC_PORT_DEFAULT_OFFSET
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

    /// Convert to batata_core Configuration for use with ServerMemberManager
    pub fn to_core_config(&self) -> batata_core::model::Configuration {
        batata_core::model::Configuration::from_config(self.config.clone())
    }

    // OpenTelemetry configuration
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
}

use crate::console::datasource::ConsoleDataSource;

/// Application state shared across all handlers
///
/// For merged/server deployment:
/// - database_connection and server_member_manager are Some
/// - console_datasource uses LocalDataSource (wraps the database)
///
/// For console-only remote deployment:
/// - database_connection and server_member_manager are None
/// - console_datasource uses RemoteDataSource (HTTP calls to server)
pub struct AppState {
    pub configuration: Configuration,
    pub database_connection: Option<DatabaseConnection>,
    pub server_member_manager: Option<Arc<ServerMemberManager>>,
    pub console_datasource: Arc<dyn ConsoleDataSource>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("configuration", &self.configuration)
            .field("database_connection", &self.database_connection.is_some())
            .field(
                "server_member_manager",
                &self.server_member_manager.is_some(),
            )
            .field("console_datasource", &"<dyn ConsoleDataSource>")
            .finish()
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            configuration: self.configuration.clone(),
            database_connection: self.database_connection.clone(),
            server_member_manager: self.server_member_manager.clone(),
            console_datasource: self.console_datasource.clone(),
        }
    }
}

impl AppState {
    /// Check if this is a remote console deployment (no direct DB access)
    pub fn is_remote_console(&self) -> bool {
        self.database_connection.is_none()
    }

    /// Try to get database connection, returns None if not available
    /// Use this in code that needs to handle remote console mode gracefully
    pub fn try_db(&self) -> Option<&DatabaseConnection> {
        self.database_connection.as_ref()
    }

    /// Get database connection (panics if not available)
    /// Use this only in server endpoints that require database access
    pub fn db(&self) -> &DatabaseConnection {
        self.database_connection
            .as_ref()
            .expect("Database connection not available in remote console mode")
    }

    /// Try to get server member manager, returns None if not available
    /// Use this in code that needs to handle remote console mode gracefully
    pub fn try_member_manager(&self) -> Option<&Arc<ServerMemberManager>> {
        self.server_member_manager.as_ref()
    }

    /// Get server member manager (panics if not available)
    /// Use this only in server endpoints that require cluster management
    pub fn member_manager(&self) -> &Arc<ServerMemberManager> {
        self.server_member_manager
            .as_ref()
            .expect("Server member manager not available in remote console mode")
    }
}

impl AppState {
    pub fn config_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(15);

        state.insert(
            DATASOURCE_PLATFORM_PROPERTY_STATE.to_string(),
            Some(self.configuration.datasource_platform()),
        );
        state.insert(
            NACOS_PLUGIN_DATASOURCE_LOG_STATE.to_string(),
            Some(format!("{}", self.configuration.plugin_datasource_log())),
        );
        state.insert(
            NOTIFY_CONNECT_TIMEOUT.to_string(),
            Some(format!("{}", self.configuration.notify_connect_timeout())),
        );
        state.insert(
            NOTIFY_SOCKET_TIMEOUT.to_string(),
            Some(format!("{}", self.configuration.notify_socket_timeout())),
        );
        state.insert(
            IS_HEALTH_CHECK.to_string(),
            Some(format!("{}", self.configuration.is_health_check())),
        );
        state.insert(
            MAX_HEALTH_CHECK_FAIL_COUNT.to_string(),
            Some(format!(
                "{}",
                self.configuration.max_health_check_fail_count()
            )),
        );
        state.insert(
            MAX_CONTENT.to_string(),
            Some(format!("{}", self.configuration.max_content())),
        );
        state.insert(
            IS_MANAGE_CAPACITY.to_string(),
            Some(format!("{}", self.configuration.is_manage_capacity())),
        );
        state.insert(
            IS_CAPACITY_LIMIT_CHECK.to_string(),
            Some(format!("{}", self.configuration.is_capacity_limit_check())),
        );
        state.insert(
            DEFAULT_CLUSTER_QUOTA.to_string(),
            Some(format!("{}", self.configuration.default_cluster_quota())),
        );
        state.insert(
            DEFAULT_GROUP_QUOTA.to_string(),
            Some(format!("{}", self.configuration.default_group_quota())),
        );
        state.insert(
            DEFAULT_MAX_SIZE.to_string(),
            Some(format!("{}", self.configuration.default_max_size())),
        );
        state.insert(
            DEFAULT_MAX_AGGR_COUNT.to_string(),
            Some(format!("{}", self.configuration.default_max_aggr_count())),
        );
        state.insert(
            DEFAULT_MAX_AGGR_SIZE.to_string(),
            Some(format!("{}", self.configuration.default_max_aggr_size())),
        );
        state.insert(
            CONFIG_RENTENTION_DAYS_PROPERTY_STATE.to_string(),
            Some(format!("{}", self.configuration.config_rentention_days())),
        );

        state
    }

    pub fn auth_state(&self, is_admin_request: bool) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(3);

        state.insert(
            AUTH_ENABLED.to_string(),
            Some(format!("{}", self.configuration.auth_enabled())),
        );
        state.insert(
            AUTH_SYSTEM_TYPE.to_string(),
            Some(self.configuration.auth_system_type()),
        );
        state.insert(
            AUTH_ADMIN_REQUEST.to_string(),
            Some(format!("{}", is_admin_request)),
        );

        state
    }

    pub fn env_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(4);

        state.insert(
            STARTUP_MODE_STATE.to_string(),
            Some(self.configuration.startup_mode()),
        );
        state.insert(
            FUNCTION_MODE_STATE.to_string(),
            self.configuration.function_mode(),
        );
        state.insert(
            NACOS_VERSION.to_string(),
            Some(self.configuration.version()),
        );
        state.insert(
            SERVER_PORT_STATE.to_string(),
            Some(format!("{}", self.configuration.server_main_port())),
        );

        state
    }

    pub fn console_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(2);

        state.insert(
            "console_ui_enabled".to_string(),
            Some(format!("{}", self.configuration.console_ui_enabled())),
        );
        state.insert(
            "login_page_enabled".to_string(),
            Some(format!("{}", self.configuration.auth_console_enabled())),
        );

        state
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorResult {
    pub timestamp: String,
    pub status: i32,
    pub error: String,
    pub message: String,
    pub path: String,
}

impl ErrorResult {
    pub fn new(status: i32, error: String, message: String, path: String) -> Self {
        ErrorResult {
            timestamp: chrono::Utc::now().to_rfc3339(),
            status,
            error,
            message,
            path,
        }
    }

    pub fn forbidden(message: &str, path: &str) -> Self {
        ErrorResult {
            timestamp: chrono::Utc::now().to_rfc3339(),
            status: actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
            error: actix_web::http::StatusCode::FORBIDDEN
                .canonical_reason()
                .unwrap_or_default()
                .to_string(),
            message: message.to_string(),
            path: path.to_string(),
        }
    }

    pub fn http_response_forbidden(code: i32, message: &str, path: &str) -> HttpResponse {
        HttpResponse::Forbidden().json(ErrorResult::forbidden(
            format!("Code: {}, Message: {}", code, message).as_str(),
            path,
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsoleException {}

impl ConsoleException {
    pub fn handle_access_exception(message: String) -> HttpResponse {
        HttpResponse::Forbidden().body(message)
    }

    pub fn handle_illegal_argument_exception(message: String) -> HttpResponse {
        HttpResponse::BadRequest().body(format!("caused: {}", message))
    }

    pub fn handle_runtime_exception(code: u16, message: String) -> HttpResponse {
        HttpResponseBuilder::new(StatusCode::from_u16(code).unwrap_or_default())
            .body(format!("caused: {}", message))
    }

    pub fn handle_exception(uri: String, message: String) -> HttpResponse {
        if uri.contains(NACOS_SERVER_VERSION_V2) {
            HttpResponse::InternalServerError().json(Result::new(
                500,
                htmlescape::encode_minimal(format!("caused: {}", message).as_str()),
                "",
            ))
        } else {
            HttpResponse::InternalServerError().body(format!("caused: {}", message))
        }
    }
}
