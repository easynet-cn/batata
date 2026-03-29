//! Prompt model types — aligned with Nacos 3.2 API
//!
//! Storage: Prompts are stored as configs in the config service with group `nacos-ai-prompt`.
//! No dedicated database tables — all data is JSON configs with naming conventions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Client-facing model
// ============================================================================

/// Client-facing prompt object returned by gRPC QueryPromptResponse
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub prompt_key: String,
    pub version: String,
    pub template: String,
    pub md5: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<PromptVariable>>,
}

impl Prompt {
    /// Render the template with variable substitution.
    /// Uses `{{varName}}` placeholder syntax.
    pub fn render(&self, user_variables: &HashMap<String, String>) -> String {
        let mut merged = HashMap::new();

        // Apply defaults first
        if let Some(ref vars) = self.variables {
            for var in vars {
                if let Some(ref default) = var.default_value {
                    merged.insert(var.name.clone(), default.clone());
                }
            }
        }

        // Override with user values
        for (k, v) in user_variables {
            merged.insert(k.clone(), v.clone());
        }

        // Replace placeholders
        let mut result = self.template.clone();
        for (name, value) in &merged {
            let placeholder = format!("{{{{{}}}}}", name);
            result = result.replace(&placeholder, value);
        }
        result
    }
}

/// Variable definition for a prompt template
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptVariable {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ============================================================================
// Admin/metadata models
// ============================================================================

/// Prompt metadata summary (for list views)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMetaSummary {
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    pub prompt_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub biz_tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<i64>,
}

/// Full prompt metadata (extends summary with versions and labels)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMetaInfo {
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    pub prompt_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub biz_tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<i64>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Prompt version summary (for version list views)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptVersionSummary {
    pub prompt_key: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<i64>,
}

/// Full prompt version info (extends summary with content)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptVersionInfo {
    pub prompt_key: String,
    pub version: String,
    pub template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<PromptVariable>>,
}

impl PromptVersionInfo {
    /// Convert to client-facing Prompt
    pub fn to_client_prompt(&self) -> Prompt {
        Prompt {
            prompt_key: self.prompt_key.clone(),
            version: self.version.clone(),
            template: self.template.clone(),
            md5: self.md5.clone().unwrap_or_default(),
            variables: self.variables.clone(),
        }
    }
}

// ============================================================================
// Internal storage models (stored as JSON configs)
// ============================================================================

/// Prompt descriptor — metadata stored independently from versions.
/// DataId: `{promptKey}.descriptor.json`
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDescriptor {
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    pub prompt_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub biz_tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<i64>,
}

/// Label-to-version mapping — runtime label resolution.
/// DataId: `{promptKey}.label-version-mapping.json`
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptLabelVersionMapping {
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    pub prompt_key: String,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<i64>,
}

fn default_schema_version() -> i32 {
    1
}

// ============================================================================
// DataId utilities (Nacos naming conventions)
// ============================================================================

/// Fixed group for all prompt configs
pub const PROMPT_GROUP: &str = "nacos-ai-prompt";

/// Build DataId for latest version mirror: `{promptKey}.json`
pub fn build_latest_data_id(prompt_key: &str) -> String {
    format!("{}.json", prompt_key)
}

/// Build DataId for a specific version: `{promptKey}.{version}.json`
pub fn build_version_data_id(prompt_key: &str, version: &str) -> String {
    format!("{}.{}.json", prompt_key, version)
}

/// Build DataId for descriptor: `{promptKey}.descriptor.json`
pub fn build_descriptor_data_id(prompt_key: &str) -> String {
    format!("{}.descriptor.json", prompt_key)
}

/// Build DataId for label-version mapping: `{promptKey}.label-version-mapping.json`
pub fn build_label_version_mapping_data_id(prompt_key: &str) -> String {
    format!("{}.label-version-mapping.json", prompt_key)
}

/// Check if a DataId is a descriptor
pub fn is_descriptor_data_id(data_id: &str) -> bool {
    data_id.ends_with(".descriptor.json")
}

