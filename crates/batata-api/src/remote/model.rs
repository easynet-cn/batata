//! Remote API models for Nacos protocol communication
//!
//! This module defines request/response models used in Nacos remote communication.

use std::collections::HashMap;

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    grpc::{Metadata, Payload},
    model::{INTERNAL_MODULE, Member},
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
        let bytes = value.body.clone().unwrap_or_default().value.to_vec();
        match serde_json::from_slice::<T>(&bytes) {
            Ok(v) => v,
            Err(e) => {
                let payload_type = value
                    .metadata
                    .as_ref()
                    .map(|m| m.r#type.as_str())
                    .unwrap_or("unknown");
                let body_str = String::from_utf8_lossy(&bytes);
                tracing::error!(
                    payload_type = %payload_type,
                    error = %e,
                    body = %body_str,
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
#[serde(rename_all = "camelCase", default)]
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
#[serde(rename_all = "camelCase", default)]
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

/// Trait for configuration-specific requests
pub trait ConfigRequestTrait {
    fn data_id(&self) -> String {
        String::default()
    }

    fn group_name(&self) -> String {
        String::default()
    }

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
    pub internal_request: InternalRequest,
    pub server_ip: String,
    pub server_port: String,
}

impl ConnectResetRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConnectResetRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConnectResetRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

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
    pub internal_request: InternalRequest,
}

impl ServerCheckRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl RequestTrait for ServerCheckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServerCheckRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

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
    pub internal_request: InternalRequest,
    pub client_version: String,
    pub tenant: String,
    pub labels: HashMap<String, String>,
    pub client_abilities: ClientAbilities,
}

impl ConnectionSetupRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConnectionSetupRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ConnectionSetupRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

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
    pub internal_request: InternalRequest,
}

impl ServerLoaderInfoRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl RequestTrait for ServerLoaderInfoRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServerLoaderInfoRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

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
    pub internal_request: InternalRequest,
}

impl ServerReloadRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl RequestTrait for ServerReloadRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServerReloadRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

impl From<&Payload> for ServerReloadRequest {
    fn from(value: &Payload) -> Self {
        ServerReloadRequest::from_payload(value)
    }
}

/// Generic server request with module information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerRequest {
    #[serde(flatten)]
    pub request: Request,
    #[serde(
        serialize_with = "serialize_internal_module",
        deserialize_with = "deserialize_internal_module"
    )]
    module: String,
}

impl ServerRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ServerRequest {
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

/// Client detection request for checking client status
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientDetectionRequest {
    #[serde(flatten)]
    pub server_requst: ServerRequest,
}

impl RequestTrait for ClientDetectionRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_requst.headers()
    }

    fn request_type(&self) -> &'static str {
        "ClientDetectionRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_requst.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_requst.request_id()
    }
}

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
    pub server_requst: ServerRequest,
}

impl RequestTrait for SetupAckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_requst.headers()
    }

    fn request_type(&self) -> &'static str {
        "SetupAckRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_requst.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_requst.request_id()
    }
}

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
    pub response: Response,
    pub connection_id: String,
    pub support_ability_negotiation: bool,
}

impl ResponseTrait for ServerCheckResponse {
    fn response_type(&self) -> &'static str {
        "ServerCheckResponse"
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
    pub response: Response,
}

impl ClientDetectionResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ClientDetectionResponse {
    fn response_type(&self) -> &'static str {
        "ClientDetectionResponse"
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
    pub response: Response,
    pub loader_metrics: HashMap<String, String>,
}

impl ServerLoaderInfoResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            loader_metrics: HashMap::new(),
        }
    }
}

impl ResponseTrait for ServerLoaderInfoResponse {
    fn response_type(&self) -> &'static str {
        "ServerLoaderInfoResponse"
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
    pub response: Response,
}

impl ServerReloadResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ServerReloadResponse {
    fn response_type(&self) -> &'static str {
        "ServerReloadResponse"
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
    pub response: Response,
}

impl ConnectResetResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ConnectResetResponse {
    fn response_type(&self) -> &'static str {
        "ConnectResetResponse"
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
    pub response: Response,
}

impl SetupAckResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for SetupAckResponse {
    fn response_type(&self) -> &'static str {
        "SetupAckResponse"
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
    pub internal_request: InternalRequest,
}

impl PushAckRequest {
    pub fn new() -> Self {
        Self {
            internal_request: InternalRequest::new(),
        }
    }
}

impl RequestTrait for PushAckRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "PushAckRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

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
    pub internal_request: InternalRequest,
    pub node: Option<Member>,
}

impl RequestTrait for MemberReportRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.internal_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "MemberReportRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.internal_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.internal_request.request_id()
    }
}

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
    pub response: Response,
    pub node: Option<Member>,
}

impl MemberReportResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            node: None,
        }
    }
}

