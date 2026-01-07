// Configuration data models and structures
// This file defines various data structures for configuration management operations

use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use batata_persistence::entity;

// Form structure for configuration creation/update requests
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ConfigForm {
    pub data_id: String,
    pub group_name: String,
    pub namespace_id: String,
    pub content: String,
    pub tag: Option<String>,
    pub app_name: String,
    pub src_user: Option<String>,
    pub config_tags: String,
    pub encrypted_data_key: Option<String>,
    pub gray_name: Option<String>,
    pub gray_rule_exp: Option<String>,
    pub gray_version: Option<String>,
    pub gray_priority: Option<i32>,
    pub desc: String,
    pub r#use: Option<String>,
    pub effect: Option<String>,
    pub r#type: String,
    pub schema: Option<String>,
}

// Request metadata for configuration operations
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequestInfo {
    pub src_ip: String,
    pub src_type: String,
    pub request_ip_app: String,
    pub beta_ips: String,
    pub cas_md5: String,
    pub namespace_transferred: String,
    pub update_for_exist: bool,
}

// Base configuration information structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoBase {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    #[serde(flatten)]
    pub config_info_base: ConfigInfoBase,
    pub tenant: String,
    pub app_name: String,
    pub r#type: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAllInfo {
    #[serde(flatten)]
    pub config_info: ConfigInfo,
    pub create_time: i64,
    pub modify_time: i64,
    pub create_user: String,
    pub create_ip: String,
    pub desc: String,
    pub r#use: String,
    pub effect: String,
    pub schema: String,
    pub config_tags: String,
}

