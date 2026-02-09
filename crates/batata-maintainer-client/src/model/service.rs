// Service detail and related model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::naming::Instance;

/// Service detail information with cluster map
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceDetailInfo {
    pub namespace_id: String,
    pub service_name: String,
    pub group_name: String,
    pub cluster_map: HashMap<String, ClusterInfo>,
    pub metadata: HashMap<String, String>,
    pub protect_threshold: f32,
    pub selector: Option<serde_json::Value>,
    pub ephemeral: Option<bool>,
}

/// Cluster information within a service
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClusterInfo {
    pub cluster_name: String,
    pub health_checker: Option<serde_json::Value>,
    pub healthy_check_port: i32,
    pub use_instance_port_for_check: bool,
    pub metadata: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<Instance>>,
}

/// Service view for list operations (without full detail)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceView {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub cluster_count: i32,
    pub ip_count: i32,
    pub healthy_instance_count: i32,
    pub trigger_flag: bool,
}

/// Subscriber information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubscriberInfo {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub ip: String,
    pub port: i32,
    pub agent: String,
    pub app_name: String,
}

/// Naming metrics information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MetricsInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_based_client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral_ip_port_client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent_ip_port_client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responsible_client_count: Option<i32>,
}

/// Instance metadata batch operation result
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceMetadataBatchResult {
    pub updated: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_detail_info_serialization() {
        let info = ServiceDetailInfo {
            namespace_id: "public".to_string(),
            service_name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            protect_threshold: 0.5,
            ..Default::default()
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"serviceName\":\"test-service\""));
        assert!(json.contains("\"protectThreshold\":0.5"));

        let deserialized: ServiceDetailInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.service_name, "test-service");
    }

    #[test]
    fn test_cluster_info_serialization() {
        let info = ClusterInfo {
            cluster_name: "DEFAULT".to_string(),
            healthy_check_port: 80,
            use_instance_port_for_check: true,
            ..Default::default()
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"clusterName\":\"DEFAULT\""));

        let deserialized: ClusterInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cluster_name, "DEFAULT");
    }

    #[test]
    fn test_subscriber_info_serialization() {
        let info = SubscriberInfo {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-service".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 8080,
            agent: "Nacos-Java-Client:v2.4.0".to_string(),
            app_name: "my-app".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"ip\":\"192.168.1.1\""));

        let deserialized: SubscriberInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ip, "192.168.1.1");
    }

    #[test]
    fn test_metrics_info_serialization() {
        let info = MetricsInfo {
            status: Some("UP".to_string()),
            service_count: Some(10),
            instance_count: Some(20),
            ..Default::default()
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"status\":\"UP\""));
        assert!(json.contains("\"serviceCount\":10"));
        // Null fields should be skipped
        assert!(!json.contains("clientCount"));

        let deserialized: MetricsInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, Some("UP".to_string()));
    }

    #[test]
    fn test_service_view_serialization() {
        let view = ServiceView {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-service".to_string(),
            cluster_count: 1,
            ip_count: 3,
            healthy_instance_count: 2,
            trigger_flag: false,
        };

        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("\"ipCount\":3"));

        let deserialized: ServiceView = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ip_count, 3);
    }
}
