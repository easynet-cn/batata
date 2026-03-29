//! Conversation history models for multi-turn interactions

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

/// A single message in conversation history
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    /// "user", "assistant", "system"
    pub role: String,
    pub content: String,
}