impl From<entity::config_info::Model> for ConfigAllInfo {
    fn from(value: entity::config_info::Model) -> Self {
        Self {
            config_info: ConfigInfo {
                config_info_base: ConfigInfoBase {
                    id: value.id,
                    data_id: value.data_id,
                    group: value.group_id.unwrap_or_default(),
                    content: value.content.unwrap_or_default(),
                    md5: value.md5.unwrap_or_default(),
                    encrypted_data_key: value.encrypted_data_key.unwrap_or_default(),
                },
                tenant: value.tenant_id.unwrap_or_default(),
                app_name: value.app_name.unwrap_or_default(),
                r#type: value.r#type.unwrap_or_default(),
            },
            create_time: value
                .gmt_create
                .unwrap_or_default()
                .and_utc()
                .timestamp_millis(),
            modify_time: value
                .gmt_modified
                .unwrap_or_default()
                .and_utc()
                .timestamp_millis(),
            create_user: value.src_user.unwrap_or_default(),
            create_ip: value.src_ip.unwrap_or_default(),
            desc: value.c_desc.unwrap_or_default(),
            r#use: value.c_use.unwrap_or_default(),
            effect: value.effect.unwrap_or_default(),
            schema: value.c_schema.unwrap_or_default(),
            config_tags: String::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoGrayWrapper {
    #[serde(flatten)]
    pub config_info: ConfigInfo,
    pub last_modified: i64,
    pub gray_name: String,
    pub gray_rule: String,
    pub src_user: String,
}

impl From<entity::config_info_gray::Model> for ConfigInfoGrayWrapper {
    fn from(value: entity::config_info_gray::Model) -> Self {
        Self {
            config_info: ConfigInfo {
                config_info_base: ConfigInfoBase {
                    id: value.id as i64,
                    data_id: value.data_id,
                    group: value.group_id,
                    content: value.content,
                    md5: value.md5.unwrap_or_default(),
                    encrypted_data_key: value.encrypted_data_key,
                },
                tenant: value.tenant_id.unwrap_or_default(),
                app_name: value.app_name.unwrap_or_default(),
                r#type: "".to_string(),
            },
            last_modified: 0,
            gray_name: value.gray_name,
            gray_rule: value.gray_rule,
            src_user: value.src_user.unwrap_or_default(),
        }
    }
}

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryInfo {
    #[serde_as(as = "DisplayFromStr")]
    pub id: u64,
    pub last_id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub app_name: String,
    pub md5: String,
    pub content: String,
    pub src_ip: String,
    pub src_user: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub ext_info: String,
    pub created_time: i64,
    pub last_modified_time: i64,
    pub encrypted_data_key: String,
}

impl From<entity::his_config_info::Model> for ConfigHistoryInfo {
    fn from(value: entity::his_config_info::Model) -> Self {
        Self {
            id: value.id,
            last_id: -1,
            data_id: value.data_id,
            group: value.group_id,
            tenant: value.tenant_id.unwrap_or_default(),
            app_name: value.app_name.unwrap_or_default(),
            md5: value.md5.unwrap_or_default(),
            content: value.content,
            src_ip: value.src_ip.unwrap_or_default(),
            src_user: value.src_user.unwrap_or_default(),
            op_type: value.op_type.unwrap_or_default(),
            publish_type: value.publish_type.unwrap_or_default(),
            gray_name: value.gray_name.unwrap_or_default(),
            ext_info: value.ext_info.unwrap_or_default(),
            created_time: value.gmt_create.and_utc().timestamp_millis(),
            last_modified_time: value.gmt_modified.and_utc().timestamp_millis(),
            encrypted_data_key: value.encrypted_data_key,
        }
    }
}

impl From<&entity::his_config_info::Model> for ConfigHistoryInfo {
    fn from(value: &entity::his_config_info::Model) -> Self {
        Self {
            id: value.id,
            last_id: -1,
            data_id: value.data_id.to_string(),
            group: value.group_id.to_string(),
            tenant: value.tenant_id.clone().unwrap_or_default(),
            app_name: value.app_name.clone().unwrap_or_default(),
            md5: value.md5.clone().unwrap_or_default(),
            content: value.content.to_string(),
            src_ip: value.src_ip.clone().unwrap_or_default(),
            src_user: value.src_user.clone().unwrap_or_default(),
            op_type: value.op_type.clone().unwrap_or_default(),
            publish_type: value.publish_type.clone().unwrap_or_default(),
            gray_name: value.gray_name.clone().unwrap_or_default(),
            ext_info: value.ext_info.clone().unwrap_or_default(),
            created_time: value.gmt_create.and_utc().timestamp_millis(),
            last_modified_time: value.gmt_modified.and_utc().timestamp_millis(),
            encrypted_data_key: value.encrypted_data_key.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoWrapper {
    #[serde_as(as = "DisplayFromStr")]
    pub id: Option<u64>,
    pub namespace_id: String,
    pub group_name: String,
    pub data_id: String,
    pub md5: Option<String>,
    pub r#type: String,
    pub app_name: String,
    pub create_time: i64,
    pub modify_time: i64,
}

impl From<entity::config_info::Model> for ConfigInfoWrapper {
    fn from(value: entity::config_info::Model) -> Self {
        Self {
            id: Some(value.id as u64),
            namespace_id: value.tenant_id.unwrap_or_default(),
            group_name: value.group_id.unwrap_or_default(),
            data_id: value.data_id,
            md5: value.md5,
            r#type: value.r#type.unwrap_or_default(),
            app_name: value.app_name.unwrap_or_default(),
            create_time: value
                .gmt_create
                .map_or(0, |e| e.and_utc().timestamp_millis()),
            modify_time: value
                .gmt_modified
                .map_or(0, |e| e.and_utc().timestamp_millis()),
        }
    }
}

impl From<&entity::config_info::Model> for ConfigInfoWrapper {
    fn from(value: &entity::config_info::Model) -> Self {
        Self {
            id: Some(value.id as u64),
            namespace_id: value.tenant_id.clone().unwrap_or_default(),
            group_name: value.group_id.clone().unwrap_or_default(),
            data_id: value.data_id.to_string(),
            md5: value.md5.clone(),
            r#type: value.r#type.clone().unwrap_or_default(),
            app_name: value.app_name.clone().unwrap_or_default(),
            create_time: value
                .gmt_create
                .map_or(0, |e| e.and_utc().timestamp_millis()),
            modify_time: value
                .gmt_modified
                .map_or(0, |e| e.and_utc().timestamp_millis()),
        }
    }
}

#[derive(Default)]
pub enum ConfigType {
    Properties,
    Xml,
    Json,
    #[default]
    Text,
    Html,
    Yaml,
    Toml,
}

impl ConfigType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigType::Properties => "properties",
            ConfigType::Xml => "xml",
            ConfigType::Json => "json",
            ConfigType::Text => "text",
            ConfigType::Html => "html",
            ConfigType::Yaml => "yaml",
            ConfigType::Toml => "toml",
        }
    }
}

impl Display for ConfigType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ConfigType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "properties" => Ok(ConfigType::Properties),
            "xml" => Ok(ConfigType::Xml),
            "json" => Ok(ConfigType::Json),
            "text" => Ok(ConfigType::Text),
            "html" => Ok(ConfigType::Html),
            "yaml" => Ok(ConfigType::Yaml),
            "toml" => Ok(ConfigType::Toml),
            _ => Err(format!("Invalid config type: {}", s)),
        }
    }
}

/// Basic config info for list queries
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

