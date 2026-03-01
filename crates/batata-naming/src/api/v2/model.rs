//! Request and response models for V2 Naming API
//!
//! These models follow the Nacos V2 API specification with camelCase JSON serialization.

use batata_common::impl_or_default;
use serde::{Deserialize, Serialize};

/// Default namespace ID used when none is specified
pub const DEFAULT_NAMESPACE_ID: &str = "public";

/// Default group name used when none is specified
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

fn default_page_no() -> u64 {
    1
}

fn default_service_page_size() -> u64 {
    20
}

// =============================================================================
// Naming API Models - Instance
// =============================================================================

/// Request parameters for registering an instance
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRegisterParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default)]
    pub cluster_name: Option<String>,
    /// Weight for load balancing (optional, defaults to 1.0)
    #[serde(default)]
    pub weight: Option<f64>,
    /// Whether instance is healthy (optional, defaults to true)
    #[serde(default)]
    pub healthy: Option<bool>,
    /// Whether instance is enabled (optional, defaults to true)
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Whether instance is ephemeral (optional, defaults to true)
    #[serde(default)]
    pub ephemeral: Option<bool>,
    /// Instance metadata as JSON string (optional)
    #[serde(default)]
    pub metadata: Option<String>,
}

impl InstanceRegisterParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(pub, cluster_name_or_default, cluster_name, "DEFAULT");
}

/// Request parameters for deregistering an instance
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDeregisterParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default)]
    pub cluster_name: Option<String>,
    /// Whether instance is ephemeral (optional, defaults to true)
    #[serde(default)]
    pub ephemeral: Option<bool>,
}

impl InstanceDeregisterParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(pub, cluster_name_or_default, cluster_name, "DEFAULT");
}

/// Request parameters for updating an instance
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpdateParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default)]
    pub cluster_name: Option<String>,
    /// Weight for load balancing (optional)
    #[serde(default)]
    pub weight: Option<f64>,
    /// Whether instance is healthy (optional)
    #[serde(default)]
    pub healthy: Option<bool>,
    /// Whether instance is enabled (optional)
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Whether instance is ephemeral (optional)
    #[serde(default)]
    pub ephemeral: Option<bool>,
    /// Instance metadata as JSON string (optional)
    #[serde(default)]
    pub metadata: Option<String>,
}

impl InstanceUpdateParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(pub, cluster_name_or_default, cluster_name, "DEFAULT");
}

/// Request parameters for getting instance detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDetailParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default)]
    pub cluster_name: Option<String>,
}

impl InstanceDetailParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(pub, cluster_name_or_default, cluster_name, "DEFAULT");
}

/// Request parameters for getting instance list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceListParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Cluster name filter (optional, comma-separated)
    #[serde(default, alias = "clusters")]
    pub cluster_name: Option<String>,
    /// Only return healthy instances (optional, defaults to false)
    #[serde(default)]
    pub healthy_only: Option<bool>,
    /// IP filter (optional)
    #[serde(default)]
    pub ip: Option<String>,
    /// Port filter (optional)
    #[serde(default)]
    pub port: Option<i32>,
    /// App name filter (optional)
    #[serde(default)]
    pub app: Option<String>,
}

impl InstanceListParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Request parameters for batch metadata update
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchMetadataParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Instance list as JSON (ip:port format, comma-separated)
    pub instances: String,
    /// Metadata to add/update (JSON string)
    pub metadata: String,
    /// Consistency type (optional)
    #[serde(default)]
    pub consistency_type: Option<String>,
}

impl BatchMetadataParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Response data for instance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceResponse {
    pub instance_id: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Response data for instance list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceListResponse {
    pub name: String,
    pub group_name: String,
    pub clusters: String,
    pub cache_millis: i64,
    pub hosts: Vec<InstanceResponse>,
    pub last_ref_time: i64,
    pub checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reach_protection_threshold: Option<bool>,
}

// =============================================================================
// Naming API Models - Service
// =============================================================================

/// Request parameters for creating a service
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCreateParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Protection threshold (optional, 0.0-1.0)
    #[serde(default)]
    pub protect_threshold: Option<f32>,
    /// Service metadata as JSON string (optional)
    #[serde(default)]
    pub metadata: Option<String>,
    /// Selector type (optional)
    #[serde(default)]
    pub selector: Option<String>,
    /// Whether service is ephemeral (optional, defaults to true)
    #[serde(default)]
    pub ephemeral: Option<bool>,
}

impl ServiceCreateParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Request parameters for deleting a service
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDeleteParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
}

impl ServiceDeleteParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Request parameters for updating a service
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceUpdateParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Protection threshold (optional, 0.0-1.0)
    #[serde(default)]
    pub protect_threshold: Option<f32>,
    /// Service metadata as JSON string (optional)
    #[serde(default)]
    pub metadata: Option<String>,
    /// Selector type (optional)
    #[serde(default)]
    pub selector: Option<String>,
}

