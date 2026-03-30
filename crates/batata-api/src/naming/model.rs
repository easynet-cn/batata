// Naming module API models for service discovery
// This file defines request/response structures for service registration, discovery, and subscription

use std::collections::{HashMap, HashSet};

use prost_types::Any;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
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

fn deserialize_naming_module<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let _: serde::de::IgnoredAny = serde::Deserialize::deserialize(deserializer)?;
    Ok(NAMING_MODULE.to_string())
}

// Instance type constants
pub const INSTANCE_TYPE_EPHEMERAL: &str = "ephemeral";
pub const INSTANCE_TYPE_PERSISTENT: &str = "persistent";

/// Registration source for service instances.
/// Used to isolate instances registered via different protocols.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RegisterSource {
    /// Registered via Batata (Nacos-compatible) SDK or HTTP V2 API
    #[default]
    Batata,
    /// Registered via Consul compatibility API
    Consul,
}

// Preserved metadata keys (Nacos stores heartbeat timeouts as metadata, not top-level fields)
pub const PRESERVED_HEART_BEAT_INTERVAL: &str = "preserved.heart.beat.interval";
pub const PRESERVED_HEART_BEAT_TIMEOUT: &str = "preserved.heart.beat.timeout";
pub const PRESERVED_IP_DELETE_TIMEOUT: &str = "preserved.ip.delete.timeout";
pub const PRESERVED_INSTANCE_ID_GENERATOR: &str = "preserved.instance.id.generator";
pub const PRESERVED_REGISTER_SOURCE: &str = "preserved.register.source";

// Request type constants
pub const REGISTER_INSTANCE: &str = "registerInstance";
pub const DE_REGISTER_INSTANCE: &str = "deregisterInstance";
pub const BATCH_REGISTER_INSTANCE: &str = "batchRegisterInstance";
pub const BATCH_DE_REGISTER_INSTANCE: &str = "batchDeregisterInstance";

// Service instance information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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
    /// Registration source — used to isolate instances from different protocols
    pub register_source: RegisterSource,
}

/// Clamp weight to valid range [0.0, 10000.0], defaulting to 1.0 for negative values
pub fn clamp_weight(weight: f64) -> f64 {
    if weight < crate::model::MIN_WEIGHT_VALUE {
        crate::model::DEFAULT_INSTANCE_WEIGHT
    } else if weight > crate::model::MAX_WEIGHT_VALUE {
        crate::model::MAX_WEIGHT_VALUE
    } else {
        weight
    }
}

/// Generate the client-facing instance ID: ip#port#clusterName#serviceName
pub fn generate_instance_id(ip: &str, port: i32, cluster_name: &str, service_name: &str) -> String {
    format!("{}#{}#{}#{}", ip, port, cluster_name, service_name)
}

impl Instance {
    pub fn new(ip: String, port: i32) -> Self {
        Self {
            ip,
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            ..Default::default()
        }
    }

    /// Generate instance key for internal map storage (service-scoped)
    pub fn key(&self) -> String {
        format!("{}#{}#{}", self.ip, self.port, self.cluster_name)
    }

    /// Get heartbeat interval from preserved metadata (default 5000ms)
    pub fn get_heartbeat_interval(&self) -> i64 {
        self.get_metadata_i64(
            PRESERVED_HEART_BEAT_INTERVAL,
            crate::model::DEFAULT_HEART_BEAT_INTERVAL,
        )
    }

    /// Get heartbeat timeout from preserved metadata (default 15000ms)
    pub fn get_heartbeat_timeout(&self) -> i64 {
        self.get_metadata_i64(
            PRESERVED_HEART_BEAT_TIMEOUT,
            crate::model::DEFAULT_HEART_BEAT_TIMEOUT,
        )
    }

    /// Get IP delete timeout from preserved metadata (default 30000ms)
    pub fn get_ip_delete_timeout(&self) -> i64 {
        self.get_metadata_i64(
            PRESERVED_IP_DELETE_TIMEOUT,
            crate::model::DEFAULT_IP_DELETE_TIMEOUT,
        )
    }

    /// Get instance ID generator from preserved metadata (default "simple")
    pub fn get_instance_id_generator(&self) -> String {
        self.metadata
            .get(PRESERVED_INSTANCE_ID_GENERATOR)
            .cloned()
            .unwrap_or_else(|| crate::model::DEFAULT_INSTANCE_ID_GENERATOR.to_string())
    }

