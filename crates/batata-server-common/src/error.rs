// Error handling and response types for Batata application
// Re-exports from batata_common with actix-web specific implementations

use std::fmt::{Display, Formatter};

use actix_web::HttpResponse;

// Re-export error types and codes from batata_common
pub use batata_common::error::{
    ACCESS_DENIED, API_DEPRECATED, API_FUNCTION_DISABLED, CONFIG_GRAY_NAME_UNRECOGNIZED_ERROR,
    CONFIG_GRAY_OVER_MAX_VERSION_COUNT, CONFIG_GRAY_RULE_FORMAT_INVALID,
    CONFIG_GRAY_VERSION_INVALID, CONFIG_LISTENER_ERROR, CONFIG_LISTENER_IS_NULL, DATA_ACCESS_ERROR,
    DATA_EMPTY, DATA_VALIDATION_FAILED, EXPORT_NO_DATA, FUZZY_WATCH_PATTERN_MATCH_COUNT_OVER_LIMIT,
    FUZZY_WATCH_PATTERN_OVER_LIMIT, HEALTH_CHECK_STILL_RUNNING, HEALTHY_PARAM_MISS,
    ILLEGAL_NAMESPACE, ILLEGAL_STATE, IMPORT_CONFLICT_ABORT, IMPORT_FILE_EMPTY,
    IMPORT_FILE_INVALID, INSTANCE_ERROR, INSTANCE_METADATA_ERROR, INSTANCE_NOT_FOUND,
    INVALID_DATA_ID, MCP_SERVER_NOT_FOUND, MCP_SERVER_REF_ENDPOINT_SERVICE_NOT_FOUND,
    MCP_SERVER_VERSION_EXIST, MCP_SERVER_VERSION_NOT_FOUND, MEDIA_TYPE_ERROR, METADATA_ILLEGAL,
    NAMESPACE_ALREADY_EXIST, NAMESPACE_NOT_EXIST, NO_SELECTED_CONFIG, NODE_DOWN_FAILURE,
    NODE_INFO_ERROR, OVER_CLUSTER_QUOTA, OVER_GROUP_QUOTA, OVER_MAX_SIZE, OVER_TENANT_QUOTA,
    PARAMETER_MISMATCH, PARAMETER_MISSING, PARAMETER_VALIDATE_ERROR, PARSING_DATA_FAILED,
    RESOURCE_CONFLICT, RESOURCE_NOT_FOUND, SELECTOR_ERROR, SERVER_ERROR, SERVICE_ALREADY_EXIST,
    SERVICE_DELETE_FAILURE, SERVICE_METADATA_ERROR, SERVICE_NAME_ERROR, SERVICE_NOT_EXIST, SUCCESS,
    TENANT_PARAM_ERROR, WEIGHT_ERROR,
};
pub use batata_common::error::{BatataError, ErrorCode};

use crate::model::response as common;

// Local wrapper for application errors to implement actix-web error handling
// (Cannot impl foreign trait for foreign type due to orphan rules)
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
    pub fn inner(&self) -> &anyhow::Error {
        &self.inner
    }

    pub fn downcast_ref<E: std::error::Error + Send + Sync + 'static>(&self) -> Option<&E> {
        self.inner.downcast_ref::<E>()
    }
}

impl actix_web::error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        if let Some(e) = self.downcast_ref::<BatataError>() {
            match e {
                BatataError::IllegalArgument(message) => common::Result::<String>::http_response(
                    400,
                    PARAMETER_VALIDATE_ERROR.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::UserNotExist(message) => common::Result::<String>::http_response(
                    400,
                    RESOURCE_NOT_FOUND.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::ApiError(status, code, message, data) => {
                    common::Result::<String>::http_response(
                        *status as u16,
                        *code,
                        message.to_string(),
                        data.to_string(),
                    )
                }
                BatataError::NetworkError(message) => common::Result::<String>::http_response(
                    503,
                    SERVER_ERROR.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::DatabaseError(message) => common::Result::<String>::http_response(
                    500,
                    DATA_ACCESS_ERROR.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::AuthError(message) => common::Result::<String>::http_response(
                    401,
                    ACCESS_DENIED.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::ConfigError(message) => common::Result::<String>::http_response(
                    400,
                    PARAMETER_VALIDATE_ERROR.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::InternalError(message) => common::Result::<String>::http_response(
                    500,
                    SERVER_ERROR.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::NamespaceNotExist(message) => common::Result::<String>::http_response(
                    404,
                    NAMESPACE_NOT_EXIST.code,
                    message.to_string(),
                    String::new(),
                ),
                BatataError::NamespaceAlreadyExist(message) => {
                    common::Result::<String>::http_response(
                        409,
                        NAMESPACE_ALREADY_EXIST.code,
                        message.to_string(),
                        String::new(),
                    )
                }
            }
        } else {
            common::Result::<String>::http_response(
                500,
                SERVER_ERROR.code,
                self.inner.to_string(),
                String::new(),
            )
        }
    }
}

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
