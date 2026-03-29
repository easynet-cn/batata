//! Skill model types — aligned with Nacos 3.x Skills API
//!
//! Skills are stored in ai_resource / ai_resource_version tables with type = "skill".
//! Each skill has governance metadata (labels, scope, biz_tags) and multiple versions
//! going through a draft → reviewing → online/offline lifecycle.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// AI resource type constant for skills
pub const SKILL_TYPE: &str = "skill";

/// Default namespace for skills
pub const SKILL_DEFAULT_NAMESPACE: &str = "public";

/// Default source for locally created skills
pub const SKILL_DEFAULT_FROM: &str = "local";

/// Default initial version
pub const SKILL_DEFAULT_VERSION: &str = "0.0.1";

/// Max upload ZIP size (10 MB)
pub const MAX_UPLOAD_ZIP_BYTES: u64 = 10 * 1024 * 1024;

// ============================================================================
// Version statuses
// ============================================================================

pub const VERSION_STATUS_DRAFT: &str = "draft";
pub const VERSION_STATUS_REVIEWING: &str = "reviewing";
pub const VERSION_STATUS_ONLINE: &str = "online";
pub const VERSION_STATUS_OFFLINE: &str = "offline";

// ============================================================================
// Resource statuses
// ============================================================================

pub const RESOURCE_STATUS_ENABLE: &str = "enable";
pub const RESOURCE_STATUS_DISABLE: &str = "disable";

// ============================================================================
// Scope values
// ============================================================================

pub const SCOPE_PUBLIC: &str = "PUBLIC";
pub const SCOPE_PRIVATE: &str = "PRIVATE";

// ============================================================================
// Domain models
// ============================================================================

/// Full Skill content (for editing/viewing a specific version)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// SKILL.md content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_md: Option<String>,
    /// Resources: key = "type::name" (resource identifier)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub resource: HashMap<String, SkillResource>,
}

/// A resource within a skill (file/tool/prompt)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillResource {
    pub name: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl SkillResource {
    pub fn resource_identifier(&self) -> String {
        format!("{}::{}", self.resource_type, self.name)
    }
}

/// Skill metadata with governance info + version summaries (admin detail view)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMeta {
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<String>,
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
    pub versions: Vec<SkillVersionSummary>,
}

/// Skill summary for list views (governance metadata, no versions)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<String>,
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

/// Version summary (nested in SkillMeta)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillVersionSummary {
    pub version: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_pipeline_info: Option<String>,
    pub download_count: i64,
}

/// Internal JSON stored in ai_resource.version_info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillVersionInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editing_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewing_version: Option<String>,
    #[serde(default)]
    pub online_cnt: i64,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Skill basic info (for client search results)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBasicInfo {
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<String>,
}

// ============================================================================
// Storage model (JSON in ai_resource_version.storage)
// ============================================================================

/// Storage info for a skill version
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillStorage {
    #[serde(default)]
    pub files: Vec<SkillStorageFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_key: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillStorageFile {
    pub name: String,
    #[serde(rename = "type")]
    pub file_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ============================================================================
// Request forms (query parameters and body)
// ============================================================================

/// Base query params for skill operations
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillForm {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(alias = "skillName")]
    pub skill_name: Option<String>,
    pub version: Option<String>,
}

/// List query params
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillListForm {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(alias = "skillName")]
    pub skill_name: Option<String>,
    pub search: Option<String>,
    pub order_by: Option<String>,
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

/// Draft create form (POST body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDraftCreateForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    /// If forking from an existing version
    pub based_on_version: Option<String>,
    /// Target version for the draft
    pub target_version: Option<String>,
    /// Full skill content as JSON string (required for new skill, optional for fork)
    pub skill_card: Option<String>,
}

/// Draft update form (PUT body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUpdateForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    pub version: Option<String>,
    /// Full skill content as JSON string
    pub skill_card: Option<String>,
    #[serde(default)]
    pub set_as_latest: bool,
}

/// Submit form (POST body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSubmitForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    pub version: String,
}

/// Publish form (POST body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPublishForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    pub version: String,
    #[serde(default = "default_true")]
    pub update_latest_label: bool,
}

