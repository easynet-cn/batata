//! Remote API models for Batata protocol communication
//!
//! This module defines request/response models used in Batata remote communication.
//! Wire-compatible with the Nacos gRPC protocol.

use std::collections::HashMap;

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    grpc::{Metadata, Payload},
    model::{INTERNAL_MODULE, Member},
};

// Constants for connection labels
/// Connection label: source.
pub const LABEL_SOURCE: &str = "source";
/// Connection label: source is an SDK client.
pub const LABEL_SOURCE_SDK: &str = "sdk";
/// Connection label: source is a cluster node.
pub const LABEL_SOURCE_CLUSTER: &str = "cluster";
/// Connection label: module.
pub const LABEL_MODULE: &str = "module";
/// Connection label: module is config.
pub const LABEL_MODULE_CONFIG: &str = "config";
/// Connection label: module is naming.
pub const LABEL_MODULE_NAMING: &str = "naming";
/// Monitor label: none.
pub const MONITOR_LABEL_NONE: &str = "none";
/// Connection label: module is lock.
pub const LABEL_MODULE_LOCK: &str = "lock";
/// Connection label: module is AI.
pub const LABEL_MODULE_AI: &str = "ai";

fn serialize_internal_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(INTERNAL_MODULE)
}

fn deserialize_internal_module<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let _: serde::de::IgnoredAny = serde::Deserialize::deserialize(deserializer)?;
    Ok(INTERNAL_MODULE.to_string())
}

/// Deserialize a value, returning the default if the JSON value is `null`.
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// Base trait for all request models
pub trait RequestTrait {
    /// The `headers` method.
    fn headers(&self) -> HashMap<String, String>;

    /// Get a reference to a specific header value without cloning the entire map.
    fn get_header(&self, key: &str) -> Option<String> {
        self.headers().get(key).cloned()
    }

    /// The `request_type` method.
    fn request_type(&self) -> &'static str {
        ""
    }

    /// The `body` method.
    fn body(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The `insert_headers` method.
    fn insert_headers(&mut self, headers: HashMap<String, String>);

    /// The `request_id` method.
    fn request_id(&self) -> String {
        String::default()
    }

    /// The `string_to_sign` method.
    fn string_to_sign(&self) -> String {
        String::default()
    }

    /// The `function` function.
    fn from_payload<T>(value: &Payload) -> T
    where
        T: for<'a> Deserialize<'a> + Default,
    {
        // Access body bytes by reference — avoid cloning Option<Any> and copying bytes
        let bytes: &[u8] = match value.body.as_ref() {
            Some(any) => &any.value,
            None => &[],
        };
        match serde_json::from_slice::<T>(bytes) {
            Ok(v) => v,
            Err(e) => {
                let payload_type = value
                    .metadata
                    .as_ref()
                    .map(|m| m.r#type.as_str())
                    .unwrap_or("unknown");
                tracing::error!(
                    payload_type = %payload_type,
                    error = %e,
                    body = %String::from_utf8_lossy(bytes),
                    "Failed to deserialize gRPC payload"
                );
                T::default()
            }
        }
    }

    /// Convert the request to a protobuf Any type for gRPC transmission
    fn to_any(&self) -> Any
    where
        Self: Serialize,
    {
        Any {
            type_url: String::default(),
            value: self.body(),
        }
    }

    /// Convert the request to a gRPC payload with metadata
    fn to_payload(&self, metadata: Option<Metadata>) -> Payload
    where
        Self: Serialize,
    {
        Payload {
            metadata,
            body: Some(self.to_any()),
        }
    }

    /// Build a complete gRPC payload with auto-generated metadata (for server push)
    fn build_server_push_payload(&self) -> Payload
    where
        Self: Serialize,
    {
        let metadata = Metadata {
            r#type: self.request_type().to_string(),
            ..Default::default()
        };
        self.to_payload(Some(metadata))
    }
}

/// Base request structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Request {
    /// The `headers` field.
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `request_id` field.
    pub request_id: String,
}

impl Request {
    /// Creates a new instance.
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

