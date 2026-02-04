//! Common constants for Batata server
//!
//! This module re-exports constants from batata_api and defines server-specific constants.

// Re-export common constants from batata_api::model
pub use batata_api::model::{
    // Header constants
    ACCEPT_ENCODING,
    ACCESS_TOKEN,
    ALL_PATTERN,
    AMORY_TAG,
    ANY_PATTERN,
    APP_CONN_LABELS_KEY,
    APP_CONN_LABELS_PREFERRED,
    APP_CONN_PREFIX,
    APPNAME,
    ASYNC_UPDATE_ADDRESS_INTERVAL,
    ATOMIC_MAX_SIZE,
    // Path constants
    BASE_PATH,
    CHARSET_KEY,
    CLIENT_APPNAME_HEADER,
    CLIENT_IP,
    CLIENT_MODULE_TYPE,
    CLIENT_REQUEST_TOKEN_HEADER,
    CLIENT_REQUEST_TS_HEADER,
    // Client constants
    CLIENT_VERSION,
    CLIENT_VERSION_KEY,
    CLUSTER_GRPC_PORT_DEFAULT_OFFSET,
    CLUSTER_NAME_PATTERN_STRING,
    COLON,
    CONFIG_CONTROLLER_PATH,
    CONFIG_GRAY_LABEL,
    CONFIG_LONG_POLL_TIMEOUT,
    CONFIG_RETRY_TIME,
    CONFIG_TYPE,
    CONFIG_VERSION,
    CONTENT_ENCODING,
    CONTENT_MD5,
    DAILY_DOMAINNAME,
    // Data identifiers
    DATA_ID,
    DATA_IN_BODY_VERSION,
    DEFAULT_CLUSTER_NAME,
    DEFAULT_DOMAINNAME,
    // Group/namespace constants
    DEFAULT_GROUP,
    DEFAULT_HEART_BEAT_INTERVAL,
    DEFAULT_HEART_BEAT_TIMEOUT,
    DEFAULT_INSTANCE_ID_GENERATOR,
    DEFAULT_IP_DELETE_TIMEOUT,
    DEFAULT_NAMESPACE_ID,
    DEFAULT_PROTECT_THRESHOLD,
    DEFAULT_REDO_DELAY_TIME,
    DEFAULT_REDO_THREAD_COUNT,
    DEFAULT_USE_CLOUD_NAMESPACE_PARSING,
    DEFAULT_USE_RAM_INFO_PARSING,
    DOT,
    // Encoding and separators
    ENCODE,
    ENCRYPTED_DATA_KEY,
    ENV_KEY,
    FLOW_CONTROL_INTERVAL,
    FLOW_CONTROL_SLOT,
    FLOW_CONTROL_THRESHOLD,
    GLOBAL_ADMIN,
    GROUP,
    GROUP_NAME,
    HTTP_PREFIX,
    IF_MODIFIED_SINCE,
    JVM_KEY,
    LAST_MODIFIED,
    LINE_BREAK,
    LINE_SEPARATOR,
    LOCATION_TAG,
    LONGPOLLING_LINE_SEPARATOR,
    MAX_RETRY,
    MIN_CONFIG_LONG_POLL_TIMEOUT,
    NAMESPACE_ID,
    NAMING_HTTP_HEADER_SPLITTER,
    NAMING_INSTANCE_ID_SEG_COUNT,
    NAMING_INSTANCE_ID_SPLITTER,
    NULL,
    NULL_STRING,
    NUMBER_PATTERN_STRING,
    // Timeout constants
    ONCE_TIMEOUT,
    POLLING_INTERVAL_TIME,
    POUND,
    PROBE_MODIFY_REQUEST,
    PROBE_MODIFY_RESPONSE,
    PROBE_MODIFY_RESPONSE_NEW,
    PROPERTIES_KEY,
    // Page type
    Page,
    RECV_WAIT_TIMEOUT,
    // Port offsets
    SDK_GRPC_PORT_DEFAULT_OFFSET,
    SERVICE_INFO_SPLIT_COUNT,
    SERVICE_INFO_SPLITER,
    SNOWFLAKE_INSTANCE_ID_GENERATOR,
    SO_TIMEOUT,
    SPACING_INTERVAL,
    TENANT,
    // Token constants
    TOKEN,
    TOKEN_REFRESH_WINDOW,
    TOKEN_TTL,
    // Misc constants
    UNKNOWN_APP,
    USE_ZIP,
    USERNAME,
    VIPSERVER_TAG,
    WEIGHT,
    WORD_SEPARATOR,
    WRITE_REDIRECT_CODE,
};

