//! Naming service data models
//!
//! This module defines core data structures for service discovery:
//! - Service instance information
//! - Service metadata
//! - Query parameters

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// Instance type constants
pub const INSTANCE_TYPE_EPHEMERAL: &str = "ephemeral";
pub const INSTANCE_TYPE_PERSISTENT: &str = "persistent";

// Request type constants
pub const REGISTER_INSTANCE: &str = "registerInstance";
pub const DE_REGISTER_INSTANCE: &str = "deregisterInstance";

/// Service instance information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub instance_id: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub service_name: String,
    pub metadata: HashMap<String, String>,
    pub instance_heart_beat_interval: i64,
    pub instance_heart_beat_time_out: i64,
    pub ip_delete_timeout: i64,
    pub instance_id_generator: String,
}

impl Instance {
    pub fn new(ip: String, port: i32) -> Self {
        Self {
            ip,
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: "simple".to_string(),
            ..Default::default()
        }
    }

    /// Generate instance key for map storage
    pub fn key(&self) -> String {
        format!("{}#{}#{}", self.ip, self.port, self.cluster_name)
    }
}

/// Service information containing list of instances
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub name: String,
    pub group_name: String,
    pub clusters: String,
    pub cache_millis: i64,
    pub hosts: Vec<Instance>,
    pub last_ref_time: i64,
    pub checksum: String,
    pub all_ips: bool,
    pub reach_protection_threshold: bool,
}

impl Service {
    pub fn new(name: String, group_name: String) -> Self {
        Self {
            name,
            group_name,
            cache_millis: 10000,
            ..Default::default()
        }
    }

    /// Get healthy instances
    pub fn healthy_hosts(&self) -> Vec<&Instance> {
        self.hosts.iter().filter(|h| h.healthy && h.enabled).collect()
    }
}

/// Service list item for listing services
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub group_name: String,
    pub cluster_count: i32,
    pub ip_count: i32,
    pub healthy_instance_count: i32,
    pub trigger_flag: bool,
}

/// Query parameters for service discovery
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServiceQuery {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub clusters: String,
    pub healthy_only: bool,
}

/// Instance registration parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InstanceRegisterForm {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub enabled: bool,
    pub healthy: bool,
    pub ephemeral: bool,
    pub cluster_name: String,
    pub metadata: Option<String>,
}

impl InstanceRegisterForm {
    pub fn to_instance(&self) -> Instance {
        let metadata: HashMap<String, String> = self
            .metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or_default();

        Instance {
            ip: self.ip.clone(),
            port: self.port,
            weight: if self.weight <= 0.0 { 1.0 } else { self.weight },
            enabled: self.enabled,
            healthy: self.healthy,
            ephemeral: self.ephemeral,
            cluster_name: if self.cluster_name.is_empty() {
                "DEFAULT".to_string()
            } else {
                self.cluster_name.clone()
            },
            service_name: self.service_name.clone(),
            metadata,
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: "simple".to_string(),
            ..Default::default()
        }
    }
}

