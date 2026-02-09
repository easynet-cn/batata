// Cluster model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Cluster member information for console responses
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

/// Cluster health summary
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

/// Cluster health response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthResponse {
    pub is_healthy: bool,
    pub summary: ClusterHealthSummary,
    pub standalone: bool,
}

/// Self member information response
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
    fn test_member_serialization() {
        let mut extend_info = HashMap::new();
        extend_info.insert(
            "version".to_string(),
            serde_json::Value::String("3.1.0".to_string()),
        );

        let member = Member {
            ip: "192.168.1.1".to_string(),
            port: 8848,
            state: "UP".to_string(),
            extend_info,
            address: "192.168.1.1:8848".to_string(),
            abilities: NodeAbilities::default(),
            grpc_report_enabled: true,
            fail_access_cnt: 0,
        };

        let json = serde_json::to_string(&member).unwrap();
        assert!(json.contains("\"ip\":\"192.168.1.1\""));
        assert!(json.contains("\"port\":8848"));
        assert!(json.contains("\"state\":\"UP\""));

        let deserialized: Member = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ip, "192.168.1.1");
        assert_eq!(deserialized.port, 8848);
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

        let deserialized: ClusterHealthResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_healthy);
        assert_eq!(deserialized.summary.total, 3);
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
        assert!(json.contains("\"isStandalone\":false"));

        let deserialized: SelfMemberResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ip, "127.0.0.1");
    }

    #[test]
    fn test_node_abilities_serialization() {
        let abilities = NodeAbilities::default();
        let json = serde_json::to_string(&abilities).unwrap();
        assert!(json.contains("\"remoteAbility\""));
        assert!(json.contains("\"supportRemoteConnection\":true"));
        assert!(json.contains("\"supportJraft\":true"));
    }
}