// ============================================================================
// System Constants
// ============================================================================

pub const SYS_MODULE: &str = "sys";
pub const STANDALONE_SPRING_PROFILE: &str = "standalone";
pub const STANDALONE_MODE_PROPERTY_NAME: &str = "nacos.standalone";
pub const STARTUP_MODE_STATE: &str = "startup_mode";
pub const FUNCTION_MODE_PROPERTY_NAME: &str = "nacos.functionMode";
pub const FUNCTION_MODE_STATE: &str = "function_mode";
pub const PREFER_HOSTNAME_OVER_IP_PROPERTY_NAME: &str = "nacos.preferHostnameOverIp";

// ============================================================================
// Web Context Constants
// ============================================================================

pub const ROOT_WEB_CONTEXT_PATH: &str = "/";
pub const NACOS_VERSION: &str = "version";
pub const NACOS_SERVER_IP: &str = "nacos.server.ip";
pub const NACOS_SERVER_IP_STATE: &str = "nacos_server_ip";
pub const SERVER_PORT_STATE: &str = "server_port";
pub const WEB_CONTEXT_PATH: &str = "server.servlet.context-path";
pub const NACOS_SERVER_HEADER: &str = "Nacos-Server";
pub const REQUEST_PATH_SEPARATOR: &str = "-->";
pub const NACOS_SERVER_CONTEXT: &str = "/nacos";

// ============================================================================
// Network Constants
// ============================================================================

pub const USE_ONLY_SITE_INTERFACES: &str = "nacos.inetutils.use-only-site-local-interfaces";
pub const PREFERRED_NETWORKS: &str = "nacos.inetutils.preferred-networks";
pub const IGNORED_INTERFACES: &str = "nacos.inetutils.ignored-interfaces";
pub const AUTO_REFRESH_TIME: &str = "nacos.core.inet.auto-refresh";
pub const IP_ADDRESS: &str = "nacos.inetutils.ip-address";
pub const PREFER_HOSTNAME_OVER_IP: &str = "nacos.inetutils.prefer-hostname-over-ip";
pub const SYSTEM_PREFER_HOSTNAME_OVER_IP: &str = "nacos.preferHostnameOverIp";
pub const COMMA_DIVISION: &str = ",";

// ============================================================================
// Deployment Type Constants
// ============================================================================

pub const AVAILABLE_PROCESSORS_BASIC: &str = "nacos.core.sys.basic.processors";
pub const NACOS_DEPLOYMENT_TYPE: &str = "nacos.deployment.type";
pub const NACOS_DEPLOYMENT_TYPE_MERGED: &str = "merged";
pub const NACOS_DEPLOYMENT_TYPE_SERVER: &str = "server";
pub const NACOS_DEPLOYMENT_TYPE_CONSOLE: &str = "console";
pub const NACOS_DEPLOYMENT_TYPE_SERVER_WITH_MCP: &str = "serverWithMcp";

// ============================================================================
// Console Mode Constants
// ============================================================================

pub const NACOS_CONSOLE_MODE: &str = "nacos.console.mode";
pub const NACOS_CONSOLE_MODE_LOCAL: &str = "local";
pub const NACOS_CONSOLE_MODE_REMOTE: &str = "remote";
pub const NACOS_CONSOLE_REMOTE_SERVER_ADDR: &str = "nacos.console.remote.server_addr";
pub const NACOS_CONSOLE_REMOTE_USERNAME: &str = "nacos.console.remote.username";
pub const NACOS_CONSOLE_REMOTE_PASSWORD: &str = "nacos.console.remote.password";
pub const NACOS_CONSOLE_REMOTE_CONNECT_TIMEOUT_MS: &str = "nacos.console.remote.connect_timeout_ms";
pub const NACOS_CONSOLE_REMOTE_READ_TIMEOUT_MS: &str = "nacos.console.remote.read_timeout_ms";

// ============================================================================
// Persistence Constants
// ============================================================================

