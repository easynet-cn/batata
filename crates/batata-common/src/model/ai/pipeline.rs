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
    /// The `execution_id` field.
    pub execution_id: String,
    /// The `resource_type` field.
    pub resource_type: String,
    /// The `resource_name` field.
    pub resource_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `namespace_id` field.
    pub namespace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `version` field.
    pub version: Option<String>,
    /// The `status` field.
    pub status: String,
    #[serde(default)]
    /// The `pipeline` field.
    pub pipeline: Vec<PipelineNodeResult>,
    /// The `create_time` field.
    pub create_time: i64,
    /// The `update_time` field.
    pub update_time: i64,
}

/// Pipeline execution status
pub const PIPELINE_STATUS_IN_PROGRESS: &str = "IN_PROGRESS";
/// Pipeline status: approved by reviewers.
pub const PIPELINE_STATUS_APPROVED: &str = "APPROVED";
/// Pipeline status: rejected by reviewers.
pub const PIPELINE_STATUS_REJECTED: &str = "REJECTED";

/// Individual pipeline node execution result
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineNodeResult {
    /// The `node_id` field.
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `executed_at` field.
    pub executed_at: Option<String>,
    #[serde(default)]
    /// The `passed` field.
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `message` field.
    pub message: Option<String>,
    /// "text", "json", "markdown", "html"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The `checkpoints` field.
    pub checkpoints: Vec<Checkpoint>,
    #[serde(default)]
    /// The `duration_ms` field.
    pub duration_ms: i64,
}

/// Checkpoint within a pipeline node
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `name` field.
    pub name: Option<String>,
    #[serde(default)]
    /// The `passed` field.
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `message` field.
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
    /// The `resource_name` field.
    pub resource_name: Option<String>,
    #[serde(alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: Option<String>,
    /// The `version` field.
    pub version: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    /// The `page_no` field.
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    /// The `page_size` field.
    pub page_size: u64,
}

fn default_page_no() -> u64 {
    1
}
fn default_page_size() -> u64 {
    10
}
