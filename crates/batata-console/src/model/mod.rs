//! Console model types
//!
//! This module defines data structures specific to the console APIs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export types from batata-config
pub use batata_config::model::{Namespace, NamespaceForm};

/// Cluster member info for console responses
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub ip: String,
    pub port: u16,
    pub state: String,
    pub extend_info: HashMap<String, String>,
    pub address: String,
    pub fail_access_cnt: i32,
    pub abilities: HashMap<String, bool>,
}

impl From<batata_api::model::Member> for Member {
    fn from(value: batata_api::model::Member) -> Self {
        let extend_info = value
            .extend_info
            .read()
            .ok()
            .map(|info| {
                info.iter()
                    .map(|(k, v)| (k.clone(), v.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            ip: value.ip,
            port: value.port,
            state: value.state.to_string(),
            extend_info,
            address: value.address,
            fail_access_cnt: value.fail_access_cnt,
            abilities: HashMap::new(),
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
}
