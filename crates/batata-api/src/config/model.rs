//! Configuration management API models
//!
//! This module defines request/response models for configuration management operations.

use std::collections::{HashMap, HashSet};

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    grpc::Payload,
    model::CONFIG_MODULE,
    remote::model::{Request, RequestTrait, Response, ResponseTrait, ServerRequest},
};

fn serialize_config_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(CONFIG_MODULE)
}

fn deserialize_config_module<'de, D>(_: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(CONFIG_MODULE.to_string())
}

fn default_listen_true() -> bool {
    true
}

/// Base configuration request structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigRequest {
    #[serde(flatten)]
    pub request: Request,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ConfigRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(base ConfigRequest, request);

/// Configuration clone information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCloneInfo {
    pub config_id: i64,
    pub target_group_name: String,
    pub target_data_id: String,
}

/// Configuration listener information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenerInfo {
    pub query_type: String,
    pub listeners_status: HashMap<String, String>,
}

impl ConfigListenerInfo {
    pub const QUERY_TYPE_CONFIG: &str = "config";
    pub const QUERY_TYPE_IP: &str = "ip";
}

/// Policy for handling same configuration during import
#[derive(Default)]
pub enum SameConfigPolicy {
    #[default]
    Abort,
    Skip,
    Overwrite,
}

impl SameConfigPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            SameConfigPolicy::Abort => "ABORT",
            SameConfigPolicy::Skip => "SKIP",
            SameConfigPolicy::Overwrite => "OVERWRITE",
        }
    }
}

impl std::str::FromStr for SameConfigPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ABORT" => Ok(SameConfigPolicy::Abort),
            "SKIP" => Ok(SameConfigPolicy::Skip),
            "OVERWRITE" => Ok(SameConfigPolicy::Overwrite),
            _ => Err(format!("Invalid same config policy: {}", s)),
        }
    }
}

/// Configuration listen context
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigListenContext {
    pub group: String,
    pub md5: String,
    pub data_id: String,
    pub tenant: String,
}

/// Batch listen request for configuration changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigBatchListenRequest {
    #[serde(flatten)]
    pub config_request: ConfigRequest,
    #[serde(default = "default_listen_true")]
    pub listen: bool,
    pub config_listen_contexts: Vec<ConfigListenContext>,
}

impl ConfigBatchListenRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            listen: true,
            config_listen_contexts: Vec::new(),
        }
    }
}

impl_request_trait!(ConfigBatchListenRequest, config_request);

impl From<&Payload> for ConfigBatchListenRequest {
    fn from(value: &Payload) -> Self {
        ConfigBatchListenRequest::from_payload(value)
    }
}

/// Request to publish configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigPublishRequest {
    #[serde(flatten)]
    pub config_request: ConfigRequest,
    pub content: String,
    pub cas_md5: String,
    pub addition_map: HashMap<String, String>,
}

impl ConfigPublishRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            addition_map: HashMap::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConfigPublishRequest, config_request);

impl From<&Payload> for ConfigPublishRequest {
    fn from(value: &Payload) -> Self {
        ConfigPublishRequest::from_payload(value)
    }
}

/// Request to query configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigQueryRequest {
    #[serde(flatten)]
    pub config_request: ConfigRequest,
    pub tag: String,
}

impl ConfigQueryRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConfigQueryRequest, config_request);

impl From<&Payload> for ConfigQueryRequest {
    fn from(value: &Payload) -> Self {
        ConfigQueryRequest::from_payload(value)
    }
}

/// Request to remove configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigRemoveRequest {
    #[serde(flatten)]
    pub config_request: ConfigRequest,
    pub tag: String,
}

impl ConfigRemoveRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConfigRemoveRequest, config_request);

impl From<&Payload> for ConfigRemoveRequest {
    fn from(value: &Payload) -> Self {
        ConfigRemoveRequest::from_payload(value)
    }
}

/// Base fuzzy watch notify request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FuzzyWatchNotifyRequest {
    #[serde(flatten)]
    pub server_request: ServerRequest,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl FuzzyWatchNotifyRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(base FuzzyWatchNotifyRequest, server_request);

/// Configuration fuzzy watch change notify request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigFuzzyWatchChangeNotifyRequest {
    #[serde(flatten)]
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    pub group_key: String,
    pub change_type: String,
}

