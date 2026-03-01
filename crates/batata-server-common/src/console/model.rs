//! Console model types
//!
//! This module defines data structures specific to the console APIs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Namespace information
#[derive(Clone, Debug, Serialize, Deserialize)]
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

/// Default namespace ID
pub const DEFAULT_NAMESPACE_ID: &str = "public";
/// Default namespace display name
pub const DEFAULT_NAMESPACE_SHOW_NAME: &str = "Public";
/// Default namespace description
pub const DEFAULT_NAMESPACE_DESCRIPTION: &str = "Public Namespace";
/// Default namespace quota
pub const DEFAULT_NAMESPACE_QUOTA: i32 = 200;

impl Default for Namespace {
    fn default() -> Self {
        Namespace {
            namespace: String::from(DEFAULT_NAMESPACE_ID),
            namespace_show_name: String::from(DEFAULT_NAMESPACE_SHOW_NAME),
            namespace_desc: String::from(DEFAULT_NAMESPACE_DESCRIPTION),
            quota: DEFAULT_NAMESPACE_QUOTA,
            config_count: 0,
            type_: 0,
        }
    }
}

impl From<batata_persistence::NamespaceInfo> for Namespace {
    fn from(value: batata_persistence::NamespaceInfo) -> Self {
        let type_ = if value.namespace_id == DEFAULT_NAMESPACE_ID {
            0
        } else {
            2
        };
        Self {
            namespace: value.namespace_id,
            namespace_show_name: value.namespace_name,
            namespace_desc: value.namespace_desc,
            quota: value.quota,
            config_count: value.config_count,
            type_,
        }
    }
}

/// Namespace creation form
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NamespaceForm {
    pub namespace_id: String,
    pub namespace_name: String,
    pub namespace_desc: String,
}

/// Remote connection ability information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAbility {
    pub support_remote_connection: bool,
    pub grpc_report_enabled: bool,
}

impl Default for RemoteAbility {
    fn default() -> Self {
        Self {
            support_remote_connection: true,
            grpc_report_enabled: true,
        }
    }
}

/// Configuration management ability information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAbility {
    pub support_remote_metrics: bool,
}

/// Naming/service discovery ability information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingAbility {
    pub support_jraft: bool,
}

impl Default for NamingAbility {
    fn default() -> Self {
        Self {
            support_jraft: true,
        }
    }
}

/// Aggregated node abilities matching Nacos V3 response format
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeAbilities {
    pub remote_ability: RemoteAbility,
    pub config_ability: ConfigAbility,
    pub naming_ability: NamingAbility,
}

/// Cluster member info for console responses
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub ip: String,
    pub port: u16,
    pub state: String,
    pub extend_info: HashMap<String, serde_json::Value>,
    pub address: String,
    pub abilities: NodeAbilities,
    pub grpc_report_enabled: bool,
    pub fail_access_cnt: i32,
}

impl From<batata_api::model::Member> for Member {
    fn from(value: batata_api::model::Member) -> Self {
        let extend_info = value
            .extend_info
            .read()
            .ok()
            .map(|info| info.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Self {
            ip: value.ip,
            port: value.port,
            state: value.state.to_string(),
            extend_info,
            address: value.address,
            abilities: NodeAbilities::default(),
            grpc_report_enabled: true,
            fail_access_cnt: value.fail_access_cnt,
        }
    }
}

/// Cluster health summary for console responses
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthSummary {
    pub total: usize,
    pub up: usize,
    pub down: usize,
    pub suspicious: usize,
    pub starting: usize,
    pub isolation: usize,
}

impl From<batata_core::cluster::ClusterHealthSummary> for ClusterHealthSummary {
    fn from(value: batata_core::cluster::ClusterHealthSummary) -> Self {
        Self {
            total: value.total,
            up: value.up,
            down: value.down,
            suspicious: value.suspicious,
            starting: value.starting,
            isolation: value.isolation,
        }
    }
}

/// Cluster health response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthResponse {
    pub is_healthy: bool,
    pub summary: ClusterHealthSummary,
    pub standalone: bool,
}

/// Self member response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfMemberResponse {
    pub ip: String,
    pub port: u16,
    pub address: String,
    pub state: String,
    pub is_standalone: bool,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_health_summary_default() {
        let summary = ClusterHealthSummary::default();
        assert_eq!(summary.total, 0);
        assert_eq!(summary.up, 0);
    }

    #[test]
    fn test_cluster_health_response_serialization() {
        let response = ClusterHealthResponse {
            is_healthy: true,
            summary: ClusterHealthSummary {
                total: 3,
                up: 2,
                down: 1,
                ..Default::default()
            },
            standalone: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"isHealthy\":true"));
        assert!(json.contains("\"total\":3"));
    }

    #[test]
    fn test_self_member_response_serialization() {
        let response = SelfMemberResponse {
            ip: "127.0.0.1".to_string(),
            port: 8848,
            address: "127.0.0.1:8848".to_string(),
            state: "UP".to_string(),
            is_standalone: false,
            version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"ip\":\"127.0.0.1\""));
        assert!(json.contains("\"port\":8848"));
    }

    #[test]
    fn test_member_from_api_member() {
        let api_member = batata_api::model::Member::new("192.168.3.47".to_string(), 8848);
        {
            let mut info = api_member.extend_info.write().unwrap();
            info.insert(
                "version".to_string(),
                serde_json::Value::String("3.1.0".to_string()),
            );
            info.insert(
                "raftPort".to_string(),
                serde_json::Value::String("7848".to_string()),
            );
        }

        let console_member: Member = api_member.into();
        assert_eq!(console_member.ip, "192.168.3.47");
        assert_eq!(console_member.port, 8848);
        assert_eq!(console_member.state, "UP");
        assert_eq!(console_member.address, "192.168.3.47:8848");
        assert!(console_member.grpc_report_enabled);
        assert_eq!(console_member.fail_access_cnt, 0);
    }

    #[test]
    fn test_node_abilities_serialization() {
        let abilities = NodeAbilities::default();
        let json = serde_json::to_string(&abilities).unwrap();
        assert!(json.contains("\"remoteAbility\""));
        assert!(json.contains("\"supportRemoteConnection\":true"));
    }
}
