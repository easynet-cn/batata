//! Client model types
//!
//! This module defines data structures used by the client for API responses.

use serde::{Deserialize, Serialize};

/// Generic API response wrapper
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
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
    pub namespace: String,
    pub namespace_show_name: String,
    pub namespace_desc: String,
    pub quota: i32,
    pub config_count: i32,
    #[serde(rename = "type")]
    pub type_: i32,
}

/// Basic configuration info for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBasicInfo {
    pub id: i64,
    pub namespace_id: String,
    pub group_name: String,
    pub data_id: String,
    pub md5: String,
    pub r#type: String,
    pub app_name: String,
    pub create_time: i64,
    pub modify_time: i64,
}

/// Full configuration info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAllInfo {
    #[serde(default)]
    pub id: i64,
    pub data_id: String,
    #[serde(alias = "groupName")]
    pub group: String,
    pub content: String,
    #[serde(default)]
    pub md5: String,
    #[serde(alias = "namespaceId", default)]
    pub tenant: String,
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub create_time: i64,
    #[serde(default)]
    pub modify_time: i64,
    #[serde(default)]
    pub create_user: String,
    #[serde(default)]
    pub create_ip: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub r#use: String,
    #[serde(default)]
    pub effect: String,
    #[serde(default)]
    pub schema: String,
    #[serde(alias = "configTags", default)]
    pub config_tags: String,
    #[serde(default)]
    pub encrypted_data_key: String,
}

/// Gray/beta configuration info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    #[serde(default)]
    pub id: i64,
    pub data_id: String,
    #[serde(alias = "groupName")]
    pub group: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub md5: String,
    #[serde(alias = "namespaceId", default)]
    pub tenant: String,
    #[serde(default)]
    pub gray_name: String,
    #[serde(default)]
    pub gray_rule: String,
    #[serde(default)]
    pub src_user: String,
    #[serde(default)]
    pub r#type: String,
}

/// Basic history info for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    #[serde(default)]
    pub id: u64,
    pub data_id: String,
    #[serde(alias = "groupName")]
    pub group: String,
    #[serde(alias = "namespaceId", default)]
    pub tenant: String,
    #[serde(default)]
    pub op_type: String,
    #[serde(default)]
    pub publish_type: String,
    #[serde(default)]
    pub gray_name: String,
    #[serde(default)]
    pub src_user: String,
    #[serde(default)]
    pub src_ip: String,
    #[serde(default)]
    pub created_time: i64,
    #[serde(default)]
    pub last_modified_time: i64,
}

/// Detailed history info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    #[serde(default)]
    pub id: u64,
    pub data_id: String,
    #[serde(alias = "groupName")]
    pub group: String,
    #[serde(alias = "namespaceId", default)]
    pub tenant: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub md5: String,
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub op_type: String,
    #[serde(default)]
    pub publish_type: String,
    #[serde(default)]
    pub gray_name: String,
    #[serde(default)]
    pub ext_info: String,
    #[serde(default)]
    pub src_user: String,
    #[serde(default)]
    pub src_ip: String,
    #[serde(default)]
    pub created_time: i64,
    #[serde(default)]
    pub last_modified_time: i64,
    #[serde(default)]
    pub encrypted_data_key: String,
}

/// Remote connection ability information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAbility {
    #[serde(default)]
    pub support_remote_connection: bool,
    #[serde(default)]
    pub grpc_report_enabled: bool,
}

/// Configuration management ability information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAbility {
    #[serde(default)]
    pub support_remote_metrics: bool,
}

/// Naming/service discovery ability information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingAbility {
    #[serde(default)]
    pub support_jraft: bool,
}

/// Aggregated node abilities
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeAbilities {
    #[serde(default)]
    pub remote_ability: RemoteAbility,
    #[serde(default)]
    pub config_ability: ConfigAbility,
    #[serde(default)]
    pub naming_ability: NamingAbility,
}