impl ConfigFuzzyWatchChangeNotifyRequest {
    pub fn new() -> Self {
        Self {
            fuzzy_watch_notify_request: FuzzyWatchNotifyRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(
    ConfigFuzzyWatchChangeNotifyRequest,
    fuzzy_watch_notify_request
);

impl From<&Payload> for ConfigFuzzyWatchChangeNotifyRequest {
    fn from(value: &Payload) -> Self {
        ConfigFuzzyWatchChangeNotifyRequest::from_payload(value)
    }
}

/// Context for fuzzy watch sync
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub group_key: String,
    pub change_type: String,
}

/// Configuration fuzzy watch sync request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigFuzzyWatchSyncRequest {
    #[serde(flatten)]
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    pub group_key_pattern: String,
    pub sync_type: String,
    pub total_batch: i32,
    pub current_batch: i32,
    pub contexts: HashSet<Context>,
}

impl ConfigFuzzyWatchSyncRequest {
    pub fn new() -> Self {
        Self {
            fuzzy_watch_notify_request: FuzzyWatchNotifyRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConfigFuzzyWatchSyncRequest, fuzzy_watch_notify_request);

impl From<&Payload> for ConfigFuzzyWatchSyncRequest {
    fn from(value: &Payload) -> Self {
        ConfigFuzzyWatchSyncRequest::from_payload(value)
    }
}

/// Metrics key for client config metrics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsKey {
    pub r#type: String,
    pub key: String,
}

impl MetricsKey {
    pub const CACHE_DATA: &str = "cacheData";
    pub const SNAPSHOT_DATA: &str = "snapshotData";

    pub fn new() -> Self {
        MetricsKey::default()
    }
}

/// Client configuration metric request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientConfigMetricRequest {
    #[serde(flatten)]
    pub server_request: ServerRequest,
    pub metrics_keys: Vec<MetricsKey>,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ClientConfigMetricRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            metrics_keys: Vec::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ClientConfigMetricRequest, server_request);

impl From<&Payload> for ClientConfigMetricRequest {
    fn from(value: &Payload) -> Self {
        ClientConfigMetricRequest::from_payload(value)
    }
}

/// Configuration change notify request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigChangeNotifyRequest {
    #[serde(flatten)]
    pub server_request: ServerRequest,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ConfigChangeNotifyRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            ..Default::default()
        }
    }

    /// Create a new config change notification for a specific config
    pub fn for_config(data_id: &str, group: &str, tenant: &str) -> Self {
        Self {
            server_request: ServerRequest::new(),
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            module: "config".to_string(),
        }
    }
}

impl_request_trait!(ConfigChangeNotifyRequest, server_request);

impl From<&Payload> for ConfigChangeNotifyRequest {
    fn from(value: &Payload) -> Self {
        ConfigChangeNotifyRequest::from_payload(value)
    }
}

/// Configuration fuzzy watch request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigFuzzyWatchRequest {
    pub request: Request,
    pub group_key_pattern: String,
    pub received_group_keys: HashSet<String>,
    pub watch_type: String,
    pub is_initializing: bool,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ConfigFuzzyWatchRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConfigFuzzyWatchRequest, request);

impl From<&Payload> for ConfigFuzzyWatchRequest {
    fn from(value: &Payload) -> Self {
        ConfigFuzzyWatchRequest::from_payload(value)
    }
}

/// Request for syncing configuration changes across cluster nodes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigChangeClusterSyncRequest {
    #[serde(flatten)]
    pub config_request: ConfigRequest,
    pub last_modified: i64,
    pub gray_name: String,
}

impl ConfigChangeClusterSyncRequest {
    pub fn new() -> Self {
        Self {
            config_request: ConfigRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConfigChangeClusterSyncRequest, config_request);

impl From<&Payload> for ConfigChangeClusterSyncRequest {
    fn from(value: &Payload) -> Self {
        ConfigChangeClusterSyncRequest::from_payload(value)
    }
}

/// Configuration context
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigContext {
    pub group: String,
    pub data_id: String,
    pub tenant: String,
}

/// Response for batch listen configuration changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeBatchListenResponse {
    #[serde(flatten)]
    pub response: Response,
    pub changed_configs: Vec<ConfigContext>,
}

impl ConfigChangeBatchListenResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl_response_trait!(ConfigChangeBatchListenResponse);

impl From<ConfigChangeBatchListenResponse> for Any {
    fn from(val: ConfigChangeBatchListenResponse) -> Self {
        val.to_any()
    }
}

/// Response for publish configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPublishResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ConfigPublishResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConfigPublishResponse);

impl From<ConfigPublishResponse> for Any {
    fn from(val: ConfigPublishResponse) -> Self {
        val.to_any()
    }
}

/// Response for query configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigQueryResponse {
    #[serde(flatten)]
    pub response: Response,
    pub content: String,
    pub encrypted_data_key: String,
    pub content_type: String,
    pub md5: String,
    pub last_modified: i64,
    pub is_beta: bool,
    pub tag: Option<String>,
}

