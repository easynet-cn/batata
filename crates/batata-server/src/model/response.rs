//! HTTP response types for Batata server
//!
//! This module provides common response structures for API responses.

use actix_web::{http::StatusCode, HttpResponse, HttpResponseBuilder};
use serde::{Deserialize, Serialize};

use super::constants::NACOS_SERVER_VERSION_V2;

/// Generic result wrapper for API responses
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

/// Error result for API error responses
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

/// Console exception handling utilities
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
