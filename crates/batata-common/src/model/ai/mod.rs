//! AI Capabilities Data Models
//!
//! Data models for MCP (Model Content Protocol) server registration,
//! A2A (Agent-to-Agent) communication, Prompt management, Skills, AgentSpecs,
//! and Pipeline execution.

pub mod a2a;
pub mod agentspec;
pub mod mcp;
pub mod pipeline;
pub mod prompt;
pub mod skill;
#[cfg(feature = "skill-zip")]
pub mod skill_zip;

use serde::{Deserialize, Serialize};

// =============================================================================
// Shared types used by both MCP and A2A models
// =============================================================================

/// Version index shared by every AI resource type.
///
/// This is the payload stored in `ai_resource.version_info`, aligned with Nacos
/// `com.alibaba.nacos.ai.service.resource.ResourceVersionInfo`. The latest
/// published version is tracked through the server-managed `latest` label
/// (see `AiResourceConstants.LABEL_LATEST`) rather than a dedicated field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceVersionInfo {
    /// Version currently being edited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editing_version: Option<String>,

    /// Version currently under review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewing_version: Option<String>,

    /// Number of versions currently online.
    #[serde(default)]
    pub online_cnt: i64,

    /// Label to version mappings. The `latest` entry is managed by the server.
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
}

impl ResourceVersionInfo {
    /// Return the latest published version, recorded under the `latest` label.
    pub fn latest_version(&self) -> Option<&String> {
        self.labels.get("latest")
    }

    /// Mark `version` as the latest published version.
    pub fn set_latest(&mut self, version: &str) {
        self.labels.insert("latest".to_string(), version.to_string());
    }

    /// Drop the `latest` label.
    pub fn clear_latest(&mut self) {
        self.labels.remove("latest");
    }
}

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Status unknown
    #[default]
    Unknown,
    /// Server is healthy
    Healthy,
    /// Server is unhealthy
    Unhealthy,
    /// Server is degraded
    Degraded,
}

/// Version detail for a single version entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    /// Version string (e.g., "1.0.0")
    pub version: String,

    /// Release date (ISO 8601)
    #[serde(default)]
    pub release_date: String,

    /// Whether this is the latest published version
    #[serde(default)]
    pub is_latest: bool,
}

// =============================================================================
// Shared default functions used by MCP and A2A models
// =============================================================================

pub(crate) fn default_namespace() -> String {
    "default".to_string()
}

pub(crate) fn default_version() -> String {
    "1.0.0".to_string()
}

pub(crate) fn default_page() -> u32 {
    1
}

pub(crate) fn default_page_size() -> u32 {
    20
}

pub(crate) fn default_true() -> bool {
    true
}

// =============================================================================
// Re-exports for convenience
// =============================================================================

pub use a2a::*;
pub use mcp::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// The payload is stored in `ai_resource.version_info`, so its JSON keys
    /// must match Nacos, which uses camelCase.
    #[test]
    fn resource_version_info_uses_nacos_field_names() {
        let mut info = ResourceVersionInfo::default();
        info.editing_version = Some("1.0.0".to_string());
        info.reviewing_version = Some("2.0.0".to_string());
        info.online_cnt = 3;
        info.set_latest("2.0.0");

        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["editingVersion"], "1.0.0");
        assert_eq!(value["reviewingVersion"], "2.0.0");
        assert_eq!(value["onlineCnt"], 3);
        assert_eq!(value["labels"]["latest"], "2.0.0");
        // No snake_case key must leak into the stored payload.
        assert!(value.get("editing_version").is_none());
        assert!(value.get("online_cnt").is_none());
    }

    /// Absent optional fields are omitted rather than written as null.
    #[test]
    fn resource_version_info_omits_unset_optional_fields() {
        let info = ResourceVersionInfo::default();
        let value = serde_json::to_value(&info).unwrap();
        assert!(value.get("editingVersion").is_none());
        assert!(value.get("reviewingVersion").is_none());
        assert_eq!(value["onlineCnt"], 0);
    }

    #[test]
    fn latest_label_helpers_round_trip() {
        let mut info = ResourceVersionInfo::default();
        assert_eq!(info.latest_version(), None);

        info.set_latest("1.2.3");
        assert_eq!(info.latest_version().map(String::as_str), Some("1.2.3"));

        info.set_latest("1.3.0");
        assert_eq!(info.latest_version().map(String::as_str), Some("1.3.0"));

        info.clear_latest();
        assert_eq!(info.latest_version(), None);
    }
}
