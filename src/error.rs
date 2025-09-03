use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum BatataError {
    #[error("caused: {0}")]
    IllegalArgument(String),
    #[error("user '{0}' not exist!")]
    UserNotExist(String),
    #[error("{2}")]
    ApiError(i32, i32, String, String),
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

pub const CONFIG_GRAY_OVER_MAX_VERSION_COUNT: ErrorCode<'static> = ErrorCode {
    code: 20010,
    message: "config gray version version over max count",
};

pub const CONFIG_GRAY_RULE_FORMAT_INVALID: ErrorCode<'static> = ErrorCode {
    code: 20011,
    message: "config gray rule format invalid",
};

pub const CONFIG_GRAY_VERSION_INVALID: ErrorCode<'static> = ErrorCode {
    code: 20012,
    message: "config gray rule version invalid",
};

pub const CONFIG_GRAY_NAME_UNRECOGNIZED_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20013,
    message: "config gray name not recognized",
};

pub const OVER_CLUSTER_QUOTA: ErrorCode<'static> = ErrorCode {
    code: 5031,
    message: "cluster capacity reach quota",
};

pub const OVER_GROUP_QUOTA: ErrorCode<'static> = ErrorCode {
    code: 5032,
    message: "group capacity reach quota",
};

pub const OVER_TENANT_QUOTA: ErrorCode<'static> = ErrorCode {
    code: 5033,
    message: "tenant capacity reach quota",
};

pub const OVER_MAX_SIZE: ErrorCode<'static> = ErrorCode {
    code: 5034,
    message: "config content size is over limit",
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
    message: "health check still running",
};

pub const ILLEGAL_NAMESPACE: ErrorCode<'static> = ErrorCode {
    code: 22000,
    message: "illegal namespace",
};

pub const NAMESPACE_NOT_EXIST: ErrorCode<'static> = ErrorCode {
    code: 22001,
    message: "namespace not exist",
};

pub const NAMESPACE_ALREADY_EXIST: ErrorCode<'static> = ErrorCode {
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

pub const API_DEPRECATED: ErrorCode<'static> = ErrorCode {
    code: 40000,
    message: "API deprecated.",
};

pub const API_FUNCTION_DISABLED: ErrorCode<'static> = ErrorCode {
    code: 40001,
    message: "API function disabled.",
};

pub const MCP_SERVER_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50000,
    message: "MCP server not found",
};

pub const MCP_SERVER_MCP_SEVER_VERSION_NOT_FOUNDNOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50001,
    message: "MCP server version not found",
};

pub const MCP_SERVER_VERSION_EXIST: ErrorCode<'static> = ErrorCode {
    code: 50002,
    message: "MCP server version has existed",
};

pub const MCP_SERVER_REF_ENDPOINT_SERVICE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50003,
    message: "MCP server ref endpoint service not found",
};

pub const METADATA_ILLEGAL: ErrorCode<'static> = ErrorCode {
    code: 100002,
    message: "Imported metadata is invalid",
};

pub const DATA_VALIDATION_FAILED: ErrorCode<'static> = ErrorCode {
    code: 100003,
    message: "No valid data was read",
};

pub const PARSING_DATA_FAILED: ErrorCode<'static> = ErrorCode {
    code: 100004,
    message: "Failed to parse data",
};

pub const DATA_EMPTY: ErrorCode<'static> = ErrorCode {
    code: 100005,
    message: "Imported file data is empty",
};

pub const NO_SELECTED_CONFIG: ErrorCode<'static> = ErrorCode {
    code: 100006,
    message: "No configuration selected",
};

pub const FUZZY_WATCH_PATTERN_OVER_LIMIT: ErrorCode<'static> = ErrorCode {
    code: 50310,
    message: "fuzzy watch pattern over limit",
};

pub const FUZZY_WATCH_PATTERN_MATCH_COUNT_OVER_LIMIT: ErrorCode<'static> = ErrorCode {
    code: 50311,
    message: "fuzzy watch pattern matched count over limit",
};
