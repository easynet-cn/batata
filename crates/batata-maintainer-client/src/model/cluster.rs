//! Cluster model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Remote connection ability information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAbility {
    /// The `support_remote_connection` field.
    pub support_remote_connection: bool,
    /// The `grpc_report_enabled` field.
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
    /// The `support_remote_metrics` field.
    pub support_remote_metrics: bool,
}

/// Naming/service discovery ability information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingAbility {
    /// The `support_jraft` field.
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
    /// The `remote_ability` field.
    pub remote_ability: RemoteAbility,
    /// The `config_ability` field.
    pub config_ability: ConfigAbility,
    /// The `naming_ability` field.
    pub naming_ability: NamingAbility,
}

/// Cluster member information for console responses
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// The `ip` field.
    pub ip: String,
    /// The `port` field.
    pub port: u16,
    /// The `state` field.
    pub state: String,
    /// The `extend_info` field.
    pub extend_info: HashMap<String, serde_json::Value>,
    /// The `address` field.
    pub address: String,
    /// The `abilities` field.
    pub abilities: NodeAbilities,
    /// The `grpc_report_enabled` field.
    pub grpc_report_enabled: bool,
    /// The `fail_access_cnt` field.
    pub fail_access_cnt: i32,
}

/// Cluster health summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthSummary {
    /// The `total` field.
    pub total: usize,
    /// The `up` field.
    pub up: usize,
    /// The `down` field.
    pub down: usize,
    /// The `suspicious` field.
    pub suspicious: usize,
    /// The `starting` field.
    pub starting: usize,
    /// The `isolation` field.
    pub isolation: usize,
}

/// Cluster health response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthResponse {
    /// The `is_healthy` field.
    pub is_healthy: bool,
    /// The `summary` field.
    pub summary: ClusterHealthSummary,
    /// The `standalone` field.
    pub standalone: bool,
}

/// Self member information response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfMemberResponse {
    /// The `ip` field.
    pub ip: String,
    /// The `port` field.
    pub port: u16,
    /// The `address` field.
    pub address: String,
    /// The `state` field.
    pub state: String,
    /// The `is_standalone` field.
    pub is_standalone: bool,
    /// The `version` field.
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
