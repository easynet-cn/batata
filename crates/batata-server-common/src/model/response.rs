//! HTTP response types for Batata server
//!
//! This module provides common response structures for API responses.

use actix_web::{HttpResponse, HttpResponseBuilder, http::StatusCode};
use serde::{Deserialize, Serialize};

/// Generic result wrapper for API responses
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Result<T> {
    /// The result code; 0 indicates success.
    pub code: i32,
    /// A human-readable message describing the result.
    pub message: String,
    /// The response payload.
    pub data: T,
}

impl<T> Result<T> {
/// Performs the `new` operation.
    pub fn new(code: i32, message: String, data: T) -> Self {
        Result::<T> {
            code,
            message,
            data,
        }
    }

/// Performs the `success` operation.
    pub fn success(data: T) -> Result<T> {
        Result::<T> {
            code: 0,
            message: "success".to_string(),
            data,
        }
    }

/// Performs the `fail` operation.
    pub fn fail(message: String) -> Result<()> {
        Result::<()> {
            code: 500,
            message,
            data: (),
        }
    }

/// Performs the `http_success` operation.
    pub fn http_success(data: impl Serialize) -> HttpResponse {
        HttpResponse::Ok().json(Result::success(data))
    }

/// Performs the `http_response` operation.
    pub fn http_response(
        status: u16,
        code: i32,
        message: String,
        data: impl Serialize,
    ) -> HttpResponse {
        HttpResponseBuilder::new(StatusCode::from_u16(status).unwrap_or_default())
            .json(Result::new(code, message, data))
    }

    /// Build a NOT_FOUND error response from an ErrorCode and detail message
    pub fn http_not_found(
        error_code: &batata_common::error::ErrorCode,
        detail: impl Into<String>,
    ) -> HttpResponse {
        Result::<String>::http_response(
            StatusCode::NOT_FOUND.as_u16(),
            error_code.code,
            error_code.message.to_string(),
            detail.into(),
        )
    }

    /// Build a BAD_REQUEST error response from an ErrorCode and detail message
    pub fn http_bad_request(
        error_code: &batata_common::error::ErrorCode,
        detail: impl Into<String>,
    ) -> HttpResponse {
        Result::<String>::http_response(
            StatusCode::BAD_REQUEST.as_u16(),
            error_code.code,
            error_code.message.to_string(),
            detail.into(),
        )
    }

    /// Build a FORBIDDEN error response from an ErrorCode and detail message
    pub fn http_forbidden(
        error_code: &batata_common::error::ErrorCode,
        detail: impl Serialize,
    ) -> HttpResponse {
        Result::<String>::http_response(
            StatusCode::FORBIDDEN.as_u16(),
            error_code.code,
            error_code.message.to_string(),
            detail,
        )
    }

    /// Build an INTERNAL_SERVER_ERROR response from an error
    pub fn http_internal_error(detail: impl std::fmt::Display) -> HttpResponse {
        Result::<String>::http_response(
            StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            crate::error::SERVER_ERROR.code,
            format!("Internal error: {}", detail),
            String::new(),
        )
    }
}

/// Error result for API error responses
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorResult {
    /// The time the error occurred.
    pub timestamp: String,
    /// The HTTP status code.
    pub status: i32,
    /// The error reason phrase.
    pub error: String,
    /// A human-readable error message.
    pub message: String,
    /// The request path that produced the error.
    pub path: String,
}

impl ErrorResult {
/// Performs the `new` operation.
    pub fn new(status: i32, error: String, message: String, path: String) -> Self {
        ErrorResult {
            timestamp: chrono::Utc::now().to_rfc3339(),
            status,
            error,
            message,
            path,
        }
    }

/// Performs the `forbidden` operation.
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

/// Performs the `http_response_forbidden` operation.
    pub fn http_response_forbidden(code: i32, message: &str, path: &str) -> HttpResponse {
        HttpResponse::Forbidden().json(ErrorResult::forbidden(
            format!("Code: {}, Message: {}", code, message).as_str(),
            path,
        ))
    }
}

/// Console exception handling utilities
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsoleException {}

impl ConsoleException {
/// Performs the `handle_access_exception` operation.
    pub fn handle_access_exception(message: String) -> HttpResponse {
        Result::<String>::http_response(
            403,
            crate::error::ACCESS_DENIED.code,
            message,
            String::new(),
        )
    }

/// Performs the `handle_illegal_argument_exception` operation.
    pub fn handle_illegal_argument_exception(message: String) -> HttpResponse {
        Result::<String>::http_response(
            400,
            crate::error::PARAMETER_VALIDATE_ERROR.code,
            format!("caused: {}", message),
            String::new(),
        )
    }

/// Performs the `handle_runtime_exception` operation.
    pub fn handle_runtime_exception(code: u16, message: String) -> HttpResponse {
        Result::<String>::http_response(
            code,
            crate::error::SERVER_ERROR.code,
            format!("caused: {}", message),
            String::new(),
        )
    }

/// Performs the `handle_exception` operation.
    pub fn handle_exception(_uri: String, message: String) -> HttpResponse {
        Result::<String>::http_response(
            500,
            crate::error::SERVER_ERROR.code,
            html_escape::encode_text(&format!("caused: {}", message)).to_string(),
            String::new(),
        )
    }
}
