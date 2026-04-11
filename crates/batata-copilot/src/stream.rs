//! LLM streaming client — OpenAI-compatible Chat Completions API with SSE
//!
//! Supports DashScope, OpenAI, and any OpenAI-compatible provider.
//! Uses the /v1/chat/completions endpoint with `stream: true`.

use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;
use tracing::{debug, warn};

use crate::model::response::StreamChunk;

/// OpenAI Chat Completions request body
#[derive(Debug, Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Streaming SSE response chunk from OpenAI-compatible API
#[derive(Debug, Deserialize)]
struct ChatCompletionsChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    // role field exists in API but is unused — suppress warning
    #[serde(default)]
    #[allow(dead_code)]
    role: Option<String>,
}

/// Create a streaming LLM chat completion.
/// All parameters are owned to ensure the stream outlives local variables.
pub async fn chat_stream(
    base_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
    user_message: String,
) -> anyhow::Result<impl Stream<Item = StreamChunk>> {
    let client = Client::new();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_message,
        },
    ];

    let body = ChatCompletionsRequest {
        model: model.clone(),
        messages,
        stream: true,
        temperature: Some(0.7),
        max_tokens: Some(4096),
    };

    debug!(url = %url, model = %model, "Starting LLM chat stream");

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "LLM API error: {} - {}",
            status,
            error_body.chars().take(500).collect::<String>()
        );
    }

    let byte_stream = response.bytes_stream();

    Ok(parse_sse_stream(byte_stream))
}

/// Create a streaming LLM chat with conversation history
pub async fn chat_stream_with_history(
    base_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
    history: Vec<(String, String)>, // (role, content) pairs
    user_message: String,
) -> anyhow::Result<impl Stream<Item = StreamChunk>> {
    let client = Client::new();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    }];

    for (role, content) in history {
        messages.push(ChatMessage { role, content });
    }

    messages.push(ChatMessage {
        role: "user".to_string(),
        content: user_message,
    });

    let body = ChatCompletionsRequest {
        model,
        messages,
        stream: true,
        temperature: Some(0.7),
        max_tokens: Some(4096),
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "LLM API error: {} - {}",
            status,
            error_body.chars().take(500).collect::<String>()
        );
    }

    Ok(parse_sse_stream(response.bytes_stream()))
}

/// Parse OpenAI-style SSE stream into StreamChunk events
fn parse_sse_stream(
    byte_stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = StreamChunk> {
    let buffer = String::new();

    futures::stream::unfold(
        (Box::pin(byte_stream), buffer),
        |(mut stream, mut buf)| async move {
            loop {
                // Try to extract a complete SSE event from buffer
                if let Some(pos) = buf.find("\n\n") {
                    let event = buf[..pos].to_string();
                    buf = buf[pos + 2..].to_string();

                    if let Some(chunk) = parse_sse_event(&event) {
                        return Some((chunk, (stream, buf)));
                    }
                    continue;
                }

                // Need more data
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Some(Err(e)) => {
                        warn!("SSE stream error: {}", e);
                        return Some((
                            StreamChunk::error(&format!("Stream error: {}", e)),
                            (stream, buf),
                        ));
                    }
                    None => {
                        // Stream ended — emit done if buffer has remaining data
                        if !buf.trim().is_empty()
                            && let Some(chunk) = parse_sse_event(&buf)
                        {
                            buf.clear();
                            return Some((chunk, (stream, buf)));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

/// Parse a single SSE event line into a StreamChunk
fn parse_sse_event(event: &str) -> Option<StreamChunk> {
    for line in event.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();

            // [DONE] marker
            if data == "[DONE]" {
                return Some(StreamChunk::done(None));
            }

            // Parse JSON chunk
            match serde_json::from_str::<ChatCompletionsChunk>(data) {
                Ok(chunk) => {
                    if let Some(choice) = chunk.choices.first() {
                        // Check for reasoning/thinking content
                        if let Some(ref thinking) = choice.delta.reasoning_content
                            && !thinking.is_empty()
                        {
                            return Some(StreamChunk::thinking(thinking));
                        }

                        // Regular content
                        if let Some(ref content) = choice.delta.content
                            && !content.is_empty()
                        {
                            return Some(StreamChunk::content(content));
                        }

                        // Finish reason
                        if choice.finish_reason.is_some() {
                            return None; // Will get [DONE] next
                        }
                    }
                }
                Err(e) => {
                    // Skip unparseable chunks (role-only, empty, etc.)
                    if !data.is_empty() && data != "{}" {
                        debug!(
                            "Skipping unparseable SSE data: {} ({})",
                            data.chars().take(100).collect::<String>(),
                            e
                        );
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::response::StreamResponseType;

    #[test]
    fn test_parse_sse_event_content() {
        let event = r#"data: {"choices":[{"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk = parse_sse_event(event).unwrap();
        assert_eq!(chunk.chunk, "Hello");
        assert!(!chunk.done);
    }

    #[test]
    fn test_parse_sse_event_thinking() {
        let event = r#"data: {"choices":[{"delta":{"reasoning_content":"Let me think..."},"finish_reason":null}]}"#;
        let chunk = parse_sse_event(event).unwrap();
        assert_eq!(chunk.chunk, "Let me think...");
        assert_eq!(chunk.chunk_type, StreamResponseType::Thinking);
    }

    #[test]
    fn test_parse_sse_event_done() {
        let event = "data: [DONE]";
        let chunk = parse_sse_event(event).unwrap();
        assert!(chunk.done);
    }

    #[test]
    fn test_stream_chunk_to_sse() {
        let chunk = StreamChunk::content("hello");
        let sse = chunk.to_sse_event();
        // SSE format: "event: message\ndata: {json}\n\n" (matches Nacos SseEmitter)
        assert!(sse.starts_with("event: message\n"));
        assert!(sse.contains("data: "));
        assert!(sse.ends_with("\n\n"));
        assert!(sse.contains("\"chunk\":\"hello\""));
        // StreamResponseType serialized as lowercase
        assert!(sse.contains("\"type\":\"content\""));
    }

    #[test]
    fn test_stream_chunk_to_sse_error() {
        let chunk = StreamChunk::error("something went wrong");
        let sse = chunk.to_sse_error_event();
        assert!(sse.starts_with("event: error\n"));
        assert!(sse.contains("data: "));
        assert!(sse.ends_with("\n\n"));
        assert!(sse.contains("\"done\":true"));
        assert!(sse.contains("\"type\":\"done\""));
    }
}
