// Naming module API models for service discovery
// This file defines request/response structures for service registration, discovery, and subscription

use std::collections::{HashMap, HashSet};

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};

use crate::api::{
    grpc::Payload,
    model::NAMING_MODULE,
    remote::model::{Request, RequestTrait, Response, ResponseTrait, ServerRequest},
};

fn serialize_naming_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(NAMING_MODULE)
}

fn deserialize_naming_module<'de, D>(_: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(NAMING_MODULE.to_string())
}

// Instance type constants
pub const INSTANCE_TYPE_EPHEMERAL: &str = "ephemeral";
pub const INSTANCE_TYPE_PERSISTENT: &str = "persistent";

// Request type constants
pub const REGISTER_INSTANCE: &str = "registerInstance";
pub const DE_REGISTER_INSTANCE: &str = "deregisterInstance";

// Service instance information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub instance_id: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub service_name: String,
    pub metadata: HashMap<String, String>,
    pub instance_heart_beat_interval: i64,
    pub instance_heart_beat_time_out: i64,
    pub ip_delete_timeout: i64,
    pub instance_id_generator: String,
}

// Service information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub name: String,
    pub group_name: String,
    pub clusters: String,
    pub cache_millis: i64,
    pub hosts: Vec<Instance>,
    pub last_ref_time: i64,
    pub checksum: String,
    pub all_ips: bool,
    pub reach_protection_threshold: bool,
}

// Service list item
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub group_name: String,
    pub cluster_count: i32,
    pub ip_count: i32,
    pub healthy_instance_count: i32,
    pub trigger_flag: bool,
}

// Base naming request structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingRequest {
    #[serde(flatten)]
    pub request: Request,
    pub namespace: String,
    pub service_name: String,
    pub group_name: String,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
    )]
    module: String,
}

impl NamingRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for NamingRequest {
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

// Instance registration/deregistration request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRequest {
    #[serde(flatten)]
    pub naming_request: NamingRequest,
    pub r#type: String, // registerInstance or deregisterInstance
    pub instance: Instance,
}

impl InstanceRequest {
    pub fn new() -> Self {
        Self {
            naming_request: NamingRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for InstanceRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.naming_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "InstanceRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.naming_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.naming_request.request_id()
    }
}

impl From<&Payload> for InstanceRequest {
    fn from(value: &Payload) -> Self {
        InstanceRequest::from_payload(value)
    }
}

// Batch instance request for multiple instances
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchInstanceRequest {
    #[serde(flatten)]
    pub naming_request: NamingRequest,
    pub r#type: String,
    pub instances: Vec<Instance>,
}

impl BatchInstanceRequest {
    pub fn new() -> Self {
        Self {
            naming_request: NamingRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for BatchInstanceRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.naming_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "BatchInstanceRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.naming_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.naming_request.request_id()
    }
}

impl From<&Payload> for BatchInstanceRequest {
    fn from(value: &Payload) -> Self {
        BatchInstanceRequest::from_payload(value)
    }
}

// Persistent instance request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistentInstanceRequest {
    #[serde(flatten)]
    pub naming_request: NamingRequest,
    pub r#type: String,
    pub instance: Instance,
}

impl PersistentInstanceRequest {
    pub fn new() -> Self {
        Self {
            naming_request: NamingRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for PersistentInstanceRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.naming_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "PersistentInstanceRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.naming_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.naming_request.request_id()
    }
}

impl From<&Payload> for PersistentInstanceRequest {
    fn from(value: &Payload) -> Self {
        PersistentInstanceRequest::from_payload(value)
    }
}

// Service list request for listing services
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListRequest {
    #[serde(flatten)]
    pub naming_request: NamingRequest,
    pub page_no: i32,
    pub page_size: i32,
    pub selector: String,
}

impl ServiceListRequest {
    pub fn new() -> Self {
        Self {
            naming_request: NamingRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ServiceListRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.naming_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServiceListRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.naming_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.naming_request.request_id()
    }
}

impl From<&Payload> for ServiceListRequest {
    fn from(value: &Payload) -> Self {
        ServiceListRequest::from_payload(value)
    }
}

// Service query request for querying service details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceQueryRequest {
    #[serde(flatten)]
    pub naming_request: NamingRequest,
    pub cluster: String,
    pub healthy_only: bool,
    pub udp_port: i32,
}

impl ServiceQueryRequest {
    pub fn new() -> Self {
        Self {
            naming_request: NamingRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ServiceQueryRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.naming_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "ServiceQueryRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.naming_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.naming_request.request_id()
    }
}

impl From<&Payload> for ServiceQueryRequest {
    fn from(value: &Payload) -> Self {
        ServiceQueryRequest::from_payload(value)
    }
}

// Subscribe service request for subscribing to service changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeServiceRequest {
    #[serde(flatten)]
    pub naming_request: NamingRequest,
    pub subscribe: bool,
    pub clusters: String,
}

impl SubscribeServiceRequest {
    pub fn new() -> Self {
        Self {
            naming_request: NamingRequest::new(),
            subscribe: true,
            ..Default::default()
        }
    }
}

impl RequestTrait for SubscribeServiceRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.naming_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "SubscribeServiceRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.naming_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.naming_request.request_id()
    }
}