/// Instance heartbeat parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HeartbeatForm {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub cluster_name: String,
    pub ip: String,
    pub port: i32,
    pub beat: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_default() {
        let inst = Instance::default();
        assert!(inst.ip.is_empty());
        assert_eq!(inst.port, 0);
        assert_eq!(inst.weight, 0.0);
    }

    #[test]
    fn test_instance_new() {
        let inst = Instance::new("127.0.0.1".to_string(), 8080);
        assert_eq!(inst.ip, "127.0.0.1");
        assert_eq!(inst.port, 8080);
        assert_eq!(inst.weight, 1.0);
        assert!(inst.healthy);
        assert!(inst.enabled);
        assert!(inst.ephemeral);
    }

    #[test]
    fn test_instance_key() {
        let inst = Instance {
            ip: "192.168.1.100".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            ..Default::default()
        };
        assert_eq!(inst.key(), "192.168.1.100#8080#DEFAULT");
    }

    #[test]
    fn test_service_new() {
        let svc = Service::new("my-service".to_string(), "DEFAULT_GROUP".to_string());
        assert_eq!(svc.name, "my-service");
        assert_eq!(svc.group_name, "DEFAULT_GROUP");
        assert_eq!(svc.cache_millis, 10000);
    }

    #[test]
    fn test_service_healthy_hosts() {
        let svc = Service {
            hosts: vec![
                Instance {
                    healthy: true,
                    enabled: true,
                    ..Default::default()
                },
                Instance {
                    healthy: false,
                    enabled: true,
                    ..Default::default()
                },
                Instance {
                    healthy: true,
                    enabled: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(svc.healthy_hosts().len(), 1);
    }

    #[test]
    fn test_instance_register_form_to_instance() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            weight: 2.0,
            enabled: true,
            healthy: true,
            ephemeral: false,
            cluster_name: "PROD".to_string(),
            service_name: "api-gateway".to_string(),
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.ip, "10.0.0.1");
        assert_eq!(inst.port, 9000);
        assert_eq!(inst.weight, 2.0);
        assert_eq!(inst.cluster_name, "PROD");
        assert!(!inst.ephemeral);
    }

    // === Additional Boundary Tests ===

    #[test]
    fn test_instance_register_form_zero_weight() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            weight: 0.0,
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.weight, 1.0); // Should default to 1.0
    }

    #[test]
    fn test_instance_register_form_negative_weight() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            weight: -10.0,
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.weight, 1.0); // Should default to 1.0
    }

    #[test]
    fn test_instance_register_form_empty_cluster_name() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            cluster_name: "".to_string(),
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.cluster_name, "DEFAULT"); // Should default to DEFAULT
    }

    #[test]
    fn test_instance_register_form_valid_metadata() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            metadata: Some(r#"{"key": "value", "num": "123"}"#.to_string()),
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.metadata.get("key"), Some(&"value".to_string()));
        assert_eq!(inst.metadata.get("num"), Some(&"123".to_string()));
    }

    #[test]
    fn test_instance_register_form_invalid_metadata() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            metadata: Some("not valid json".to_string()),
            ..Default::default()
        };
        let inst = form.to_instance();
        assert!(inst.metadata.is_empty()); // Should default to empty map
    }

    #[test]
    fn test_instance_register_form_none_metadata() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            metadata: None,
            ..Default::default()
        };
        let inst = form.to_instance();
        assert!(inst.metadata.is_empty());
    }

    #[test]
    fn test_instance_key_empty_cluster() {
        let inst = Instance {
            ip: "192.168.1.100".to_string(),
            port: 8080,
            cluster_name: "".to_string(),
            ..Default::default()
        };
        assert_eq!(inst.key(), "192.168.1.100#8080#");
    }

    #[test]
    fn test_service_healthy_hosts_all_unhealthy() {
        let svc = Service {
            hosts: vec![
                Instance {
                    healthy: false,
                    enabled: true,
                    ..Default::default()
                },
                Instance {
                    healthy: false,
                    enabled: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(svc.healthy_hosts().is_empty());
    }

    #[test]
    fn test_service_healthy_hosts_all_disabled() {
        let svc = Service {
            hosts: vec![
                Instance {
                    healthy: true,
                    enabled: false,
                    ..Default::default()
                },
                Instance {
                    healthy: true,
                    enabled: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(svc.healthy_hosts().is_empty());
    }

    #[test]
    fn test_service_healthy_hosts_empty() {
        let svc = Service::default();
        assert!(svc.healthy_hosts().is_empty());
    }

    #[test]
    fn test_service_info_default() {
        let info = ServiceInfo::default();
        assert!(info.name.is_empty());
        assert!(info.group_name.is_empty());
        assert_eq!(info.cluster_count, 0);
        assert_eq!(info.ip_count, 0);
        assert_eq!(info.healthy_instance_count, 0);
        assert!(!info.trigger_flag);
    }

    #[test]
    fn test_service_query_default() {
        let query = ServiceQuery::default();
        assert!(query.namespace_id.is_empty());
        assert!(query.group_name.is_empty());
        assert!(query.service_name.is_empty());
        assert!(query.clusters.is_empty());
        assert!(!query.healthy_only);
    }

    #[test]
    fn test_heartbeat_form_default() {
        let form = HeartbeatForm::default();
        assert!(form.namespace_id.is_empty());
        assert!(form.group_name.is_empty());
        assert!(form.service_name.is_empty());
        assert!(form.cluster_name.is_empty());
        assert!(form.ip.is_empty());
        assert_eq!(form.port, 0);
        assert!(form.beat.is_none());
    }

    #[test]
    fn test_instance_serialization() {
        let inst = Instance::new("127.0.0.1".to_string(), 8080);
        let json = serde_json::to_string(&inst).unwrap();
        let deserialized: Instance = serde_json::from_str(&json).unwrap();
        assert_eq!(inst.ip, deserialized.ip);
        assert_eq!(inst.port, deserialized.port);
        assert_eq!(inst.weight, deserialized.weight);
    }

    #[test]
    fn test_service_serialization() {
        let svc = Service::new("my-service".to_string(), "DEFAULT_GROUP".to_string());
        let json = serde_json::to_string(&svc).unwrap();
        let deserialized: Service = serde_json::from_str(&json).unwrap();
        assert_eq!(svc.name, deserialized.name);
        assert_eq!(svc.group_name, deserialized.group_name);
    }

    #[test]
    fn test_instance_type_constants() {
        assert_eq!(INSTANCE_TYPE_EPHEMERAL, "ephemeral");
        assert_eq!(INSTANCE_TYPE_PERSISTENT, "persistent");
    }

    #[test]
    fn test_request_type_constants() {
        assert_eq!(REGISTER_INSTANCE, "registerInstance");
        assert_eq!(DE_REGISTER_INSTANCE, "deregisterInstance");
    }
}