/// Labels update form (PUT body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLabelsUpdateForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    /// Labels as JSON string: {"latest": "0.0.1", "stable": "0.0.0"}
    pub labels: String,
}

/// Biz tags update form (PUT body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBizTagsUpdateForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    /// Biz tags as JSON array string
    pub biz_tags: String,
}

/// Online/offline form (POST body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOnlineForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    /// "skill" for global online/offline, otherwise version-level
    pub scope: Option<String>,
    pub version: Option<String>,
}

/// Scope update form (PUT body)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillScopeForm {
    #[serde(default)]
    pub namespace_id: String,
    pub skill_name: String,
    /// "PUBLIC" or "PRIVATE"
    pub scope: String,
}

/// Client search form (for discovery)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSearchForm {
    #[serde(default)]
    pub namespace_id: String,
    pub keyword: Option<String>,
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

/// Client query form
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillQueryForm {
    #[serde(default)]
    pub namespace_id: String,
    pub name: String,
    pub version: Option<String>,
    pub label: Option<String>,
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

// ============================================================================
// Version utility functions
// ============================================================================

/// Normalize pure semver: "v1.2.3" → "1.2.3", "1.2.3" → "1.2.3"
pub fn normalize_semver(v: &str) -> &str {
    v.strip_prefix('v').unwrap_or(v)
}

/// Parse semver into (major, minor, patch) tuple
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let v = normalize_semver(v);
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = parts[2].parse().ok()?;
    Some((major, minor, patch))
}

/// Compare two semver strings
pub fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}

/// Increment patch version: "1.2.3" → "1.2.4"
pub fn next_patch_version(v: &str) -> String {
    if let Some((major, minor, patch)) = parse_semver(v) {
        format!("{}.{}.{}", major, minor, patch + 1)
    } else if let Some(n) = parse_legacy_version(v) {
        format!("v{}", n + 1)
    } else {
        format!("{}.1", v)
    }
}

// ============================================================================
// Legacy version (vN) support
// ============================================================================

/// Parse legacy version number from "vN" format (e.g., "v1" → 1, "v23" → 23)
pub fn parse_legacy_version(v: &str) -> Option<u64> {
    let stripped = v.strip_prefix('v').or_else(|| v.strip_prefix('V'))?;
    // Must be only digits (no dots)
    if stripped.is_empty() || stripped.contains('.') {
        return None;
    }
    stripped.parse().ok()
}

