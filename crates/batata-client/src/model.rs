//! Client model types
//!
//! This module defines data structures used by the client for API responses.

use serde::{Deserialize, Serialize};

/// Generic API response wrapper
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    /// Response code (0 indicates success).
    pub code: i32,
    /// Response message from the server.
    pub message: String,
    /// Response payload.
    pub data: T,
}

/// A boolean-like type that also accepts string "ok" from V3 console APIs.
/// Many Nacos V3 console endpoints return `"ok"` instead of `true`.
#[derive(Debug)]
pub struct OkOrBool(pub bool);

impl<'de> serde::Deserialize<'de> for OkOrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Bool(b) => Ok(OkOrBool(b)),
            serde_json::Value::String(s) => Ok(OkOrBool(
                s == "ok" || s == "true" || s.contains("ok") || s.contains("success"),
            )),
            _ => Ok(OkOrBool(false)),
        }
    }
}

/// Namespace information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    /// Namespace ID.
    pub namespace: String,
    /// Display name of the namespace.
    pub namespace_show_name: String,
    /// Namespace description.
    pub namespace_desc: String,
    /// Config quota for the namespace.
    pub quota: i32,
    /// Number of configs in the namespace.
    pub config_count: i32,
    #[serde(rename = "type")]
    /// Namespace type.
    pub type_: i32,
}

/// Basic configuration info for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBasicInfo {
    /// Config ID.
    pub id: i64,
    /// Namespace ID.
    pub namespace_id: String,
    /// Group name.
    pub group_name: String,
    /// Config data ID.
    pub data_id: String,
    /// MD5 hash of the config content.
    pub md5: String,
    /// Config type (e.g. "text", "yaml", "json").
    pub r#type: String,
    /// Owning application name.
    pub app_name: String,
    /// Creation timestamp (millis).
    pub create_time: i64,
    /// Last modification timestamp (millis).
    pub modify_time: i64,
}

/// Full configuration info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAllInfo {
    #[serde(default)]
    /// Config ID.
    pub id: i64,
    /// Config data ID.
    pub data_id: String,
    #[serde(alias = "groupName")]
    /// Group name.
    pub group: String,
    /// Config content.
    pub content: String,
    #[serde(default)]
    /// MD5 hash of the config content.
    pub md5: String,
    #[serde(alias = "namespaceId", default)]
    /// Namespace/tenant ID.
    pub tenant: String,
    #[serde(default)]
    /// Owning application name.
    pub app_name: String,
    #[serde(default)]
    /// Config type (e.g. "text", "yaml", "json").
    pub r#type: String,
    #[serde(default)]
    /// Creation timestamp (millis).
    pub create_time: i64,
    #[serde(default)]
    /// Last modification timestamp (millis).
    pub modify_time: i64,
    #[serde(default)]
    /// User that created the config.
    pub create_user: String,
    #[serde(default)]
    /// IP that created the config.
    pub create_ip: String,
    #[serde(default)]
    /// Config description.
    pub desc: String,
    #[serde(default)]
    /// Usage tag of the config.
    pub r#use: String,
    #[serde(default)]
    /// Effect scope of the config.
    pub effect: String,
    #[serde(default)]
    /// Config schema (e.g. for form-based editing).
    pub schema: String,
    #[serde(alias = "configTags", default)]
    /// Config tags.
    pub config_tags: String,
    #[serde(default)]
    /// Encrypted data key, if the config is encrypted.
    pub encrypted_data_key: String,
}

/// Gray/beta configuration info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    #[serde(default)]
    /// Gray config ID.
    pub id: i64,
    /// Config data ID.
    pub data_id: String,
    #[serde(alias = "groupName")]
    /// Group name.
    pub group: String,
    #[serde(default)]
    /// Config content.
    pub content: String,
    #[serde(default)]
    /// MD5 hash of the config content.
    pub md5: String,
    #[serde(alias = "namespaceId", default)]
    /// Namespace/tenant ID.
    pub tenant: String,
    #[serde(default)]
    /// Gray/beta release name.
    pub gray_name: String,
    #[serde(default)]
    /// Gray release rule.
    pub gray_rule: String,
    #[serde(default)]
    /// Source user.
    pub src_user: String,
    #[serde(default)]
    /// Config type.
    pub r#type: String,
}

/// Basic history info for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    #[serde(default)]
    /// History record ID.
    pub id: u64,
    /// Config data ID.
    pub data_id: String,
    #[serde(alias = "groupName")]
    /// Group name.
    pub group: String,
    #[serde(alias = "namespaceId", default)]
    /// Namespace/tenant ID.
    pub tenant: String,
    #[serde(default)]
    /// Operation type.
    pub op_type: String,
    #[serde(default)]
    /// Publish type.
    pub publish_type: String,
    #[serde(default)]
    /// Gray/beta release name.
    pub gray_name: String,
    #[serde(default)]
    /// Source user.
    pub src_user: String,
    #[serde(default)]
    /// Source IP.
    pub src_ip: String,
    #[serde(default)]
    /// Creation timestamp (millis).
    pub created_time: i64,
    #[serde(default)]
    /// Last modified timestamp (millis).
    pub last_modified_time: i64,
}