    fn get_header(&self, key: &str) -> Option<String> {
        self.headers.get(key).cloned()
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
#[serde(rename_all = "camelCase", default)]
pub struct InternalRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    #[serde(
        serialize_with = "serialize_internal_module",
        deserialize_with = "deserialize_internal_module"
    )]
    module: String,
}

impl InternalRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(base InternalRequest, request);

/// Health check request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HealthCheckRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
}

impl HealthCheckRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl_request_trait!(HealthCheckRequest, internal_request);

impl From<&Payload> for HealthCheckRequest {
    fn from(value: &Payload) -> Self {
        HealthCheckRequest::from_payload(value)
    }
}

/// Response status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseCode {
    /// The `variant` variant.
    Success = 200,
    /// The `variant` variant.
    Fail = 500,
}

impl ResponseCode {
    /// The `code` method.
    pub fn code(&self) -> i32 {
        *self as i32
    }

    /// The `desc` method.
    pub fn desc(&self) -> &'static str {
        match self {
            ResponseCode::Success => "Response ok",
            ResponseCode::Fail => "Response fail",
        }
    }
}

/// Base trait for all response models
pub trait ResponseTrait {
    /// The `response_type` method.
    fn response_type(&self) -> &'static str {
        ""
    }

    /// The `request_id` method.
    fn request_id(&mut self, request_id: String);

    /// The `body` method.
    fn body(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The `error_code` method.
    fn error_code(&self) -> i32 {
        ResponseCode::Success.code()
    }

    /// The `result_code` method.
    fn result_code(&self) -> i32;

    /// The `message` method.
    fn message(&self) -> String {
        String::default()
    }

    /// Converts to any.
    fn to_any(&self) -> Any
    where
        Self: Serialize,
    {
        Any {
            type_url: String::default(),
            value: self.body(),
        }
    }

    /// Converts to payload.
    fn to_payload(&self, metadata: Option<Metadata>) -> Payload
    where
        Self: Serialize,
    {
        Payload {
            metadata,
            body: Some(self.to_any()),
        }
    }

    /// Build a complete gRPC payload with auto-generated metadata.
    /// This is a convenience method that combines response_type() and to_payload().
    fn build_payload(&self) -> Payload
    where
        Self: Serialize,
    {
        let metadata = Metadata {
            r#type: self.response_type().to_string(),
            ..Default::default()
        };
        self.to_payload(Some(metadata))
    }
}

/// Base response structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Response {
    /// The `result_code` field.
    pub result_code: i32,
    /// The `error_code` field.
    pub error_code: i32,
    /// The `success` field.
    pub success: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `message` field.
    pub message: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `request_id` field.
    pub request_id: String,
}

impl Response {
    /// Creates a new instance.
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
    /// The `response` field.
    pub response: Response,
}

impl HealthCheckResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(HealthCheckResponse);

impl From<HealthCheckResponse> for Any {
    fn from(val: HealthCheckResponse) -> Self {
        val.to_any()
    }
}

/// Trait for configuration-specific requests
pub trait ConfigRequestTrait {
    /// The `data_id` method.
    fn data_id(&self) -> String {
        String::default()
    }

    /// The `group_name` method.
    fn group_name(&self) -> String {
        String::default()
    }

    /// The `namespace_id` method.
    fn namespace_id(&self) -> String {
        String::default()
    }
}

/// Client capabilities information sent during connection setup
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAbilities {}

/// Connection reset request to restart a connection
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectResetRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
    /// The `server_ip` field.
    pub server_ip: String,
    /// The `server_port` field.
    pub server_port: String,
}

impl ConnectResetRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConnectResetRequest, internal_request);

impl From<&Payload> for ConnectResetRequest {
    fn from(value: &Payload) -> Self {
        ConnectResetRequest::from_payload(value)
    }
}

/// Server check request to verify server availability
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerCheckRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
}

impl ServerCheckRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl_request_trait!(ServerCheckRequest, internal_request);

