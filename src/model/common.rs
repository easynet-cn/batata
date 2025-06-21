use std::collections::HashMap;

use config::Config;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Common constants.
pub const CLIENT_VERSION: &str = "3.0.0";
pub const DATA_IN_BODY_VERSION: i32 = 204;
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";
pub const APPNAME: &str = "AppName";
pub const CLIENT_VERSION_KEY: &str = "ClientVersion";
pub const CLIENT_IP: &str = "ClientIp";
pub const UNKNOWN_APP: &str = "UnknownApp";
pub const DEFAULT_DOMAINNAME: &str = "commonconfig.config-host.taobao.com";
pub const DAILY_DOMAINNAME: &str = "commonconfig.taobao.net";
pub const NULL: &str = "";
pub const DATA_ID: &str = "dataId";
pub const TENANT: &str = "tenant";
pub const GROUP: &str = "group";
pub const NAMESPACE_ID: &str = "namespaceId";
pub const LAST_MODIFIED: &str = "Last-Modified";
pub const ACCEPT_ENCODING: &str = "Accept-Encoding";
pub const CONTENT_ENCODING: &str = "Content-Encoding";
pub const PROBE_MODIFY_REQUEST: &str = "Listening-Configs";
pub const PROBE_MODIFY_RESPONSE: &str = "Probe-Modify-Response";
pub const PROBE_MODIFY_RESPONSE_NEW: &str = "Probe-Modify-Response-New";
pub const USE_ZIP: &str = "true";
pub const CONTENT_MD5: &str = "Content-MD5";
pub const CONFIG_VERSION: &str = "Config-Version";
pub const CONFIG_TYPE: &str = "Config-Type";
pub const ENCRYPTED_DATA_KEY: &str = "Encrypted-Data-Key";
pub const IF_MODIFIED_SINCE: &str = "If-Modified-Since";
pub const SPACING_INTERVAL: &str = "client-spacing-interval";
pub const BASE_PATH: &str = "/v1/cs";
pub const CONFIG_CONTROLLER_PATH: &str = "/v1/cs/configs";
pub const TOKEN: &str = "token";
pub const ACCESS_TOKEN: &str = "accessToken";
pub const TOKEN_TTL: &str = "tokenTtl";
pub const GLOBAL_ADMIN: &str = "globalAdmin";
pub const USERNAME: &str = "username";
pub const TOKEN_REFRESH_WINDOW: &str = "tokenRefreshWindow";
pub const SDK_GRPC_PORT_DEFAULT_OFFSET: i32 = 1000;
pub const CLUSTER_GRPC_PORT_DEFAULT_OFFSET: i32 = 1001;

// second.
pub const ASYNC_UPDATE_ADDRESS_INTERVAL: i32 = 300;

// second.
pub const POLLING_INTERVAL_TIME: i32 = 15;

// millisecond.
pub const ONCE_TIMEOUT: i32 = 2000;

// millisecond.
pub const SO_TIMEOUT: i32 = 60000;

//millisecond.
pub const CONFIG_LONG_POLL_TIMEOUT: i32 = 30000;

// millisecond.
pub const MIN_CONFIG_LONG_POLL_TIMEOUT: i32 = 10000;

// millisecond.
pub const CONFIG_RETRY_TIME: i32 = 2000;

// Maximum number of retries.
pub const MAX_RETRY: i32 = 3;

// millisecond.
pub const RECV_WAIT_TIMEOUT: i32 = ONCE_TIMEOUT * 5;