impl From<&Payload> for SubscribeServiceRequest {
    fn from(value: &Payload) -> Self {
        SubscribeServiceRequest::from_payload(value)
    }
}

// Notify subscriber request for pushing service changes to subscribers
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifySubscriberRequest {
    #[serde(flatten)]
    pub server_request: ServerRequest,
    pub namespace: String,
    pub service_name: String,
    pub group_name: String,
    pub service_info: Service,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
    )]
    module: String,
}

impl NotifySubscriberRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for NotifySubscriberRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "NotifySubscriberRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_request.request_id()
    }
}

impl From<&Payload> for NotifySubscriberRequest {
    fn from(value: &Payload) -> Self {
        NotifySubscriberRequest::from_payload(value)
    }
}

// Fuzzy watch notify request base
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyWatchNotifyRequest {
    pub server_request: ServerRequest,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
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

impl RequestTrait for FuzzyWatchNotifyRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.server_request.headers()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.server_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.server_request.request_id()
    }
}

// Naming fuzzy watch request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingFuzzyWatchRequest {
    #[serde(flatten)]
    pub request: Request,
    pub namespace: String,
    pub service_name_pattern: String,
    pub group_name_pattern: String,
    pub received_service_keys: HashSet<String>,
    pub watch_type: String,
    pub is_initializing: bool,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
    )]
    module: String,
}

impl NamingFuzzyWatchRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for NamingFuzzyWatchRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn request_type(&self) -> &'static str {
        "NamingFuzzyWatchRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id()
    }
}

impl From<&Payload> for NamingFuzzyWatchRequest {
    fn from(value: &Payload) -> Self {
        NamingFuzzyWatchRequest::from_payload(value)
    }
}

// Naming fuzzy watch change notify context
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingContext {
    pub service_key: String,
    pub change_type: String,
}

// Naming fuzzy watch change notify request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingFuzzyWatchChangeNotifyRequest {
    #[serde(flatten)]
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    pub service_key: String,
    pub change_type: String,
}

impl NamingFuzzyWatchChangeNotifyRequest {
    pub fn new() -> Self {
        Self {
            fuzzy_watch_notify_request: FuzzyWatchNotifyRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for NamingFuzzyWatchChangeNotifyRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.fuzzy_watch_notify_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "NamingFuzzyWatchChangeNotifyRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.fuzzy_watch_notify_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.fuzzy_watch_notify_request.request_id()
    }
}

impl From<&Payload> for NamingFuzzyWatchChangeNotifyRequest {
    fn from(value: &Payload) -> Self {
        NamingFuzzyWatchChangeNotifyRequest::from_payload(value)
    }
}

// Naming fuzzy watch sync request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingFuzzyWatchSyncRequest {
    #[serde(flatten)]
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    pub pattern_namespace: String,
    pub pattern_service_name: String,
    pub pattern_group_name: String,
    pub sync_type: String,
    pub total_batch: i32,
    pub current_batch: i32,
    pub contexts: HashSet<NamingContext>,
}