impl From<&Payload> for ServerCheckRequest {
    fn from(value: &Payload) -> Self {
        ServerCheckRequest::from_payload(value)
    }
}

/// Connection setup request sent when establishing a new connection
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionSetupRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
    /// The `client_version` field.
    pub client_version: String,
    /// The `tenant` field.
    pub tenant: String,
    /// The `labels` field.
    pub labels: HashMap<String, String>,
    /// The `client_abilities` field.
    pub client_abilities: ClientAbilities,
    /// Client ability table for capability negotiation (Nacos 3.x compatible)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ability_table: Option<HashMap<String, bool>>,
}

impl ConnectionSetupRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(ConnectionSetupRequest, internal_request);

impl From<&Payload> for ConnectionSetupRequest {
    fn from(value: &Payload) -> Self {
        ConnectionSetupRequest::from_payload(value)
    }
}

/// Request to get server loader information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLoaderInfoRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
}

impl ServerLoaderInfoRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl_request_trait!(ServerLoaderInfoRequest, internal_request);

impl From<&Payload> for ServerLoaderInfoRequest {
    fn from(value: &Payload) -> Self {
        ServerLoaderInfoRequest::from_payload(value)
    }
}

/// Request to reload server configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerReloadRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
}

impl ServerReloadRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl_request_trait!(ServerReloadRequest, internal_request);

impl From<&Payload> for ServerReloadRequest {
    fn from(value: &Payload) -> Self {
        ServerReloadRequest::from_payload(value)
    }
}

/// Generic server request base — module field is NOT included here.
/// Each concrete type (NotifySubscriberRequest, ConfigChangeNotifyRequest, etc.)
/// defines its own `module` field with the appropriate value.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
}

impl ServerRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            request: Request {
                request_id: id.to_string(),
                headers: HashMap::new(),
            },
        }
    }
}

impl_request_trait!(base ServerRequest, request);

/// Client detection request for checking client status
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientDetectionRequest {
    #[serde(flatten)]
    /// The `server_requst` field.
    pub server_requst: ServerRequest,
    #[serde(
        serialize_with = "serialize_internal_module",
        deserialize_with = "deserialize_internal_module"
    )]
    module: String,
}

impl_request_trait!(ClientDetectionRequest, server_requst);

impl From<&Payload> for ClientDetectionRequest {
    fn from(value: &Payload) -> Self {
        ClientDetectionRequest::from_payload(value)
    }
}

/// Setup acknowledgment request for connection setup confirmation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SetupAckRequest {
    #[serde(flatten)]
    /// The `server_requst` field.
    pub server_requst: ServerRequest,
    /// Server ability table sent to client during connection setup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ability_table: Option<std::collections::HashMap<String, bool>>,
    #[serde(
        serialize_with = "serialize_internal_module",
        deserialize_with = "deserialize_internal_module"
    )]
    module: String,
}

impl_request_trait!(SetupAckRequest, server_requst);

impl From<&Payload> for SetupAckRequest {
    fn from(value: &Payload) -> Self {
        SetupAckRequest::from_payload(value)
    }
}

/// Server check response with connection information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCheckResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// The `connection_id` field.
    pub connection_id: String,
    /// The `support_ability_negotiation` field.
    pub support_ability_negotiation: bool,
}

impl_response_trait!(ServerCheckResponse);

impl From<ServerCheckResponse> for Any {
    fn from(val: ServerCheckResponse) -> Self {
        val.to_any()
    }
}

/// Client detection response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDetectionResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl ClientDetectionResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ClientDetectionResponse);

impl From<ClientDetectionResponse> for Any {
    fn from(val: ClientDetectionResponse) -> Self {
        val.to_any()
    }
}

/// Server loader info response with load metrics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLoaderInfoResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// The `loader_metrics` field.
    pub loader_metrics: HashMap<String, String>,
}

impl ServerLoaderInfoResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            loader_metrics: HashMap::new(),
        }
    }
}

impl_response_trait!(ServerLoaderInfoResponse);