impl ResponseTrait for MemberReportResponse {
    fn response_type(&self) -> &'static str {
        "MemberReportResponse"
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

impl From<MemberReportResponse> for Any {
    fn from(val: MemberReportResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// Lock: LockOperation
// =============================================================================

/// Lock instance for distributed locking
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockInstance {
    pub key: String,
    pub expired_time: i64,
    pub lock_type: String,
    pub params: HashMap<String, String>,
}

/// Request for distributed lock operations (acquire/release)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LockOperationRequest {
    #[serde(flatten)]
    pub request: Request,
    pub module: String,
    pub lock_instance: Option<LockInstance>,
    #[serde(alias = "lockOperationEnum")]
    pub lock_operation: String,
}

impl RequestTrait for LockOperationRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "LockOperationRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

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
    pub response: Response,
    pub result: bool,
}

impl LockOperationResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            result: false,
        }
    }
}

impl ResponseTrait for LockOperationResponse {
    fn response_type(&self) -> &'static str {
        "LockOperationResponse"
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

impl From<LockOperationResponse> for Any {
    fn from(val: LockOperationResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// AI-MCP: McpServerEndpoint, QueryMcpServer, ReleaseMcpServer
// =============================================================================

/// Request to register/deregister an MCP server endpoint
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerEndpointRequest {
    #[serde(flatten)]
    pub request: Request,
    pub module: String,
    pub namespace_id: String,
    #[serde(default)]
    pub mcp_id: String,
    pub mcp_name: String,
    pub address: String,
    pub port: u16,
    pub version: String,
    #[serde(rename = "type")]
    pub operation_type: String,
}

impl RequestTrait for McpServerEndpointRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "McpServerEndpointRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

impl From<&Payload> for McpServerEndpointRequest {
    fn from(value: &Payload) -> Self {
        McpServerEndpointRequest::from_payload(value)
    }
}

/// Response to MCP server endpoint register/deregister
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerEndpointResponse {
    #[serde(flatten)]
    pub response: Response,
    #[serde(rename = "type")]
    pub operation_type: String,
}

impl McpServerEndpointResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            operation_type: String::new(),
        }
    }
}

impl ResponseTrait for McpServerEndpointResponse {
    fn response_type(&self) -> &'static str {
        "McpServerEndpointResponse"
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

impl From<McpServerEndpointResponse> for Any {
    fn from(val: McpServerEndpointResponse) -> Self {
        val.to_any()
    }
}

/// Request to query MCP server details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryMcpServerRequest {
    #[serde(flatten)]
    pub request: Request,
    pub module: String,
    pub namespace_id: String,
    pub mcp_name: String,
    pub version: String,
}

impl RequestTrait for QueryMcpServerRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "QueryMcpServerRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

impl From<&Payload> for QueryMcpServerRequest {
    fn from(value: &Payload) -> Self {
        QueryMcpServerRequest::from_payload(value)
    }
}

/// Response containing MCP server details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryMcpServerResponse {
    #[serde(flatten)]
    pub response: Response,
    pub mcp_server_detail_info: serde_json::Value,
}

impl QueryMcpServerResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            mcp_server_detail_info: serde_json::Value::Null,
        }
    }
}

impl ResponseTrait for QueryMcpServerResponse {
    fn response_type(&self) -> &'static str {
        "QueryMcpServerResponse"
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

impl From<QueryMcpServerResponse> for Any {
    fn from(val: QueryMcpServerResponse) -> Self {
        val.to_any()
    }
}

/// Request to release (publish) an MCP server
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseMcpServerRequest {
    #[serde(flatten)]
    pub request: Request,
    pub module: String,
    pub namespace_id: String,
    pub mcp_name: String,
    pub server_specification: serde_json::Value,
    #[serde(default)]
    pub tool_specification: serde_json::Value,
    #[serde(default)]
    pub endpoint_specification: serde_json::Value,
}

impl RequestTrait for ReleaseMcpServerRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ReleaseMcpServerRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

impl From<&Payload> for ReleaseMcpServerRequest {
    fn from(value: &Payload) -> Self {
        ReleaseMcpServerRequest::from_payload(value)
    }
}

/// Response for releasing (publishing) an MCP server
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseMcpServerResponse {
    #[serde(flatten)]
    pub response: Response,
    pub mcp_id: String,
}

impl ReleaseMcpServerResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            mcp_id: String::new(),
        }
    }
}

