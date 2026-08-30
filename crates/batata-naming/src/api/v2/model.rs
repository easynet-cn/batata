//! Request and response models for V2 Naming API
//!
//! These models follow the Nacos V2 API specification with camelCase JSON serialization.

use batata_common::{
    DEFAULT_GROUP, DEFAULT_NAMESPACE_ID, default_page_no, default_page_size_small, impl_or_default,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Naming API Models - Instance
// =============================================================================

/// Request parameters for registering an instance
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRegisterParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default, alias = "clusterName")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default, alias = "clusterName")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default, alias = "clusterName")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default, alias = "clusterName")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Cluster name filter (optional, comma-separated)
    #[serde(default, alias = "clusters", alias = "clusterName")]
    pub cluster_name: Option<String>,
    /// Only return healthy instances (optional, defaults to false)
    #[serde(default, alias = "healthyOnly")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Instance list as JSON (ip:port format, comma-separated)
    pub instances: String,
    /// Metadata to add/update (JSON string)
    pub metadata: String,
    /// Consistency type (optional)
    #[serde(default, alias = "consistencyType")]
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
    /// The `instance_id` value.
    pub instance_id: String,
    /// The `ip` value.
    pub ip: String,
    /// The `port` value.
    pub port: i32,
    /// The `weight` value.
    pub weight: f64,
    /// The `healthy` value.
    pub healthy: bool,
    /// The `enabled` value.
    pub enabled: bool,
    /// The `ephemeral` value.
    pub ephemeral: bool,
    /// The `cluster_name` value.
    pub cluster_name: String,
    /// The `service_name` value.
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `metadata` value.
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Response data for instance list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceListResponse {
    /// The `name` value.
    pub name: String,
    /// The `group_name` value.
    pub group_name: String,
    /// The `clusters` value.
    pub clusters: String,
    /// The `cache_millis` value.
    pub cache_millis: i64,
    /// The `hosts` value.
    pub hosts: Vec<InstanceResponse>,
    /// The `last_ref_time` value.
    pub last_ref_time: i64,
    /// The `checksum` value.
    pub checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `reach_protection_threshold` value.
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Protection threshold (optional, 0.0-1.0)
    #[serde(default, alias = "protectThreshold")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Protection threshold (optional, 0.0-1.0)
    #[serde(default, alias = "protectThreshold")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    /// Page size (defaults to 10)
    #[serde(default = "default_page_size_small", alias = "pageSize")]
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
    /// The `namespace` value.
    pub namespace: String,
    /// The `group_name` value.
    pub group_name: String,
    /// The `service_name` value.
    pub service_name: String,
    /// The `protect_threshold` value.
    pub protect_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `metadata` value.
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `selector` value.
    pub selector: Option<SelectorResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `cluster_map` value.
    pub cluster_map: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// The `ephemeral` value.
    pub ephemeral: bool,
}

/// Response data for selector
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectorResponse {
    /// The `type` value.
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `expression` value.
    pub expression: Option<String>,
}

/// Response data for service list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListResponse {
    /// The `count` value.
    pub count: i32,
    /// The `services` value.
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
    #[serde(default, alias = "clientType")]
    pub client_type: Option<String>,
}

/// Request parameters for getting client detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDetailParam {
    /// Client ID (required)
    #[serde(alias = "clientId")]
    pub client_id: String,
}

/// Request parameters for getting client published/subscribed services
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientServiceListParam {
    /// Client ID (required)
    #[serde(alias = "clientId")]
    pub client_id: String,
}

/// Request parameters for getting service publisher/subscriber list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceClientListParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
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
    /// The `count` value.
    pub count: i32,
    /// The `client_ids` value.
    pub client_ids: Vec<String>,
}

/// Response data for client detail
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDetailResponse {
    /// The `client_id` value.
    pub client_id: String,
    /// The `client_type` value.
    pub client_type: String,
    /// The `client_ip` value.
    pub client_ip: String,
    /// The `client_port` value.
    pub client_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `connect_type` value.
    pub connect_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `app_name` value.
    pub app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `version` value.
    pub version: Option<String>,
    /// The `create_time` value.
    pub create_time: i64,
    /// The `last_active_time` value.
    pub last_active_time: i64,
}

/// Published/Subscribed service info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientServiceInfo {
    /// The `namespace` value.
    pub namespace: String,
    /// The `group_name` value.
    pub group_name: String,
    /// The `service_name` value.
    pub service_name: String,
}

/// Response data for client published/subscribed services
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientServiceListResponse {
    /// The `count` value.
    pub count: i32,
    /// The `services` value.
    pub services: Vec<ClientServiceInfo>,
}

/// Publisher/Subscriber client info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceClientInfo {
    /// The `client_id` value.
    pub client_id: String,
    /// The `client_ip` value.
    pub client_ip: String,
    /// The `client_port` value.
    pub client_port: u16,
}

/// Response data for service publisher/subscriber list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceClientListResponse {
    /// The `count` value.
    pub count: i32,
    /// The `clients` value.
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

/// Response data for system switches (matches Nacos SwitchDomain)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchesResponse {
    /// Fixed constant: UtilsAndCommons.SWITCH_DOMAIN_NAME
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `masters` value.
    pub masters: Option<Vec<String>>,
    /// The `default_push_cache_millis` value.
    pub default_push_cache_millis: i64,
    /// The `client_beat_interval` value.
    pub client_beat_interval: i64,
    /// The `default_cache_millis` value.
    pub default_cache_millis: i64,
    /// The `distro_threshold` value.
    pub distro_threshold: f32,
    /// The `health_check_enabled` value.
    pub health_check_enabled: bool,
    /// The `auto_change_health_check_enabled` value.
    pub auto_change_health_check_enabled: bool,
    /// The `distro_enabled` value.
    pub distro_enabled: bool,
    /// The `enable_standalone` value.
    pub enable_standalone: bool,
    /// The `push_enabled` value.
    pub push_enabled: bool,
    /// The `check_times` value.
    pub check_times: i32,
    /// The `http_health_params` value.
    pub http_health_params: SwitchHealthParams,
    /// The `tcp_health_params` value.
    pub tcp_health_params: SwitchHealthParams,
    /// The `mysql_health_params` value.
    pub mysql_health_params: SwitchHealthParams,
    /// The `incremental_list` value.
    pub incremental_list: Vec<String>,
    /// The `default_instance_ephemeral` value.
    pub default_instance_ephemeral: bool,
    /// The `light_beat_enabled` value.
    pub light_beat_enabled: bool,
    /// The `disable_add_ip` value.
    pub disable_add_ip: bool,
    /// The `send_beat_only` value.
    pub send_beat_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `overridden_server_status` value.
    pub overridden_server_status: Option<String>,
}

/// Health check timing params (matches Nacos SwitchDomain.HealthParams)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchHealthParams {
    /// The `max` value.
    pub max: i32,
    /// The `min` value.
    pub min: i32,
    /// The `factor` value.
    pub factor: f32,
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
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Group name (optional, defaults to "DEFAULT_GROUP")
    #[serde(default, alias = "groupName")]
    pub group_name: Option<String>,
    /// Service name (required)
    #[serde(alias = "serviceName")]
    pub service_name: String,
    /// Instance IP (required)
    pub ip: String,
    /// Instance port (required)
    pub port: i32,
    /// Cluster name (optional, defaults to "DEFAULT")
    #[serde(default, alias = "clusterName")]
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
