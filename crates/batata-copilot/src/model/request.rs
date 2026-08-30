//! Copilot request models

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ConversationHistory;

/// Skill generation request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillGenerationRequest {
    /// Background information describing the skill to generate.
    pub background_info: String,
    /// MCP tools the user selected for integration into the skill.
    #[serde(default)]
    pub selected_mcp_tools: Vec<serde_json::Value>,
    /// Optional multi-turn conversation history to inform generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_history: Option<ConversationHistory>,
    /// Additional parameters (e.g. `selectedMcpTools` from the controller form).
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

/// Skill optimization request — matches Nacos SkillOptimizationRequest
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOptimizationRequest {
    /// The skill object to optimize (JSON).
    pub skill: serde_json::Value,
    /// Optional natural-language optimization goal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimization_goal: Option<String>,
    /// Optional multi-turn conversation history to inform optimization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_history: Option<ConversationHistory>,
    /// Name of the specific file within the skill to optimize.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file_name: Option<String>,
    /// MCP tools the user selected for integration into the skill.
    #[serde(default)]
    pub selected_mcp_tools: Vec<serde_json::Value>,
    /// Additional parameters (e.g., selectedMcpTools from controller form)
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

/// Prompt optimization request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptOptimizationRequest {
    /// The original prompt to optimize.
    pub prompt: String,
    /// Optional natural-language optimization goal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimization_goal: Option<String>,
}

/// Prompt debug request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDebugRequest {
    /// The system prompt under test (alias `systemPrompt`).
    #[serde(alias = "systemPrompt")]
    pub prompt: String,
    /// The user input to send against the prompt under test.
    pub user_input: String,
}
