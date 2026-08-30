//! Service detail and related model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::naming::Instance;

/// Service detail information with cluster map
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceDetailInfo {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `service_name` field.
    pub service_name: String,
    /// The `group_name` field.
    pub group_name: String,
    /// The `cluster_map` field.
    pub cluster_map: HashMap<String, ClusterInfo>,
    /// The `metadata` field.
    pub metadata: HashMap<String, String>,
    /// The `protect_threshold` field.
    pub protect_threshold: f32,
    /// The `selector` field.
    pub selector: Option<serde_json::Value>,
    /// The `ephemeral` field.
    pub ephemeral: Option<bool>,
}

/// Cluster information within a service
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClusterInfo {
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `health_checker` field.
    pub health_checker: Option<serde_json::Value>,
    /// The `healthy_check_port` field.
    pub healthy_check_port: i32,
    /// The `use_instance_port_for_check` field.
    pub use_instance_port_for_check: bool,
    /// The `metadata` field.
    pub metadata: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `hosts` field.
    pub hosts: Option<Vec<Instance>>,
}

/// Service view for list operations (without full detail)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceView {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `group_name` field.
    pub group_name: String,
    /// The `service_name` field.
    pub service_name: String,
    /// The `cluster_count` field.
    pub cluster_count: i32,
    /// The `ip_count` field.
    pub ip_count: i32,
    /// The `healthy_instance_count` field.
    pub healthy_instance_count: i32,
    /// The `trigger_flag` field.
    pub trigger_flag: bool,
}

/// Subscriber information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubscriberInfo {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `group_name` field.
    pub group_name: String,
    /// The `service_name` field.
    pub service_name: String,
    /// The `ip` field.
    pub ip: String,
    /// The `port` field.
    pub port: i32,
    /// The `agent` field.
    pub agent: String,
    /// The `app_name` field.
    pub app_name: String,
}

/// Naming metrics information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MetricsInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `status` field.
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `service_count` field.
    pub service_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `instance_count` field.
    pub instance_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `subscribe_count` field.
    pub subscribe_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `client_count` field.
    pub client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `connection_based_client_count` field.
    pub connection_based_client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `ephemeral_ip_port_client_count` field.
    pub ephemeral_ip_port_client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `persistent_ip_port_client_count` field.
    pub persistent_ip_port_client_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `responsible_client_count` field.
    pub responsible_client_count: Option<i32>,
}

/// Instance metadata batch operation result
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceMetadataBatchResult {
    /// The `updated` field.
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