impl From<ServerLoaderInfoResponse> for Any {
    fn from(val: ServerLoaderInfoResponse) -> Self {
        val.to_any()
    }
}

/// Server reload response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerReloadResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl ServerReloadResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ServerReloadResponse);

impl From<ServerReloadResponse> for Any {
    fn from(val: ServerReloadResponse) -> Self {
        val.to_any()
    }
}

/// Connect reset response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResetResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl ConnectResetResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConnectResetResponse);

impl From<ConnectResetResponse> for Any {
    fn from(val: ConnectResetResponse) -> Self {
        val.to_any()
    }
}

/// Setup acknowledgment response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupAckResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl SetupAckResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(SetupAckResponse);

impl From<SetupAckResponse> for Any {
    fn from(val: SetupAckResponse) -> Self {
        val.to_any()
    }
}

/// Push acknowledgment request for confirming server push
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PushAckRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
}

impl PushAckRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl_request_trait!(PushAckRequest, internal_request);

impl From<&Payload> for PushAckRequest {
    fn from(value: &Payload) -> Self {
        PushAckRequest::from_payload(value)
    }
}

// =============================================================================
// Cluster: MemberReport
// =============================================================================

/// Request for cluster member heartbeat reporting between nodes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MemberReportRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
    /// The `node` field.
    pub node: Option<Member>,
}

impl_request_trait!(MemberReportRequest, internal_request);

impl From<&Payload> for MemberReportRequest {
    fn from(value: &Payload) -> Self {
        MemberReportRequest::from_payload(value)
    }
}

/// Response for cluster member heartbeat reporting
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberReportResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// The `node` field.
    pub node: Option<Member>,
}

impl MemberReportResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            node: None,
        }
    }
}

impl_response_trait!(MemberReportResponse);