/// Detailed history info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    #[serde(default)]
    /// History record ID.
    pub id: u64,
    /// Config data ID.
    pub data_id: String,
    #[serde(alias = "groupName")]
    /// Group name.
    pub group: String,
    #[serde(alias = "namespaceId", default)]
    /// Namespace/tenant ID.
    pub tenant: String,
    #[serde(default)]
    /// Config content.
    pub content: String,
    #[serde(default)]
    /// MD5 hash of the config content.
    pub md5: String,
    #[serde(default)]
    /// Owning application name.
    pub app_name: String,
    #[serde(default)]
    /// Operation type.
    pub op_type: String,
    #[serde(default)]
    /// Publish type.
    pub publish_type: String,
    #[serde(default)]
    /// Gray/beta release name.
    pub gray_name: String,
    #[serde(default)]
    /// Extended info.
    pub ext_info: String,
    #[serde(default)]
    /// Source user.
    pub src_user: String,
    #[serde(default)]
    /// Source IP.
    pub src_ip: String,
    #[serde(default)]
    /// Creation timestamp (millis).
    pub created_time: i64,
    #[serde(default)]
    /// Last modified timestamp (millis).
    pub last_modified_time: i64,
    #[serde(default)]
    /// Encrypted data key, if the config is encrypted.
    pub encrypted_data_key: String,
}

/// Remote connection ability information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAbility {
    #[serde(default)]
    /// Whether remote connection is supported.
    pub support_remote_connection: bool,
    #[serde(default)]
    /// Whether gRPC metrics reporting is enabled.
    pub grpc_report_enabled: bool,
}

/// Configuration management ability information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAbility {
    #[serde(default)]
    /// Whether remote metrics are supported.
    pub support_remote_metrics: bool,
}

/// Naming/service discovery ability information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingAbility {
    #[serde(default)]
    /// Whether JRaft consensus is supported.
    pub support_jraft: bool,
}

/// Aggregated node abilities
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeAbilities {
    #[serde(default)]
    /// Remote connection abilities.
    pub remote_ability: RemoteAbility,
    #[serde(default)]
    /// Config management abilities.
    pub config_ability: ConfigAbility,
    #[serde(default)]
    /// Naming abilities.
    pub naming_ability: NamingAbility,
}

/// Cluster member info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// Member IP address.
    pub ip: String,
    /// Member port.
    pub port: i32,
    /// Member state (e.g. "UP").
    pub state: String,
    #[serde(default)]
    /// Extended metadata key-value pairs.
    pub extend_info: std::collections::HashMap<String, serde_json::Value>,
    /// Member address (host:port).
    pub address: String,
    /// Number of consecutive failed accesses.
    pub fail_access_cnt: i32,
    #[serde(default)]
    /// Node abilities.
    pub abilities: NodeAbilities,
    #[serde(default)]
    /// Whether gRPC metrics reporting is enabled.
    pub grpc_report_enabled: bool,
}

/// Cluster health response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthResponse {
    #[serde(default)]
    /// Whether the cluster is healthy.
    pub healthy: bool,
    #[serde(default)]
    /// Total number of members.
    pub member_count: usize,
    #[serde(default)]
    /// Number of healthy members.
    pub healthy_count: usize,
    #[serde(default)]
    /// Number of unhealthy members.
    pub unhealthy_count: usize,
    #[serde(default)]
    /// Server status string.
    pub server_status: String,
    #[serde(default)]
    /// Whether the server runs in standalone mode.
    pub standalone: bool,
}

/// Self member response (flat structure matching server's NodeSelfResponse)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfMemberResponse {
    /// Member IP address.
    pub ip: String,
    /// Member port.
    pub port: u16,
    /// Member address (host:port).
    pub address: String,
    /// Member state (e.g. "UP").
    pub state: String,
    #[serde(default)]
    /// Extended metadata.
    pub extend_info: serde_json::Value,
    #[serde(default)]
    /// Number of consecutive failed accesses.
    pub fail_access_cnt: u64,
    #[serde(default)]
    /// Node abilities (raw JSON).
    pub abilities: serde_json::Value,
}

/// Client list response from `/v3/admin/ns/client/list`
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientListResponse {
    #[serde(default)]
    /// Number of connected clients.
    pub count: i32,
    #[serde(alias = "clients", default)]
    /// Connected client IDs.
    pub client_ids: Vec<String>,
}

/// Paginated response (re-exported from batata-common)
pub use batata_common::model::Page;

// ============== Service/Naming Models ==============

