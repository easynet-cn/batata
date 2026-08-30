//! Naming/service discovery model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Service instance information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Instance {
    /// The `instance_id` field.
    pub instance_id: String,
    /// The `ip` field.
    pub ip: String,
    /// The `port` field.
    pub port: i32,
    /// The `weight` field.
    pub weight: f64,
    /// The `healthy` field.
    pub healthy: bool,
    /// The `enabled` field.
    pub enabled: bool,
    /// The `ephemeral` field.
    pub ephemeral: bool,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `service_name` field.
    pub service_name: String,
    /// The `metadata` field.
    pub metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_serialization() {
        let mut metadata = HashMap::new();
        metadata.insert("env".to_string(), "prod".to_string());

        let instance = Instance {
            instance_id: "192.168.1.1#8080#DEFAULT#test-service".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 8080,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-service".to_string(),
            metadata,
        };

        let json = serde_json::to_string(&instance).unwrap();
        assert!(json.contains("\"ip\":\"192.168.1.1\""));
        assert!(json.contains("\"port\":8080"));

        let deserialized: Instance = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ip, "192.168.1.1");
        assert_eq!(deserialized.port, 8080);
        assert!(deserialized.healthy);
    }
}