pub const DEFAULT_ENCODE: &str = "UTF-8";
pub const DATASOURCE_PLATFORM_PROPERTY_OLD: &str = "spring.datasource.platform";
pub const DATASOURCE_PLATFORM_PROPERTY: &str = "spring.sql.init.platform";
pub const MYSQL: &str = "mysql";
pub const EMPTY_DATASOURCE_PLATFORM: &str = "";
pub const EMBEDDED_STORAGE: &str = "embeddedStorage";
pub const DERBY_BASE_DIR: &str = "derby-data";
pub const NACOS_PLUGIN_DATASOURCE_LOG: &str = "nacos.plugin.datasource.log.enabled";
pub const NACOS_PLUGIN_DATASOURCE_LOG_STATE: &str = "plugin_datasource_log_enabled";
pub const DATASOURCE_PLATFORM_PROPERTY_STATE: &str = "datasource_platform";

// ============================================================================
// Config Model Constants
// ============================================================================

pub const EXTEND_NEED_READ_UNTIL_HAVE_DATA: &str = "00--0-read-join-0--00";
pub const CONFIG_MODEL_RAFT_GROUP: &str = "nacos_config";
pub const CLIENT_VERSION_HEADER: &str = "Client-Version";
pub const CONFIG_RENTENTION_DAYS_PROPERTY_STATE: &str = "config_retention_days";
pub const BASE_DIR: &str = "config-data";
pub const DATAID: &str = "dataId";
pub const CONN_TIMEOUT: i32 = 2000;

// ============================================================================
// API Path Constants
// ============================================================================

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
pub const NACOS_SERVER_VERSION: &str = "/v1";
pub const NACOS_SERVER_VERSION_V2: &str = "/v2";
pub const NACOS_SERVER_VERSION_V3: &str = "/v3";
pub const DEFAULT_NACOS_CORE_CONTEXT: &str = "/v1/core";
pub const NACOS_CORE_CONTEXT: &str = "/v1/core";
pub const NACOS_CORE_CONTEXT_V2: &str = "/v2/core";
pub const NACOS_ADMIN_CORE_CONTEXT_V3: &str = "/v3/admin/core";

// ============================================================================
// Encoding Constants
// ============================================================================

pub const ENCODE_GBK: &str = "GBK";
pub const ENCODE_UTF8: &str = "UTF-8";
pub const MAP_FILE: &str = "map-file.js";
pub const NACOS_LINE_SEPARATOR: &str = "\r\n";
pub const DEFAULT_NACOS_ENCODE: &str = "UTF-8";
pub const NACOS_PERSIST_ENCODE_KEY: &str = "nacosPersistEncodingKey";

// ============================================================================
// Timeout and Threshold Constants
// ============================================================================

pub const TOTALTIME_FROM_SERVER: i64 = 10000;
pub const TOTALTIME_INVALID_THRESHOLD: i64 = 60000;

// ============================================================================
// Batch Operation Constants
// ============================================================================

pub const BATCH_OP_ERROR: i32 = -1;
pub const BATCH_OP_ERROR_IO_MSG: &str = "get config dump error";
pub const BATCH_OP_ERROR_CONFLICT_MSG: &str = "config get conflicts";
pub const BATCH_QUERY_EXISTS: i32 = 1;
pub const BATCH_QUERY_EXISTS_MSG: &str = "config exits";
pub const BATCH_QUERY_NONEXISTS: i32 = 2;
pub const BATCH_QUERY_NONEEXISTS_MSG: &str = "config not exits";
pub const BATCH_ADD_SUCCESS: i32 = 3;
pub const BATCH_UPDATE_SUCCESS: i32 = 4;

// ============================================================================
// Max Count Constants
// ============================================================================

pub const MAX_UPDATE_FAIL_COUNT: i32 = 5;
pub const MAX_UPDATEALL_FAIL_COUNT: i32 = 5;
pub const MAX_REMOVE_FAIL_COUNT: i32 = 5;
pub const MAX_REMOVEALL_FAIL_COUNT: i32 = 5;
pub const MAX_NOTIFY_COUNT: i32 = 5;
pub const MAX_ADDACK_COUNT: i32 = 5;

// ============================================================================
// Version Constants
// ============================================================================

pub const FIRST_VERSION: i32 = 1;
pub const POISON_VERSION: i32 = -1;
pub const TEMP_VERSION: i32 = 0;

// ============================================================================
// Get Config Constants
// ============================================================================

pub const GETCONFIG_LOCAL_SERVER_SNAPSHOT: i32 = 1;
pub const GETCONFIG_LOCAL_SNAPSHOT_SERVER: i32 = 2;

// ============================================================================
// Request/Response Constants
// ============================================================================

pub const REQUEST_IDENTITY: &str = "Request-Identity";
pub const FORWARD_LEADER: &str = "Forward-Leader";
pub const ACL_RESPONSE: &str = "ACL-Response";
pub const LIMIT_ERROR_CODE: i32 = 429;

