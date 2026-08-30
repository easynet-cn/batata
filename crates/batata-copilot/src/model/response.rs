//! Copilot streaming response models

use serde::{Deserialize, Serialize};

/// Type of streaming chunk — serialized as lowercase to match Nacos
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum StreamResponseType {
    /// Model reasoning/thinking content.
    #[serde(rename = "thinking")]
    Thinking,
    /// A tool invocation requested by the model.
    #[serde(rename = "tool_call")]
    ToolCall,
    /// Normal text content emitted by the model.
    #[serde(rename = "content")]
    Content,
    /// Streaming finished.
    #[serde(rename = "done")]
    Done,
}

impl StreamResponseType {
    /// Parse from string code, defaults to Content for unknown codes (matches Nacos behavior)
    pub fn from_code(code: &str) -> Self {
        match code {
            "thinking" => Self::Thinking,
            "tool_call" => Self::ToolCall,
            "done" => Self::Done,
            _ => Self::Content,
        }
    }
}

/// A single SSE streaming chunk
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    /// Kind of this chunk.
    #[serde(rename = "type")]
    pub chunk_type: StreamResponseType,
    /// The text payload of the chunk.
    #[serde(default)]
    pub chunk: String,
    /// Whether the stream has finished.
    #[serde(default)]
    pub done: bool,
    /// Optional explanation or error message (used when `done` is true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

impl StreamChunk {
    /// Create a `content` chunk with the given text.
    pub fn content(text: &str) -> Self {
        Self {
            chunk_type: StreamResponseType::Content,
            chunk: text.to_string(),
            done: false,
            explanation: None,
        }
    }

    /// Create a `thinking` chunk with the given reasoning text.
    pub fn thinking(text: &str) -> Self {
        Self {
            chunk_type: StreamResponseType::Thinking,
            chunk: text.to_string(),
            done: false,
            explanation: None,
        }
    }

    /// Create a terminal `done` chunk, optionally with an explanation.
    pub fn done(explanation: Option<&str>) -> Self {
        Self {
            chunk_type: StreamResponseType::Done,
            chunk: String::new(),
            done: true,
            explanation: explanation.map(|s| s.to_string()),
        }
    }

    /// Create a terminal `done` chunk representing an error with `message`.
    pub fn error(message: &str) -> Self {
        Self {
            chunk_type: StreamResponseType::Done,
            chunk: String::new(),
            done: true,
            explanation: Some(message.to_string()),
        }
    }

    /// Format as SSE event with "message" event name (matches Nacos SseEmitter format)
    pub fn to_sse_event(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        format!("event: message\ndata: {}\n\n", json)
    }

    /// Format as SSE event with "error" event name
    pub fn to_sse_error_event(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        format!("event: error\ndata: {}\n\n", json)
    }
}