pub const ENCODE: &str = "UTF-8";
pub const FLOW_CONTROL_THRESHOLD: i32 = 20;
pub const FLOW_CONTROL_SLOT: i32 = 10;
pub const FLOW_CONTROL_INTERVAL: i32 = 1000;
pub const DEFAULT_PROTECT_THRESHOLD: f32 = 0.0;
pub const LINE_SEPARATOR: &str = "\u{1}";
pub const WORD_SEPARATOR: &str = "\u{2}";
pub const LONGPOLLING_LINE_SEPARATOR: &str = "\r\n";
pub const CLIENT_APPNAME_HEADER: &str = "Client-AppName";
pub const CLIENT_REQUEST_TS_HEADER: &str = "Client-RequestTS";
pub const CLIENT_REQUEST_TOKEN_HEADER: &str = "Client-RequestToken";
pub const ATOMIC_MAX_SIZE: i32 = 1000;
pub const NAMING_INSTANCE_ID_SPLITTER: &str = "#";
pub const NAMING_INSTANCE_ID_SEG_COUNT: i32 = 4;
pub const NAMING_HTTP_HEADER_SPLITTER: &str = "\\|";
pub const DEFAULT_CLUSTER_NAME: &str = "DEFAULT";
pub const DEFAULT_HEART_BEAT_TIMEOUT: i64 = 15000;
pub const DEFAULT_IP_DELETE_TIMEOUT: i64 = 30000;
pub const DEFAULT_HEART_BEAT_INTERVAL: i64 = 5000;
pub const DEFAULT_NAMESPACE_ID: &str = "pub";
pub const DEFAULT_USE_CLOUD_NAMESPACE_PARSING: bool = true;
pub const WRITE_REDIRECT_CODE: i32 = 307;
pub const SERVICE_INFO_SPLITER: &str = "@@";
pub const SERVICE_INFO_SPLIT_COUNT: i32 = 2;
pub const NULL_STRING: &str = "null";
pub const NUMBER_PATTERN_STRING: &str = "^\\d+$";
pub const ANY_PATTERN: &str = ".*";
pub const DEFAULT_INSTANCE_ID_GENERATOR: &str = "simple";
pub const SNOWFLAKE_INSTANCE_ID_GENERATOR: &str = "snowflake";
pub const HTTP_PREFIX: &str = "http";
pub const ALL_PATTERN: &str = "*";
pub const COLON: &str = ":";
pub const LINE_BREAK: &str = "\n";
pub const POUND: &str = "#";
pub const VIPSERVER_TAG: &str = "Vipserver-Tag";
pub const AMORY_TAG: &str = "Amory-Tag";
pub const LOCATION_TAG: &str = "Location-Tag";
pub const CHARSET_KEY: &str = "charset";
pub const CLUSTER_NAME_PATTERN_STRING: &str = "^[0-9a-zA-Z-]+$";

/**
 * millisecond.
 */
pub const DEFAULT_REDO_DELAY_TIME: i64 = 3000;
pub const DEFAULT_REDO_THREAD_COUNT: i32 = 1;
pub const APP_CONN_LABELS_KEY: &str = "nacos.app.conn.labels";

pub const DOT: &str = ".";

pub const WEIGHT: &str = "weight";

pub const PROPERTIES_KEY: &str = "properties";

pub const JVM_KEY: &str = "jvm";

pub const ENV_KEY: &str = "env";

pub const APP_CONN_LABELS_PREFERRED: &str = "nacos_app_conn_labels_preferred";

pub const APP_CONN_PREFIX: &str = "app_";

pub const CONFIG_GRAY_LABEL: &str = "nacos.config.gray.label";

/**
 * Since 2.3.3, For some situation like java agent using nacos-client which can't use env ram info.
 */
pub const DEFAULT_USE_RAM_INFO_PARSING: &str = "true";

pub const CLIENT_MODULE_TYPE: &str = "clientModuleType";

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_count: u64,
    pub page_number: u64,
    pub pages_available: u64,
    pub page_items: Vec<T>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            total_count: 0,
            page_number: 1,
            pages_available: 0,
            page_items: vec![],
        }
    }
}

impl<T> Page<T> {
    pub fn new(total_count: u64, page_number: u64, page_size: u64, page_items: Vec<T>) -> Self {
        Self {
            total_count: total_count,
            page_number: page_number,
            pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
            page_items: page_items,
        }
    }
}