/// Extract promptKey from descriptor DataId
pub fn extract_prompt_key_from_descriptor(data_id: &str) -> Option<&str> {
    data_id.strip_suffix(".descriptor.json")
}

// ============================================================================
// Version utilities
// ============================================================================

/// Validate semantic version format: `major.minor.patch`
pub fn is_valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
}

/// Compare two semantic versions. Returns Ordering.
pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };
    parse(v1).cmp(&parse(v2))
}

/// Resolve target version from version/label/latest.
/// Precedence: version > label > latestVersion
pub fn resolve_target_version(
    mapping: &PromptLabelVersionMapping,
    version: Option<&str>,
    label: Option<&str>,
) -> Option<String> {
    // Version takes highest priority — must exist in versions list
    if let Some(v) = version {
        if !v.is_empty() {
            return if mapping.versions.contains(&v.to_string()) {
                Some(v.to_string())
            } else {
                None // Version explicitly requested but not found
            };
        }
    }
    // Label fallback — must exist in labels map
    if let Some(l) = label {
        if !l.is_empty() {
            return mapping.labels.get(l).cloned();
        }
    }
    // Latest fallback
    mapping.latest_version.clone()
}

/// Compose PromptMetaInfo from descriptor + label-version mapping
pub fn compose_meta_info(
    descriptor: &PromptDescriptor,
    mapping: &PromptLabelVersionMapping,
) -> PromptMetaInfo {
    PromptMetaInfo {
        schema_version: descriptor.schema_version,
        prompt_key: descriptor.prompt_key.clone(),
        description: descriptor.description.clone(),
        biz_tags: descriptor.biz_tags.clone(),
        latest_version: mapping.latest_version.clone(),
        gmt_modified: descriptor.gmt_modified,
        versions: mapping.versions.clone(),
        labels: mapping.labels.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_version() {
        assert!(is_valid_version("1.0.0"));
        assert!(is_valid_version("0.1.0"));
        assert!(is_valid_version("10.20.30"));
        assert!(!is_valid_version("1.0"));
        assert!(!is_valid_version("v1.0.0"));
        assert!(!is_valid_version("1.0.0.0"));
        assert!(!is_valid_version(""));
    }

    #[test]
    fn test_compare_versions() {
        assert_eq!(
            compare_versions("1.0.0", "1.0.0"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_versions("1.1.0", "1.0.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("1.0.0", "2.0.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("1.0.1", "1.0.0"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_data_id_builders() {
        assert_eq!(build_latest_data_id("greeting"), "greeting.json");
        assert_eq!(
            build_version_data_id("greeting", "1.0.0"),
            "greeting.1.0.0.json"
        );
        assert_eq!(
            build_descriptor_data_id("greeting"),
            "greeting.descriptor.json"
        );
        assert_eq!(
            build_label_version_mapping_data_id("greeting"),
            "greeting.label-version-mapping.json"
        );
    }

    #[test]
    fn test_is_descriptor_data_id() {
        assert!(is_descriptor_data_id("greeting.descriptor.json"));
        assert!(!is_descriptor_data_id("greeting.json"));
        assert!(!is_descriptor_data_id("greeting.1.0.0.json"));
    }

    #[test]
    fn test_extract_prompt_key() {
        assert_eq!(
            extract_prompt_key_from_descriptor("greeting.descriptor.json"),
            Some("greeting")
        );
        assert_eq!(
            extract_prompt_key_from_descriptor("my-prompt.descriptor.json"),
            Some("my-prompt")
        );
        assert_eq!(extract_prompt_key_from_descriptor("greeting.json"), None);
    }

    #[test]
    fn test_prompt_render() {
        let prompt = Prompt {
            template: "Hello {{name}}, welcome to {{place}}!".to_string(),
            variables: Some(vec![
                PromptVariable {
                    name: "name".to_string(),
                    default_value: Some("User".to_string()),
                    description: None,
                },
                PromptVariable {
                    name: "place".to_string(),
                    default_value: Some("Batata".to_string()),
                    description: None,
                },
            ]),
            ..Default::default()
        };

        // With defaults only
        let result = prompt.render(&HashMap::new());
        assert_eq!(result, "Hello User, welcome to Batata!");

        // With user override
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        let result = prompt.render(&vars);
        assert_eq!(result, "Hello Alice, welcome to Batata!");
    }

    #[test]
    fn test_resolve_target_version() {
        let mapping = PromptLabelVersionMapping {
            prompt_key: "test".to_string(),
            versions: vec!["1.0.0".to_string(), "1.1.0".to_string()],
            labels: {
                let mut m = HashMap::new();
                m.insert("prod".to_string(), "1.0.0".to_string());
                m.insert("dev".to_string(), "1.1.0".to_string());
                m
            },
            latest_version: Some("1.1.0".to_string()),
            ..Default::default()
        };

        // Version takes priority
        assert_eq!(
            resolve_target_version(&mapping, Some("1.0.0"), Some("dev")),
            Some("1.0.0".to_string())
        );

        // Label fallback
        assert_eq!(
            resolve_target_version(&mapping, None, Some("prod")),
            Some("1.0.0".to_string())
        );

        // Latest fallback
        assert_eq!(
            resolve_target_version(&mapping, None, None),
            Some("1.1.0".to_string())
        );

        // Unknown version returns None (Nacos throws NOT_FOUND)
        assert_eq!(resolve_target_version(&mapping, Some("9.9.9"), None), None);
    }

    #[test]
    fn test_version_info_to_client_prompt() {
        let info = PromptVersionInfo {
            prompt_key: "greeting".to_string(),
            version: "1.0.0".to_string(),
            template: "Hello {{name}}!".to_string(),
            md5: Some("abc123".to_string()),
            variables: Some(vec![PromptVariable {
                name: "name".to_string(),
                default_value: Some("World".to_string()),
                description: Some("Name to greet".to_string()),
            }]),
            ..Default::default()
        };

        let prompt = info.to_client_prompt();
        assert_eq!(prompt.prompt_key, "greeting");
        assert_eq!(prompt.version, "1.0.0");
        assert_eq!(prompt.md5, "abc123");
        assert!(prompt.variables.is_some());
    }

    #[test]
    fn test_compose_meta_info() {
        let descriptor = PromptDescriptor {
            prompt_key: "test".to_string(),
            description: Some("Test prompt".to_string()),
            biz_tags: vec!["demo".to_string()],
            gmt_modified: Some(1700000000000),
            ..Default::default()
        };

        let mapping = PromptLabelVersionMapping {
            prompt_key: "test".to_string(),
            versions: vec!["1.0.0".to_string()],
            latest_version: Some("1.0.0".to_string()),
            labels: {
                let mut m = HashMap::new();
                m.insert("latest".to_string(), "1.0.0".to_string());
                m
            },
            ..Default::default()
        };

        let meta = compose_meta_info(&descriptor, &mapping);
        assert_eq!(meta.prompt_key, "test");
        assert_eq!(meta.description, Some("Test prompt".to_string()));
        assert_eq!(meta.versions.len(), 1);
        assert_eq!(meta.latest_version, Some("1.0.0".to_string()));
        assert!(meta.labels.contains_key("latest"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let prompt = Prompt {
            prompt_key: "test".to_string(),
            version: "1.0.0".to_string(),
            template: "Hello {{name}}".to_string(),
            md5: "abc".to_string(),
            variables: Some(vec![PromptVariable {
                name: "name".to_string(),
                default_value: Some("World".to_string()),
                description: Some("Name".to_string()),
            }]),
        };

        let json = serde_json::to_string(&prompt).unwrap();
        assert!(json.contains("\"promptKey\":\"test\""));
        assert!(json.contains("\"defaultValue\":\"World\""));

        let deserialized: Prompt = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.prompt_key, "test");
        assert_eq!(deserialized.variables.unwrap()[0].name, "name");
    }
}
