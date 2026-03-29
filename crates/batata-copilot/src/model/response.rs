//! Copilot streaming response models

use serde::{Deserialize, Serialize};

/// Type of streaming chunk
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamResponseType {
    Thinking,
    ToolCall,
    Content,
    Done,
}

/// A single SSE streaming chunk
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    #[serde(rename = "type")]
    pub chunk_type: StreamResponseType,
    #[serde(default)]
    pub chunk: String,
    #[serde(default)]
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

impl StreamChunk {
    pub fn content(text: &str) -> Self {
        Self {
            chunk_type: StreamResponseType::Content,
            chunk: text.to_string(),
            done: false,
            explanation: None,
        }
    }

    pub fn thinking(text: &str) -> Self {
        Self {
            chunk_type: StreamResponseType::Thinking,
            chunk: text.to_string(),
            done: false,
            explanation: None,
        }
    }

    pub fn done(explanation: Option<&str>) -> Self {
        Self {
            chunk_type: StreamResponseType::Done,
            chunk: String::new(),
            done: true,
            explanation: explanation.map(|s| s.to_string()),
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            chunk_type: StreamResponseType::Done,
            chunk: String::new(),
            done: true,
            explanation: Some(message.to_string()),
        }
    }

    /// Format as SSE event string
    pub fn to_sse_event(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        format!("data: {}\n\n", json)
    }
}
