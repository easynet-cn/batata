//! Core/cluster model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ID generator information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IdGeneratorInfo {
    /// The `resource` field.
    pub resource: String,
    /// The `info` field.
    pub info: IdInfo,
}

/// ID info within a generator
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IdInfo {
    /// The `current_id` field.
    pub current_id: i64,
    /// The `work_id` field.
    pub work_id: i64,
}

/// Connection information for a client
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionInfo {
    /// The `connection_id` field.
    pub connection_id: String,
    /// The `client_ip` field.
    pub client_ip: String,
    /// The `remote_ip` field.
    pub remote_ip: String,
    /// The `remote_port` field.
    pub remote_port: i32,
    /// The `connect_type` field.
    pub connect_type: String,
    /// The `app_name` field.
    pub app_name: String,
    /// The `version` field.
    pub version: String,
    /// The `create_time` field.
    pub create_time: String,
    /// The `last_active_time` field.
    pub last_active_time: String,
    /// The `labels` field.
    pub labels: HashMap<String, String>,
    /// The `metadata_info` field.
    pub metadata_info: serde_json::Value,
}

/// Server loader metrics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerLoaderMetrics {
    /// The `detail` field.
    pub detail: Vec<ServerLoaderInfo>,
    /// The `total` field.
    pub total: i32,
    /// The `max` field.
    pub max: i32,
    /// The `min` field.
    pub min: i32,
    /// The `avg` field.
    pub avg: f64,
    /// The `member_count` field.
    pub member_count: i32,
    /// The `threshold` field.
    pub threshold: f64,
    /// The `completed` field.
    pub completed: bool,
}

/// Individual server loader info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerLoaderInfo {
    /// The `address` field.
    pub address: String,
    /// The `metric` field.
    pub metric: f64,
    /// The `load` field.
    pub load: f64,
    /// The `sdk_conn_count` field.
    pub sdk_conn_count: i32,
    /// The `connection_count` field.
    pub connection_count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generator_info_serialization() {
        let info = IdGeneratorInfo {
            resource: "config".to_string(),
            info: IdInfo {
                current_id: 100,
                work_id: 1,
            },
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"resource\":\"config\""));
        assert!(json.contains("\"currentId\":100"));

        let deserialized: IdGeneratorInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.resource, "config");
    }

    #[test]
    fn test_connection_info_serialization() {
        let info = ConnectionInfo {
            connection_id: "conn-1".to_string(),
            client_ip: "192.168.1.1".to_string(),
            remote_ip: "192.168.1.1".to_string(),
            remote_port: 8848,
            connect_type: "GRPC".to_string(),
            app_name: "test-app".to_string(),
            version: "2.4.0".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"connectionId\":\"conn-1\""));

        let deserialized: ConnectionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.connection_id, "conn-1");
    }

    #[test]
    fn test_server_loader_metrics_serialization() {
        let metrics = ServerLoaderMetrics {
            total: 100,
            max: 50,
            min: 10,
            avg: 33.3,
            member_count: 3,
            threshold: 0.8,
            completed: true,
            detail: vec![ServerLoaderInfo {
                address: "192.168.1.1:8848".to_string(),
                metric: 33.3,
                load: 0.5,
                sdk_conn_count: 20,
                connection_count: 33,
            }],
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"total\":100"));
        assert!(json.contains("\"memberCount\":3"));

        let deserialized: ServerLoaderMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total, 100);
        assert_eq!(deserialized.detail.len(), 1);
    }
}
