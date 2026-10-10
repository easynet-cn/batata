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
    DEFAULT_HEART_BEAT_INTERVAL,
    DEFAULT_HEART_BEAT_TIMEOUT,
    DEFAULT_INSTANCE_ID_GENERATOR,
    DEFAULT_IP_DELETE_TIMEOUT,
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

// Re-export group/namespace constants from batata-common (kept there as the
// server-side source of truth; they are plain string constants).
pub use batata_common::{DEFAULT_GROUP, DEFAULT_NAMESPACE_ID};

// ============================================================================
// System Constants
// ============================================================================

/// System module name.
pub const SYS_MODULE: &str = "sys";
/// Spring profile name for standalone deployment.
pub const STANDALONE_SPRING_PROFILE: &str = "standalone";
/// Property that selects standalone deployment mode.
pub const STANDALONE_MODE_PROPERTY_NAME: &str = "batata.standalone";
/// State key holding the startup mode.
pub const STARTUP_MODE_STATE: &str = "startup_mode";
/// Property that selects the server function mode.
pub const FUNCTION_MODE_PROPERTY_NAME: &str = "batata.function_mode";
/// State key holding the function mode.
pub const FUNCTION_MODE_STATE: &str = "function_mode";
/// Property that prefers hostname over IP when reporting addresses.
pub const PREFER_HOSTNAME_OVER_IP_PROPERTY_NAME: &str = "batata.preferHostnameOverIp";

// ============================================================================
// Web Context Constants
// ============================================================================

/// Root web context path.
pub const ROOT_WEB_CONTEXT_PATH: &str = "/";
/// Version segment used in compatibility URLs.
pub const COMPAT_VERSION: &str = "version";
/// Version key kept as `nacos_version` for Nacos SDK compatibility.
pub const COMPAT_VERSION_KEY: &str = "nacos_version";
/// Version key reported by Batata.
pub const BATATA_VERSION_KEY: &str = "batata_version";
/// Property key for the server IP.
pub const SERVER_IP_KEY: &str = "batata.server.ip";
/// State key holding the server IP.
pub const SERVER_IP_STATE: &str = "nacos_server_ip";
/// State key holding the server port.
pub const SERVER_PORT_STATE: &str = "server_port";
/// Property key for the web servlet context path.
pub const WEB_CONTEXT_PATH: &str = "server.servlet.context-path";
/// Kept as "Nacos-Server" for Nacos SDK compatibility
pub const COMPAT_SERVER_HEADER: &str = "Nacos-Server";
/// Separator used between request path segments.
pub const REQUEST_PATH_SEPARATOR: &str = "-->";
/// Kept as "/nacos" for Nacos SDK compatibility
pub const SERVER_CONTEXT_PATH: &str = "/nacos";

// ============================================================================
// Network Constants
// ============================================================================

/// Property restricting interface selection to site-local addresses.
pub const USE_ONLY_SITE_INTERFACES: &str = "batata.inetutils.use-only-site-local-interfaces";
/// Property listing preferred networks for address selection.
pub const PREFERRED_NETWORKS: &str = "batata.inetutils.preferred-networks";
/// Property listing interfaces to ignore during address selection.
pub const IGNORED_INTERFACES: &str = "batata.inetutils.ignored-interfaces";
/// Property controlling how often network interfaces are refreshed.
pub const AUTO_REFRESH_TIME: &str = "batata.core.inet.auto-refresh";
/// Property forcing a specific IP address.
pub const IP_ADDRESS: &str = "batata.inetutils.ip-address";
/// Property preferring hostname over IP for network utilities.
pub const PREFER_HOSTNAME_OVER_IP: &str = "batata.inetutils.prefer-hostname-over-ip";
/// System property preferring hostname over IP.
pub const SYSTEM_PREFER_HOSTNAME_OVER_IP: &str = "batata.preferHostnameOverIp";
/// Separator used to split comma-separated values.
pub const COMMA_DIVISION: &str = ",";

// ============================================================================
// Deployment Type Constants
// ============================================================================

/// Property defining the baseline number of available processors.
pub const AVAILABLE_PROCESSORS_BASIC: &str = "batata.core.sys.basic.processors";
/// Property selecting the deployment type.
pub const DEPLOYMENT_TYPE: &str = "batata.deployment.type";
/// Deployment type: server and console run in one process.
pub const DEPLOYMENT_TYPE_MERGED: &str = "merged";
/// Deployment type: server only.
pub const DEPLOYMENT_TYPE_SERVER: &str = "server";
/// Deployment type: console only.
pub const DEPLOYMENT_TYPE_CONSOLE: &str = "console";
/// Deployment type: server with embedded MCP support.
pub const DEPLOYMENT_TYPE_SERVER_WITH_MCP: &str = "serverWithMcp";

