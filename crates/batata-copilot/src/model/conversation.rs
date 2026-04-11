//! Conversation history models for multi-turn interactions

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Conversation history for multi-turn copilot interactions
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationHistory {
    #[serde(default)]
    pub messages: Vec<ConversationMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// A single message in conversation history.
/// Represents a user input, tool call, or model response.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    /// Message type: "user", "tool_call", "model", etc.
    #[serde(rename = "type", alias = "role")]
    pub msg_type: String,
    /// Message content
    #[serde(default)]
    pub content: String,
    /// Tool name (if type is "tool_call")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Tool input parameters (if type is "tool_call")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<HashMap<String, serde_json::Value>>,
    /// Tool output/result (if type is "tool_call")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<serde_json::Value>,
    /// Timestamp of the message (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    /// Additional metadata (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}
