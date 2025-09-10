use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::entity;

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

#[serde_as]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