// ============================================================================
// Console Mode Constants
// ============================================================================

/// Address of the remote server used in console-only deployments.
pub const CONSOLE_REMOTE_SERVER_ADDR: &str = "batata.console.remote.server_addr";
/// Username for the remote server in console-only deployments.
pub const CONSOLE_REMOTE_USERNAME: &str = "batata.console.remote.username";
/// Password for the remote server in console-only deployments.
pub const CONSOLE_REMOTE_PASSWORD: &str = "batata.console.remote.password";
/// Connect timeout in milliseconds for remote console calls.
pub const CONSOLE_REMOTE_CONNECT_TIMEOUT_MS: &str = "batata.console.remote.connect_timeout_ms";
/// Read timeout in milliseconds for remote console calls.
pub const CONSOLE_REMOTE_READ_TIMEOUT_MS: &str = "batata.console.remote.read_timeout_ms";

// ============================================================================
// Persistence Constants
// ============================================================================

/// Default character encoding.
pub const DEFAULT_ENCODE: &str = "UTF-8";
/// Property selecting the SQL datasource platform.
pub const DATASOURCE_PLATFORM_PROPERTY: &str = "batata.sql.init.platform";
/// Datasource platform name for MySQL.
pub const MYSQL: &str = "mysql";
/// Empty datasource platform value.
pub const EMPTY_DATASOURCE_PLATFORM: &str = "";
/// Datasource platform name for embedded storage.
pub const EMBEDDED_STORAGE: &str = "embeddedStorage";
/// Base directory for embedded Derby data.
pub const DERBY_BASE_DIR: &str = "derby-data";
/// Property enabling datasource logging for plugins.
pub const PLUGIN_DATASOURCE_LOG: &str = "batata.plugin.datasource.log.enabled";
/// State key for plugin datasource logging.
pub const PLUGIN_DATASOURCE_LOG_STATE: &str = "plugin_datasource_log_enabled";
/// State key holding the datasource platform.
pub const DATASOURCE_PLATFORM_PROPERTY_STATE: &str = "datasource_platform";

// ============================================================================
// Config Model Constants
// ============================================================================

/// Marker indicating a read must continue until data is available.
pub const EXTEND_NEED_READ_UNTIL_HAVE_DATA: &str = "00--0-read-join-0--00";
/// Raft group name used by the config module.
pub const CONFIG_MODEL_RAFT_GROUP: &str = "nacos_config";
/// HTTP header carrying the client version.
pub const CLIENT_VERSION_HEADER: &str = "Client-Version";
/// State key holding the config retention period in days.
pub const CONFIG_RENTENTION_DAYS_PROPERTY_STATE: &str = "config_retention_days";
/// Base directory for config data.
pub const BASE_DIR: &str = "config-data";
/// Parameter name for the config data ID.
pub const DATAID: &str = "dataId";
/// Default connection timeout in milliseconds.
pub const CONN_TIMEOUT: i32 = 2000;

// ============================================================================
// API Path Constants
// ============================================================================

/// Base path for v2 config service APIs.
pub const BASE_V2_PATH: &str = "/v2/cs";
/// Base path for v3 admin config service APIs.
pub const BASE_ADMIN_V3_PATH: &str = "/v3/admin/cs";
/// Base path for v3 admin ops APIs.
pub const OPS_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/ops";
/// Base path for v3 admin capacity APIs.
pub const CAPACITY_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/capacity";
/// Base path for v2 config APIs.
pub const CONFIG_CONTROLLER_V2_PATH: &str = "/v2/cs/config";
/// Base path for v3 admin config APIs.
pub const CONFIG_ADMIN_V3_PATH: &str = "/v3/admin/cs/config";
/// Base path for v2 config history APIs.
pub const HISTORY_CONTROLLER_V2_PATH: &str = "/v2/cs/history";
/// Base path for v3 admin config history APIs.
pub const HISTORY_ADMIN_V3_PATH: &str = "/v3/admin/cs/history";
/// Base path for v3 admin listener APIs.
pub const LISTENER_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/listener";
/// Base path for v3 admin metrics APIs.
pub const METRICS_CONTROLLER_V3_ADMIN_PATH: &str = "/v3/admin/cs/metrics";
/// Base path for v3 client config APIs.
pub const CONFIG_V3_CLIENT_API_PATH: &str = "/v3/client/cs/config";
/// Version segment for v2 APIs.
pub const SERVER_VERSION_V2: &str = "/v2";
/// Version segment for v3 APIs.
pub const SERVER_VERSION_V3: &str = "/v3";
/// Base path for v2 core APIs.
pub const CORE_CONTEXT_V2: &str = "/v2/core";
/// Base path for v3 admin core APIs.
pub const ADMIN_CORE_CONTEXT_V3: &str = "/v3/admin/core";

