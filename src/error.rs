// Error handling and response types for Batata application
// This module defines error types, HTTP error responses, and error code constants

use std::fmt::{Display, Formatter};

use actix_web::HttpResponse;
use serde::{Deserialize, Serialize};

use crate::model::common;

// Application-specific error types
#[derive(thiserror::Error, Debug)]
pub enum BatataError {
    #[error("caused: {0}")]
    IllegalArgument(String), // Invalid input parameter
    #[error("user '{0}' not exist!")]
    UserNotExist(String), // User not found
    #[error("{2}")]
    ApiError(i32, i32, String, String), // API error with status, code, message, and data
    #[error("network error: {0}")]
    NetworkError(String), // Network-related errors
    #[error("database error: {0}")]
    DatabaseError(String), // Database operation errors
    #[error("authentication error: {0}")]
    AuthError(String), // Authentication failures
    #[error("configuration error: {0}")]
    ConfigError(String), // Configuration issues
    #[error("internal error: {0}")]
    InternalError(String), // Internal server errors
}

// Wrapper for application errors to implement actix-web error handling
#[derive(Debug)]
pub struct AppError {
    inner: anyhow::Error, // Wrapped anyhow error
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        AppError { inner: value }
    }
}

impl actix_web::error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        if let Some(e) = self.inner.downcast_ref::<BatataError>() {
            match e {
                BatataError::IllegalArgument(message) => {
                    HttpResponse::BadRequest().body(message.to_string())
                }
                BatataError::UserNotExist(message) => {
                    HttpResponse::BadRequest().body(message.to_string())
                }
                BatataError::ApiError(status, code, message, data) => {
                    common::Result::<String>::http_response(
                        *status as u16,
                        *code,
                        message.to_string(),
                        data.to_string(),
                    )
                }
                BatataError::NetworkError(message) => {
                    HttpResponse::ServiceUnavailable().body(message.to_string())
                }
                BatataError::DatabaseError(message) => {
                    HttpResponse::InternalServerError().body(message.to_string())
                }
                BatataError::AuthError(message) => {
                    HttpResponse::Unauthorized().body(message.to_string())
                }
                BatataError::ConfigError(message) => {
                    HttpResponse::BadRequest().body(message.to_string())
                }
                BatataError::InternalError(message) => {
                    HttpResponse::InternalServerError().body(message.to_string())
                }
            }
        } else {
            HttpResponse::InternalServerError().body(self.inner.to_string())
        }
    }
}

// Error code structure for API responses
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ErrorCode<'a> {
    pub code: i32,        // Numeric error code
    pub message: &'a str, // Human-readable error message
}

// General success and error codes
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

// Tenant and parameter validation errors
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batata_error_display() {
        let err = BatataError::IllegalArgument("invalid param".to_string());
        assert_eq!(format!("{}", err), "caused: invalid param");

        let err = BatataError::UserNotExist("testuser".to_string());
        assert_eq!(format!("{}", err), "user 'testuser' not exist!");

        let err = BatataError::NetworkError("connection timeout".to_string());
        assert_eq!(format!("{}", err), "network error: connection timeout");

        let err = BatataError::DatabaseError("query failed".to_string());
        assert_eq!(format!("{}", err), "database error: query failed");

        let err = BatataError::AuthError("invalid token".to_string());
        assert_eq!(format!("{}", err), "authentication error: invalid token");

        let err = BatataError::ConfigError("missing key".to_string());
        assert_eq!(format!("{}", err), "configuration error: missing key");

        let err = BatataError::InternalError("unexpected".to_string());
        assert_eq!(format!("{}", err), "internal error: unexpected");
    }

    #[test]
    fn test_batata_error_api_error() {
        let err = BatataError::ApiError(400, 10000, "bad request".to_string(), "{}".to_string());
        assert_eq!(format!("{}", err), "bad request");
    }

    #[test]
    fn test_app_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("test error");
        let app_err = AppError::from(anyhow_err);
        assert_eq!(format!("{}", app_err), "test error");
    }

    #[test]
    fn test_error_code_constants() {
        assert_eq!(SUCCESS.code, 0);
        assert_eq!(SUCCESS.message, "success");

        assert_eq!(PARAMETER_MISSING.code, 10000);
        assert_eq!(ACCESS_DENIED.code, 10001);
        assert_eq!(DATA_ACCESS_ERROR.code, 10002);

        assert_eq!(TENANT_PARAM_ERROR.code, 20001);
        assert_eq!(PARAMETER_VALIDATE_ERROR.code, 20002);
        assert_eq!(RESOURCE_NOT_FOUND.code, 20004);
        assert_eq!(RESOURCE_CONFLICT.code, 20005);
    }

    #[test]
    fn test_error_code_service_errors() {
        assert_eq!(SERVICE_NAME_ERROR.code, 21000);
        assert_eq!(WEIGHT_ERROR.code, 21001);
        assert_eq!(INSTANCE_NOT_FOUND.code, 21003);
        assert_eq!(SERVICE_ALREADY_EXIST.code, 21007);
        assert_eq!(SERVICE_NOT_EXIST.code, 21008);
    }

    #[test]
    fn test_error_code_namespace_errors() {
        assert_eq!(ILLEGAL_NAMESPACE.code, 22000);
        assert_eq!(NAMESPACE_NOT_EXIST.code, 22001);
        assert_eq!(NAMESPACE_ALREADY_EXIST.code, 22002);
    }

    #[test]
    fn test_error_code_server_errors() {
        assert_eq!(SERVER_ERROR.code, 30000);
        assert_eq!(API_DEPRECATED.code, 40000);
        assert_eq!(API_FUNCTION_DISABLED.code, 40001);
    }

    #[test]
    fn test_error_code_default() {
        let default_code = ErrorCode::default();
        assert_eq!(default_code.code, 0);
        assert_eq!(default_code.message, "");
    }

    #[test]
    fn test_error_code_clone() {
        let original = PARAMETER_MISSING;
        let cloned = original.clone();
        assert_eq!(original.code, cloned.code);
        assert_eq!(original.message, cloned.message);
    }
}
