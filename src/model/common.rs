use config::Config;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RestResult<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> RestResult<T> {
    pub fn success(data: T) -> RestResult<T> {
        RestResult::<T> {
            code: 200,
            message: "".to_string(),
            data,
        }
    }
}

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
