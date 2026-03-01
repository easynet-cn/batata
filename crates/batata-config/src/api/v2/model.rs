//! Request and response models for V2 Config API
//!
//! These models follow the Nacos V2 API specification with camelCase JSON serialization.

use batata_common::impl_or_default;
use serde::{Deserialize, Serialize};

/// Default namespace ID used when none is specified
pub const DEFAULT_NAMESPACE_ID: &str = "public";

/// Default group name used when none is specified
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

// =============================================================================
// Config API Models
// =============================================================================

/// Request parameters for getting a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGetParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Tag for config filtering (optional)
    #[serde(default)]
    pub tag: Option<String>,
}

impl ConfigGetParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for publishing a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPublishParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Config content (required)
    pub content: String,
    /// Tag for config (optional)
    #[serde(default)]
    pub tag: Option<String>,
    /// Application name (optional)
    #[serde(default)]
    pub app_name: Option<String>,
    /// Source user (optional)
    #[serde(default)]
    pub src_user: Option<String>,
    /// Config tags (comma-separated, optional)
    #[serde(default)]
    pub config_tags: Option<String>,
    /// Description (optional)
    #[serde(default)]
    pub desc: Option<String>,
    /// Usage information (optional)
    #[serde(default)]
    pub r#use: Option<String>,
    /// Effect type (optional)
    #[serde(default)]
    pub effect: Option<String>,
    /// Config type (e.g., "yaml", "json", "properties", optional)
    #[serde(default)]
    pub r#type: Option<String>,
    /// Schema for config validation (optional)
    #[serde(default)]
    pub schema: Option<String>,
    /// Encrypted data key (optional)
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
}

impl ConfigPublishParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for deleting a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDeleteParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Tag for config (optional)
    #[serde(default)]
    pub tag: Option<String>,
}

impl ConfigDeleteParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Response data for config retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    /// Config ID
    pub id: String,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Config content
    pub content: String,
    /// MD5 hash of content
    pub md5: String,
    /// Encrypted data key (if encrypted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Request parameters for searching config detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSearchDetailParam {
    /// Data ID filter (optional, supports wildcard *)
    #[serde(default)]
    pub data_id: Option<String>,
    /// Group filter (optional)
    #[serde(default)]
    pub group: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default, alias = "tenant")]
    pub namespace_id: Option<String>,
    /// Application name filter (optional)
    #[serde(default)]
    pub app_name: Option<String>,
    /// Config tags filter (comma-separated, optional)
    #[serde(default)]
    pub config_tags: Option<String>,
    /// Config type filter (comma-separated, optional)
    #[serde(default)]
    pub config_type: Option<String>,
    /// Content search filter (optional)
    #[serde(default)]
    pub content: Option<String>,
    /// Search type (blur/accurate, defaults to "blur")
    #[serde(default = "default_search")]
    pub search: String,
    /// Config detail/content search filter (optional)
    #[serde(default)]
    pub config_detail: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    /// Page size (defaults to 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

impl ConfigSearchDetailParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

// =============================================================================
// History API Models
// =============================================================================

/// Request parameters for history list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryListParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    /// Page size (defaults to 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page_no() -> u64 {
    1
}

fn default_page_size() -> u64 {
    100
}

fn default_search() -> String {
    "blur".to_string()
}

impl HistoryListParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for getting a specific history entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDetailParam {
    /// History entry ID (nid)
    pub nid: u64,
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
}

impl HistoryDetailParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for getting the previous history entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPreviousParam {
    /// Current history entry ID
    pub id: u64,
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required)
    pub group: String,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
}

impl HistoryPreviousParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Request parameters for getting configs in a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceConfigsParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
}

impl NamespaceConfigsParam {
    impl_or_default!(
        pub,
        namespace_id_or_default,
        namespace_id,
        DEFAULT_NAMESPACE_ID
    );
}

/// Response data for history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemResponse {
    /// History entry ID
    pub id: String,
    /// Last modified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<i64>,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// MD5 hash of content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// Content (only in detail view)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Source IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    /// Source user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_user: Option<String>,
    /// Operation type (I=Insert, U=Update, D=Delete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_type: Option<String>,
    /// Publish type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<String>,
    /// Gray/canary release name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gray_name: Option<String>,
    /// Extended information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_info: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Creation time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    /// Last modified time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_time: Option<String>,
    /// Encrypted data key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
}

/// Response for /v2/cs/history/detail - includes original and updated versions for comparison.
/// Matches Nacos ConfigHistoryInfoDetail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryInfoDetail {
    /// History entry ID
    pub id: String,
    /// Last modified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<i64>,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Operation type (I=Insert, U=Update, D=Delete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_type: Option<String>,
    /// Publish type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<String>,
    /// Gray/canary release name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gray_name: Option<String>,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Source IP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    /// Source user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_user: Option<String>,
    /// MD5 hash of the original version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_md5: Option<String>,
    /// Content of the original version before this change
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
    /// Encrypted data key of the original version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_encrypted_data_key: Option<String>,
    /// Extended info of the original version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_ext_info: Option<String>,
    /// MD5 hash of the updated version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_md5: Option<String>,
    /// Content of the updated version (for comparison)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_content: Option<String>,
    /// Encrypted data key of the updated version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_encrypted_data_key: Option<String>,
    /// Extended info of the updated version (note: Nacos uses "updateExtInfo" not "updatedExtInfo")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_ext_info: Option<String>,
    /// Creation time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    /// Last modified time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_time: Option<String>,
}

/// Response data for config info in namespace listing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoResponse {
    /// Config ID
    pub id: String,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Config content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// MD5 hash of content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// Encrypted data key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
    /// Last modified time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}
