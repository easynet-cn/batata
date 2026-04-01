//! Pipeline execution model types — aligned with Nacos 3.x Pipeline API
//!
//! Pipeline executions track the review/approval workflow for Skills and AgentSpecs.
//! Stored in the pipeline_execution table.

use serde::{Deserialize, Serialize};

// ============================================================================
// Domain models
// ============================================================================

/// Pipeline execution record
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineExecution {
    pub execution_id: String,
    pub resource_type: String,
    pub resource_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub status: String,
    #[serde(default)]
    pub pipeline: Vec<PipelineNodeResult>,
    pub create_time: i64,
    pub update_time: i64,
}

/// Pipeline execution status
pub const PIPELINE_STATUS_IN_PROGRESS: &str = "IN_PROGRESS";
pub const PIPELINE_STATUS_APPROVED: &str = "APPROVED";
pub const PIPELINE_STATUS_REJECTED: &str = "REJECTED";

/// Individual pipeline node execution result
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineNodeResult {
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<String>,
    #[serde(default)]
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// "text", "json", "markdown", "html"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<Checkpoint>,
    #[serde(default)]
    pub duration_ms: i64,
}

/// Checkpoint within a pipeline node
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// ============================================================================
// Request forms
// ============================================================================

/// List pipeline executions query params
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineListForm {
    /// Required: resource type (e.g., "skill", "agentspec")
    #[serde(alias = "resourceType")]
    pub resource_type: String,
    #[serde(alias = "resourceName")]
    pub resource_name: Option<String>,
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    pub version: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u64,
}

fn default_page_no() -> u64 {
    1
}
fn default_page_size() -> u64 {
    10
}