impl From<MemberReportResponse> for Any {
    fn from(val: MemberReportResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// Auth: Cache Invalidation (cluster-internal)
// =============================================================================

/// Request to invalidate auth caches on peer nodes.
/// Sent via cluster gRPC port when roles, permissions, or tokens change.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AuthCacheInvalidateRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
    /// Type: "role", "permission", "token", "user", "all"
    pub invalidate_type: String,
    /// Target: username, role name, or token id. Empty for "all".
    pub target: String,
}

impl AuthCacheInvalidateRequest {
    /// Creates a new instance.
    pub fn new(invalidate_type: &str, target: &str) -> Self {
        Self {
            internal_request: InternalRequest::new(),
            invalidate_type: invalidate_type.to_string(),
            target: target.to_string(),
        }
    }
}

impl_request_trait!(AuthCacheInvalidateRequest, internal_request);

impl From<&Payload> for AuthCacheInvalidateRequest {
    fn from(value: &Payload) -> Self {
        AuthCacheInvalidateRequest::from_payload(value)
    }
}

/// Response to auth cache invalidation request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthCacheInvalidateResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl AuthCacheInvalidateResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(AuthCacheInvalidateResponse);

impl From<AuthCacheInvalidateResponse> for Any {
    fn from(val: AuthCacheInvalidateResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// Cluster: Plugin Availability (cluster-internal)
// =============================================================================

/// Plugin availability query request (cluster-internal gRPC).
///
/// Mirrors Nacos `PluginAvailabilityRequest`. Sent between cluster nodes to
/// query which plugins are available (enabled) on the target node.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginAvailabilityRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
    /// Plugin ID to query (format: `type:name`). Required when `query_all` is false.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub plugin_id: String,
    /// If true, return availability for all plugins.
    #[serde(default)]
    pub query_all: bool,
}

impl PluginAvailabilityRequest {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(PluginAvailabilityRequest, internal_request);

impl From<&Payload> for PluginAvailabilityRequest {
    fn from(value: &Payload) -> Self {
        PluginAvailabilityRequest::from_payload(value)
    }
}

/// Plugin availability query response.
///
/// Mirrors Nacos `PluginAvailabilityResponse`. In single-plugin mode
/// (`query_all == false`), `plugin_id` and `available` are populated. In
/// `query_all` mode, `plugin_availability_map` contains the full mapping.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAvailabilityResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// Queried plugin ID (single-plugin mode).
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub plugin_id: String,
    /// Whether the plugin is available (single-plugin mode).
    #[serde(default)]
    pub available: bool,
    /// Plugin ID -> enabled mapping (query_all mode).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub plugin_availability_map: Option<HashMap<String, bool>>,
}

impl PluginAvailabilityResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl_response_trait!(PluginAvailabilityResponse);

impl From<PluginAvailabilityResponse> for Any {
    fn from(val: PluginAvailabilityResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// Consul: Event Broadcast (cluster-internal)
// =============================================================================

/// Request to broadcast a Consul user event to all cluster nodes.
/// Sent via cluster gRPC port when an event is fired via `/v1/event/fire/{name}`.
/// This mirrors Consul's Serf gossip-based event propagation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConsulEventBroadcastRequest {
    #[serde(flatten)]
    /// The `internal_request` field.
    pub internal_request: InternalRequest,
    /// Event UUID
    pub event_id: String,
    /// Event name
    pub event_name: String,
    /// Base64-encoded payload (max 300 bytes raw)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// Regex filter for target nodes
    pub node_filter: String,
    /// Regex filter for target services
    pub service_filter: String,
    /// Regex filter for target tags
    pub tag_filter: String,
    /// Lamport time
    pub ltime: u64,
}

impl ConsulEventBroadcastRequest {
    /// Creates a new instance.
    pub fn new(
        event_id: String,
        event_name: String,
        payload: Option<String>,
        node_filter: String,
        service_filter: String,
        tag_filter: String,
        ltime: u64,
    ) -> Self {
        Self {
            internal_request: InternalRequest::new(),
            event_id,
            event_name,
            payload,
            node_filter,
            service_filter,
            tag_filter,
            ltime,
        }
    }
}

impl_request_trait!(ConsulEventBroadcastRequest, internal_request);

impl From<&Payload> for ConsulEventBroadcastRequest {
    fn from(value: &Payload) -> Self {
        ConsulEventBroadcastRequest::from_payload(value)
    }
}

/// Response to event broadcast
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsulEventBroadcastResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl ConsulEventBroadcastResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl_response_trait!(ConsulEventBroadcastResponse);

impl From<ConsulEventBroadcastResponse> for Any {
    fn from(val: ConsulEventBroadcastResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// Lock: LockOperation
// =============================================================================

/// Lock instance for distributed locking
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LockInstance {
    /// The `key` field.
    pub key: String,
    /// The `expired_time` field.
    pub expired_time: i64,
    /// The `lock_type` field.
    pub lock_type: String,
    #[serde(deserialize_with = "deserialize_null_default")]
    /// The `params` field.
    pub params: HashMap<String, String>,
    /// Owner of the lock (`clientId:threadId` on the wire). Optional so that
    /// pre-3.3.0 payloads that predate the field still deserialize via the
    /// struct-level `#[serde(default)]`.
    pub owner: Option<String>,
}

/// Request for distributed lock operations (acquire/release)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LockOperationRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `lock_instance` field.
    pub lock_instance: Option<LockInstance>,
    #[serde(alias = "lockOperationEnum")]
    /// The `lock_operation` field.
    pub lock_operation: String,
}

impl_request_trait!(LockOperationRequest, request);

impl From<&Payload> for LockOperationRequest {
    fn from(value: &Payload) -> Self {
        LockOperationRequest::from_payload(value)
    }
}

/// Response for distributed lock operations
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockOperationResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// The `result` field.
    pub result: bool,
}

impl LockOperationResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            result: false,
        }
    }
}

impl_response_trait!(LockOperationResponse);

