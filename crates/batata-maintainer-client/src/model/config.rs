//! Configuration management model types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Basic configuration information.
///
/// Kept as an independent type even though it currently matches the
/// data-plane SDK field-for-field: maintainer and client model separate API
/// contracts that evolve independently, and both crates are published at
/// 1.0.0. `batata-console::convert_config_basic_info` performs the explicit
/// conversion instead of sharing a type identity.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBasicInfo {
    /// Storage id; upstream sends it as a JSON string to avoid JS precision loss.
    #[serde(with = "batata_api::serde_ext::string_i64")]
    pub id: i64,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `group_name` field.
    pub group_name: String,
    /// The `data_id` field.
    pub data_id: String,
    /// The `md5` field.
    pub md5: String,
    /// The `type` field.
    pub r#type: String,
    /// The `app_name` field.
    pub app_name: String,
    /// The `create_time` field.
    pub create_time: i64,
    /// The `modify_time` field.
    pub modify_time: i64,
    /// The `desc` field.
    #[serde(default)]
    pub desc: String,
    /// The `configTags` field.
    #[serde(default)]
    pub config_tags: String,
}

/// Detailed configuration information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDetailInfo {
    #[serde(flatten)]
    /// The `config_basic_info` field.
    pub config_basic_info: ConfigBasicInfo,
    /// The `content` field.
    pub content: String,
    /// The `encrypted_data_key` field.
    pub encrypted_data_key: String,
    /// The `create_user` field.
    pub create_user: String,
    /// The `create_ip` field.
    pub create_ip: String,
}

/// Gray/beta configuration information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    /// The `config_detail_info` field.
    pub config_detail_info: ConfigDetailInfo,
    /// The `gray_name` field.
    pub gray_name: String,
    /// The `gray_rule` field.
    pub gray_rule: String,
}

/// Basic history information for config changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    #[serde(flatten)]
    /// The `config_basic_info` field.
    pub config_basic_info: ConfigBasicInfo,
    /// The `src_ip` field.
    pub src_ip: String,
    /// The `src_user` field.
    pub src_user: String,
    /// The `op_type` field.
    pub op_type: String,
    /// The `publish_type` field.
    pub publish_type: String,
}

/// Detailed history information for config changes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    #[serde(flatten)]
    /// The `config_history_basic_info` field.
    pub config_history_basic_info: ConfigHistoryBasicInfo,
    /// The `content` field.
    pub content: String,
    /// The `encrypted_data_key` field.
    pub encrypted_data_key: String,
    /// The `gray_name` field.
    pub gray_name: String,
    /// The `ext_info` field.
    pub ext_info: String,
}

/// Configuration listener information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenerInfo {
    /// The `query_type` field.
    pub query_type: String,
    /// The `listeners_status` field.
    pub listeners_status: HashMap<String, String>,
}

impl ConfigListenerInfo {
    /// The `QUERY_TYPE_CONFIG` constant.
    pub const QUERY_TYPE_CONFIG: &str = "config";
    /// The `QUERY_TYPE_IP` constant.
    pub const QUERY_TYPE_IP: &str = "ip";
}

/// Configuration clone information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCloneInfo {
    /// The `config_id` field.
    ///
    /// Upstream `ConfigCloneInfo.configId` is a Java `Long` fed from the
    /// string-shaped `ConfigBasicInfo.id`; Jackson accepts both wire shapes.
    #[serde(with = "batata_api::serde_ext::flex_i64")]
    pub config_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `target_group_name` field.
    pub target_group_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `target_data_id` field.
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
            desc: "test desc".to_string(),
            config_tags: "tag1,tag2".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(
            json.contains("\"id\":\"1\""),
            "upstream sends id as a string"
        );
        assert!(json.contains("\"namespaceId\":\"public\""));
        assert!(json.contains("\"groupName\":\"DEFAULT_GROUP\""));
        assert!(json.contains("\"dataId\":\"app.yaml\""));
        assert!(json.contains("\"desc\":\"test desc\""));
        assert!(json.contains("\"configTags\":\"tag1,tag2\""));

        let deserialized: ConfigBasicInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.data_id, "app.yaml");
        assert_eq!(deserialized.desc, "test desc");
        assert_eq!(deserialized.config_tags, "tag1,tag2");
    }

    #[test]
    fn test_config_basic_info_accepts_numeric_id() {
        let json = r#"{"id":42,"namespaceId":"public","groupName":"DEFAULT_GROUP","dataId":"app.yaml","md5":"x","type":"yaml","appName":"app","createTime":1,"modifyTime":2}"#;
        let info: ConfigBasicInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, 42);
        assert!(info.desc.is_empty());
        assert!(info.config_tags.is_empty());
    }

    #[test]
    fn test_config_detail_info_flatten() {
        let detail = ConfigDetailInfo {
            config_basic_info: ConfigBasicInfo {
                id: 1,
                data_id: "test.yaml".to_string(),
                desc: "test config".to_string(),
                config_tags: "tag1".to_string(),
                ..Default::default()
            },
            content: "key: value".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&detail).unwrap();
        // Flattened fields should appear exactly once: upstream detail inherits
        // desc/configTags from ConfigBasicInfo rather than redeclaring them.
        assert_eq!(json.matches("\"desc\":").count(), 1);
        assert_eq!(json.matches("\"configTags\":").count(), 1);
        assert!(json.contains("\"dataId\":\"test.yaml\""));
        assert!(json.contains("\"content\":\"key: value\""));

        let deserialized: ConfigDetailInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.config_basic_info.data_id, "test.yaml");
        assert_eq!(deserialized.config_basic_info.desc, "test config");
        assert_eq!(deserialized.config_basic_info.config_tags, "tag1");
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

    #[test]
    fn config_clone_id_accepts_both_wire_shapes() {
        let from_string: ConfigCloneInfo =
            serde_json::from_str(r#"{"configId":"42","targetDataId":"b","targetGroupName":"g"}"#)
                .unwrap();
        let from_number: ConfigCloneInfo =
            serde_json::from_str(r#"{"configId":42,"targetDataId":"b","targetGroupName":"g"}"#)
                .unwrap();
        assert_eq!(from_string.config_id, 42);
        assert_eq!(from_number.config_id, 42);

        let json = serde_json::to_value(&from_string).unwrap();
        assert_eq!(json["configId"], serde_json::json!(42));
    }
}