// ============================================================================
// Encoding Constants
// ============================================================================

/// GBK character encoding.
pub const ENCODE_GBK: &str = "GBK";
/// UTF-8 character encoding.
pub const ENCODE_UTF8: &str = "UTF-8";
/// Name of the map file used by the console.
pub const MAP_FILE: &str = "map-file.js";
/// CRLF line separator.
pub const CRLF_LINE_SEPARATOR: &str = "\r\n";
/// Default character set name.
pub const DEFAULT_ENCODE_CHARSET: &str = "UTF-8";
/// Kept as "nacosPersistEncodingKey" for wire-format compatibility
pub const PERSIST_ENCODE_KEY: &str = "nacosPersistEncodingKey";

// ============================================================================
// Timeout and Threshold Constants
// ============================================================================

/// Timeout in milliseconds for a total-time request to the server.
pub const TOTALTIME_FROM_SERVER: i64 = 10000;
/// Threshold in milliseconds above which a total-time value is considered invalid.
pub const TOTALTIME_INVALID_THRESHOLD: i64 = 60000;

// ============================================================================
// Batch Operation Constants
// ============================================================================

/// Batch operation result: generic error.
pub const BATCH_OP_ERROR: i32 = -1;
/// Batch operation error message: config dump failed.
pub const BATCH_OP_ERROR_IO_MSG: &str = "get config dump error";
/// Batch operation error message: conflicting config read.
pub const BATCH_OP_ERROR_CONFLICT_MSG: &str = "config get conflicts";
/// Batch query result: the config exists.
pub const BATCH_QUERY_EXISTS: i32 = 1;
/// Batch query message: the config exists.
pub const BATCH_QUERY_EXISTS_MSG: &str = "config exits";
/// Batch query result: the config does not exist.
pub const BATCH_QUERY_NONEXISTS: i32 = 2;
/// Batch query message: the config does not exist.
pub const BATCH_QUERY_NONEEXISTS_MSG: &str = "config not exits";
/// Batch operation result: added successfully.
pub const BATCH_ADD_SUCCESS: i32 = 3;
/// Batch operation result: updated successfully.
pub const BATCH_UPDATE_SUCCESS: i32 = 4;

// ============================================================================
// Max Count Constants
// ============================================================================

/// Maximum number of tolerated update failures.
pub const MAX_UPDATE_FAIL_COUNT: i32 = 5;
/// Maximum number of tolerated update-all failures.
pub const MAX_UPDATEALL_FAIL_COUNT: i32 = 5;
/// Maximum number of tolerated remove failures.
pub const MAX_REMOVE_FAIL_COUNT: i32 = 5;
/// Maximum number of tolerated remove-all failures.
pub const MAX_REMOVEALL_FAIL_COUNT: i32 = 5;
/// Maximum number of tolerated notification failures.
pub const MAX_NOTIFY_COUNT: i32 = 5;
/// Maximum number of tolerated add-ack failures.
pub const MAX_ADDACK_COUNT: i32 = 5;

// ============================================================================
// Version Constants
// ============================================================================

/// The first valid config version.
pub const FIRST_VERSION: i32 = 1;
/// Poison version marking a deleted config.
pub const POISON_VERSION: i32 = -1;
/// Temporary version used while a config is being written.
pub const TEMP_VERSION: i32 = 0;

// ============================================================================
// Get Config Constants
// ============================================================================

/// Config read policy: prefer the server, falling back to the local snapshot.
pub const GETCONFIG_LOCAL_SERVER_SNAPSHOT: i32 = 1;
/// Config read policy: prefer the local snapshot, falling back to the server.
pub const GETCONFIG_LOCAL_SNAPSHOT_SERVER: i32 = 2;

// ============================================================================
// Request/Response Constants
// ============================================================================

/// HTTP header carrying the request identity.
pub const REQUEST_IDENTITY: &str = "Request-Identity";
/// HTTP header indicating the request was forwarded to the leader.
pub const FORWARD_LEADER: &str = "Forward-Leader";
/// HTTP header carrying the ACL decision.
pub const ACL_RESPONSE: &str = "ACL-Response";
/// HTTP status code returned when a rate limit is exceeded.
pub const LIMIT_ERROR_CODE: i32 = 429;

// ============================================================================
// Config Export Constants
// ============================================================================