impl NamingFuzzyWatchSyncRequest {
    pub fn new() -> Self {
        Self {
            fuzzy_watch_notify_request: FuzzyWatchNotifyRequest::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for NamingFuzzyWatchSyncRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.fuzzy_watch_notify_request.headers()
    }

    fn request_type(&self) -> &'static str {
        "NamingFuzzyWatchSyncRequest"
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.fuzzy_watch_notify_request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.fuzzy_watch_notify_request.request_id()
    }
}

impl From<&Payload> for NamingFuzzyWatchSyncRequest {
    fn from(value: &Payload) -> Self {
        NamingFuzzyWatchSyncRequest::from_payload(value)
    }
}

// ============== Response Types ==============

// Instance response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceResponse {
    #[serde(flatten)]
    pub response: Response,
    pub r#type: String,
}

impl InstanceResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for InstanceResponse {
    fn response_type(&self) -> &'static str {
        "InstanceResponse"
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

impl Into<Any> for InstanceResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Batch instance response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchInstanceResponse {
    #[serde(flatten)]
    pub response: Response,
    pub r#type: String,
}

impl BatchInstanceResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for BatchInstanceResponse {
    fn response_type(&self) -> &'static str {
        "BatchInstanceResponse"
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

impl Into<Any> for BatchInstanceResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Service list response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListResponse {
    #[serde(flatten)]
    pub response: Response,
    pub count: i32,
    pub service_names: Vec<String>,
}

impl ServiceListResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for ServiceListResponse {
    fn response_type(&self) -> &'static str {
        "ServiceListResponse"
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

impl Into<Any> for ServiceListResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Query service response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryServiceResponse {
    #[serde(flatten)]
    pub response: Response,
    pub service_info: Service,
}

impl QueryServiceResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for QueryServiceResponse {
    fn response_type(&self) -> &'static str {
        "QueryServiceResponse"
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

impl Into<Any> for QueryServiceResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Subscribe service response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeServiceResponse {
    #[serde(flatten)]
    pub response: Response,
    pub service_info: Service,
}

impl SubscribeServiceResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            ..Default::default()
        }
    }
}

impl ResponseTrait for SubscribeServiceResponse {
    fn response_type(&self) -> &'static str {
        "SubscribeServiceResponse"
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

impl Into<Any> for SubscribeServiceResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Notify subscriber response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifySubscriberResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl NotifySubscriberResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for NotifySubscriberResponse {
    fn response_type(&self) -> &'static str {
        "NotifySubscriberResponse"
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

impl Into<Any> for NotifySubscriberResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Naming fuzzy watch response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingFuzzyWatchResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl NamingFuzzyWatchResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for NamingFuzzyWatchResponse {
    fn response_type(&self) -> &'static str {
        "NamingFuzzyWatchResponse"
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

impl Into<Any> for NamingFuzzyWatchResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Naming fuzzy watch change notify response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingFuzzyWatchChangeNotifyResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl NamingFuzzyWatchChangeNotifyResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for NamingFuzzyWatchChangeNotifyResponse {
    fn response_type(&self) -> &'static str {
        "NamingFuzzyWatchChangeNotifyResponse"
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

impl Into<Any> for NamingFuzzyWatchChangeNotifyResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}

// Naming fuzzy watch sync response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingFuzzyWatchSyncResponse {
    #[serde(flatten)]
    pub response: Response,
}

impl NamingFuzzyWatchSyncResponse {
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

impl ResponseTrait for NamingFuzzyWatchSyncResponse {
    fn response_type(&self) -> &'static str {
        "NamingFuzzyWatchSyncResponse"
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

impl Into<Any> for NamingFuzzyWatchSyncResponse {
    fn into(self) -> Any {
        self.into_any()
    }
}