    fn get_metadata_i64(&self, key: &str, default: i64) -> i64 {
        self.metadata
            .get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
}

// Service information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

impl Service {
    pub fn new(name: String, group_name: String) -> Self {
        Self {
            name,
            group_name,
            cache_millis: 1000,
            ..Default::default()
        }
    }

    /// Get healthy instances
    pub fn healthy_hosts(&self) -> Vec<&Instance> {
        self.hosts
            .iter()
            .filter(|h| h.healthy && h.enabled)
            .collect()
    }
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
#[serde(rename_all = "camelCase", default)]
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

impl_request_trait!(base NamingRequest, request);

// Instance registration/deregistration request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

impl_request_trait!(InstanceRequest, naming_request);

impl From<&Payload> for InstanceRequest {
    fn from(value: &Payload) -> Self {
        InstanceRequest::from_payload(value)
    }
}

// Batch instance request for multiple instances
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

impl_request_trait!(BatchInstanceRequest, naming_request);

impl From<&Payload> for BatchInstanceRequest {
    fn from(value: &Payload) -> Self {
        BatchInstanceRequest::from_payload(value)
    }
}

// Persistent instance request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

impl_request_trait!(PersistentInstanceRequest, naming_request);

impl From<&Payload> for PersistentInstanceRequest {
    fn from(value: &Payload) -> Self {
        PersistentInstanceRequest::from_payload(value)
    }
}

// Service list request for listing services
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

impl_request_trait!(ServiceListRequest, naming_request);

impl From<&Payload> for ServiceListRequest {
    fn from(value: &Payload) -> Self {
        ServiceListRequest::from_payload(value)
    }
}

// Service query request for querying service details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

impl_request_trait!(ServiceQueryRequest, naming_request);

impl From<&Payload> for ServiceQueryRequest {
    fn from(value: &Payload) -> Self {
        ServiceQueryRequest::from_payload(value)
    }
}

// Subscribe service request for subscribing to service changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

impl_request_trait!(SubscribeServiceRequest, naming_request);

impl From<&Payload> for SubscribeServiceRequest {
    fn from(value: &Payload) -> Self {
        SubscribeServiceRequest::from_payload(value)
    }
}

// Notify subscriber request for pushing service changes to subscribers
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

    /// Create a notification for a service change
    pub fn for_service(
        namespace: &str,
        group_name: &str,
        service_name: &str,
        service_info: Service,
    ) -> Self {
        Self {
            server_request: ServerRequest::new(),
            namespace: namespace.to_string(),
            group_name: group_name.to_string(),
            service_name: service_name.to_string(),
            service_info,
            module: "naming".to_string(),
        }
    }
}

impl_request_trait!(NotifySubscriberRequest, server_request);

impl From<&Payload> for NotifySubscriberRequest {
    fn from(value: &Payload) -> Self {
        NotifySubscriberRequest::from_payload(value)
    }
}

// Fuzzy watch notify request base
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FuzzyWatchNotifyRequest {
    #[serde(flatten)]
    pub server_request: ServerRequest,
    #[serde(
        serialize_with = "serialize_naming_module",
        deserialize_with = "deserialize_naming_module"
    )]
    module: String,
    /// Sync type: "FUZZY_WATCH_RESOURCE_CHANGED" for change notifications
    pub sync_type: String,
}

impl FuzzyWatchNotifyRequest {
    pub fn new() -> Self {
        Self {
            server_request: ServerRequest::new(),
            sync_type: crate::model::FUZZY_WATCH_RESOURCE_CHANGED.to_string(),
            ..Default::default()
        }
    }
}

impl_request_trait!(base FuzzyWatchNotifyRequest, server_request);

// Naming fuzzy watch request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NamingFuzzyWatchRequest {
    #[serde(flatten)]
    pub request: Request,
    pub namespace: String,
    pub service_name_pattern: String,
    pub group_name_pattern: String,
    #[serde(default, alias = "groupKeyPattern")]
    pub group_key_pattern: String,
    #[serde(default, alias = "receivedGroupKeys")]
    pub received_service_keys: HashSet<String>,
    pub watch_type: String,
    /// Jackson serializes Java `boolean isInitializing` as `initializing` (strips `is` prefix)
    #[serde(alias = "isInitializing")]
    pub initializing: bool,
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

impl_request_trait!(NamingFuzzyWatchRequest, request);

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
    /// Named "changedType" in Java SDK (not "changeType")
    #[serde(rename = "changedType")]
    pub changed_type: String,
}

// Naming fuzzy watch change notify request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NamingFuzzyWatchChangeNotifyRequest {
    #[serde(flatten)]
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    pub service_key: String,
    #[serde(rename = "changedType")]
    pub changed_type: String,
}

