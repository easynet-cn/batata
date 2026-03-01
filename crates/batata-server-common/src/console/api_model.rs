// Console API configuration model types
// These are the nested/composed types used by console API handlers for JSON responses.
// They differ from the flat persistence types by using nested composition with #[serde(flatten)].

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// Basic configuration information structure
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

impl From<batata_persistence::ConfigStorageData> for ConfigBasicInfo {
    fn from(value: batata_persistence::ConfigStorageData) -> Self {
        Self {
            id: value.id,
            namespace_id: value.tenant,
            group_name: value.group,
            data_id: value.data_id,
            md5: value.md5,
            r#type: value.config_type,
            app_name: value.app_name,
            create_time: value.created_time,
            modify_time: value.modified_time,
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

impl From<batata_persistence::ConfigStorageData> for ConfigDetailInfo {
    fn from(value: batata_persistence::ConfigStorageData) -> Self {
        Self {
            config_basic_info: ConfigBasicInfo {
                id: value.id,
                namespace_id: value.tenant,
                group_name: value.group,
                data_id: value.data_id,
                md5: value.md5,
                r#type: value.config_type,
                app_name: value.app_name,
                create_time: value.created_time,
                modify_time: value.modified_time,
            },
            content: value.content,
            desc: value.desc,
            encrypted_data_key: value.encrypted_data_key,
            create_user: value.src_user,
            create_ip: value.src_ip,
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

impl From<batata_persistence::ConfigHistoryStorageData> for ConfigHistoryBasicInfo {
    fn from(value: batata_persistence::ConfigHistoryStorageData) -> Self {
        Self {
            config_basic_info: ConfigBasicInfo {
                id: value.id as i64,
                namespace_id: value.tenant,
                group_name: value.group,
                data_id: value.data_id,
                md5: value.md5,
                r#type: String::default(),
                app_name: value.app_name,
                create_time: value.created_time,
                modify_time: value.modified_time,
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

impl From<batata_persistence::ConfigHistoryStorageData> for ConfigHistoryDetailInfo {
    fn from(value: batata_persistence::ConfigHistoryStorageData) -> Self {
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
                    modify_time: value.modified_time,
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

/// Import operation result summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub success_count: u32,
    pub skip_count: u32,
    pub fail_count: u32,
    pub fail_data: Vec<ImportFailItem>,
}

/// Details of a failed import item
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailItem {
    pub data_id: String,
    pub group: String,
    pub reason: String,
}

/// Import conflict resolution policy
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SameConfigPolicy {
    /// Stop import on first conflict
    #[default]
    Abort,
    /// Skip conflicting configs, continue with others
    Skip,
    /// Overwrite existing configs with imported data
    Overwrite,
}

impl Display for SameConfigPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SameConfigPolicy::Abort => write!(f, "ABORT"),
            SameConfigPolicy::Skip => write!(f, "SKIP"),
            SameConfigPolicy::Overwrite => write!(f, "OVERWRITE"),
        }
    }
}

impl FromStr for SameConfigPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ABORT" => Ok(SameConfigPolicy::Abort),
            "SKIP" => Ok(SameConfigPolicy::Skip),
            "OVERWRITE" => Ok(SameConfigPolicy::Overwrite),
            _ => Err(format!(
                "Invalid policy: {}. Valid values: ABORT, SKIP, OVERWRITE",
                s
            )),
        }
    }
}
