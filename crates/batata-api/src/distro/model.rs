//! Distro protocol API models
//!
//! This module defines request/response models for Distro cluster synchronization.

use std::collections::HashMap;

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    grpc::Payload,
    model::NAMING_MODULE,
    remote::model::{InternalRequest, RequestTrait, Response, ResponseTrait},
};

fn serialize_naming_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(NAMING_MODULE)
}

fn deserialize_naming_module<'de, D>(_: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(NAMING_MODULE.to_string())
}

/// Distro data type for synchronization
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DistroDataType {
    /// Naming service instances (ephemeral)
    #[default]
    NamingInstance,
    /// Custom data type
    Custom,
}

impl std::fmt::Display for DistroDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistroDataType::NamingInstance => write!(f, "NAMING_INSTANCE"),
            DistroDataType::Custom => write!(f, "CUSTOM"),
        }
    }
}

/// Distro data item for synchronization
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroDataItem {
    /// Data type
    pub data_type: DistroDataType,
    /// Unique key for this data (e.g., service_key for naming instances)
    pub key: String,
    /// Serialized data content (JSON encoded)
    pub content: String,
    /// Data version/timestamp for conflict resolution
    pub version: i64,
    /// Source node address
    pub source: String,
}

impl DistroDataItem {
    pub fn new(data_type: DistroDataType, key: String, content: String, source: String) -> Self {
        Self {
            data_type,
            key,
            content,
            version: chrono::Utc::now().timestamp_millis(),
            source,
        }
    }
}

/// Request to sync distro data to other cluster nodes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroDataSyncRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
    /// The distro data to sync
    pub distro_data: DistroDataItem,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
    )]
    module: String,
}

impl DistroDataSyncRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            distro_data: DistroDataItem::default(),
            module: NAMING_MODULE.to_string(),
        }
    }

    pub fn with_data(data: DistroDataItem) -> Self {
        Self {
            internal_request: InternalRequest::new(),
            distro_data: data,
            module: NAMING_MODULE.to_string(),
        }
    }
}

impl RequestTrait for DistroDataSyncRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "DistroDataSyncRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for DistroDataSyncRequest {
    fn from(value: &Payload) -> Self {
        DistroDataSyncRequest::from_payload(value)
    }
}

/// Request to verify distro data with other cluster nodes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroDataVerifyRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
    /// Data type to verify
    pub data_type: DistroDataType,
    /// Keys and their versions to verify
    pub verify_data: HashMap<String, i64>,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
    )]
    module: String,
}

impl DistroDataVerifyRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            data_type: DistroDataType::NamingInstance,
            verify_data: HashMap::new(),
            module: NAMING_MODULE.to_string(),
        }
    }
}

impl RequestTrait for DistroDataVerifyRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "DistroDataVerifyRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for DistroDataVerifyRequest {
    fn from(value: &Payload) -> Self {
        DistroDataVerifyRequest::from_payload(value)
    }
}

/// Request to get snapshot of all distro data
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroDataSnapshotRequest {
    #[serde(flatten)]
    pub internal_request: InternalRequest,
    /// Data type to get snapshot for
    pub data_type: DistroDataType,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
    )]
    module: String,
}

impl DistroDataSnapshotRequest {
    pub fn new(data_type: DistroDataType) -> Self {
        Self {
            internal_request: InternalRequest::new(),
            data_type,
            module: NAMING_MODULE.to_string(),
        }
    }
}

impl RequestTrait for DistroDataSnapshotRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "DistroDataSnapshotRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for DistroDataSnapshotRequest {
    fn from(value: &Payload) -> Self {
        DistroDataSnapshotRequest::from_payload(value)
    }
}

/// Response for distro data sync
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroDataSyncResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl DistroDataSyncResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }

    pub fn success() -> Self {
        Self {
            response: Response::new(),
        }
    }

    pub fn fail(message: &str) -> Self {
        let mut response = Response::new();
        response.success = false;
        response.result_code = 500;
        response.message = message.to_string();
        Self { response }
    }
}

impl ResponseTrait for DistroDataSyncResponse {
    fn response_type(&self) -> &'static str {
        "DistroDataSyncResponse"
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

impl From<DistroDataSyncResponse> for Any {
    fn from(val: DistroDataSyncResponse) -> Self {
        val.to_any()
    }
}

/// Response for distro data verify
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroDataVerifyResponse {
    #[serde(flatten)]
    pub response: Response,
    /// Keys that need to be synced (missing or outdated)
    pub keys_need_sync: Vec<String>,
}

impl DistroDataVerifyResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            keys_need_sync: Vec::new(),
        }
    }
}

impl ResponseTrait for DistroDataVerifyResponse {
    fn response_type(&self) -> &'static str {
        "DistroDataVerifyResponse"
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

impl From<DistroDataVerifyResponse> for Any {
    fn from(val: DistroDataVerifyResponse) -> Self {
        val.to_any()
    }
}

/// Response for distro data snapshot
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroDataSnapshotResponse {
    #[serde(flatten)]
    pub response: Response,
    /// All distro data items
    pub snapshot: Vec<DistroDataItem>,
}

impl DistroDataSnapshotResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            snapshot: Vec::new(),
        }
    }
}

impl ResponseTrait for DistroDataSnapshotResponse {
    fn response_type(&self) -> &'static str {
        "DistroDataSnapshotResponse"
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

impl From<DistroDataSnapshotResponse> for Any {
    fn from(val: DistroDataSnapshotResponse) -> Self {
        val.to_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distro_data_type_display() {
        assert_eq!(DistroDataType::NamingInstance.to_string(), "NAMING_INSTANCE");
        assert_eq!(DistroDataType::Custom.to_string(), "CUSTOM");
    }

    #[test]
    fn test_distro_data_item_creation() {
        let item = DistroDataItem::new(
            DistroDataType::NamingInstance,
            "public@@DEFAULT_GROUP@@test-service".to_string(),
            r#"{"instances":[]}"#.to_string(),
            "192.168.1.1:8848".to_string(),
        );

        assert_eq!(item.data_type, DistroDataType::NamingInstance);
        assert_eq!(item.key, "public@@DEFAULT_GROUP@@test-service");
        assert!(item.version > 0);
    }

    #[test]
    fn test_distro_sync_request() {
        let req = DistroDataSyncRequest::new();
        assert_eq!(req.request_type(), "DistroDataSyncRequest");
    }

    #[test]
    fn test_distro_sync_response() {
        let resp = DistroDataSyncResponse::success();
        assert!(resp.response.success);

        let fail_resp = DistroDataSyncResponse::fail("error");
        assert!(!fail_resp.response.success);
    }
}
