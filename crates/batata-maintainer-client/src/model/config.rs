// Configuration management model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Basic configuration information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBasicInfo {
    pub id: i64,
    pub namespace_id: String,
    pub group_name: String,
    pub data_id: String,
    pub md5: String,
    pub r#type: String,
    pub app_name: String,
    pub create_time: i64,
    pub modify_time: i64,
}

/// Detailed configuration information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDetailInfo {
    #[serde(flatten)]
    pub config_basic_info: ConfigBasicInfo,
    pub content: String,
    pub desc: String,
    pub encrypted_data_key: String,
    pub create_user: String,
    pub create_ip: String,
    pub config_tags: String,
}

/// Gray/beta configuration information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    pub config_detail_info: ConfigDetailInfo,
    pub gray_name: String,
    pub gray_rule: String,
}

/// Basic history information for config changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    #[serde(flatten)]
    pub config_basic_info: ConfigBasicInfo,
    pub src_ip: String,
    pub src_user: String,
    pub op_type: String,
    pub publish_type: String,
}

/// Detailed history information for config changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    #[serde(flatten)]
    pub config_history_basic_info: ConfigHistoryBasicInfo,
    pub content: String,
    pub encrypted_data_key: String,
    pub gray_name: String,
    pub ext_info: String,
}

/// Configuration listener information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenerInfo {
    pub query_type: String,
    pub listeners_status: HashMap<String, String>,
}

impl ConfigListenerInfo {
    pub const QUERY_TYPE_CONFIG: &str = "config";
    pub const QUERY_TYPE_IP: &str = "ip";
}

/// Configuration clone information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCloneInfo {
    pub config_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_group_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_data_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_basic_info_serialization() {
        let info = ConfigBasicInfo {
            id: 1,
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            data_id: "app.yaml".to_string(),
            md5: "abc123".to_string(),
            r#type: "yaml".to_string(),
            app_name: "test-app".to_string(),
            create_time: 1704067200000,
            modify_time: 1704067200000,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"namespaceId\":\"public\""));
        assert!(json.contains("\"groupName\":\"DEFAULT_GROUP\""));
        assert!(json.contains("\"dataId\":\"app.yaml\""));

        let deserialized: ConfigBasicInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.data_id, "app.yaml");
    }

    #[test]
    fn test_config_detail_info_flatten() {
        let detail = ConfigDetailInfo {
            config_basic_info: ConfigBasicInfo {
                id: 1,
                data_id: "test.yaml".to_string(),
                ..Default::default()
            },
            content: "key: value".to_string(),
            desc: "test config".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&detail).unwrap();
        // Flattened fields should appear at top level
        assert!(json.contains("\"dataId\":\"test.yaml\""));
        assert!(json.contains("\"content\":\"key: value\""));

        let deserialized: ConfigDetailInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.config_basic_info.data_id, "test.yaml");
        assert_eq!(deserialized.content, "key: value");
    }

    #[test]
    fn test_config_gray_info_serialization() {
        let gray = ConfigGrayInfo {
            config_detail_info: ConfigDetailInfo::default(),
            gray_name: "beta".to_string(),
            gray_rule: r#"{"type":"tag","expr":"beta"}"#.to_string(),
        };

        let json = serde_json::to_string(&gray).unwrap();
        assert!(json.contains("\"grayName\":\"beta\""));

        let deserialized: ConfigGrayInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.gray_name, "beta");
    }

    #[test]
    fn test_config_listener_info() {
        let mut listeners = HashMap::new();
        listeners.insert("192.168.1.1".to_string(), "abc123".to_string());

        let info = ConfigListenerInfo {
            query_type: ConfigListenerInfo::QUERY_TYPE_CONFIG.to_string(),
            listeners_status: listeners,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"queryType\":\"config\""));

        let deserialized: ConfigListenerInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.query_type, "config");
        assert_eq!(deserialized.listeners_status.len(), 1);
    }
}
