//! Error types and error codes for Batata
//!
//! This module defines:
//! - `BatataError`: Application-specific error enum
//! - `AppError`: Wrapper for integration with web frameworks
//! - `ErrorCode`: Structured error codes for API responses

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

/// Application-specific error types
#[derive(thiserror::Error, Debug)]
pub enum BatataError {
    #[error("caused: {0}")]
    /// The `IllegalArgument` variant.
    IllegalArgument(String),

    #[error("user '{0}' not exist!")]
    /// The `UserNotExist` variant.
    UserNotExist(String),

    #[error("{2}")]
    /// The `ApiError` variant.
    ApiError(i32, i32, String, String),

    #[error("network error: {0}")]
    /// The `NetworkError` variant.
    NetworkError(String),

    #[error("database error: {0}")]
    /// The `DatabaseError` variant.
    DatabaseError(String),

    #[error("authentication error: {0}")]
    /// The `AuthError` variant.
    AuthError(String),

    #[error("configuration error: {0}")]
    /// The `ConfigError` variant.
    ConfigError(String),

    #[error("internal error: {0}")]
    /// The `InternalError` variant.
    InternalError(String),

    #[error("namespace '{0}' not exist")]
    /// The `NamespaceNotExist` variant.
    NamespaceNotExist(String),

    #[error("namespace '{0}' already exist")]
    /// The `NamespaceAlreadyExist` variant.
    NamespaceAlreadyExist(String),
}

/// Wrapper for application errors
#[derive(Debug)]
pub struct AppError {
    inner: anyhow::Error,
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

impl AppError {
    /// The `inner` method.
    pub fn inner(&self) -> &anyhow::Error {
        &self.inner
    }

    /// The `downcast_ref` method.
    pub fn downcast_ref<E: std::error::Error + Send + Sync + 'static>(&self) -> Option<&E> {
        self.inner.downcast_ref::<E>()
    }
}

/// Error code structure for API responses
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ErrorCode<'a> {
    /// The `code` field.
    pub code: i32,
    /// The `message` field.
    pub message: &'a str,
}

// General success and error codes
/// Success.
pub const SUCCESS: ErrorCode<'static> = ErrorCode {
    code: 0,
    message: "success",
};

/// A required parameter is missing.
pub const PARAMETER_MISSING: ErrorCode<'static> = ErrorCode {
    code: 10000,
    message: "parameter missing",
};

/// Access to the requested resource is denied.
pub const ACCESS_DENIED: ErrorCode<'static> = ErrorCode {
    code: 10001,
    message: "access denied",
};

// Nacos-compatible auth error codes
/// Login failed.
pub const LOGIN_FAILED: ErrorCode<'static> = ErrorCode {
    code: 20020,
    message: "login failed",
};

/// The provided username or password is incorrect.
pub const USERNAME_OR_PASSWORD_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20021,
    message: "username or password error",
};

// Additional error codes for unified error handling
/// Login attempts are rate limited.
pub const LOGIN_RATE_LIMITED: ErrorCode<'static> = ErrorCode {
    code: 429,
    message: "login rate limited",
};

/// No authentication plugin is configured.
pub const AUTH_PLUGIN_NOT_CONFIGURED: ErrorCode<'static> = ErrorCode {
    code: 500,
    message: "auth plugin not configured",
};

/// A data access error occurred.
pub const DATA_ACCESS_ERROR: ErrorCode<'static> = ErrorCode {
    code: 10002,
    message: "data access error",
};

// Tenant and parameter validation errors
/// The `tenant` parameter is invalid.
pub const TENANT_PARAM_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20001,
    message: "'tenant' parameter error",
};

/// Parameter validation failed.
pub const PARAMETER_VALIDATE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20002,
    message: "parameter validate error",
};

/// The media type is unsupported.
pub const MEDIA_TYPE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20003,
    message: "MediaType Error",
};

/// The requested resource was not found.
pub const RESOURCE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 20004,
    message: "resource not found",
};

/// The resource conflicts with an existing one.
pub const RESOURCE_CONFLICT: ErrorCode<'static> = ErrorCode {
    code: 20005,
    message: "resource conflict",
};

