//! AgentSpec model types — aligned with Nacos 3.x AgentSpec API
//!
//! AgentSpecs are stored in ai_resource / ai_resource_version tables with type = "agentspec".
//! Follows the same lifecycle as Skills (draft → reviewing → online/offline).
//! Main file: manifest.json (vs Skills' SKILL.md).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// AI resource type constant for agentspecs
pub const AGENTSPEC_TYPE: &str = "agentspec";

/// Default namespace
pub const AGENTSPEC_DEFAULT_NAMESPACE: &str = "public";

/// Default source
pub const AGENTSPEC_DEFAULT_FROM: &str = "local";

/// Default initial version
pub const AGENTSPEC_DEFAULT_VERSION: &str = "0.0.1";

/// Max upload ZIP size (50 MB — larger than Skills' 10MB)
pub const MAX_UPLOAD_ZIP_BYTES: u64 = 50 * 1024 * 1024;

/// Main file name in ZIP
pub const AGENTSPEC_MAIN_FILE: &str = "manifest.json";

// Re-use version/status/scope constants from skill module
pub use super::skill::{
    RESOURCE_STATUS_DISABLE, RESOURCE_STATUS_ENABLE, SCOPE_PRIVATE, SCOPE_PUBLIC,
    VERSION_STATUS_DRAFT, VERSION_STATUS_OFFLINE, VERSION_STATUS_ONLINE, VERSION_STATUS_REVIEWING,
};

// Re-use version utilities from skill module
pub use super::skill::{compare_versions, next_patch_version, parse_semver};

// ============================================================================
// Domain models
// ============================================================================

/// Full AgentSpec content (for editing/viewing a specific version)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpec {
    #[serde(default)]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(default)]
    /// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `description` field.
    pub description: Option<String>,
    /// manifest.json content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `biz_tags` field.
    pub biz_tags: Option<String>,
    /// Resources: key = "type::name" (resource identifier)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub resource: HashMap<String, AgentSpecResource>,
}

/// A resource within an agentspec
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecResource {
    /// The `name` field.
    pub name: String,
    #[serde(rename = "type")]
    /// The `resource_type` field.
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `content` field.
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// The `metadata` field.
    pub metadata: HashMap<String, String>,
}

impl AgentSpecResource {
    /// The `resource_identifier` method.
    pub fn resource_identifier(&self) -> String {
        if self.resource_type.is_empty() {
            self.name.clone()
        } else {
            format!("{}::{}", self.resource_type, self.name)
        }
    }
}

/// AgentSpec metadata with governance info + version summaries (admin detail view)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecMeta {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `description` field.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `update_time` field.
    pub update_time: Option<i64>,
    #[serde(default)]
    /// The `enable` field.
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `biz_tags` field.
    pub biz_tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `from` field.
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `scope` field.
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// The `labels` field.
    pub labels: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `editing_version` field.
    pub editing_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `reviewing_version` field.
    pub reviewing_version: Option<String>,
    /// The `online_cnt` field.
    pub online_cnt: i64,
    /// The `download_count` field.
    pub download_count: i64,
    #[serde(default)]
    /// The `versions` field.
    pub versions: Vec<AgentSpecVersionSummary>,
}

/// AgentSpec summary for list views
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecSummary {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `description` field.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `update_time` field.
    pub update_time: Option<i64>,
    #[serde(default)]
    /// The `enable` field.
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `biz_tags` field.
    pub biz_tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `from` field.
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `scope` field.
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// The `labels` field.
    pub labels: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `editing_version` field.
    pub editing_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `reviewing_version` field.
    pub reviewing_version: Option<String>,
    /// The `online_cnt` field.
    pub online_cnt: i64,
    /// The `download_count` field.
    pub download_count: i64,
}

/// Version summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecVersionSummary {
    /// The `version` field.
    pub version: String,
    /// The `status` field.
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `author` field.
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `description` field.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `create_time` field.
    pub create_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `update_time` field.
    pub update_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `publish_pipeline_info` field.
    pub publish_pipeline_info: Option<String>,
    /// The `download_count` field.
    pub download_count: i64,
}

