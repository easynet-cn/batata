// Configuration data models and structures
// Re-exports types from batata_config for backward compatibility

// Re-export all config model types from batata_config
pub use batata_config::model::{
    ConfigAllInfo, ConfigForm, ConfigHistoryInfo, ConfigInfo, ConfigInfoBase,
    ConfigInfoGrayWrapper, ConfigInfoWrapper, ConfigRequestInfo, ConfigType,
};

// Re-export ConfigListenerInfo which is main-crate specific
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_form_default() {
        let form = ConfigForm::default();
        assert!(form.data_id.is_empty());
        assert!(form.group_name.is_empty());
        assert!(form.namespace_id.is_empty());
        assert!(form.content.is_empty());
        assert!(form.tag.is_none());
    }

    #[test]
    fn test_config_form_serialization() {
        let form = ConfigForm {
            data_id: "test-config".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            namespace_id: "public".to_string(),
            content: "key=value".to_string(),
            r#type: "properties".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&form).unwrap();
        assert!(json.contains("dataId"));
        assert!(json.contains("groupName"));
    }

    #[test]
    fn test_config_request_info_default() {
        let info = ConfigRequestInfo::default();
        assert!(info.src_ip.is_empty());
        assert!(!info.update_for_exist);
    }

    #[test]
    fn test_config_info_base_default() {
        let base = ConfigInfoBase::default();
        assert_eq!(base.id, 0);
        assert!(base.data_id.is_empty());
        assert!(base.group.is_empty());
    }

    #[test]
    fn test_config_info_with_base() {
        let info = ConfigInfo {
            config_info_base: ConfigInfoBase {
                id: 1,
                data_id: "app.properties".to_string(),
                group: "DEFAULT_GROUP".to_string(),
                content: "server.port=8080".to_string(),
                md5: "abc123".to_string(),
                encrypted_data_key: String::new(),
            },
            tenant: "public".to_string(),
            app_name: "my-app".to_string(),
            r#type: "properties".to_string(),
        };
        assert_eq!(info.config_info_base.id, 1);
        assert_eq!(info.tenant, "public");
    }

    #[test]
    fn test_config_all_info_default() {
        let all_info = ConfigAllInfo::default();
        assert_eq!(all_info.create_time, 0);
        assert_eq!(all_info.modify_time, 0);
        assert!(all_info.create_user.is_empty());
    }

    #[test]
    fn test_config_type_default() {
        let ct = ConfigType::default();
        assert_eq!(ct.as_str(), "text");
    }

    #[test]
    fn test_config_type_as_str() {
        assert_eq!(ConfigType::Properties.as_str(), "properties");
        assert_eq!(ConfigType::Xml.as_str(), "xml");
        assert_eq!(ConfigType::Json.as_str(), "json");
        assert_eq!(ConfigType::Text.as_str(), "text");
        assert_eq!(ConfigType::Html.as_str(), "html");
        assert_eq!(ConfigType::Yaml.as_str(), "yaml");
        assert_eq!(ConfigType::Toml.as_str(), "toml");
    }

    #[test]
    fn test_config_type_display() {
        assert_eq!(format!("{}", ConfigType::Properties), "properties");
        assert_eq!(format!("{}", ConfigType::Json), "json");
        assert_eq!(format!("{}", ConfigType::Yaml), "yaml");
    }

    #[test]
    fn test_config_type_from_str() {
        use std::str::FromStr;
        assert!(matches!(
            ConfigType::from_str("properties").unwrap(),
            ConfigType::Properties
        ));
        assert!(matches!(
            ConfigType::from_str("xml").unwrap(),
            ConfigType::Xml
        ));
        assert!(matches!(
            ConfigType::from_str("json").unwrap(),
            ConfigType::Json
        ));
        assert!(matches!(
            ConfigType::from_str("text").unwrap(),
            ConfigType::Text
        ));
        assert!(matches!(
            ConfigType::from_str("yaml").unwrap(),
            ConfigType::Yaml
        ));
        assert!(ConfigType::from_str("invalid").is_err());
    }

    #[test]
    fn test_config_form_deserialization() {
        let json = r#"{
            "dataId": "test.properties",
            "groupName": "DEFAULT_GROUP",
            "namespaceId": "public",
            "content": "key=value",
            "type": "properties"
        }"#;
        let form: ConfigForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.data_id, "test.properties");
        assert_eq!(form.group_name, "DEFAULT_GROUP");
        assert_eq!(form.r#type, "properties");
    }

    #[test]
    fn test_config_listener_info() {
        let info = ConfigListenerInfo::default();
        assert!(info.query_type.is_empty());
        assert!(info.listeners_status.is_empty());
        assert_eq!(ConfigListenerInfo::QUERY_TYPE_CONFIG, "config");
        assert_eq!(ConfigListenerInfo::QUERY_TYPE_IP, "ip");
    }
}
