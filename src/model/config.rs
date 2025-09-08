use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::entity;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
    pub tenant: String,
    pub app_name: String,
    pub _type: String,
}

impl From<entity::config_info::Model> for ConfigInfo {
    fn from(value: entity::config_info::Model) -> Self {
        Self {
            id: value.id,
            data_id: value.data_id,
            group: value.group_id.unwrap_or_default(),
            content: value.content.unwrap_or_default(),
            md5: value.md5.unwrap_or_default(),
            encrypted_data_key: value.encrypted_data_key.unwrap_or_default(),
            tenant: value.tenant_id.unwrap_or_default(),
            app_name: value.app_name.unwrap_or_default(),
            _type: value.r#type.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoWrapper {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
    pub tenant: String,
    pub app_name: String,
    pub _type: String,
    pub last_modified: i64,
}

impl From<entity::config_info::Model> for ConfigInfoWrapper {
    fn from(value: entity::config_info::Model) -> Self {
        Self {
            id: value.id,
            data_id: value.data_id,
            group: value.group_id.unwrap_or_default(),
            content: value.content.unwrap_or_default(),
            md5: value.md5.unwrap_or_default(),
            encrypted_data_key: value.encrypted_data_key.unwrap_or_default(),
            tenant: value.tenant_id.clone().unwrap_or_default(),
            app_name: value.app_name.clone().unwrap_or_default(),
            _type: value.r#type.clone().unwrap_or_default(),
            last_modified: value.gmt_modified.unwrap().and_utc().timestamp(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoStateWrapper {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub last_modified: i64,
    pub md5: String,
}

impl From<entity::config_info::Model> for ConfigInfoStateWrapper {
    fn from(value: entity::config_info::Model) -> Self {
        Self {
            id: value.id,
            data_id: value.data_id,
            group: value.group_id.unwrap_or_default(),
            tenant: value.tenant_id.unwrap_or_default(),
            last_modified: value.gmt_modified.unwrap().and_utc().timestamp(),
            md5: value.md5.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryInfo {
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
    pub created_time: NaiveDateTime,
    pub last_modified_time: NaiveDateTime,
    pub encrypted_data_key: String,
}

impl From<entity::his_config_info::Model> for ConfigHistoryInfo {
    fn from(value: entity::his_config_info::Model) -> Self {
        Self {
            id: value.nid,
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
            created_time: value.gmt_create,
            last_modified_time: value.gmt_modified,
            encrypted_data_key: value.encrypted_data_key,
        }
    }
}

pub enum ConfigType {
    Properties,
    Xml,
    Json,
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

    pub fn from_str(s: &str) -> Result<Self, String> {
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

impl Default for ConfigType {
    fn default() -> Self {
        ConfigType::Text
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
        ConfigType::from_str(s)
    }
}
