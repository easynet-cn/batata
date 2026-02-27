//! Request and response models for V2 API
//!
//! These models follow the Nacos V2 API specification with camelCase JSON serialization.

use serde::{Deserialize, Serialize};

/// Default namespace ID used when none is specified
pub const DEFAULT_NAMESPACE_ID: &str = "public";

/// Default group name used when none is specified
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

// =============================================================================
// Config API Models
// =============================================================================

/// Request parameters for getting a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGetParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Tag for config filtering (optional)
    #[serde(default)]
    pub tag: Option<String>,
}

impl ConfigGetParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for publishing a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPublishParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Config content (required)
    pub content: String,
    /// Tag for config (optional)
    #[serde(default)]
    pub tag: Option<String>,
    /// Application name (optional)
    #[serde(default)]
    pub app_name: Option<String>,
    /// Source user (optional)
    #[serde(default)]
    pub src_user: Option<String>,
    /// Config tags (comma-separated, optional)
    #[serde(default)]
    pub config_tags: Option<String>,
    /// Description (optional)
    #[serde(default)]
    pub desc: Option<String>,
    /// Usage information (optional)
    #[serde(default)]
    pub r#use: Option<String>,
    /// Effect type (optional)
    #[serde(default)]
    pub effect: Option<String>,
    /// Config type (e.g., "yaml", "json", "properties", optional)
    #[serde(default)]
    pub r#type: Option<String>,
    /// Schema for config validation (optional)
    #[serde(default)]
    pub schema: Option<String>,
    /// Beta IPs for gray release (optional)
    #[serde(default, alias = "betaIps")]
    pub beta_ips: Option<String>,
}

impl ConfigPublishParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for deleting a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDeleteParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Tag for config (optional)
    #[serde(default)]
    pub tag: Option<String>,
}

impl ConfigDeleteParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Response data for config retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    /// Config ID
    pub id: String,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Config content
    pub content: String,
    /// MD5 hash of content
    pub md5: String,
    /// Encrypted data key (if encrypted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Request parameters for searching config detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSearchDetailParam {
    /// Data ID filter (optional, supports wildcard *)
    #[serde(default)]
    pub data_id: Option<String>,
    /// Group filter (optional)
    #[serde(default)]
    pub group: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Application name filter (optional)
    #[serde(default)]
    pub app_name: Option<String>,
    /// Config tags filter (comma-separated, optional)
    #[serde(default)]
    pub config_tags: Option<String>,
    /// Config type filter (comma-separated, optional)
    #[serde(default)]
    pub config_type: Option<String>,
    /// Content search filter (optional)
    #[serde(default)]
    pub content: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    /// Page size (defaults to 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

impl ConfigSearchDetailParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

// =============================================================================
// History API Models
// =============================================================================

/// Request parameters for history list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryListParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    /// Page size (defaults to 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page_no() -> u64 {
    1
}

fn default_page_size() -> u64 {
    100
}

impl HistoryListParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for getting a specific history entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDetailParam {
    /// History entry ID (nid)
    pub nid: u64,
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
}

impl HistoryDetailParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for getting the previous history entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPreviousParam {
    /// Current history entry ID
    pub id: u64,
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
}

impl HistoryPreviousParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for getting configs in a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceConfigsParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
}

impl NamespaceConfigsParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Response data for history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemResponse {
    /// History entry ID
    pub id: String,
    /// Last modified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<i64>,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// MD5 hash of content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// Content (only in detail view)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Source IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    /// Source user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_user: Option<String>,
    /// Operation type (I=Insert, U=Update, D=Delete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_type: Option<String>,
    /// Publish type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<String>,
    /// Extended information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_info: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Creation time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    /// Last modified time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_time: Option<String>,
    /// Encrypted data key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
}

/// Response data for config info in namespace listing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoResponse {
    /// Config ID
    pub id: String,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
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
    #[serde(default)]
    pub clusters: Option<String>,
    /// Only return healthy instances (optional, defaults to false)
    #[serde(default)]
    pub healthy_only: Option<bool>,
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

