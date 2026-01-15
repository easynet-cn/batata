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
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub tenant: String,
    pub app_name: String,
    pub r#type: String,
    pub create_time: i64,
    pub modify_time: i64,
    pub create_user: String,
    pub create_ip: String,
    pub desc: String,
    pub r#use: String,
    pub effect: String,
    pub schema: String,
    pub config_tags: String,
    pub encrypted_data_key: String,
}

/// Gray/beta configuration info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub tenant: String,
    pub gray_name: String,
    pub gray_rule: String,
    pub src_user: String,
    pub r#type: String,
}

/// Basic history info for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    pub id: u64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub last_modified_time: i64,
}

/// Detailed history info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    pub id: u64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub ext_info: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub last_modified_time: i64,
    pub encrypted_data_key: String,
}

/// Cluster member info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub ip: String,
    pub port: i32,
    pub state: String,
    pub extend_info: std::collections::HashMap<String, String>,
    pub address: String,
    pub fail_access_cnt: i32,
    pub abilities: std::collections::HashMap<String, bool>,
}

/// Cluster health response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthResponse {
    pub healthy: bool,
    pub member_count: usize,
    pub healthy_count: usize,
    pub unhealthy_count: usize,
}

/// Self member response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfMemberResponse {
    pub member: Member,
    pub is_leader: bool,
}

/// Paginated response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_count: u64,
    pub page_number: u64,
    pub pages_available: u64,
    pub page_items: Vec<T>,
}

// ============== Service/Naming Models ==============

/// Service detail for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetail {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub protect_threshold: f32,
    pub metadata: std::collections::HashMap<String, String>,
    pub selector: ServiceSelector,
    pub clusters: Vec<ClusterInfo>,
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
    pub name: String,
    pub health_checker: HealthChecker,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Health checker configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthChecker {
    #[serde(rename = "type")]
    pub check_type: String,
    pub port: i32,
    pub use_instance_port: bool,
}

/// Service list item for pagination
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListItem {
    pub name: String,
    pub group_name: String,
    pub cluster_count: u32,
    pub ip_count: u32,
    pub healthy_instance_count: u32,
    pub trigger_flag: bool,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Subscriber info for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriberInfo {
    pub address: String,
    pub agent: String,
    pub app: String,
}

/// Instance info for API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub service_name: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub instance_heart_beat_interval: i64,
    pub instance_heart_beat_timeout: i64,
    pub ip_delete_timeout: i64,
}

/// Config listener info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenerInfo {
    pub connection_id: String,
    pub client_ip: String,
    pub data_id: String,
    pub group: String,
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