impl NamingFuzzyWatchChangeNotifyRequest {
    pub fn new() -> Self {
        Self {
            fuzzy_watch_notify_request: FuzzyWatchNotifyRequest::new(),
            ..Default::default()
        }
    }
}

impl_request_trait!(
    NamingFuzzyWatchChangeNotifyRequest,
    fuzzy_watch_notify_request
);

impl From<&Payload> for NamingFuzzyWatchChangeNotifyRequest {
    fn from(value: &Payload) -> Self {
        NamingFuzzyWatchChangeNotifyRequest::from_payload(value)
    }
}

// Naming fuzzy watch sync request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NamingFuzzyWatchSyncRequest {
    #[serde(flatten)]
    pub fuzzy_watch_notify_request: FuzzyWatchNotifyRequest,
    /// Combined pattern in format "namespace>>group>>service" (Java SDK field name)
    pub group_key_pattern: String,
    /// Kept for backward compatibility with internal code
    #[serde(skip_serializing)]
    pub pattern_namespace: String,
    #[serde(skip_serializing)]
    pub pattern_service_name: String,
    #[serde(skip_serializing)]
    pub pattern_group_name: String,
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

impl_request_trait!(NamingFuzzyWatchSyncRequest, fuzzy_watch_notify_request);

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

impl_response_trait!(InstanceResponse);

