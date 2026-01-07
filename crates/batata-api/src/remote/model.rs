//! Remote API models for Nacos protocol communication
//!
//! This module defines request/response models used in Nacos remote communication.

use std::collections::HashMap;

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    grpc::{Metadata, Payload},
    model::INTERNAL_MODULE,
};

// Constants for connection labels
pub const LABEL_SOURCE: &str = "source";
pub const LABEL_SOURCE_SDK: &str = "sdk";
pub const LABEL_SOURCE_CLUSTER: &str = "cluster";
pub const LABEL_MODULE: &str = "module";
pub const LABEL_MODULE_CONFIG: &str = "config";
pub const LABEL_MODULE_NAMING: &str = "naming";
pub const MONITOR_LABEL_NONE: &str = "none";
pub const LABEL_MODULE_LOCK: &str = "lock";
pub const LABEL_MODULE_AI: &str = "ai";

fn serialize_internal_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(INTERNAL_MODULE)
}

fn deserialize_internal_module<'de, D>(_: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(INTERNAL_MODULE.to_string())
}

/// Base trait for all request models
pub trait RequestTrait {
    fn headers(&self) -> HashMap<String, String>;

    fn request_type(&self) -> &'static str {
        ""
    }

    fn body(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        serde_json::to_vec(self).unwrap_or_default()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>);

    fn request_id(&self) -> String {
        String::default()
    }

    fn string_to_sign(&self) -> String {
        String::default()
    }

    fn from_payload<T>(value: &Payload) -> T
    where
        T: for<'a> Deserialize<'a> + Default,
    {
        serde_json::from_slice::<T>(&value.body.clone().unwrap_or_default().value.to_vec())
            .unwrap_or_default()
    }
}

/// Base request structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(skip)]
    pub headers: HashMap<String, String>,
    pub request_id: String,
}

impl Request {
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for Request {
    fn headers(&self) -> HashMap<String, String> {
        self.headers.clone()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        if self.headers.is_empty() {
            self.headers = HashMap::with_capacity(headers.len());
        }
        for (k, v) in headers {
            self.headers.insert(k, v);
        }
    }

    fn request_id(&self) -> String {
        self.request_id.clone()
    }
}

/// Internal request with module information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalRequest {
    #[serde(flatten)]
    pub request: Request,
    #[serde(
        serialize_with = "serialize_internal_module",
        deserialize_with = "deserialize_internal_module"
    )]
    module: String,
}

impl InternalRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for InternalRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

/// Health check request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
}

impl HealthCheckRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl RequestTrait for HealthCheckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "HealthCheckRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for HealthCheckRequest {
    fn from(value: &Payload) -> Self {
        HealthCheckRequest::from_payload(value)
    }
}

/// Response status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseCode {
    Success = 200,
    Fail = 500,
}

impl ResponseCode {
    pub fn code(&self) -> i32 {
        *self as i32
    }

    pub fn desc(&self) -> &'static str {
        match self {
            ResponseCode::Success => "Response ok",
            ResponseCode::Fail => "Response fail",
        }
    }
}

/// Base trait for all response models
pub trait ResponseTrait {
    fn response_type(&self) -> &'static str {
        ""
    }

    fn request_id(&mut self, request_id: String);

    fn body(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        serde_json::to_vec(self).unwrap_or_default()
    }

    fn error_code(&self) -> i32 {
        ResponseCode::Success.code()
    }

    fn result_code(&self) -> i32;

    fn message(&self) -> String {
        String::default()
    }

    fn to_any(&self) -> Any
    where
        Self: Serialize,
    {
        Any {
            type_url: String::default(),
            value: self.body(),
        }
    }

    fn to_payload(&self, metadata: Option<Metadata>) -> Payload
    where
        Self: Serialize,
    {
        Payload {
            metadata,
            body: Some(self.to_any()),
        }
    }
}

/// Base response structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub result_code: i32,
    pub error_code: i32,
    pub success: bool,
    pub message: String,
    pub request_id: String,
}

impl Response {
    pub fn new() -> Self {
        Self {
            result_code: ResponseCode::Success.code(),
            success: true,
            ..Default::default()
        }
    }
}

impl ResponseTrait for Response {
    fn request_id(&mut self, request_id: String) {
        self.request_id = request_id
    }

    fn error_code(&self) -> i32 {
        self.error_code
    }

    fn result_code(&self) -> i32 {
        self.result_code
    }

    fn message(&self) -> String {
        self.message.clone()
    }
}

/// Health check response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl HealthCheckResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for HealthCheckResponse {
    fn response_type(&self) -> &'static str {
        "HealthCheckResponse"
    }

    fn request_id(&mut self, request_id: String) {
        self.response.request_id = request_id;
    }

    fn error_code(&self) -> i32 {
        self.response.error_code
    }

    fn result_code(&self) -> i32 {
        self.response.result_code
    }
}

impl From<HealthCheckResponse> for Any {
    fn from(val: HealthCheckResponse) -> Self {
        val.to_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_request() {
        let req = HealthCheckRequest::new();
        assert_eq!(req.request_type(), "HealthCheckRequest");
    }

    #[test]
    fn test_response_code() {
        assert_eq!(ResponseCode::Success.code(), 200);
        assert_eq!(ResponseCode::Fail.code(), 500);
    }
}
