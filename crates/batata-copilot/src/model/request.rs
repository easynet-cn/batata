//! Copilot request models

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ConversationHistory;

/// Skill generation request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillGenerationRequest {
    pub background_info: String,
    #[serde(default)]
    pub selected_mcp_tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_history: Option<ConversationHistory>,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

/// Skill optimization request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOptimizationRequest {
    pub skill: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimization_goal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_history: Option<ConversationHistory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file_name: Option<String>,
    #[serde(default)]
    pub selected_mcp_tools: Vec<serde_json::Value>,
}

/// Prompt optimization request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptOptimizationRequest {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimization_goal: Option<String>,
}

/// Prompt debug request
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDebugRequest {
    #[serde(alias = "systemPrompt")]
    pub prompt: String,
    pub user_input: String,
}
