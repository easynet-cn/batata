// Configuration management API models
// This file defines request/response structures and data models for configuration management operations

use serde::{Deserialize, Serialize};

use crate::config::model::{
    ConfigAllInfo, ConfigHistoryInfo, ConfigInfoGrayWrapper, ConfigInfoWrapper,
};

// Re-export all config API types from batata-api
pub use batata_api::config::{
    ClientConfigMetricRequest, ClientConfigMetricResponse, ConfigBatchListenRequest,
    ConfigChangeBatchListenResponse, ConfigChangeClusterSyncRequest,
    ConfigChangeClusterSyncResponse, ConfigChangeNotifyRequest, ConfigChangeNotifyResponse,
    ConfigCloneInfo, ConfigContext, ConfigFuzzyWatchChangeNotifyRequest,
    ConfigFuzzyWatchChangeNotifyResponse, ConfigFuzzyWatchRequest, ConfigFuzzyWatchResponse,
    ConfigFuzzyWatchSyncRequest, ConfigFuzzyWatchSyncResponse, ConfigListenContext,
    ConfigListenerInfo, ConfigPublishRequest, ConfigPublishResponse, ConfigQueryRequest,
    ConfigQueryResponse, ConfigRemoveRequest, ConfigRemoveResponse, ConfigRequest, Context,
    FuzzyWatchNotifyRequest, MetricsKey, SameConfigPolicy,
};

// Basic configuration information structure
// This type has local From implementations that depend on local types
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

impl From<batata_config::ConfigBasicInfo> for ConfigBasicInfo {
    fn from(value: batata_config::ConfigBasicInfo) -> Self {
        Self {
            id: value.id,
            namespace_id: value.namespace_id,
            group_name: value.group_name,
            data_id: value.data_id,
            md5: value.md5,
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
