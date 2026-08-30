//! Naming client model types

use serde::{Deserialize, Serialize};

/// Client summary information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientSummaryInfo {
    /// The `client_id` field.
    pub client_id: String,
    /// The `ephemeral` field.
    pub ephemeral: bool,
    /// The `last_updated_time` field.
    pub last_updated_time: i64,
    /// The `client_type` field.
    pub client_type: String,
    /// The `connect_type` field.
    pub connect_type: String,
    /// The `app_name` field.
    pub app_name: String,
    /// The `version` field.
    pub version: String,
    /// The `client_ip` field.
    pub client_ip: String,
    /// The `client_port` field.
    pub client_port: i32,
}

/// Client service relationship information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientServiceInfo {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `group_name` field.
    pub group_name: String,
    /// The `service_name` field.
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `publisher_info` field.
    pub publisher_info: Option<ClientPublisherInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `subscriber_info` field.
    pub subscriber_info: Option<ClientSubscriberInfo>,
}

/// Client publisher information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientPublisherInfo {
    /// The `client_id` field.
    pub client_id: String,
    /// The `ip` field.
    pub ip: String,
    /// The `port` field.
    pub port: i32,
    /// The `cluster_name` field.
    pub cluster_name: String,
}

/// Client subscriber information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientSubscriberInfo {
    /// The `client_id` field.
    pub client_id: String,
    /// The `app_name` field.
    pub app_name: String,
    /// The `agent` field.
    pub agent: String,
    /// The `address` field.
    pub address: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_summary_info_serialization() {
        let info = ClientSummaryInfo {
            client_id: "192.168.1.1:8080#true".to_string(),
            ephemeral: true,
            last_updated_time: 1704067200000,
            client_type: "connection".to_string(),
            connect_type: "GRPC".to_string(),
            app_name: "test-app".to_string(),
            version: "2.4.0".to_string(),
            client_ip: "192.168.1.1".to_string(),
            client_port: 8080,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"clientId\":\"192.168.1.1:8080#true\""));
        assert!(json.contains("\"clientType\":\"connection\""));

        let deserialized: ClientSummaryInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.client_id, "192.168.1.1:8080#true");
        assert!(deserialized.ephemeral);
    }

    #[test]
    fn test_client_service_info_serialization() {
        let info = ClientServiceInfo {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-service".to_string(),
            publisher_info: Some(ClientPublisherInfo {
                client_id: "client-1".to_string(),
                ip: "192.168.1.1".to_string(),
                port: 8080,
                cluster_name: "DEFAULT".to_string(),
            }),
            subscriber_info: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"serviceName\":\"test-service\""));
        assert!(json.contains("\"publisherInfo\""));
        assert!(!json.contains("\"subscriberInfo\""));

        let deserialized: ClientServiceInfo = serde_json::from_str(&json).unwrap();
        assert!(deserialized.publisher_info.is_some());
        assert!(deserialized.subscriber_info.is_none());
    }

    #[test]
    fn test_client_publisher_info_serialization() {
        let info = ClientPublisherInfo {
            client_id: "client-1".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"ip\":\"192.168.1.1\""));

        let deserialized: ClientPublisherInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ip, "192.168.1.1");
    }

    #[test]
    fn test_client_subscriber_info_serialization() {
        let info = ClientSubscriberInfo {
            client_id: "client-1".to_string(),
            app_name: "test-app".to_string(),
            agent: "Nacos-Java-Client:v2.4.0".to_string(),
            address: "192.168.1.1:8080".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"agent\":\"Nacos-Java-Client:v2.4.0\""));

        let deserialized: ClientSubscriberInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent, "Nacos-Java-Client:v2.4.0");
    }
}