/// Compare versions supporting both SemVer (x.y.z) and legacy (vN).
/// SemVer versions are always considered greater than legacy versions.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some(sa), Some(sb)) => sa.cmp(&sb),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => {
            // Both legacy
            match (parse_legacy_version(a), parse_legacy_version(b)) {
                (Some(la), Some(lb)) => la.cmp(&lb),
                _ => a.cmp(b),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Version utility tests
    // ========================================================================

    #[test]
    fn test_normalize_semver_strips_v_prefix() {
        assert_eq!(normalize_semver("v1.2.3"), "1.2.3");
        assert_eq!(normalize_semver("1.2.3"), "1.2.3");
        assert_eq!(normalize_semver("v0.0.1"), "0.0.1");
    }

    #[test]
    fn test_parse_semver_valid() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("0.0.1"), Some((0, 0, 1)));
        assert_eq!(parse_semver("10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn test_parse_semver_invalid() {
        assert_eq!(parse_semver("1.2"), None);
        assert_eq!(parse_semver("abc"), None);
        assert_eq!(parse_semver("1.2.3.4"), None);
        assert_eq!(parse_semver(""), None);
        assert_eq!(parse_semver("1.a.3"), None);
    }

    #[test]
    fn test_compare_semver_ordering() {
        use std::cmp::Ordering;
        assert_eq!(compare_semver("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_semver("1.0.1", "1.0.0"), Ordering::Greater);
        assert_eq!(compare_semver("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_semver("2.0.0", "1.9.9"), Ordering::Greater);
        assert_eq!(compare_semver("0.1.0", "0.0.99"), Ordering::Greater);
        assert_eq!(compare_semver("v1.0.0", "1.0.0"), Ordering::Equal);
    }

    #[test]
    fn test_next_patch_version() {
        assert_eq!(next_patch_version("1.2.3"), "1.2.4");
        assert_eq!(next_patch_version("0.0.1"), "0.0.2");
        assert_eq!(next_patch_version("v1.0.0"), "1.0.1");
        assert_eq!(next_patch_version("1.0.99"), "1.0.100");
    }

    #[test]
    fn test_next_patch_version_invalid_fallback() {
        // Non-semver, non-legacy strings get ".1" appended
        assert_eq!(next_patch_version("abc"), "abc.1");
    }

    #[test]
    fn test_next_patch_version_legacy() {
        assert_eq!(next_patch_version("v1"), "v2");
        assert_eq!(next_patch_version("v10"), "v11");
    }

    #[test]
    fn test_parse_legacy_version() {
        assert_eq!(parse_legacy_version("v1"), Some(1));
        assert_eq!(parse_legacy_version("v23"), Some(23));
        assert_eq!(parse_legacy_version("V5"), Some(5));
        assert_eq!(parse_legacy_version("v1.2.3"), None); // SemVer, not legacy
        assert_eq!(parse_legacy_version("abc"), None);
        assert_eq!(parse_legacy_version("v"), None);
    }

    #[test]
    fn test_compare_versions_mixed() {
        use std::cmp::Ordering;
        // SemVer vs SemVer
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        // Legacy vs Legacy
        assert_eq!(compare_versions("v1", "v2"), Ordering::Less);
        assert_eq!(compare_versions("v10", "v2"), Ordering::Greater);
        // SemVer > Legacy
        assert_eq!(compare_versions("0.0.1", "v99"), Ordering::Greater);
        assert_eq!(compare_versions("v1", "1.0.0"), Ordering::Less);
    }

    // ========================================================================
    // Model serialization tests
    // ========================================================================

    #[test]
    fn test_skill_resource_identifier() {
        let res = SkillResource {
            name: "compute".to_string(),
            resource_type: "md5".to_string(),
            content: None,
            metadata: HashMap::new(),
        };
        assert_eq!(res.resource_identifier(), "md5::compute");
    }

    #[test]
    fn test_skill_version_info_defaults() {
        let vi = SkillVersionInfo::default();
        assert!(vi.editing_version.is_none());
        assert!(vi.reviewing_version.is_none());
        assert_eq!(vi.online_cnt, 0);
        assert!(vi.labels.is_empty());
    }

    #[test]
    fn test_skill_version_info_serde_roundtrip() {
        let mut labels = HashMap::new();
        labels.insert("latest".to_string(), "0.0.1".to_string());
        labels.insert("stable".to_string(), "0.0.0".to_string());

        let vi = SkillVersionInfo {
            editing_version: Some("0.0.2".to_string()),
            reviewing_version: None,
            online_cnt: 2,
            labels,
        };

        let json = serde_json::to_string(&vi).unwrap();
        let parsed: SkillVersionInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.editing_version, Some("0.0.2".to_string()));
        assert!(parsed.reviewing_version.is_none());
        assert_eq!(parsed.online_cnt, 2);
        assert_eq!(parsed.labels.len(), 2);
        assert_eq!(parsed.labels.get("latest").unwrap(), "0.0.1");
        assert_eq!(parsed.labels.get("stable").unwrap(), "0.0.0");
    }

    #[test]
    fn test_skill_summary_serde() {
        let summary = SkillSummary {
            namespace_id: "public".to_string(),
            name: "test-skill".to_string(),
            description: Some("A test skill".to_string()),
            update_time: None,
            enable: true,
            biz_tags: Some("[\"ai\",\"test\"]".to_string()),
            from: Some("local".to_string()),
            scope: Some("PRIVATE".to_string()),
            labels: HashMap::new(),
            editing_version: Some("0.0.1".to_string()),
            reviewing_version: None,
            online_cnt: 0,
            download_count: 0,
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"namespaceId\":\"public\""));
        assert!(json.contains("\"name\":\"test-skill\""));
        assert!(json.contains("\"enable\":true"));
        assert!(json.contains("\"editingVersion\":\"0.0.1\""));
        // reviewingVersion should be omitted (skip_serializing_if = "Option::is_none")
        assert!(!json.contains("reviewingVersion"));
    }

    #[test]
    fn test_skill_storage_serde_roundtrip() {
        let storage = SkillStorage {
            files: vec![
                SkillStorageFile {
                    name: "SKILL.md".to_string(),
                    file_type: "markdown".to_string(),
                    content: Some("# My Skill".to_string()),
                },
                SkillStorageFile {
                    name: "tool.json".to_string(),
                    file_type: "json".to_string(),
                    content: Some("{\"key\": \"value\"}".to_string()),
                },
            ],
            storage_key: None,
        };

        let json = serde_json::to_string(&storage).unwrap();
        let parsed: SkillStorage = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.files.len(), 2);
        assert_eq!(parsed.files[0].name, "SKILL.md");
        assert_eq!(parsed.files[0].file_type, "markdown");
        assert_eq!(parsed.files[0].content.as_deref(), Some("# My Skill"));
        assert_eq!(parsed.files[1].name, "tool.json");
        assert!(parsed.storage_key.is_none());
    }

    #[test]
    fn test_skill_form_deserialization() {
        let json = r#"{"namespaceId":"ns1","skillName":"my-skill","version":"0.0.1"}"#;
        let form: SkillForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.namespace_id, "ns1");
        assert_eq!(form.skill_name.as_deref(), Some("my-skill"));
        assert_eq!(form.version.as_deref(), Some("0.0.1"));
    }

    #[test]
    fn test_skill_list_form_defaults() {
        let json = r#"{"namespaceId":"public"}"#;
        let form: SkillListForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.namespace_id, "public");
        assert!(form.skill_name.is_none());
        assert!(form.search.is_none());
        assert_eq!(form.page_no, 1);
        assert_eq!(form.page_size, 10);
    }

    #[test]
    fn test_skill_publish_form_update_latest_default() {
        let json = r#"{"namespaceId":"public","skillName":"s1","version":"0.0.1"}"#;
        let form: SkillPublishForm = serde_json::from_str(json).unwrap();
        assert!(
            form.update_latest_label,
            "updateLatestLabel should default to true"
        );
    }

    // ========================================================================
    // Constants tests
    // ========================================================================

    #[test]
    fn test_constants() {
        assert_eq!(SKILL_TYPE, "skill");
        assert_eq!(SKILL_DEFAULT_NAMESPACE, "public");
        assert_eq!(SKILL_DEFAULT_FROM, "local");
        assert_eq!(SKILL_DEFAULT_VERSION, "0.0.1");
        assert_eq!(MAX_UPLOAD_ZIP_BYTES, 10 * 1024 * 1024);
        assert_eq!(VERSION_STATUS_DRAFT, "draft");
        assert_eq!(VERSION_STATUS_REVIEWING, "reviewing");
        assert_eq!(VERSION_STATUS_ONLINE, "online");
        assert_eq!(VERSION_STATUS_OFFLINE, "offline");
        assert_eq!(RESOURCE_STATUS_ENABLE, "enable");
        assert_eq!(RESOURCE_STATUS_DISABLE, "disable");
        assert_eq!(SCOPE_PUBLIC, "PUBLIC");
        assert_eq!(SCOPE_PRIVATE, "PRIVATE");
    }

    #[test]
    fn test_skill_with_resources_roundtrip() {
        let mut resource = HashMap::new();
        resource.insert(
            "tool::calculator".to_string(),
            SkillResource {
                name: "calculator".to_string(),
                resource_type: "tool".to_string(),
                content: Some("fn calc() {}".to_string()),
                metadata: HashMap::new(),
            },
        );

        let skill = Skill {
            namespace_id: "public".to_string(),
            name: "math-skill".to_string(),
            description: Some("Math utilities".to_string()),
            skill_md: Some("# Math Skill\nProvides calculators.".to_string()),
            resource,
        };

        let json = serde_json::to_string(&skill).unwrap();
        let parsed: Skill = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "math-skill");
        assert_eq!(parsed.description.as_deref(), Some("Math utilities"));
        assert!(parsed.skill_md.is_some());
        assert_eq!(parsed.resource.len(), 1);
        let tool = parsed.resource.get("tool::calculator").unwrap();
        assert_eq!(tool.name, "calculator");
        assert_eq!(tool.resource_type, "tool");
        assert_eq!(tool.content.as_deref(), Some("fn calc() {}"));
    }
}
