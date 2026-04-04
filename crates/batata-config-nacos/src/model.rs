//! Nacos config domain models
//!
//! These models represent Nacos's native configuration management semantics.
//! NO Consul KV concepts (no flat key paths, no sessions, no CAS index).

use std::collections::HashMap;

use md5::Digest;
use serde::{Deserialize, Serialize};

// ============================================================================
// Config Item
// ============================================================================

/// A Nacos configuration item
///
/// Identified by: `namespace + group + dataId`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NacosConfig {
    /// Configuration data ID
    pub data_id: String,
    /// Group name (default: "DEFAULT_GROUP")
    pub group: String,
    /// Namespace/tenant (default: "public")
    pub namespace: String,
    /// Configuration content
    pub content: String,
    /// Content type (json, yaml, xml, properties, text, toml, html)
    pub content_type: String,
    /// MD5 hash of content for change detection
    pub md5: String,
    /// Application name
    pub app_name: String,
    /// Description
    pub description: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Source user who last modified
    pub src_user: String,
    /// Creation timestamp
    pub created_at: i64,
    /// Last modification timestamp
    pub modified_at: i64,
}

impl NacosConfig {
    /// Build config key: "namespace+group+dataId"
    pub fn config_key(namespace: &str, group: &str, data_id: &str) -> String {
        format!("{namespace}+{group}+{data_id}")
    }

    /// Calculate MD5 from content
    pub fn calculate_md5(content: &str) -> String {
        const_hex::encode(md5::Md5::digest(content))
    }

    /// Recalculate and update MD5
    pub fn refresh_md5(&mut self) {
        self.md5 = Self::calculate_md5(&self.content);
    }
}

impl Default for NacosConfig {
    fn default() -> Self {
        Self {
            data_id: String::new(),
            group: "DEFAULT_GROUP".to_string(),
            namespace: "public".to_string(),
            content: String::new(),
            content_type: "text".to_string(),
            md5: String::new(),
            app_name: String::new(),
            description: String::new(),
            tags: Vec::new(),
            src_user: String::new(),
            created_at: 0,
            modified_at: 0,
        }
    }
}

// ============================================================================
// Gray Release
// ============================================================================

/// Gray release configuration
///
/// Allows publishing config to a subset of clients before full rollout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrayConfig {
    /// Gray rule name (e.g., "beta_ips", "canary_tags")
    pub gray_name: String,
    /// Gray rule type
    pub gray_type: GrayRuleType,
    /// Rule expression (format depends on type)
    pub gray_rule_expression: String,
    /// Priority (higher = evaluated first)
    pub priority: i32,
    /// Version identifier
    pub version: String,
    /// Config content for this gray rule
    pub content: String,
    /// MD5 of gray content
    pub md5: String,
}

/// Gray rule types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrayRuleType {
    /// Match by client IP list
    Beta,
    /// Match by client labels/tags
    Tag,
    /// Match by traffic percentage
    Percentage,
    /// Match by IP CIDR range
    IpRange,
}

/// A gray rule matcher
pub trait GrayRule: Send + Sync {
    /// Check if the given client labels match this rule
    fn matches(&self, client_ip: &str, labels: &HashMap<String, String>) -> bool;

    /// Validate the rule expression
    fn is_valid(&self) -> bool;

    /// Get rule type
    fn rule_type(&self) -> GrayRuleType;

    /// Get priority
    fn priority(&self) -> i32;
}

// ============================================================================
// Config Change Notification
// ============================================================================

/// Config change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub namespace: String,
    pub group: String,
    pub data_id: String,
    pub content: Option<String>,
    pub change_type: ConfigChangeType,
}

/// Config change types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigChangeType {
    /// New config created
    Created,
    /// Existing config updated
    Updated,
    /// Config deleted
    Deleted,
}

// ============================================================================
// Config Listener (long-polling)
// ============================================================================

/// A config listener request item
///
/// Used by the long-polling listener API.
#[derive(Debug, Clone)]
pub struct ListenItem {
    pub namespace: String,
    pub group: String,
    pub data_id: String,
    /// Client-side MD5 for comparison
    pub md5: String,
}

// ============================================================================
// Import/Export
// ============================================================================

/// Import policy when conflicts exist
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPolicy {
    /// Skip existing configurations
    Skip,
    /// Overwrite existing configurations
    Overwrite,
    /// Abort if any conflict exists
    Abort,
}

/// Import result
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub created: u32,
    pub updated: u32,
    pub skipped: u32,
    pub failed: u32,
    pub errors: Vec<String>,
}

// ============================================================================
// Query
// ============================================================================

/// Query parameters for Nacos config search
#[derive(Debug, Clone, Default)]
pub struct NacosConfigQuery {
    pub namespace: Option<String>,
    pub group: Option<String>,
    pub data_id: Option<String>,
    pub app_name: Option<String>,
    pub content_pattern: Option<String>,
    pub tags: Vec<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