/// Cluster member info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub ip: String,
    pub port: i32,
    pub state: String,
    #[serde(default)]
    pub extend_info: std::collections::HashMap<String, serde_json::Value>,
    pub address: String,
    pub fail_access_cnt: i32,
    #[serde(default)]
    pub abilities: NodeAbilities,
    #[serde(default)]
    pub grpc_report_enabled: bool,
}

/// Cluster health response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthResponse {
    #[serde(default)]
    pub healthy: bool,
    #[serde(default)]
    pub member_count: usize,
    #[serde(default)]
    pub healthy_count: usize,
    #[serde(default)]
    pub unhealthy_count: usize,
    #[serde(default)]
    pub server_status: String,
    #[serde(default)]
    pub standalone: bool,
}

/// Self member response (flat structure matching server's NodeSelfResponse)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfMemberResponse {
    pub ip: String,
    pub port: u16,
    pub address: String,
    pub state: String,
    #[serde(default)]
    pub extend_info: serde_json::Value,
    #[serde(default)]
    pub fail_access_cnt: u64,
    #[serde(default)]
    pub abilities: serde_json::Value,
}

/// Client list response from `/v3/admin/ns/client/list`
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientListResponse {
    #[serde(default)]
    pub count: i32,
    #[serde(alias = "clients", default)]
    pub client_ids: Vec<String>,
}

/// Paginated response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    #[serde(alias = "count", default)]
    pub total_count: u64,
    #[serde(default)]
    pub page_number: u64,
    #[serde(default)]
    pub pages_available: u64,
    #[serde(alias = "serviceList", alias = "configList", alias = "hosts", alias = "subscribers", alias = "list", default)]
    pub page_items: Vec<T>,
}

// ============== Service/Naming Models ==============

/// Service detail for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetail {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(alias = "groupName", default)]
    pub group_name: String,
    #[serde(alias = "name", default)]
    pub service_name: String,
    #[serde(default)]
    pub protect_threshold: f32,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub selector: Option<ServiceSelector>,
    #[serde(default)]
    pub clusters: Vec<ClusterInfo>,
    #[serde(default)]
    pub ip_count: i32,
    #[serde(default)]
    pub healthy_instance_count: i32,
    #[serde(default)]
    pub cluster_count: i32,
    #[serde(default)]
    pub trigger_flag: bool,
}

/// Service selector
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSelector {
    #[serde(rename = "type")]
    pub selector_type: String,
    pub expression: String,
}

/// Cluster info in service detail
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub health_checker: HealthChecker,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Health checker configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthChecker {
    #[serde(rename = "type", default)]
    pub check_type: String,
    #[serde(default)]
    pub port: i32,
    #[serde(default)]
    pub use_instance_port: bool,
}

/// Service list item for pagination
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListItem {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub cluster_count: u32,
    #[serde(default)]
    pub ip_count: u32,
    #[serde(default)]
    pub healthy_instance_count: u32,
    #[serde(default)]
    pub trigger_flag: bool,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Subscriber info for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriberInfo {
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub app: String,
}

/// Instance info for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub port: i32,
    #[serde(default)]
    pub weight: f64,
    #[serde(default)]
    pub healthy: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub ephemeral: bool,
    #[serde(default)]
    pub cluster_name: String,
    #[serde(default)]
    pub service_name: String,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub instance_heart_beat_interval: i64,
    #[serde(default)]
    pub instance_heart_beat_timeout: i64,
    #[serde(default)]
    pub ip_delete_timeout: i64,
}

/// Config listener info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenerInfo {
    pub connection_id: String,
    pub client_ip: String,
    pub data_id: String,
    #[serde(alias = "groupName")]
    pub group: String,
    #[serde(alias = "namespaceId", default)]
    pub tenant: String,
    pub md5: String,
}

/// Clone result
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneResult {
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Import operation result summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub success_count: u32,
    pub skip_count: u32,
    pub fail_count: u32,
    pub fail_data: Vec<ImportFailItem>,
}

/// Details of a failed import item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailItem {
    pub data_id: String,
    #[serde(alias = "groupName")]
    pub group: String,
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