/// A config listener is null.
pub const CONFIG_LISTENER_IS_NULL: ErrorCode<'static> = ErrorCode {
    code: 20006,
    message: "config listener is null",
};

/// A config listener error occurred.
pub const CONFIG_LISTENER_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20007,
    message: "config listener error",
};

/// The dataId is invalid.
pub const INVALID_DATA_ID: ErrorCode<'static> = ErrorCode {
    code: 20008,
    message: "invalid dataId",
};

/// Parameters do not match.
pub const PARAMETER_MISMATCH: ErrorCode<'static> = ErrorCode {
    code: 20009,
    message: "parameter mismatch",
};

/// The config gray version count exceeds the limit.
pub const CONFIG_GRAY_OVER_MAX_VERSION_COUNT: ErrorCode<'static> = ErrorCode {
    code: 20010,
    message: "config gray version version over max count",
};

/// The config gray rule format is invalid.
pub const CONFIG_GRAY_RULE_FORMAT_INVALID: ErrorCode<'static> = ErrorCode {
    code: 20011,
    message: "config gray rule format invalid",
};

/// The config gray rule version is invalid.
pub const CONFIG_GRAY_VERSION_INVALID: ErrorCode<'static> = ErrorCode {
    code: 20012,
    message: "config gray rule version invalid",
};

/// The config gray name is not recognized.
pub const CONFIG_GRAY_NAME_UNRECOGNIZED_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20013,
    message: "config gray name not recognized",
};

/// The cluster capacity has reached its quota.
pub const OVER_CLUSTER_QUOTA: ErrorCode<'static> = ErrorCode {
    code: 5031,
    message: "cluster capacity reach quota",
};

/// The group capacity has reached its quota.
pub const OVER_GROUP_QUOTA: ErrorCode<'static> = ErrorCode {
    code: 5032,
    message: "group capacity reach quota",
};

/// The tenant capacity has reached its quota.
pub const OVER_TENANT_QUOTA: ErrorCode<'static> = ErrorCode {
    code: 5033,
    message: "tenant capacity reach quota",
};

/// The config content size exceeds the limit.
pub const OVER_MAX_SIZE: ErrorCode<'static> = ErrorCode {
    code: 5034,
    message: "config content size is over limit",
};

/// The service name is invalid.
pub const SERVICE_NAME_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21000,
    message: "service name error",
};

/// The instance weight is invalid.
pub const WEIGHT_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21001,
    message: "weight error",
};

/// The instance metadata is invalid.
pub const INSTANCE_METADATA_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21002,
    message: "instance metadata error",
};

/// The instance was not found.
pub const INSTANCE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 21003,
    message: "instance not found",
};

/// An instance error occurred.
pub const INSTANCE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21004,
    message: "instance error",
};

/// The service metadata is invalid.
pub const SERVICE_METADATA_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21005,
    message: "service metadata error",
};

/// The service selector is invalid.
pub const SELECTOR_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21006,
    message: "selector error",
};

/// The service already exists.
pub const SERVICE_ALREADY_EXIST: ErrorCode<'static> = ErrorCode {
    code: 21007,
    message: "service already exist",
};

/// The service does not exist.
pub const SERVICE_NOT_EXIST: ErrorCode<'static> = ErrorCode {
    code: 21008,
    message: "service not exist",
};

/// Failed to delete the service.
pub const SERVICE_DELETE_FAILURE: ErrorCode<'static> = ErrorCode {
    code: 21009,
    message: "service delete failure",
};

/// The healthy parameter is missing.
pub const HEALTHY_PARAM_MISS: ErrorCode<'static> = ErrorCode {
    code: 21010,
    message: "healthy param miss",
};

/// A health check is still running.
pub const HEALTH_CHECK_STILL_RUNNING: ErrorCode<'static> = ErrorCode {
    code: 21011,
    message: "health check still running",
};

/// The namespace is illegal.
pub const ILLEGAL_NAMESPACE: ErrorCode<'static> = ErrorCode {
    code: 22000,
    message: "illegal namespace",
};

/// The namespace does not exist.
pub const NAMESPACE_NOT_EXIST: ErrorCode<'static> = ErrorCode {
    code: 22001,
    message: "namespace not exist",
};

