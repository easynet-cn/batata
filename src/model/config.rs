use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::entity;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBase {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Default, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAllInfo {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
    pub tenant: String,
    pub app_name: String,
    pub _type: String,
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
            id: value.id,
            data_id: value.data_id,
            group: value.group_id.unwrap_or_default(),
            content: value.content.unwrap_or_default(),
            md5: value.md5.unwrap_or_default(),
            encrypted_data_key: value.encrypted_data_key.unwrap_or_default(),
            app_name: value.app_name.unwrap_or_default(),
            tenant: value.tenant_id.unwrap_or_default(),
            _type: value.r#type.unwrap_or_default(),
            create_time: value.gmt_create.unwrap().and_utc().timestamp(),
            modify_time: value.gmt_modified.unwrap().and_utc().timestamp(),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