/// Path separator used between exported config item files.
pub const CONFIG_EXPORT_ITEM_FILE_SEPARATOR: &str = "/";
/// Legacy export metadata file name.
pub const CONFIG_EXPORT_METADATA: &str = ".meta.yml";
/// Current export metadata file name.
pub const CONFIG_EXPORT_METADATA_NEW: &str = ".metadata.yml";

// ============================================================================
// Config Search Constants
// ============================================================================

/// Config search mode: fuzzy match.
pub const CONFIG_SEARCH_BLUR: &str = "blur";
/// Config search mode: exact match.
pub const CONFIG_SEARCH_ACCURATE: &str = "accurate";

// ============================================================================
// Gray Rule Constants
// ============================================================================

/// Gray rule key: rule type.
pub const GRAY_RULE_TYPE: &str = "type";
/// Gray rule key: match expression.
pub const GRAY_RULE_EXPR: &str = "expr";
/// Gray rule key: rule version.
pub const GRAY_RULE_VERSION: &str = "version";
/// Gray rule key: rule priority.
pub const GRAY_RULE_PRIORITY: &str = "priority";

// ============================================================================
// Publish Type Constants
// ============================================================================

/// Publish type: formal (non-gray) release.
pub const FORMAL: &str = "formal";
/// Publish type: gray release.
pub const GRAY: &str = "gray";

// ============================================================================
// Request Source Type Constants
// ============================================================================

/// Request source type: HTTP.
pub const HTTP: &str = "http";
/// Request source type: RPC.
pub const RPC: &str = "rpc";

// ============================================================================
// Property Constants
// ============================================================================

/// Property for the config notify connect timeout.
pub const NOTIFY_CONNECT_TIMEOUT: &str = "batata.config.notify.connect_timeout";
/// Property for the config notify socket timeout.
pub const NOTIFY_SOCKET_TIMEOUT: &str = "batata.config.notify.socket_timeout";
/// Property enabling config health checks.
pub const IS_HEALTH_CHECK: &str = "batata.config.health_check.enabled";
/// Property for the maximum tolerated health check failures.
pub const MAX_HEALTH_CHECK_FAIL_COUNT: &str = "batata.config.health_check.max_fail_count";
/// Property for the maximum allowed config content size.
pub const MAX_CONTENT: &str = "batata.config.max_content";
/// Property enabling capacity management.
pub const IS_MANAGE_CAPACITY: &str = "batata.config.capacity.manage_enabled";
/// Property enabling capacity limit checks.
pub const IS_CAPACITY_LIMIT_CHECK: &str = "batata.config.capacity.limit_check";
/// Property for the default cluster quota.
pub const DEFAULT_CLUSTER_QUOTA: &str = "batata.config.capacity.default_cluster_quota";
/// Property for the default group quota.
pub const DEFAULT_GROUP_QUOTA: &str = "batata.config.capacity.default_group_quota";
/// Property for the default maximum config size.
pub const DEFAULT_MAX_SIZE: &str = "batata.config.capacity.default_max_size";
/// Property for the default maximum aggregated config count.
pub const DEFAULT_MAX_AGGR_COUNT: &str = "batata.config.capacity.default_max_aggr_count";
/// Property for the default maximum aggregated config size.
pub const DEFAULT_MAX_AGGR_SIZE: &str = "batata.config.capacity.default_max_aggr_size";

// ============================================================================
// Auth Module Constants
// ============================================================================

/// Auth module name.
pub const AUTH_MODULE: &str = "auth";
/// Property enabling authentication.
pub const AUTH_ENABLED: &str = "auth_enabled";
/// Property selecting the authentication system type.
pub const AUTH_SYSTEM_TYPE: &str = "auth_system_type";
/// Marker identifying an admin request for authorization purposes.
pub const AUTH_ADMIN_REQUEST: &str = "auth_admin_request";

// ============================================================================
// Standalone Mode Constants
// ============================================================================

/// Standalone mode value: single-node deployment.
pub const STANDALONE_MODE_ALONE: &str = "standalone";
/// Standalone mode value: cluster deployment.
pub const STANDALONE_MODE_CLUSTER: &str = "cluster";

// ============================================================================
// Function Mode Constants
// ============================================================================

/// Function mode value: config management only.
pub const FUNCTION_MODE_CONFIG: &str = "config";
/// Function mode value: service discovery only.
pub const FUNCTION_MODE_NAMING: &str = "naming";

// ============================================================================
// Home Directory Constants
// ============================================================================

/// Property pointing to the Batata home directory.
pub const HOME_KEY: &str = "batata.home";

// ============================================================================
// Internal Configuration Constants
// ============================================================================

/// Property for the main server port.
pub const SERVER_PORT_PROPERTY: &str = "batata.server.main.port";
/// Default main server port.
pub const DEFAULT_SERVER_PORT: i32 = 8849;
