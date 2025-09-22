use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    api::{
        model::CONFIG_MODULE,
        remote::model::{Request, RequestTrait},
    },
    config::model::{ConfigAllInfo, ConfigHistoryInfo, ConfigInfoGrayWrapper, ConfigInfoWrapper},
};

fn serialize_config_module<S>(_: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(CONFIG_MODULE)
}

fn deserialize_config_module<'de, D>(_: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(CONFIG_MODULE.to_string())
}

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

impl From<ConfigAllInfo> for ConfigDetailInfo {
    fn from(value: ConfigAllInfo) -> Self {
        Self {
            config_basic_info: ConfigBasicInfo {
                id: value.config_info.config_info_base.id,
                namespace_id: value.config_info.tenant,
                group_name: value.config_info.config_info_base.group,
                data_id: value.config_info.config_info_base.data_id,
                md5: value.config_info.config_info_base.md5,
                r#type: value.config_info.r#type,
                app_name: value.config_info.app_name,
                create_time: value.create_time,
                modify_time: value.modify_time,
            },
            content: value.config_info.config_info_base.content,
            desc: value.desc,
            encrypted_data_key: value.config_info.config_info_base.encrypted_data_key,
            create_user: value.create_user,
            create_ip: value.create_ip,
            config_tags: value.config_tags,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGrayInfo {
    pub config_detail_info: ConfigDetailInfo,
    pub gray_name: String,
    pub gray_rule: String,
}

impl From<ConfigInfoGrayWrapper> for ConfigGrayInfo {
    fn from(value: ConfigInfoGrayWrapper) -> Self {
        Self {
            config_detail_info: ConfigDetailInfo {
                config_basic_info: ConfigBasicInfo {
                    id: value.config_info.config_info_base.id,
                    namespace_id: value.config_info.tenant,
                    group_name: value.config_info.config_info_base.group,
                    data_id: value.config_info.config_info_base.data_id,
                    md5: value.config_info.config_info_base.md5,
                    r#type: value.config_info.r#type,
                    app_name: value.config_info.app_name,
                    create_time: 0,
                    modify_time: value.last_modified,
                },
                content: value.config_info.config_info_base.content,
                desc: "".to_string(),
                encrypted_data_key: value.config_info.config_info_base.encrypted_data_key,
                create_user: value.src_user,
                create_ip: "".to_string(),
                config_tags: "".to_string(),
            },
            gray_name: value.gray_name,
            gray_rule: value.gray_rule,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCloneInfo {
    pub config_id: i64,
    pub target_group_name: String,
    pub target_data_id: String,
}

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

impl From<ConfigHistoryInfo> for ConfigHistoryBasicInfo {
    fn from(value: ConfigHistoryInfo) -> Self {
        Self {
            config_basic_info: ConfigBasicInfo {
                id: value.id as i64,
                namespace_id: value.tenant,
                group_name: value.group,
                data_id: value.data_id,
                md5: value.md5,
                r#type: "".to_string(),
                app_name: value.app_name,
                create_time: value.created_time,
                modify_time: value.last_modified_time,
            },
            src_ip: value.src_ip,
            src_user: value.src_user,
            op_type: value.op_type,
            publish_type: value.publish_type,
        }
    }
}

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

impl From<ConfigHistoryInfo> for ConfigHistoryDetailInfo {
    fn from(value: ConfigHistoryInfo) -> Self {
        Self {
            config_history_basic_info: ConfigHistoryBasicInfo {
                config_basic_info: ConfigBasicInfo {
                    id: value.id as i64,
                    namespace_id: value.tenant,
                    group_name: value.group,
                    data_id: value.data_id,
                    md5: value.md5,
                    r#type: String::default(),
                    app_name: value.app_name,
                    create_time: value.created_time,
                    modify_time: value.last_modified_time,
                },
                src_ip: value.src_ip,
                src_user: value.src_user,
                op_type: value.op_type,
                publish_type: value.publish_type,
            },
            content: value.content,
            encrypted_data_key: value.encrypted_data_key,
            gray_name: value.gray_name,
            ext_info: value.ext_info,
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

pub enum SameConfigPolicy {
    Abort,
    Skip,
    Overwrite,
}

impl SameConfigPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            SameConfigPolicy::Abort => "ABORT",
            SameConfigPolicy::Skip => "SKIP",
            SameConfigPolicy::Overwrite => "OVERWRITE",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "ABORT" => Ok(SameConfigPolicy::Abort),
            "SKIP" => Ok(SameConfigPolicy::Skip),
            "OVERWRITE" => Ok(SameConfigPolicy::Overwrite),
            _ => Err(format!("Invalid same config policy: {}", s)),
        }
    }
}

impl Default for SameConfigPolicy {
    fn default() -> Self {
        SameConfigPolicy::Abort
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequest {
    #[serde(flatten)]
    pub request: Request,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    #[serde(
        serialize_with = "serialize_config_module",
        deserialize_with = "deserialize_config_module"
    )]
    module: String,
}

impl ConfigRequest {
    pub fn new() -> Self {
        Self {
            request: Request::new(),
            ..Default::default()
        }
    }
}

impl RequestTrait for ConfigRequest {
    fn headers(&self) -> HashMap<String, String> {
        self.request.headers()
    }

    fn insert_headers(&mut self, headers: HashMap<String, String>) {
        self.request.insert_headers(headers);
    }

    fn request_id(&self) -> String {
        self.request.request_id.clone()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBatchListenRequest {
    pub config_request: ConfigRequest,
    pub listen: bool,
    pub config_listen_contexts: Vec<ConfigListenContext>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListenContext {
    pub group: String,
    pub md5: String,
    pub data_id: String,
    pub tenant: String,
}