impl ResponseTrait for ReleaseMcpServerResponse {
    fn response_type(&self) -> &'static str {
        "ReleaseMcpServerResponse"
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

impl From<ReleaseMcpServerResponse> for Any {
    fn from(val: ReleaseMcpServerResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// AI-A2A: AgentEndpoint, QueryAgentCard, ReleaseAgentCard
// =============================================================================

/// Agent endpoint information for gRPC registration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpoint {
    pub address: String,
    pub port: u16,
    pub version: String,
    pub transport: String,
    pub path: String,
    pub support_tls: bool,
}

/// Request to register/deregister an agent endpoint
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpointRequest {
    #[serde(flatten)]
    pub request: Request,
    pub module: String,
    pub namespace_id: String,
    pub agent_name: String,
    pub endpoint: Option<AgentEndpoint>,
    #[serde(rename = "type")]
    pub operation_type: String,
}

impl RequestTrait for AgentEndpointRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "AgentEndpointRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

impl From<&Payload> for AgentEndpointRequest {
    fn from(value: &Payload) -> Self {
        AgentEndpointRequest::from_payload(value)
    }
}

/// Response for agent endpoint register/deregister
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpointResponse {
    #[serde(flatten)]
    pub response: Response,
    #[serde(rename = "type")]
    pub operation_type: String,
}

impl AgentEndpointResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            operation_type: String::new(),
        }
    }
}

impl ResponseTrait for AgentEndpointResponse {
    fn response_type(&self) -> &'static str {
        "AgentEndpointResponse"
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

impl From<AgentEndpointResponse> for Any {
    fn from(val: AgentEndpointResponse) -> Self {
        val.to_any()
    }
}

/// Request to query agent card details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryAgentCardRequest {
    #[serde(flatten)]
    pub request: Request,
    pub module: String,
    pub namespace_id: String,
    pub agent_name: String,
    pub version: String,
    #[serde(default)]
    pub registration_type: String,
}

impl RequestTrait for QueryAgentCardRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "QueryAgentCardRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

impl From<&Payload> for QueryAgentCardRequest {
    fn from(value: &Payload) -> Self {
        QueryAgentCardRequest::from_payload(value)
    }
}

/// Response containing agent card details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryAgentCardResponse {
    #[serde(flatten)]
    pub response: Response,
    pub agent_card_detail_info: serde_json::Value,
}

impl QueryAgentCardResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            agent_card_detail_info: serde_json::Value::Null,
        }
    }
}

impl ResponseTrait for QueryAgentCardResponse {
    fn response_type(&self) -> &'static str {
        "QueryAgentCardResponse"
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

impl From<QueryAgentCardResponse> for Any {
    fn from(val: QueryAgentCardResponse) -> Self {
        val.to_any()
    }
}

fn default_registration_type() -> String {
    "service".to_string()
}

/// Request to release (publish) an agent card
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAgentCardRequest {
    #[serde(flatten)]
    pub request: Request,
    pub module: String,
    pub namespace_id: String,
    pub agent_name: String,
    pub agent_card: serde_json::Value,
    pub set_as_latest: bool,
    #[serde(default = "default_registration_type")]
    pub registration_type: String,
}

impl RequestTrait for ReleaseAgentCardRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ReleaseAgentCardRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

impl From<&Payload> for ReleaseAgentCardRequest {
    fn from(value: &Payload) -> Self {
        ReleaseAgentCardRequest::from_payload(value)
    }
}

/// Response for releasing (publishing) an agent card
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAgentCardResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl ReleaseAgentCardResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for ReleaseAgentCardResponse {
    fn response_type(&self) -> &'static str {
        "ReleaseAgentCardResponse"
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

impl From<ReleaseAgentCardResponse> for Any {
    fn from(val: ReleaseAgentCardResponse) -> Self {
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
        };
        let json = serde_json::to_string(&lock).unwrap();
        assert!(json.contains("test-lock"));
        let parsed: LockInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, "test-lock");
        assert_eq!(parsed.expired_time, 30000);
    }

    #[test]
    fn test_mcp_server_endpoint_request() {
        let req = McpServerEndpointRequest::default();
        assert_eq!(req.request_type(), "McpServerEndpointRequest");
    }

    #[test]
    fn test_query_mcp_server_request() {
        let req = QueryMcpServerRequest::default();
        assert_eq!(req.request_type(), "QueryMcpServerRequest");
    }

    #[test]
    fn test_release_mcp_server_request() {
        let req = ReleaseMcpServerRequest::default();
        assert_eq!(req.request_type(), "ReleaseMcpServerRequest");
    }

    #[test]
    fn test_agent_endpoint_request() {
        let req = AgentEndpointRequest::default();
        assert_eq!(req.request_type(), "AgentEndpointRequest");
    }

    #[test]
    fn test_query_agent_card_request() {
        let req = QueryAgentCardRequest::default();
        assert_eq!(req.request_type(), "QueryAgentCardRequest");
    }

    #[test]
    fn test_release_agent_card_request() {
        let req = ReleaseAgentCardRequest::default();
        assert_eq!(req.request_type(), "ReleaseAgentCardRequest");
    }

    #[test]
    fn test_agent_endpoint_serialization() {
        let ep = AgentEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8080,
            version: "1.0".to_string(),
            transport: "http".to_string(),
            path: "/agent".to_string(),
            support_tls: true,
        };
        let json = serde_json::to_string(&ep).unwrap();
        let parsed: AgentEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.address, "127.0.0.1");
        assert_eq!(parsed.port, 8080);
        assert!(parsed.support_tls);
    }
}