#[derive(Error, Clone, Debug, Serialize, Deserialize)]
pub enum BusinessError {
    #[error("user '{0}' not exist!")]
    UserNotExist(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ErrorCode<'a> {
    pub code: i32,
    pub message: &'a str,
}

pub const SUCCESS: ErrorCode<'static> = ErrorCode {
    code: 0,
    message: "success",
};

pub const PARAMETER_MISSING: ErrorCode<'static> = ErrorCode {
    code: 10000,
    message: "parameter missing",
};

pub const ACCESS_DENIED: ErrorCode<'static> = ErrorCode {
    code: 10001,
    message: "access denied",
};

pub const DATA_ACCESS_ERROR: ErrorCode<'static> = ErrorCode {
    code: 10002,
    message: "data access error",
};

pub const TENANT_PARAM_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20001,
    message: "'tenant' parameter error",
};

pub const PARAMETER_VALIDATE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20002,
    message: "parameter validate error",
};

pub const MEDIA_TYPE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20003,
    message: "MediaType Error",
};

pub const RESOURCE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 20004,
    message: "resource not found",
};

pub const RESOURCE_CONFLICT: ErrorCode<'static> = ErrorCode {
    code: 20005,
    message: "resource conflict",
};

pub const CONFIG_LISTENER_IS_NULL: ErrorCode<'static> = ErrorCode {
    code: 20006,
    message: "config listener is null",
};

pub const CONFIG_LISTENER_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20007,
    message: "config listener error",
};

pub const INVALID_DATA_ID: ErrorCode<'static> = ErrorCode {
    code: 20008,
    message: "invalid dataId",
};

pub const PARAMETER_MISMATCH: ErrorCode<'static> = ErrorCode {
    code: 20009,
    message: "parameter mismatch",
};

pub const SERVICE_NAME_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21000,
    message: "service name error",
};

pub const WEIGHT_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21001,
    message: "weight error",
};

pub const INSTANCE_METADATA_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21002,
    message: "instance metadata error",
};

pub const INSTANCE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 21003,
    message: "instance not found",
};

pub const INSTANCE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21004,
    message: "instance error",
};

pub const SERVICE_METADATA_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21005,
    message: "service metadata error",
};

pub const SELECTOR_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21006,
    message: "selector error",
};

pub const SERVICE_ALREADY_EXIST: ErrorCode<'static> = ErrorCode {
    code: 21007,
    message: "service already exist",
};

pub const SERVICE_NOT_EXIST: ErrorCode<'static> = ErrorCode {
    code: 21008,
    message: "service not exist",
};

pub const SERVICE_DELETE_FAILURE: ErrorCode<'static> = ErrorCode {
    code: 21009,
    message: "service delete failure",
};

pub const HEALTHY_PARAM_MISS: ErrorCode<'static> = ErrorCode {
    code: 21010,
    message: "healthy param miss",
};

pub const HEALTH_CHECK_STILL_RUNNING: ErrorCode<'static> = ErrorCode {
    code: 21011,
    message: "health check still runnin",
};

pub const ILLEGAL_NAMESPACE: ErrorCode<'static> = ErrorCode {
    code: 22000,
    message: "illegal namespace",
};

pub const NAMESPACE_NOT_EXIST: ErrorCode<'static> = ErrorCode {
    code: 22002,
    message: "namespace already exist",
};

pub const ILLEGAL_STATE: ErrorCode<'static> = ErrorCode {
    code: 23000,
    message: "illegal state",
};

pub const NODE_INFO_ERROR: ErrorCode<'static> = ErrorCode {
    code: 23001,
    message: "node info error",
};

pub const NODE_DOWN_FAILURE: ErrorCode<'static> = ErrorCode {
    code: 23002,
    message: "node down failure",
};