impl ConfigQueryResponse {
    pub const CONFIG_NOT_FOUND: i32 = 300;
    pub const CONFIG_QUERY_CONFLICT: i32 = 400;
    pub const NO_RIGHT: i32 = 403;

    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl_response_trait!(ConfigQueryResponse);

impl From<ConfigQueryResponse> for Any {
    fn from(val: ConfigQueryResponse) -> Self {
        val.to_any()
    }
}

/// Response for remove configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRemoveResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ConfigRemoveResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConfigRemoveResponse);

impl From<ConfigRemoveResponse> for Any {
    fn from(val: ConfigRemoveResponse) -> Self {
        val.to_any()
    }
}

/// Response for fuzzy watch change notify
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchChangeNotifyResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ConfigFuzzyWatchChangeNotifyResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConfigFuzzyWatchChangeNotifyResponse);

impl From<ConfigFuzzyWatchChangeNotifyResponse> for Any {
    fn from(val: ConfigFuzzyWatchChangeNotifyResponse) -> Self {
        val.to_any()
    }
}

/// Response for fuzzy watch sync
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchSyncResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ConfigFuzzyWatchSyncResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConfigFuzzyWatchSyncResponse);

impl From<ConfigFuzzyWatchSyncResponse> for Any {
    fn from(val: ConfigFuzzyWatchSyncResponse) -> Self {
        val.to_any()
    }
}

/// Response for client config metric
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigMetricResponse {
    #[serde(flatten)]
    pub response: Response,
    pub metrics: HashMap<String, Value>,
}

impl ClientConfigMetricResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl_response_trait!(ClientConfigMetricResponse);

impl From<ClientConfigMetricResponse> for Any {
    fn from(val: ClientConfigMetricResponse) -> Self {
        val.to_any()
    }
}

/// Response for config change notify
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeNotifyResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ConfigChangeNotifyResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConfigChangeNotifyResponse);

impl From<ConfigChangeNotifyResponse> for Any {
    fn from(val: ConfigChangeNotifyResponse) -> Self {
        val.to_any()
    }
}

/// Response for fuzzy watch
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFuzzyWatchResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ConfigFuzzyWatchResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConfigFuzzyWatchResponse);

impl From<ConfigFuzzyWatchResponse> for Any {
    fn from(val: ConfigFuzzyWatchResponse) -> Self {
        val.to_any()
    }
}

/// Response for cluster sync
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeClusterSyncResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ConfigChangeClusterSyncResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConfigChangeClusterSyncResponse);

impl From<ConfigChangeClusterSyncResponse> for Any {
    fn from(val: ConfigChangeClusterSyncResponse) -> Self {
        val.to_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_request() {
        let req = ConfigRequest::new();
        assert!(req.headers().is_empty());
    }

    #[test]
    fn test_config_change_cluster_sync_request() {
        let req = ConfigChangeClusterSyncRequest::new();
        assert_eq!(req.request_type(), "ConfigChangeClusterSyncRequest");
        assert_eq!(req.last_modified, 0);
    }

    #[test]
    fn test_config_publish_request_creation() {
        let mut req = ConfigPublishRequest::new();
        req.config_request.data_id = "test.yaml".to_string();
        req.config_request.group = "DEFAULT_GROUP".to_string();
        req.content = "key: value".to_string();

        assert_eq!(req.config_request.data_id, "test.yaml");
        assert_eq!(req.content, "key: value");
    }

    #[test]
    fn test_config_query_request_creation() {
        let mut req = ConfigQueryRequest::new();
        req.config_request.data_id = "app.properties".to_string();
        req.config_request.group = "DEFAULT_GROUP".to_string();
        req.config_request.tenant = "public".to_string();

        assert_eq!(req.config_request.data_id, "app.properties");
        assert_eq!(req.config_request.tenant, "public");
    }

    #[test]
    fn test_config_remove_request_creation() {
        let mut req = ConfigRemoveRequest::new();
        req.config_request.data_id = "old-config.yaml".to_string();
        req.config_request.group = "DEFAULT_GROUP".to_string();

        assert_eq!(req.config_request.data_id, "old-config.yaml");
    }

    #[test]
    fn test_config_batch_listen_request() {
        let req = ConfigBatchListenRequest {
            config_request: ConfigRequest::new(),
            listen: true,
            config_listen_contexts: vec![],
        };
        assert!(req.listen);
        assert!(req.config_listen_contexts.is_empty());
    }
}