/// Service detail for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetail {
    #[serde(default)]
    /// Namespace ID.
    pub namespace_id: String,
    #[serde(alias = "groupName", default)]
    /// Group name.
    pub group_name: String,
    #[serde(alias = "name", default)]
    /// Service name.
    pub service_name: String,
    #[serde(default)]
    /// Protection threshold (0-1).
    pub protect_threshold: f32,
    #[serde(default)]
    /// Service metadata.
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    /// Service selector.
    pub selector: Option<ServiceSelector>,
    #[serde(default)]
    /// Clusters belonging to the service.
    pub clusters: Vec<ClusterInfo>,
    #[serde(default)]
    /// Total instance count.
    pub ip_count: i32,
    #[serde(default)]
    /// Healthy instance count.
    pub healthy_instance_count: i32,
    #[serde(default)]
    /// Number of clusters.
    pub cluster_count: i32,
    #[serde(default)]
    /// Trigger flag.
    pub trigger_flag: bool,
}

/// Service selector
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSelector {
    #[serde(rename = "type")]
    /// Selector type.
    pub selector_type: String,
    /// Selector expression.
    pub expression: String,
}

/// Cluster info in service detail
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterInfo {
    #[serde(default)]
    /// Cluster name.
    pub name: String,
    #[serde(default)]
    /// Health checker configuration.
    pub health_checker: HealthChecker,
    #[serde(default)]
    /// Cluster metadata.
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Health checker configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthChecker {
    #[serde(rename = "type", default)]
    /// Health check type.
    pub check_type: String,
    #[serde(default)]
    /// Health check port.
    pub port: i32,
    #[serde(default)]
    /// Whether to use the instance port for health checks.
    pub use_instance_port: bool,
}

/// Service list item for pagination
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListItem {
    #[serde(default)]
    /// Service name.
    pub name: String,
    #[serde(default)]
    /// Group name.
    pub group_name: String,
    #[serde(default)]
    /// Number of clusters.
    pub cluster_count: u32,
    #[serde(default)]
    /// Total instance count.
    pub ip_count: u32,
    #[serde(default)]
    /// Healthy instance count.
    pub healthy_instance_count: u32,
    #[serde(default)]
    /// Trigger flag.
    pub trigger_flag: bool,
    #[serde(default)]
    /// Service metadata.
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Subscriber info for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriberInfo {
    #[serde(default)]
    /// Subscriber address.
    pub address: String,
    #[serde(default)]
    /// Subscriber agent.
    pub agent: String,
    #[serde(default)]
    /// Subscriber application name.
    pub app: String,
}

/// Instance info for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    #[serde(default)]
    /// Instance IP.
    pub ip: String,
    #[serde(default)]
    /// Instance port.
    pub port: i32,
    #[serde(default)]
    /// Instance weight.
    pub weight: f64,
    #[serde(default)]
    /// Whether the instance is healthy.
    pub healthy: bool,
    #[serde(default)]
    /// Whether the instance is enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Whether the instance is ephemeral.
    pub ephemeral: bool,
    #[serde(default)]
    /// Cluster name.
    pub cluster_name: String,
    #[serde(default)]
    /// Service name.
    pub service_name: String,
    #[serde(default)]
    /// Instance metadata.
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    /// Heartbeat interval (millis).
    pub instance_heart_beat_interval: i64,
    #[serde(default)]
    /// Heartbeat timeout (millis).
    pub instance_heart_beat_timeout: i64,
    #[serde(default)]
    /// IP delete timeout (millis).
    pub ip_delete_timeout: i64,
}

/// Config listener info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenerInfo {
    /// Connection ID.
    pub connection_id: String,
    /// Client IP.
    pub client_ip: String,
    /// Config data ID.
    pub data_id: String,
    #[serde(alias = "groupName")]
    /// Group name.
    pub group: String,
    #[serde(alias = "namespaceId", default)]
    /// Namespace/tenant ID.
    pub tenant: String,
    /// MD5 hash of the listened config content.
    pub md5: String,
}

/// Clone result
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneResult {
    /// Number of succeeded items.
    pub succeeded: usize,
    /// Number of skipped items.
    pub skipped: usize,
    /// Number of failed items.
    pub failed: usize,
}

/// Import operation result summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// Number of successfully imported items.
    pub success_count: u32,
    /// Number of skipped items.
    pub skip_count: u32,
    /// Number of failed items.
    pub fail_count: u32,
    /// Details of failed items.
    pub fail_data: Vec<ImportFailItem>,
}

/// Details of a failed import item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailItem {
    /// Config data ID.
    pub data_id: String,
    #[serde(alias = "groupName")]
    /// Group name.
    pub group: String,
    /// Failure reason.
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_default() {
        let ns = Namespace::default();
        assert!(ns.namespace.is_empty());
        assert_eq!(ns.quota, 0);
    }

    #[test]
    fn test_config_basic_info_serialization() {
        let info = ConfigBasicInfo {
            id: 1,
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            data_id: "test.yaml".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("namespaceId"));
        assert!(json.contains("groupName"));
    }

    #[test]
    fn test_page_default() {
        let page: Page<ConfigBasicInfo> = Page::default();
        assert_eq!(page.total_count, 0);
        assert!(page.page_items.is_empty());
    }
}