/// The namespace already exists.
pub const NAMESPACE_ALREADY_EXIST: ErrorCode<'static> = ErrorCode {
    code: 22002,
    message: "namespace already exist",
};

/// The current state is illegal for this operation.
pub const ILLEGAL_STATE: ErrorCode<'static> = ErrorCode {
    code: 23000,
    message: "illegal state",
};

/// The node info is invalid.
pub const NODE_INFO_ERROR: ErrorCode<'static> = ErrorCode {
    code: 23001,
    message: "node info error",
};

/// Failed to bring the node down.
pub const NODE_DOWN_FAILURE: ErrorCode<'static> = ErrorCode {
    code: 23002,
    message: "node down failure",
};

/// A server error occurred.
pub const SERVER_ERROR: ErrorCode<'static> = ErrorCode {
    code: 30000,
    message: "server error",
};

/// The API is deprecated.
pub const API_DEPRECATED: ErrorCode<'static> = ErrorCode {
    code: 40000,
    message: "API deprecated.",
};

/// The API function is disabled.
pub const API_FUNCTION_DISABLED: ErrorCode<'static> = ErrorCode {
    code: 40001,
    message: "API function disabled.",
};

/// The MCP server was not found.
pub const MCP_SERVER_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50000,
    message: "MCP server not found",
};

/// The MCP server version was not found.
pub const MCP_SERVER_VERSION_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50001,
    message: "MCP server version not found",
};

/// The MCP server version already exists.
pub const MCP_SERVER_VERSION_EXIST: ErrorCode<'static> = ErrorCode {
    code: 50002,
    message: "MCP server version has existed",
};

/// The MCP server referenced endpoint service was not found.
pub const MCP_SERVER_REF_ENDPOINT_SERVICE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50003,
    message: "MCP server ref endpoint service not found",
};

/// The agent was not found.
pub const AGENT_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50100,
    message: "agent not found",
};

/// The agent version was not found.
pub const AGENT_VERSION_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50101,
    message: "agent version not found",
};

/// The agent version already exists.
pub const AGENT_VERSION_EXIST: ErrorCode<'static> = ErrorCode {
    code: 50102,
    message: "agent version has existed",
};

/// The skill was not found.
pub const SKILL_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50200,
    message: "skill not found",
};

/// The skill version was not found.
pub const SKILL_VERSION_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50201,
    message: "skill version not found",
};

/// The agent spec was not found.
pub const AGENTSPEC_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 50300,
    message: "agentspec not found",
};

/// The imported metadata is invalid.
pub const METADATA_ILLEGAL: ErrorCode<'static> = ErrorCode {
    code: 100002,
    message: "Imported metadata is invalid",
};

/// No valid data was read.
pub const DATA_VALIDATION_FAILED: ErrorCode<'static> = ErrorCode {
    code: 100003,
    message: "No valid data was read",
};

/// Failed to parse the data.
pub const PARSING_DATA_FAILED: ErrorCode<'static> = ErrorCode {
    code: 100004,
    message: "Failed to parse data",
};

/// The imported file data is empty.
pub const DATA_EMPTY: ErrorCode<'static> = ErrorCode {
    code: 100005,
    message: "Imported file data is empty",
};

/// No configuration was selected.
pub const NO_SELECTED_CONFIG: ErrorCode<'static> = ErrorCode {
    code: 100006,
    message: "No configuration selected",
};

/// The fuzzy watch pattern exceeds the limit.
pub const FUZZY_WATCH_PATTERN_OVER_LIMIT: ErrorCode<'static> = ErrorCode {
    code: 50310,
    message: "fuzzy watch pattern over limit",
};

/// The fuzzy watch pattern matched count exceeds the limit.
pub const FUZZY_WATCH_PATTERN_MATCH_COUNT_OVER_LIMIT: ErrorCode<'static> = ErrorCode {
    code: 50311,
    message: "fuzzy watch pattern matched count over limit",
};

// Import/Export error codes
/// The import file is empty.
pub const IMPORT_FILE_EMPTY: ErrorCode<'static> = ErrorCode {
    code: 100010,
    message: "Import file is empty",
};