pub const SERVER_ERROR: ErrorCode<'static> = ErrorCode {
    code: 30000,
    message: "server error",
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Result<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> Result<T> {
    pub fn success(data: T) -> Result<T> {
        Result::<T> {
            code: SUCCESS.code,
            message: SUCCESS.message.to_string(),
            data,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub app_config: Config,
    pub database_connection: DatabaseConnection,
    pub context_path: String,
    pub token_secret_key: String,
}

impl AppState {
    pub fn server_port(&self) -> i32 {
        self.app_config.get_int("server.port").unwrap_or(8081) as i32
    }

    pub fn datasource_platform(&self) -> String {
        self.app_config
            .get_string(DATASOURCE_PLATFORM_PROPERTY)
            .unwrap_or("false".to_string())
    }

    pub fn plugin_datasource_log(&self) -> bool {
        self.app_config
            .get_bool(NACOS_PLUGIN_DATASOURCE_LOG)
            .unwrap_or(false)
    }

    pub fn notify_connect_timeout(&self) -> i32 {
        self.app_config
            .get_int(NOTIFY_CONNECT_TIMEOUT)
            .unwrap_or(100) as i32
    }

    pub fn notify_socket_timeout(&self) -> i32 {
        self.app_config
            .get_int(NOTIFY_SOCKET_TIMEOUT)
            .unwrap_or(200) as i32
    }

    pub fn is_health_check(&self) -> bool {
        self.app_config
            .get_bool(NOTIFY_SOCKET_TIMEOUT)
            .unwrap_or(true)
    }

    pub fn max_health_check_fail_count(&self) -> i32 {
        self.app_config
            .get_int(MAX_HEALTH_CHECK_FAIL_COUNT)
            .unwrap_or(12) as i32
    }

    pub fn max_content(&self) -> i32 {
        self.app_config
            .get_int(MAX_CONTENT)
            .unwrap_or(10 * 1024 * 1024) as i32
    }

    pub fn is_manage_capacity(&self) -> bool {
        self.app_config.get_bool(IS_MANAGE_CAPACITY).unwrap_or(true)
    }

    pub fn is_capacity_limit_check(&self) -> bool {
        self.app_config
            .get_bool(IS_CAPACITY_LIMIT_CHECK)
            .unwrap_or(false)
    }

    pub fn default_cluster_quota(&self) -> i32 {
        self.app_config
            .get_int(DEFAULT_CLUSTER_QUOTA)
            .unwrap_or(100000) as i32
    }

    pub fn default_group_quota(&self) -> i32 {
        self.app_config.get_int(DEFAULT_GROUP_QUOTA).unwrap_or(200) as i32
    }

    pub fn default_max_size(&self) -> i32 {
        self.app_config
            .get_int(DEFAULT_MAX_SIZE)
            .unwrap_or(100 * 1024) as i32
    }

    pub fn default_max_aggr_count(&self) -> i32 {
        self.app_config
            .get_int(DEFAULT_MAX_AGGR_COUNT)
            .unwrap_or(10000) as i32
    }

    pub fn default_max_aggr_size(&self) -> i32 {
        self.app_config
            .get_int(DEFAULT_MAX_AGGR_SIZE)
            .unwrap_or(1024) as i32
    }

    pub fn config_rentention_days(&self) -> i32 {
        self.app_config
            .get_int(CONFIG_RENTENTION_DAYS)
            .unwrap_or(30) as i32
    }

    pub fn auth_enabled(&self) -> bool {
        self.app_config
            .get_bool("nacos.core.auth.enabled")
            .unwrap_or(false)
            || self
                .app_config
                .get_bool("nacos.core.auth.admin.enabled")
                .unwrap_or(false)
    }

    pub fn auth_system_type(&self) -> String {
        self.app_config
            .get_string("nacos.core.auth.system.type")
            .unwrap_or("nacos".to_string())
    }

    pub fn is_standalone(&self) -> bool {
        self.app_config
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
        if let Ok(v) = self.app_config.get_string(FUNCTION_MODE_PROPERTY_NAME) {
            Some(v)
        } else {
            None
        }
    }

    pub fn version(&self) -> String {
        self.app_config
            .get_string("nacos.version")
            .unwrap_or("".to_string())
    }

    pub fn console_ui_enabled(&self) -> bool {
        self.app_config
            .get_bool("nacos.console.ui.enabled")
            .unwrap_or(true)
    }

    pub fn auth_console_enabled(&self) -> bool {
        self.app_config
            .get_bool("nacos.core.auth.console.enabled")
            .unwrap_or(true)
    }

    pub fn config_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(15);

        state.insert(
            DATASOURCE_PLATFORM_PROPERTY_STATE.to_string(),
            Some(self.datasource_platform()),
        );
        state.insert(
            NACOS_PLUGIN_DATASOURCE_LOG_STATE.to_string(),
            Some(format!("{}", self.plugin_datasource_log())),
        );
        state.insert(
            NOTIFY_CONNECT_TIMEOUT.to_string(),
            Some(format!("{}", self.notify_connect_timeout())),
        );
        state.insert(
            NOTIFY_SOCKET_TIMEOUT.to_string(),
            Some(format!("{}", self.notify_socket_timeout())),
        );
        state.insert(
            IS_HEALTH_CHECK.to_string(),
            Some(format!("{}", self.is_health_check())),
        );
        state.insert(
            MAX_HEALTH_CHECK_FAIL_COUNT.to_string(),
            Some(format!("{}", self.max_health_check_fail_count())),
        );
        state.insert(
            MAX_CONTENT.to_string(),
            Some(format!("{}", self.max_content())),
        );
        state.insert(
            IS_MANAGE_CAPACITY.to_string(),
            Some(format!("{}", self.is_manage_capacity())),
        );
        state.insert(
            IS_CAPACITY_LIMIT_CHECK.to_string(),
            Some(format!("{}", self.is_capacity_limit_check())),
        );
        state.insert(
            DEFAULT_CLUSTER_QUOTA.to_string(),
            Some(format!("{}", self.default_cluster_quota())),
        );
        state.insert(
            DEFAULT_GROUP_QUOTA.to_string(),
            Some(format!("{}", self.default_group_quota())),
        );
        state.insert(
            DEFAULT_MAX_SIZE.to_string(),
            Some(format!("{}", self.default_max_size())),
        );
        state.insert(
            DEFAULT_MAX_AGGR_COUNT.to_string(),
            Some(format!("{}", self.default_max_aggr_count())),
        );
        state.insert(
            DEFAULT_MAX_AGGR_SIZE.to_string(),
            Some(format!("{}", self.default_max_aggr_size())),
        );
        state.insert(
            CONFIG_RENTENTION_DAYS_PROPERTY_STATE.to_string(),
            Some(format!("{}", self.config_rentention_days())),
        );

        state
    }

    pub fn auth_state(&self, is_admin_request: bool) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(3);

        state.insert(
            AUTH_ENABLED.to_string(),
            Some(format!("{}", self.auth_enabled())),
        );
        state.insert(AUTH_SYSTEM_TYPE.to_string(), Some(self.auth_system_type()));
        state.insert(
            AUTH_ADMIN_REQUEST.to_string(),
            Some(format!("{}", is_admin_request)),
        );

        state
    }

    pub fn env_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(4);

        state.insert(STARTUP_MODE_STATE.to_string(), Some(self.startup_mode()));
        state.insert(FUNCTION_MODE_STATE.to_string(), self.function_mode());
        state.insert(NACOS_VERSION.to_string(), Some(self.version()));
        state.insert(
            SERVER_PORT_STATE.to_string(),
            Some(format!("{}", self.server_port())),
        );

        state
    }

    pub fn console_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(2);

        state.insert(
            "console_ui_enabled".to_string(),
            Some(format!("{}", self.console_ui_enabled())),
        );
        state.insert(
            "login_page_enabled".to_string(),
            Some(format!("{}", self.auth_console_enabled())),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpRequestExtensions {
    pub namespace_id: String,
    pub group: String,
    pub resource_name: String,
    pub username: String,
    pub roles: Vec<String>,
    pub permissions: Vec<super::auth::PermissionInfo>,
}
