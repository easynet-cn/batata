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
    pub namespace_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// manifest.json content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biz_tags: Option<String>,
    /// Resources: key = "type::name" (resource identifier)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub resource: HashMap<String, AgentSpecResource>,
}

/// A resource within an agentspec
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecResource {
    pub name: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl AgentSpecResource {
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
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
    #[serde(default)]
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biz_tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub labels: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editing_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewing_version: Option<String>,
    pub online_cnt: i64,
    pub download_count: i64,
    #[serde(default)]
    pub versions: Vec<AgentSpecVersionSummary>,
}

/// AgentSpec summary for list views
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecSummary {
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
    #[serde(default)]
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biz_tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub labels: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editing_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewing_version: Option<String>,
    pub online_cnt: i64,
    pub download_count: i64,
}

/// Version summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecVersionSummary {
    pub version: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_pipeline_info: Option<String>,
    pub download_count: i64,
}

/// Internal JSON stored in ai_resource.version_info (same structure as Skills)
pub use super::skill::SkillVersionInfo as AgentSpecVersionInfo;

/// AgentSpec basic info (for client search results)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecBasicInfo {
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pub files: Vec<AgentSpecStorageFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_key: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecStorageFile {
    pub name: String,
    #[serde(rename = "type")]
    pub file_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ============================================================================
// Request forms
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecListForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: Option<String>,
    pub search: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
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
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: Option<String>,
    #[serde(alias = "basedOnVersion")]
    pub based_on_version: Option<String>,
    #[serde(alias = "targetVersion")]
    pub target_version: Option<String>,
    #[serde(alias = "agentSpecCard")]
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
    pub namespace_id: String,
    #[serde(default, alias = "agentSpecName")]
    pub agent_spec_name: Option<String>,
    pub version: Option<String>,
    #[serde(alias = "agentSpecCard")]
    pub agent_spec_card: Option<String>,
    #[serde(default, alias = "setAsLatest")]
    pub set_as_latest: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecSubmitForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecPublishForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: String,
    pub version: String,
    #[serde(default = "default_true", alias = "updateLatestLabel")]
    pub update_latest_label: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecLabelsUpdateForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: String,
    pub labels: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecBizTagsUpdateForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: String,
    #[serde(alias = "bizTags")]
    pub biz_tags: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecOnlineForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: String,
    pub scope: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecScopeForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(alias = "agentSpecName")]
    pub agent_spec_name: String,
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecQueryForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    pub name: String,
    pub version: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecSearchForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    pub keyword: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecUploadQuery {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(default)]
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