impl From<ConfigInfoWrapper> for ConfigBasicInfo {
    fn from(value: ConfigInfoWrapper) -> Self {
        Self {
            id: value.id.unwrap_or_default() as i64,
            namespace_id: value.namespace_id,
            group_name: value.group_name,
            data_id: value.data_id,
            md5: value.md5.unwrap_or_default(),
            r#type: value.r#type,
            app_name: value.app_name,
            create_time: value.create_time,
            modify_time: value.modify_time,
        }
    }
}

impl From<entity::config_info::Model> for ConfigBasicInfo {
    fn from(value: entity::config_info::Model) -> Self {
        ConfigInfoWrapper::from(value).into()
    }
}

/// Gray/beta configuration info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub tenant: String,
    pub gray_name: String,
    pub gray_rule: String,
    pub src_user: String,
    pub r#type: String,
}

impl From<entity::config_info_gray::Model> for ConfigGrayInfo {
    fn from(value: entity::config_info_gray::Model) -> Self {
        Self {
            id: value.id as i64,
            data_id: value.data_id,
            group: value.group_id,
            content: value.content,
            md5: value.md5.unwrap_or_default(),
            tenant: value.tenant_id.unwrap_or_default(),
            gray_name: value.gray_name,
            gray_rule: value.gray_rule,
            src_user: value.src_user.unwrap_or_default(),
            r#type: String::new(),
        }
    }
}

impl From<ConfigInfoGrayWrapper> for ConfigGrayInfo {
    fn from(value: ConfigInfoGrayWrapper) -> Self {
        Self {
            id: value.config_info.config_info_base.id,
            data_id: value.config_info.config_info_base.data_id,
            group: value.config_info.config_info_base.group,
            content: value.config_info.config_info_base.content,
            md5: value.config_info.config_info_base.md5,
            tenant: value.config_info.tenant,
            gray_name: value.gray_name,
            gray_rule: value.gray_rule,
            src_user: value.src_user,
            r#type: value.config_info.r#type,
        }
    }
}

/// Basic history info for list queries
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryBasicInfo {
    pub id: u64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub last_modified_time: i64,
}

impl From<ConfigHistoryInfo> for ConfigHistoryBasicInfo {
    fn from(value: ConfigHistoryInfo) -> Self {
        Self {
            id: value.id,
            data_id: value.data_id,
            group: value.group,
            tenant: value.tenant,
            op_type: value.op_type,
            publish_type: value.publish_type,
            gray_name: value.gray_name,
            src_user: value.src_user,
            src_ip: value.src_ip,
            created_time: value.created_time,
            last_modified_time: value.last_modified_time,
        }
    }
}

impl From<entity::his_config_info::Model> for ConfigHistoryBasicInfo {
    fn from(value: entity::his_config_info::Model) -> Self {
        ConfigHistoryInfo::from(value).into()
    }
}

/// Detailed history info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetailInfo {
    pub id: u64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub ext_info: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub last_modified_time: i64,
    pub encrypted_data_key: String,
}

impl From<ConfigHistoryInfo> for ConfigHistoryDetailInfo {
    fn from(value: ConfigHistoryInfo) -> Self {
        Self {
            id: value.id,
            data_id: value.data_id,
            group: value.group,
            tenant: value.tenant,
            content: value.content,
            md5: value.md5,
            app_name: value.app_name,
            op_type: value.op_type,
            publish_type: value.publish_type,
            gray_name: value.gray_name,
            ext_info: value.ext_info,
            src_user: value.src_user,
            src_ip: value.src_ip,
            created_time: value.created_time,
            last_modified_time: value.last_modified_time,
            encrypted_data_key: value.encrypted_data_key,
        }
    }
}

impl From<entity::his_config_info::Model> for ConfigHistoryDetailInfo {
    fn from(value: entity::his_config_info::Model) -> Self {
        ConfigHistoryInfo::from(value).into()
    }
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
        assert!(matches!(
            "properties".parse::<ConfigType>().unwrap(),
            ConfigType::Properties
        ));
        assert!(matches!(
            "xml".parse::<ConfigType>().unwrap(),
            ConfigType::Xml
        ));
        assert!(matches!(
            "json".parse::<ConfigType>().unwrap(),
            ConfigType::Json
        ));
        assert!(matches!(
            "text".parse::<ConfigType>().unwrap(),
            ConfigType::Text
        ));
        assert!(matches!(
            "yaml".parse::<ConfigType>().unwrap(),
            ConfigType::Yaml
        ));
        assert!("invalid".parse::<ConfigType>().is_err());
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
}
