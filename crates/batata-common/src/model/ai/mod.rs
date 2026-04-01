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
