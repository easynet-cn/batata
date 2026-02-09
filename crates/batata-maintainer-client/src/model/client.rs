// Naming client model types

use serde::{Deserialize, Serialize};

/// Client summary information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientSummaryInfo {
    pub client_id: String,
    pub ephemeral: bool,
    pub last_updated_time: i64,
    pub client_type: String,
    pub connect_type: String,
    pub app_name: String,
    pub version: String,
    pub client_ip: String,
    pub client_port: i32,
}

/// Client service relationship information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientServiceInfo {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher_info: Option<ClientPublisherInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscriber_info: Option<ClientSubscriberInfo>,
}

/// Client publisher information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientPublisherInfo {
    pub client_id: String,
    pub ip: String,
    pub port: i32,
    pub cluster_name: String,
}

/// Client subscriber information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientSubscriberInfo {
    pub client_id: String,
    pub app_name: String,
    pub agent: String,
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