fn default_service_page_size() -> u64 {
    10
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
    pub name: String,
    pub protect_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<SelectorResponse>,
    pub cluster_count: i32,
    pub ip_count: i32,
    pub healthy_instance_count: i32,
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

// =============================================================================
// Cluster API Models
// =============================================================================

/// Request parameters for node list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeListParam {
    /// Filter by address keyword (optional)
    #[serde(default)]
    pub keyword: Option<String>,
}

/// Request parameters for switching lookup mode
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupSwitchParam {
    /// Lookup type: "file" or "address-server"
    pub r#type: String,
}

/// Response data for current node (self)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSelfResponse {
    /// Node IP address
    pub ip: String,
    /// Node port
    pub port: u16,
    /// Full address (ip:port)
    pub address: String,
    /// Node state (UP, DOWN, STARTING, etc.)
    pub state: String,
    /// Extended information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// Fail access count
    pub fail_access_cnt: i32,
    /// Abilities of this node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abilities: Option<NodeAbilities>,
}

/// Node abilities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeAbilities {
    /// Naming service abilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naming_ability: Option<NamingAbility>,
    /// Config service abilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_ability: Option<ConfigAbility>,
}

/// Naming service abilities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingAbility {
    /// Support gRPC push
    pub support_push: bool,
    /// Support remote delta updates
    pub support_delta_push: bool,
}

/// Config service abilities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAbility {
    /// Support remote config
    pub support_remote: bool,
}

/// Response data for node in list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeResponse {
    /// Node IP address
    pub ip: String,
    /// Node port
    pub port: u16,
    /// Full address (ip:port)
    pub address: String,
    /// Node state (UP, DOWN, STARTING, etc.)
    pub state: String,
    /// Extended information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// Fail access count
    pub fail_access_cnt: i32,
}

/// Response data for node health
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeHealthResponse {
    /// Whether this node is healthy
    pub healthy: bool,
}

/// Response data for lookup switch
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupSwitchResponse {
    /// Whether the switch was successful
    pub success: bool,
    /// Current lookup type
    pub current_type: String,
}

// =============================================================================
// Namespace API Models
// =============================================================================

/// Request parameters for getting namespace detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceDetailParam {
    /// Namespace ID (required)
    pub namespace_id: String,
}

/// Request parameters for creating a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceCreateParam {
    /// Custom namespace ID (optional, will generate UUID if not provided)
    #[serde(default)]
    pub custom_namespace_id: Option<String>,
    /// Namespace ID (alternative to custom_namespace_id)
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Namespace name (required)
    pub namespace_name: String,
    /// Namespace description (optional)
    #[serde(default)]
    pub namespace_desc: Option<String>,
}

impl NamespaceCreateParam {
    /// Get the namespace ID, preferring custom_namespace_id over namespace_id
    pub fn get_namespace_id(&self) -> Option<&str> {
        self.custom_namespace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| self.namespace_id.as_deref().filter(|s| !s.is_empty()))
    }
}

/// Request parameters for updating a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceUpdateParam {
    /// Namespace ID (required)
    pub namespace_id: String,
    /// Namespace name (required)
    pub namespace_name: String,
    /// Namespace description (optional)
    #[serde(default)]
    pub namespace_desc: Option<String>,
}

/// Request parameters for deleting a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceDeleteParam {
    /// Namespace ID (required)
    pub namespace_id: String,
}

/// Response data for namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceResponse {
    /// Namespace ID
    pub namespace: String,
    /// Namespace display name
    pub namespace_show_name: String,
    /// Namespace description
    pub namespace_desc: String,
    /// Quota (max configs allowed)
    pub quota: i32,
    /// Current config count
    pub config_count: i32,
    /// Namespace type (0=global, 1=default, 2=custom)
    #[serde(rename = "type")]
    pub type_: i32,
}
