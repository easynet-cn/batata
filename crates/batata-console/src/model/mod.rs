//! Console model types
//!
//! This module re-exports shared console types from batata-server-common
//! and additional types from batata-config.

// Re-export types from batata-config
pub use batata_config::model::{Namespace, NamespaceForm};

// Re-export all console model types from server-common
pub use batata_server_common::console::model::{
    ClusterHealthResponse, ClusterHealthSummary, ConfigAbility, Member, NamingAbility,
    NodeAbilities, RemoteAbility, SelfMemberResponse,
};

// Re-export console API config types from server-common
pub use batata_server_common::console::api_model::{
    ConfigBasicInfo, ConfigDetailInfo, ConfigGrayInfo, ConfigHistoryBasicInfo,
    ConfigHistoryDetailInfo,
};

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