impl From<InstanceResponse> for Any {
    fn from(val: InstanceResponse) -> Self {
        val.to_any()
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

impl_response_trait!(BatchInstanceResponse);

impl From<BatchInstanceResponse> for Any {
    fn from(val: BatchInstanceResponse) -> Self {
        val.to_any()
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

impl_response_trait!(ServiceListResponse);

impl From<ServiceListResponse> for Any {
    fn from(val: ServiceListResponse) -> Self {
        val.to_any()
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

impl_response_trait!(QueryServiceResponse);

impl From<QueryServiceResponse> for Any {
    fn from(val: QueryServiceResponse) -> Self {
        val.to_any()
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

impl_response_trait!(SubscribeServiceResponse);

impl From<SubscribeServiceResponse> for Any {
    fn from(val: SubscribeServiceResponse) -> Self {
        val.to_any()
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

impl_response_trait!(NotifySubscriberResponse);

impl From<NotifySubscriberResponse> for Any {
    fn from(val: NotifySubscriberResponse) -> Self {
        val.to_any()
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

impl_response_trait!(NamingFuzzyWatchResponse);

impl From<NamingFuzzyWatchResponse> for Any {
    fn from(val: NamingFuzzyWatchResponse) -> Self {
        val.to_any()
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

impl_response_trait!(NamingFuzzyWatchChangeNotifyResponse);

impl From<NamingFuzzyWatchChangeNotifyResponse> for Any {
    fn from(val: NamingFuzzyWatchChangeNotifyResponse) -> Self {
        val.to_any()
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

impl_response_trait!(NamingFuzzyWatchSyncResponse);

impl From<NamingFuzzyWatchSyncResponse> for Any {
    fn from(val: NamingFuzzyWatchSyncResponse) -> Self {
        val.to_any()
    }
}

// ============== Form Types for HTTP API ==============

/// Query parameters for service discovery
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServiceQuery {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub clusters: String,
    pub healthy_only: bool,
}

/// Instance registration parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InstanceRegisterForm {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub enabled: bool,
    pub healthy: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub metadata: Option<String>,
}

impl InstanceRegisterForm {
    pub fn to_instance(&self) -> Instance {
        let metadata: HashMap<String, String> = self
            .metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or_default();

        let cluster_name = if self.cluster_name.is_empty() {
            "DEFAULT".to_string()
        } else {
            self.cluster_name.clone()
        };

        Instance {
            instance_id: generate_instance_id(
                &self.ip,
                self.port,
                &cluster_name,
                &self.service_name,
            ),
            ip: self.ip.clone(),
            port: self.port,
            weight: if self.weight <= 0.0 { 1.0 } else { self.weight },
            enabled: self.enabled,
            healthy: self.healthy,
            ephemeral: self.ephemeral,
            cluster_name,
            service_name: self.service_name.clone(),
            metadata,
            register_source: RegisterSource::default(),
        }
    }
}

/// Instance heartbeat parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HeartbeatForm {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub cluster_name: String,
    pub ip: String,
    pub port: i32,
    pub beat: Option<String>,
}

/// Create cluster form
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateClusterForm {
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    #[serde(alias = "groupName")]
    pub group_name: Option<String>,
    #[serde(alias = "serviceName")]
    pub service_name: String,
    #[serde(alias = "clusterName")]
    pub cluster_name: String,
    #[serde(alias = "healthChecker")]
    pub health_checker: Option<HealthCheckerConfigForm>,
    pub metadata: Option<HashMap<String, String>>,
}

/// Health checker configuration form
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HealthCheckerConfigForm {
    #[serde(default = "default_health_check_type")]
    pub r#type: String,
    #[serde(alias = "checkPort")]
    pub check_port: Option<i32>,
    #[serde(alias = "useInstancePort")]
    pub use_instance_port: Option<bool>,
    pub path: Option<String>,
    pub headers: Option<String>,
    #[serde(alias = "expectedCode")]
    pub expected_code: Option<String>,
}

fn default_health_check_type() -> String {
    "TCP".to_string()
}

// ============== Service-level Types ==============

/// Service-level metadata stored separately from instances
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ServiceMetadata {
    /// Protection threshold (0.0 to 1.0)
    pub protect_threshold: f32,
    /// Service metadata key-value pairs
    pub metadata: HashMap<String, String>,
    /// Service selector type (e.g., "none", "label")
    pub selector_type: String,
    /// Service selector expression
    pub selector_expression: String,
    /// Whether this service's instances are ephemeral by default
    pub ephemeral: bool,
    /// Revision counter for change detection / cache invalidation
    pub revision: u64,
}

/// Protection threshold information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProtectionInfo {
    /// Configured protection threshold (0.0 to 1.0)
    pub threshold: f32,
    /// Total number of instances
    pub total_instances: usize,
    /// Number of healthy instances
    pub healthy_instances: usize,
    /// Current healthy ratio (healthy_instances / total_instances)
    pub healthy_ratio: f32,
    /// Whether protection threshold was triggered
    pub triggered: bool,
}

impl ProtectionInfo {
    /// Check if the service is degraded (protection triggered)
    pub fn is_degraded(&self) -> bool {
        self.triggered
    }

    /// Get the number of unhealthy instances
    pub fn unhealthy_instances(&self) -> usize {
        self.total_instances.saturating_sub(self.healthy_instances)
    }
}

/// Cluster statistics information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterStatistics {
    /// Cluster name
    pub cluster_name: String,
    /// Total number of instances
    pub total_instances: usize,
    /// Number of healthy instances
    pub healthy_instances: usize,
    /// Number of unhealthy instances
    pub unhealthy_instances: usize,
    /// Number of disabled instances
    pub disabled_instances: usize,
    /// Healthy ratio (healthy_instances / enabled_instances)
    pub healthy_ratio: f32,
    /// Number of enabled instances
    pub enabled_instances: usize,
}

impl ClusterStatistics {
    /// Calculate cluster statistics from instances
    pub fn from_instances(cluster_name: &str, instances: &[Instance]) -> Self {
        Self::from_instance_refs(cluster_name, &instances.iter().collect::<Vec<_>>())
    }

    /// Calculate cluster statistics from instance references
    pub fn from_instance_refs(cluster_name: &str, instances: &[&Instance]) -> Self {
        let total = instances.len();
        let healthy = instances.iter().filter(|i| i.healthy && i.enabled).count();
        let unhealthy = instances.iter().filter(|i| !i.healthy && i.enabled).count();
        let disabled = instances.iter().filter(|i| !i.enabled).count();
        let enabled = instances.iter().filter(|i| i.enabled).count();

        let healthy_ratio = if enabled > 0 {
            healthy as f32 / enabled as f32
        } else {
            0.0
        };

        Self {
            cluster_name: cluster_name.to_string(),
            total_instances: total,
            healthy_instances: healthy,
            unhealthy_instances: unhealthy,
            disabled_instances: disabled,
            healthy_ratio,
            enabled_instances: enabled,
        }
    }
}

/// Cluster-level configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Cluster name
    pub name: String,
    /// Parent service name
    pub service_name: String,
    /// Health checker type (e.g., "TCP", "HTTP", "NONE")
    pub health_check_type: String,
    /// Health check port
    pub check_port: i32,
    /// Default port for instances in this cluster
    pub default_port: i32,
    /// Use instance port for health check
    pub use_instance_port: bool,
    /// Cluster metadata
    pub metadata: HashMap<String, String>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            name: "DEFAULT".to_string(),
            service_name: String::new(),
            health_check_type: "TCP".to_string(),
            check_port: 80,
            default_port: 80,
            use_instance_port: true,
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_default() {
        let instance = Instance::default();
        assert!(instance.ip.is_empty());
        assert_eq!(instance.port, 0);
        assert_eq!(instance.weight, 0.0);
        assert!(!instance.healthy);
        assert!(!instance.enabled);
    }

    #[test]
    fn test_instance_new() {
        let instance = Instance::new("192.168.1.1".to_string(), 8080);
        assert_eq!(instance.ip, "192.168.1.1");
        assert_eq!(instance.port, 8080);
        assert_eq!(instance.weight, 1.0);
        assert!(instance.healthy);
        assert!(instance.enabled);
        assert!(instance.ephemeral);
    }

    #[test]
    fn test_instance_key() {
        let instance = Instance {
            ip: "192.168.1.1".to_string(),
            port: 8080,
            ..Default::default()
        };
        let key = instance.key();
        assert!(key.contains("192.168.1.1"));
        assert!(key.contains("8080"));
    }

    #[test]
    fn test_instance_weight_clamping() {
        // Negative values get clamped to DEFAULT_INSTANCE_WEIGHT (1.0)
        assert_eq!(clamp_weight(-1.0), 1.0);
        // Zero is at the boundary (not less than MIN_WEIGHT_VALUE 0.0)
        assert_eq!(clamp_weight(0.0), 0.0);
        // Normal values pass through
        assert_eq!(clamp_weight(0.01), 0.01);
        assert_eq!(clamp_weight(1.0), 1.0);
        assert_eq!(clamp_weight(10000.0), 10000.0);
        // Over max gets clamped
        assert_eq!(clamp_weight(10001.0), 10000.0);
        assert_eq!(clamp_weight(99999.0), 10000.0);
    }

    #[test]
    fn test_instance_heartbeat_intervals() {
        let instance = Instance::new("127.0.0.1".to_string(), 8080);
        assert_eq!(instance.get_heartbeat_interval(), 5000);
        assert_eq!(instance.get_heartbeat_timeout(), 15000);
        assert_eq!(instance.get_ip_delete_timeout(), 30000);
    }

    #[test]
    fn test_instance_custom_heartbeat() {
        let mut instance = Instance::new("127.0.0.1".to_string(), 8080);
        instance.metadata.insert(
            "preserved.heart.beat.interval".to_string(),
            "3000".to_string(),
        );
        assert_eq!(instance.get_heartbeat_interval(), 3000);
    }

    #[test]
    fn test_service_creation() {
        let service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            clusters: "DEFAULT".to_string(),
            ..Default::default()
        };
        assert_eq!(service.name, "test-service");
    }

    #[test]
    fn test_service_healthy_hosts() {
        let mut service = Service::default();
        service.hosts.push(Instance {
            ip: "10.0.0.1".to_string(),
            port: 8080,
            healthy: true,
            enabled: true,
            ..Default::default()
        });
        service.hosts.push(Instance {
            ip: "10.0.0.2".to_string(),
            port: 8080,
            healthy: false,
            enabled: true,
            ..Default::default()
        });
        let healthy = service.healthy_hosts();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].ip, "10.0.0.1");
    }

    #[test]
    fn test_generate_instance_id() {
        let id = generate_instance_id("192.168.1.1", 8080, "DEFAULT", "test-service");
        assert!(!id.is_empty());
        assert!(id.contains("192.168.1.1"));
        assert!(id.contains("8080"));
        assert!(id.contains("DEFAULT"));
        assert!(id.contains("test-service"));
    }

    #[test]
    fn test_service_new() {
        let service = Service::new("my-service".to_string(), "DEFAULT_GROUP".to_string());
        assert_eq!(service.name, "my-service");
        assert_eq!(service.group_name, "DEFAULT_GROUP");
        assert_eq!(service.cache_millis, 1000);
    }

    #[test]
    fn test_instance_id_generator_default() {
        let instance = Instance::new("127.0.0.1".to_string(), 8080);
        assert_eq!(instance.get_instance_id_generator(), "simple");
    }

    #[test]
    fn test_instance_register_form_to_instance() {
        let form = InstanceRegisterForm {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-svc".to_string(),
            ip: "10.0.0.1".to_string(),
            port: 8080,
            weight: 1.5,
            enabled: true,
            healthy: true,
            ephemeral: true,
            cluster_name: "".to_string(),
            metadata: None,
        };
        let instance = form.to_instance();
        assert_eq!(instance.ip, "10.0.0.1");
        assert_eq!(instance.port, 8080);
        assert_eq!(instance.weight, 1.5);
        assert_eq!(instance.cluster_name, "DEFAULT");
    }
}