impl From<LockOperationResponse> for Any {
    fn from(val: LockOperationResponse) -> Self {
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

    #[test]
    fn test_member_report_request() {
        let req = MemberReportRequest::default();
        assert_eq!(req.request_type(), "MemberReportRequest");
        assert!(req.node.is_none());
    }

    #[test]
    fn test_lock_operation_request() {
        let req = LockOperationRequest::default();
        assert_eq!(req.request_type(), "LockOperationRequest");
    }

    #[test]
    fn test_lock_instance_serialization() {
        let lock = LockInstance {
            key: "test-lock".to_string(),
            expired_time: 30000,
            lock_type: "reentrant".to_string(),
            params: HashMap::new(),
            owner: None,
        };
        let json = serde_json::to_string(&lock).unwrap();
        assert!(json.contains("test-lock"));
        let parsed: LockInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, "test-lock");
        assert_eq!(parsed.expired_time, 30000);
    }

    #[test]
    fn test_lock_instance_owner_roundtrip() {
        // GA 3.3.0 LockInstance carries an `owner` field; it must survive a
        // JSON round-trip under the camelCase wire format.
        let lock = LockInstance {
            key: "test-lock".to_string(),
            expired_time: 30000,
            lock_type: "reentrant".to_string(),
            params: HashMap::new(),
            owner: Some("client-1:ThreadId(7)".to_string()),
        };
        let json = serde_json::to_string(&lock).unwrap();
        assert!(
            json.contains("\"owner\":\"client-1:ThreadId(7)\""),
            "owner must be present on the wire: {json}"
        );
        let parsed: LockInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.owner.as_deref(), Some("client-1:ThreadId(7)"));
        assert_eq!(parsed.key, "test-lock");
        assert_eq!(parsed.expired_time, 30000);
    }

    #[test]
    fn test_lock_instance_owner_absent_defaults_to_none() {
        // Pre-3.3.0 Java clients omit `owner` entirely: struct-level
        // #[serde(default)] must yield None, not a deserialize error.
        let json = r#"{"key":"test-lock","expiredTime":30000,"lockType":"nacosMutexLock"}"#;
        let parsed: LockInstance = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.owner, None);
        assert_eq!(parsed.key, "test-lock");

        // An explicit JSON null is also accepted (Option handles null).
        let json_null = r#"{"key":"test-lock","expiredTime":30000,"lockType":"mutex","params":null,"owner":null}"#;
        let parsed_null: LockInstance = serde_json::from_str(json_null).unwrap();
        assert_eq!(parsed_null.owner, None);
        assert!(parsed_null.params.is_empty());
    }

    #[test]
    fn test_lock_instance_null_params() {
        // Java client may send null for params field
        let json =
            r#"{"key":"test-lock","expiredTime":30000,"lockType":"nacosMutexLock","params":null}"#;
        let parsed: LockInstance = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.key, "test-lock");
        assert_eq!(parsed.expired_time, 30000);
        assert_eq!(parsed.lock_type, "nacosMutexLock");
        assert!(parsed.params.is_empty());
    }

    #[test]
    fn test_lock_instance_missing_params() {
        // Java client may omit params field entirely
        let json = r#"{"key":"test-lock","expiredTime":30000,"lockType":"nacosMutexLock"}"#;
        let parsed: LockInstance = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.key, "test-lock");
        assert!(parsed.params.is_empty());
    }

    #[test]
    fn test_lock_operation_request_from_java_client() {
        // Simulate the exact JSON sent by the Nacos Java SDK client
        let json = r#"{
            "lockInstance": {"key": "test-lock", "expiredTime": 30000, "lockType": "nacosMutexLock", "params": null},
            "lockOperationEnum": "ACQUIRE",
            "module": "lock",
            "requestId": "req-123"
        }"#;
        let parsed: LockOperationRequest = serde_json::from_str(json).unwrap();
        assert!(parsed.lock_instance.is_some());
        let lock = parsed.lock_instance.unwrap();
        assert_eq!(lock.key, "test-lock");
        assert_eq!(lock.expired_time, 30000);
        assert_eq!(lock.lock_type, "nacosMutexLock");
        assert_eq!(parsed.lock_operation, "ACQUIRE");
        assert_eq!(parsed.module, "lock");
        assert_eq!(parsed.request.request_id, "req-123");
    }
}
