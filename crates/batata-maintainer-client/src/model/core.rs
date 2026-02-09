// Core/cluster model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ID generator information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IdGeneratorInfo {
    pub resource: String,
    pub info: IdInfo,
}

/// ID info within a generator
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IdInfo {
    pub current_id: i64,
    pub work_id: i64,
}

/// Connection information for a client
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionInfo {
    pub connection_id: String,
    pub client_ip: String,
    pub remote_ip: String,
    pub remote_port: i32,
    pub connect_type: String,
    pub app_name: String,
    pub version: String,
    pub create_time: String,
    pub last_active_time: String,
    pub labels: HashMap<String, String>,
    pub metadata_info: serde_json::Value,
}

/// Server loader metrics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerLoaderMetrics {
    pub detail: Vec<ServerLoaderInfo>,
    pub total: i32,
    pub max: i32,
    pub min: i32,
    pub avg: f64,
    pub member_count: i32,
    pub threshold: f64,
    pub completed: bool,
}

/// Individual server loader info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerLoaderInfo {
    pub address: String,
    pub metric: f64,
    pub load: f64,
    pub sdk_conn_count: i32,
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
