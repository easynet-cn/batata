// Console API configuration model types
// These are the nested/composed types used by console API handlers for JSON responses.
// They differ from the flat persistence types by using nested composition with #[serde(flatten)].

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// Basic configuration information structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Configuration for `ConfigBasicInfo`.
pub struct ConfigBasicInfo {
    /// The config ID.
    pub id: i64,
    /// The namespace the config belongs to.
    pub namespace_id: String,
    /// The group the config belongs to.
    pub group_name: String,
    /// The data ID of the config.
    pub data_id: String,
    /// The MD5 checksum of the config content.
    pub md5: String,
    /// The config content type.
    pub r#type: String,
    /// The associated application name.
    pub app_name: String,
    /// The creation time in milliseconds.
    pub create_time: i64,
    /// The last modification time in milliseconds.
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
/// Configuration for `ConfigDetailInfo`.
pub struct ConfigDetailInfo {
    #[serde(flatten)]
    /// The basic config information.
    pub config_basic_info: ConfigBasicInfo,
    /// The config content.
    pub content: String,
    /// The description of the config.
    pub desc: String,
    /// The encrypted data key.
    pub encrypted_data_key: String,
    /// The user who created the config.
    pub create_user: String,
    /// The IP the config was created from.
    pub create_ip: String,
    /// Comma-separated config tags.
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
/// Configuration for `ConfigGrayInfo`.
pub struct ConfigGrayInfo {
    #[serde(flatten)]
    /// The underlying config detail.
    pub config_detail_info: ConfigDetailInfo,
    /// The name of the gray release.
    pub gray_name: String,
    /// The gray rule expression.
    pub gray_rule: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Configuration for `ConfigHistoryBasicInfo`.
pub struct ConfigHistoryBasicInfo {
    #[serde(flatten)]
    /// The basic config information at the time of the change.
    pub config_basic_info: ConfigBasicInfo,
    /// The IP the operation originated from.
    pub src_ip: String,
    /// The user who performed the operation.
    pub src_user: String,
    /// The operation type (e.g. insert, update, delete).
    pub op_type: String,
    /// The publish type (formal or gray).
    pub publish_type: String,
}

impl From<batata_persistence::ConfigHistoryStorageData> for ConfigHistoryBasicInfo {
    fn from(value: batata_persistence::ConfigHistoryStorageData) -> Self {
        Self {
            config_basic_info: ConfigBasicInfo {
                id: value.id,
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
/// Configuration for `ConfigHistoryDetailInfo`.
pub struct ConfigHistoryDetailInfo {
    #[serde(flatten)]
    /// The basic history information.
    pub config_history_basic_info: ConfigHistoryBasicInfo,
    /// The config content at the time of the change.
    pub content: String,
    /// The encrypted data key.
    pub encrypted_data_key: String,
    /// The gray release name, if applicable.
    pub gray_name: String,
    /// Extended information about the change.
    pub ext_info: String,
}

impl From<batata_persistence::ConfigHistoryStorageData> for ConfigHistoryDetailInfo {
    fn from(value: batata_persistence::ConfigHistoryStorageData) -> Self {
        Self {
            config_history_basic_info: ConfigHistoryBasicInfo {
                config_basic_info: ConfigBasicInfo {
                    id: value.id,
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
    /// The number of successfully imported configs.
    pub success_count: u32,
    /// The number of skipped configs.
    pub skip_count: u32,
    /// The number of failed configs.
    pub fail_count: u32,
    /// Details of the failed items.
    pub fail_data: Vec<ImportFailItem>,
}

/// Details of a failed import item
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailItem {
    /// The data ID of the failed config.
    pub data_id: String,
    /// The group of the failed config.
    pub group: String,
    /// The failure reason.
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