impl ServiceUpdateParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Request parameters for getting service detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetailParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
}

impl ServiceDetailParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Request parameters for getting service list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    /// Page size (defaults to 10)
    #[serde(default = "default_service_page_size")]
    pub page_size: u64,
    /// Selector expression (optional)
    #[serde(default)]
    pub selector: Option<String>,
}

impl ServiceListParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Response data for service detail
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetailResponse {
    pub namespace: String,
    pub group_name: String,
    pub service_name: String,
    pub protect_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<SelectorResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_map: Option<std::collections::HashMap<String, serde_json::Value>>,
    pub ephemeral: bool,
}

/// Response data for selector
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectorResponse {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

/// Response data for service list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListResponse {
    pub count: i32,
    pub services: Vec<String>,
}

// =============================================================================
// Client API Models
// =============================================================================

/// Request parameters for getting client list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientListParam {
    /// Client type filter (optional)
    #[serde(default)]
    pub client_type: Option<String>,
}

/// Request parameters for getting client detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDetailParam {
    /// Client ID (required)
    pub client_id: String,
}

/// Request parameters for getting client published/subscribed services
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientServiceListParam {
    /// Client ID (required)
    pub client_id: String,
}

/// Request parameters for getting service publisher/subscriber list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceClientListParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
}

impl ServiceClientListParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Response data for client list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientListResponse {
    pub count: i32,
    pub client_ids: Vec<String>,
}

/// Response data for client detail
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDetailResponse {
    pub client_id: String,
    pub client_type: String,
    pub client_ip: String,
    pub client_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub create_time: i64,
    pub last_active_time: i64,
}

/// Published/Subscribed service info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientServiceInfo {
    pub namespace: String,
    pub group_name: String,
    pub service_name: String,
}

/// Response data for client published/subscribed services
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientServiceListResponse {
    pub count: i32,
    pub services: Vec<ClientServiceInfo>,
}

/// Publisher/Subscriber client info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceClientInfo {
    pub client_id: String,
    pub client_ip: String,
    pub client_port: u16,
}

/// Response data for service publisher/subscriber list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceClientListResponse {
    pub count: i32,
    pub clients: Vec<ServiceClientInfo>,
}

// =============================================================================
// Operator API Models
// =============================================================================

/// Request parameters for updating system switches
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchUpdateParam {
    /// Switch entry name (required)
    pub entry: String,
    /// Switch value (required)
    pub value: String,
    /// Enable debug mode (optional)
    #[serde(default)]
    pub debug: Option<bool>,
}

/// Response data for system switches
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchesResponse {
    /// Naming module name
    pub name: String,
    /// Master node address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masters: Option<String>,
    /// Address server domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addr_server_domain: Option<String>,
    /// Address server port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addr_server_port: Option<String>,
    /// Default push cache time in milliseconds
    pub default_push_cache_millis: i64,
    /// Client beat interval in milliseconds
    pub client_beat_interval: i64,
    /// Default cache time in milliseconds
    pub default_cache_millis: i64,
    /// Distro threshold
    pub distro_threshold: f32,
    /// Whether health check is enabled
    pub health_check_enabled: bool,
    /// Whether distro is enabled
    pub distro_enabled: bool,
    /// Whether push is enabled
    pub push_enabled: bool,
    /// Check times for health check
    pub check_times: i32,
    /// HTTP health params
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_health_params: Option<std::collections::HashMap<String, String>>,
    /// TCP health params
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_health_params: Option<std::collections::HashMap<String, String>>,
    /// MySQL health params
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mysql_health_params: Option<std::collections::HashMap<String, String>>,
    /// Incremental list flag
    pub incremental_list: bool,
    /// Servers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<String>>,
    /// Overridden server status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overridden_server_status: Option<String>,
    /// Default instance ephemeral flag
    pub default_instance_ephemeral: bool,
}

/// Response data for naming service metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingMetricsResponse {
    /// Total service count
    pub service_count: i32,
    /// Total instance count
    pub instance_count: i32,
    /// Total subscription count
    pub subscribe_count: i32,
    /// Cluster node count
    pub cluster_node_count: i32,
    /// Responsible service count (services this node is responsible for)
    pub responsible_service_count: i32,
    /// Responsible instance count
    pub responsible_instance_count: i32,
    /// CPU usage (percentage)
    pub cpu: f64,
    /// Memory usage (percentage)
    pub load: f64,
    /// Memory used in bytes
    pub mem: f64,
}

/// Request parameters for updating instance health
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceHealthParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default)]
    pub group_name: Option<String>,
    /// Service name (required)
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default)]
    pub cluster_name: Option<String>,
    /// Health status (required)
    pub healthy: bool,
}

impl InstanceHealthParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );

    impl_or_default!(pub, group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(pub, cluster_name_or_default, cluster_name, "DEFAULT");
}