/// The import file format is invalid.
pub const IMPORT_FILE_INVALID: ErrorCode<'static> = ErrorCode {
    code: 100011,
    message: "Import file format is invalid",
};

/// The import was aborted due to a conflict.
pub const IMPORT_CONFLICT_ABORT: ErrorCode<'static> = ErrorCode {
    code: 100012,
    message: "Import aborted due to conflict",
};

/// No configurations were found to export.
pub const EXPORT_NO_DATA: ErrorCode<'static> = ErrorCode {
    code: 100013,
    message: "No configurations found to export",
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
    }

    #[test]
    fn test_error_code_constants() {
        assert_eq!(SUCCESS.code, 0);
        assert_eq!(SUCCESS.message, "success");
        assert_eq!(PARAMETER_MISSING.code, 10000);
        assert_eq!(ACCESS_DENIED.code, 10001);
    }

    #[test]
    fn test_app_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("test error");
        let app_err = AppError::from(anyhow_err);
        assert_eq!(format!("{}", app_err), "test error");
    }

    #[test]
    fn test_batata_error_all_variants() {
        let errors = vec![
            (
                BatataError::IllegalArgument("bad arg".into()),
                "caused: bad arg",
            ),
            (
                BatataError::UserNotExist("alice".into()),
                "user 'alice' not exist!",
            ),
            (
                BatataError::NetworkError("timeout".into()),
                "network error: timeout",
            ),
            (
                BatataError::DatabaseError("connection lost".into()),
                "database error: connection lost",
            ),
            (
                BatataError::AuthError("invalid token".into()),
                "authentication error: invalid token",
            ),
            (
                BatataError::ConfigError("missing key".into()),
                "configuration error: missing key",
            ),
            (
                BatataError::InternalError("panic".into()),
                "internal error: panic",
            ),
            (
                BatataError::NamespaceNotExist("ns1".into()),
                "namespace 'ns1' not exist",
            ),
            (
                BatataError::NamespaceAlreadyExist("ns1".into()),
                "namespace 'ns1' already exist",
            ),
        ];

        for (err, expected) in errors {
            assert_eq!(format!("{}", err), expected);
        }
    }

    #[test]
    fn test_batata_error_api_error() {
        let err = BatataError::ApiError(400, 20002, "validation failed".into(), "detail".into());

        assert_eq!(format!("{}", err), "validation failed");
    }

    #[test]
    fn test_app_error_downcast() {
        let batata_err = BatataError::AuthError("forbidden".to_string());
        let anyhow_err: anyhow::Error = batata_err.into();
        let app_err = AppError::from(anyhow_err);

        let downcast = app_err.downcast_ref::<BatataError>();

        assert!(downcast.is_some());

        match downcast.unwrap() {
            BatataError::AuthError(msg) => assert_eq!(msg, "forbidden"),
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn test_app_error_display() {
        let err = AppError::from(anyhow::anyhow!("something went wrong"));

        assert_eq!(format!("{}", err), "something went wrong");
    }

    #[test]
    fn test_error_code_values() {
        assert_eq!(PARAMETER_VALIDATE_ERROR.code, 20002);
        assert_eq!(RESOURCE_NOT_FOUND.code, 20004);
        assert_eq!(RESOURCE_CONFLICT.code, 20005);
        assert_eq!(SERVICE_NAME_ERROR.code, 21000);
        assert_eq!(INSTANCE_NOT_FOUND.code, 21003);
        assert_eq!(NAMESPACE_NOT_EXIST.code, 22001);
        assert_eq!(NAMESPACE_ALREADY_EXIST.code, 22002);
        assert_eq!(SERVER_ERROR.code, 30000);
        assert_eq!(MCP_SERVER_NOT_FOUND.code, 50000);
        assert_eq!(AGENT_NOT_FOUND.code, 50100);
    }

    #[test]
    fn test_error_code_serialization() {
        let code = SUCCESS;
        let json = serde_json::to_string(&code).unwrap();

        assert!(json.contains("\"code\":0"));
        assert!(json.contains("\"message\":\"success\""));
    }

    #[test]
    fn test_error_code_default() {
        let code = ErrorCode::default();

        assert_eq!(code.code, 0);
        assert_eq!(code.message, "");
    }
}