/// Internal JSON stored in ai_resource.version_info (same structure as Skills)
pub use super::skill::SkillVersionInfo as AgentSpecVersionInfo;

/// AgentSpec basic info (for client search results)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecBasicInfo {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `description` field.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `update_time` field.
    pub update_time: Option<i64>,
}

// ============================================================================
// Storage model (JSON in ai_resource_version.storage)
// ============================================================================

/// Storage info for an agentspec version
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecStorage {
    #[serde(default)]
    /// The `files` field.
    pub files: Vec<AgentSpecStorageFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `storage_key` field.
    pub storage_key: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecStorageFile` struct.
pub struct AgentSpecStorageFile {
    /// The `name` field.
    pub name: String,
    #[serde(rename = "type")]
    /// The `file_type` field.
    pub file_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `content` field.
    pub content: Option<String>,
}

// ============================================================================
// Request forms
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecForm` struct.
pub struct AgentSpecForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: Option<String>,
    /// The `version` field.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecListForm` struct.
pub struct AgentSpecListForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: Option<String>,
    /// The `search` field.
    pub search: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    /// The `page_no` field.
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    /// The `page_size` field.
    pub page_size: u64,
}

/// Draft create form (POST body)
///
/// `agentSpecName` may be optional when creating new (can be in agentSpecCard JSON).
/// Required when `basedOnVersion` is set (forking).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecDraftCreateForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: Option<String>,
    #[serde(alias = "basedOnVersion")]
    /// The `based_on_version` field.
    pub based_on_version: Option<String>,
    #[serde(alias = "targetVersion")]
    /// The `target_version` field.
    pub target_version: Option<String>,
    #[serde(alias = "agentSpecCard")]
    /// The `agent_spec_card` field.
    pub agent_spec_card: Option<String>,
}

/// AgentSpec update form (PUT body)
///
/// Aligned with Nacos AgentSpecUpdateForm (extends AgentSpecDetailForm):
/// - `agentSpecName` is optional — can be resolved from `agentSpecCard` JSON content.
/// - `agentSpecCard` is required.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecUpdateForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(default, alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: Option<String>,
    /// The `version` field.
    pub version: Option<String>,
    #[serde(alias = "agentSpecCard")]
    /// The `agent_spec_card` field.
    pub agent_spec_card: Option<String>,
    #[serde(default, alias = "setAsLatest")]
    /// The `set_as_latest` field.
    pub set_as_latest: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecSubmitForm` struct.
pub struct AgentSpecSubmitForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: String,
    /// The `version` field.
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecPublishForm` struct.
pub struct AgentSpecPublishForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: String,
    /// The `version` field.
    pub version: String,
    #[serde(default = "default_true", alias = "updateLatestLabel")]
    /// The `update_latest_label` field.
    pub update_latest_label: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecLabelsUpdateForm` struct.
pub struct AgentSpecLabelsUpdateForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: String,
    /// The `labels` field.
    pub labels: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecBizTagsUpdateForm` struct.
pub struct AgentSpecBizTagsUpdateForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: String,
    #[serde(alias = "bizTags")]
    /// The `biz_tags` field.
    pub biz_tags: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecOnlineForm` struct.
pub struct AgentSpecOnlineForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: String,
    /// The `scope` field.
    pub scope: Option<String>,
    /// The `version` field.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecScopeForm` struct.
pub struct AgentSpecScopeForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    /// The `agent_spec_name` field.
    pub agent_spec_name: String,
    /// The `scope` field.
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecQueryForm` struct.
pub struct AgentSpecQueryForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `name` field.
    pub name: String,
    /// The `version` field.
    pub version: Option<String>,
    /// The `label` field.
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecSearchForm` struct.
pub struct AgentSpecSearchForm {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `keyword` field.
    pub keyword: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    /// The `page_no` field.
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    /// The `page_size` field.
    pub page_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The `AgentSpecUploadQuery` struct.
pub struct AgentSpecUploadQuery {
    #[serde(default, alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(default)]
    /// The `overwrite` field.
    pub overwrite: bool,
}

fn default_page_no() -> u64 {
    1
}
fn default_page_size() -> u64 {
    10
}
fn default_true() -> bool {
    true
}