// ============================================================================
// Config Export Constants
// ============================================================================

pub const CONFIG_EXPORT_ITEM_FILE_SEPARATOR: &str = "/";
pub const CONFIG_EXPORT_METADATA: &str = ".meta.yml";
pub const CONFIG_EXPORT_METADATA_NEW: &str = ".metadata.yml";

// ============================================================================
// Config Search Constants
// ============================================================================

pub const CONFIG_SEARCH_BLUR: &str = "blur";
pub const CONFIG_SEARCH_ACCURATE: &str = "accurate";

// ============================================================================
// Gray Rule Constants
// ============================================================================

pub const GRAY_RULE_TYPE: &str = "type";
pub const GRAY_RULE_EXPR: &str = "expr";
pub const GRAY_RULE_VERSION: &str = "version";
pub const GRAY_RULE_PRIORITY: &str = "priority";

// ============================================================================
// Publish Type Constants
// ============================================================================

pub const FORMAL: &str = "formal";
pub const GRAY: &str = "gray";

// ============================================================================
// Request Source Type Constants
// ============================================================================

pub const HTTP: &str = "http";
pub const RPC: &str = "rpc";

// ============================================================================
// Property Constants
// ============================================================================

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
pub const NACOS_DUPLICATE_BEAN_ENHANCEMENT_ENABLED: &str =
    "nacos.sys.duplicate.bean.enhancement.enabled";

// ============================================================================
// Auth Module Constants
// ============================================================================

pub const AUTH_MODULE: &str = "auth";
pub const AUTH_ENABLED: &str = "auth_enabled";
pub const AUTH_SYSTEM_TYPE: &str = "auth_system_type";
pub const AUTH_ADMIN_REQUEST: &str = "auth_admin_request";

// ============================================================================
// Standalone Mode Constants
// ============================================================================

pub const STANDALONE_MODE_ALONE: &str = "standalone";
pub const STANDALONE_MODE_CLUSTER: &str = "cluster";

// ============================================================================
// Function Mode Constants
// ============================================================================

pub const FUNCTION_MODE_CONFIG: &str = "config";
pub const FUNCTION_MODE_NAMING: &str = "naming";

// ============================================================================
// Home Directory Constants
// ============================================================================

pub const NACOS_HOME_KEY: &str = "nacos.home";

// ============================================================================
// Internal Configuration Constants
// ============================================================================

pub const SERVER_PORT_PROPERTY: &str = "nacos.server.main.port";
pub const DEFAULT_SERVER_PORT: i32 = 8849;

// Reserved configuration constants for Nacos compatibility
#[allow(dead_code)]
pub(crate) const FILE_PREFIX: &str = "file:";
#[allow(dead_code)]
pub(crate) const DEFAULT_WEB_CONTEXT_PATH: &str = "/nacos";
#[allow(dead_code)]
pub(crate) const MEMBER_LIST_PROPERTY: &str = "nacos.member.list";
#[allow(dead_code)]
pub(crate) const NACOS_HOME_PROPERTY: &str = "user.home";
#[allow(dead_code)]
pub(crate) const CUSTOM_CONFIG_LOCATION_PROPERTY: &str = "spring.config.additional-location";
#[allow(dead_code)]
pub(crate) const DEFAULT_CONFIG_LOCATION: &str = "application.properties";
#[allow(dead_code)]
pub(crate) const DEFAULT_RESOURCE_PATH: &str = "/application.properties";
#[allow(dead_code)]
pub(crate) const DEFAULT_ADDITIONAL_PATH: &str = "conf";
#[allow(dead_code)]
pub(crate) const DEFAULT_ADDITIONAL_FILE: &str = "cluster.conf";
#[allow(dead_code)]
pub(crate) const NACOS_HOME_ADDITIONAL_FILEPATH: &str = "nacos";
#[allow(dead_code)]
pub(crate) const NACOS_TEMP_DIR_1: &str = "data";
#[allow(dead_code)]
pub(crate) const NACOS_TEMP_DIR_2: &str = "tmp";
#[allow(dead_code)]
pub(crate) const NACOS_CUSTOM_ENVIRONMENT_ENABLED: &str = "nacos.custom.environment.enabled";
#[allow(dead_code)]
pub(crate) const NACOS_CUSTOM_CONFIG_NAME: &str = "customFirstNacosConfig";
