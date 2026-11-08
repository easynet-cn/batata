//! MCP SSE Client for tool discovery
//!
//! Connects to a remote MCP server via the SSE transport protocol,
//! performs the initialize handshake, and calls tools/list to discover available tools.
//! Follows the same behavior as Nacos's importToolsFromMcp.
//!
//! SSE parsing is implemented directly on reqwest's `bytes_stream()` —
//! no external SSE crate dependency required.

use std::time::Duration;

use futures::StreamExt;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::model::McpTool;

// =============================================================================
// JSON-RPC types (minimal, only what MCP needs)
// =============================================================================

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

// =============================================================================
// SSE event parser
// =============================================================================

/// A parsed SSE event.
#[derive(Debug, Default)]
struct SseEvent {
    event: String,
    data: String,
}

/// Lightweight SSE stream reader that wraps reqwest's bytes_stream.
///
/// Parses the SSE text/event-stream format per the W3C spec:
/// - Lines starting with "event:" set the event type
/// - Lines starting with "data:" append to the data buffer
/// - Empty lines dispatch the accumulated event
struct SseReader {
    buffer: String,
}

impl SseReader {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Feed raw bytes from the stream and extract complete SSE events.
    fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let text = String::from_utf8_lossy(chunk);
        self.buffer.push_str(&text);

        let mut events = Vec::new();
        let mut current_event = String::new();
        let mut current_data = String::new();

        // Process complete lines from buffer
        while let Some(pos) = self.buffer.find('\n') {
            let line = self.buffer[..pos].trim_end_matches('\r').to_string();
            self.buffer = self.buffer[pos + 1..].to_string();

            if line.is_empty() {
                // Empty line = dispatch event
                if !current_data.is_empty() || !current_event.is_empty() {
                    // Remove trailing newline from data (spec compliance)
                    if current_data.ends_with('\n') {
                        current_data.pop();
                    }
                    events.push(SseEvent {
                        event: if current_event.is_empty() {
                            "message".to_string()
                        } else {
                            current_event.clone()
                        },
                        data: current_data.clone(),
                    });
                    current_event.clear();
                    current_data.clear();
                }
            } else if let Some(value) = line.strip_prefix("event:") {
                current_event = value.trim_start().to_string();
            } else if let Some(value) = line.strip_prefix("data:") {
                if !current_data.is_empty() {
                    current_data.push('\n');
                }
                current_data.push_str(value.trim_start());
            } else if line.starts_with(':') {
                // Comment line, ignore
            } else if let Some(value) = line.strip_prefix("id:") {
                // ID field, ignore for our use case
                let _ = value;
            } else if let Some(value) = line.strip_prefix("retry:") {
                // Retry field, ignore for our use case
                let _ = value;
            }
        }

        events
    }
}

/// Maximum number of SSE bytes read during a single tool import.
///
/// Without a bound, a malicious MCP server can stream data forever and force
/// unbounded buffering in the SSE parser.
const MAX_SSE_BYTES: u64 = 8 * 1024 * 1024;

/// Add `chunk_len` to the running SSE byte total, failing once the cap is hit.
fn add_sse_bytes(total: &mut u64, chunk_len: usize) -> anyhow::Result<()> {
    *total = total.saturating_add(chunk_len as u64);
    if *total > MAX_SSE_BYTES {
        anyhow::bail!("SSE stream exceeded the {} byte limit", MAX_SSE_BYTES);
    }
    Ok(())
}

// =============================================================================
// MCP protocol types
// =============================================================================

#[derive(Deserialize)]
struct ListToolsResult {
    #[serde(default)]
    tools: Vec<McpToolSchema>,
}

#[derive(Deserialize)]
struct McpToolSchema {
    name: String,
    description: Option<String>,
    #[serde(rename = "inputSchema")]
    input_schema: Option<serde_json::Value>,
}

// =============================================================================
// Public API
// =============================================================================

/// Discover MCP tools from a remote server via SSE transport.
///
/// Connects to `base_url` + `endpoint` (default `/sse`), performs the MCP
/// initialize handshake, and returns the list of tools.
///
/// A slow or malicious MCP server must never hang an admin request, so the
/// whole handshake/discovery flow is bounded by `timeout` both at the client
/// level (connect + per-request) and by one hard `tokio::time::timeout` deadline.
pub async fn import_tools_from_mcp_sse(
    base_url: &str,
    endpoint: &str,
    auth_token: Option<&str>,
    timeout: Duration,
) -> anyhow::Result<Vec<McpTool>> {
    let sse_url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);

    // Build HTTP client with optional auth.
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(ACCEPT, "text/event-stream".parse()?);
    if let Some(token) = auth_token
        && !token.is_empty()
    {
        headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse()?);
    }

    // A per-request timeout alone does not bound the SSE read loops below
    // (the stream is long-lived), so we also apply one deadline to the entire
    // discovery flow via `tokio::time::timeout`.
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(timeout)
        .timeout(timeout)
        .build()?;

    tokio::time::timeout(
        timeout,
        discover_tools(&client, &sse_url, base_url, auth_token),
    )
    .await
    .map_err(|_| anyhow::anyhow!("MCP tool import timed out after {:?}", timeout))?
}

/// Perform the SSE handshake and JSON-RPC tool discovery.
///
/// Every await here is bounded by the caller's `tokio::time::timeout` and the
/// total number of SSE bytes read is capped by [`MAX_SSE_BYTES`].
async fn discover_tools(
    client: &reqwest::Client,
    sse_url: &str,
    base_url: &str,
    auth_token: Option<&str>,
) -> anyhow::Result<Vec<McpTool>> {
    // Step 1: Connect to SSE endpoint and read events.
    let response = client.get(sse_url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "SSE connection failed with status: {}",
            response.status()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut reader = SseReader::new();
    let mut bytes_read: u64 = 0;

    let post_url =
        wait_for_endpoint_event(&mut stream, &mut reader, base_url, &mut bytes_read).await?;

    tracing::debug!(post_url = %post_url, "Received MCP endpoint URL");

    // Step 2: Send initialize request.
    let init_request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: Some(1),
        method: "initialize",
        params: Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "batata",
                "version": "1.0.0"
            }
        })),
    };

    post_jsonrpc(client, &post_url, &init_request, auth_token).await?;
    wait_for_response(&mut stream, &mut reader, 1, &mut bytes_read).await?;

    // Step 3: Send initialized notification (no response expected).
    let initialized_notification = JsonRpcRequest {
        jsonrpc: "2.0",
        id: None,
        method: "notifications/initialized",
        params: None,
    };
    post_jsonrpc(client, &post_url, &initialized_notification, auth_token).await?;

    // Step 4: Send tools/list request.
    let list_tools_request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: Some(2),
        method: "tools/list",
        params: None,
    };
    post_jsonrpc(client, &post_url, &list_tools_request, auth_token).await?;
    let result_value = wait_for_response(&mut stream, &mut reader, 2, &mut bytes_read).await?;

    // Step 5: Parse tools.
    let list_result: ListToolsResult = serde_json::from_value(result_value)
        .map_err(|e| anyhow::anyhow!("Failed to parse tools/list result: {}", e))?;

    let tools = list_result
        .tools
        .into_iter()
        .map(|t| McpTool {
            name: t.name,
            description: t.description.unwrap_or_default(),
            input_schema: t.input_schema.unwrap_or_default(),
        })
        .collect();

    Ok(tools)
}

// =============================================================================
// Helper functions
// =============================================================================

/// Wait for the SSE "endpoint" event that tells us where to POST JSON-RPC messages.
async fn wait_for_endpoint_event(
    stream: &mut (impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin),
    reader: &mut SseReader,
    base_url: &str,
    bytes_read: &mut u64,
) -> anyhow::Result<String> {
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("SSE stream error: {}", e))?;
        add_sse_bytes(bytes_read, chunk.len())?;
        for ev in reader.feed(&chunk) {
            if ev.event == "endpoint" {
                let endpoint_path = ev.data.trim().to_string();
                if endpoint_path.starts_with("http://") || endpoint_path.starts_with("https://") {
                    return Ok(endpoint_path);
                }
                let base = base_url.trim_end_matches('/');
                return Ok(format!("{}{}", base, endpoint_path));
            }
        }
    }
    Err(anyhow::anyhow!(
        "SSE stream ended without receiving endpoint event"
    ))
}

/// Wait for a JSON-RPC response with the given id on the SSE stream.
async fn wait_for_response(
    stream: &mut (impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin),
    reader: &mut SseReader,
    expected_id: u64,
    bytes_read: &mut u64,
) -> anyhow::Result<serde_json::Value> {
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("SSE stream error: {}", e))?;
        add_sse_bytes(bytes_read, chunk.len())?;
        for ev in reader.feed(&chunk) {
            if (ev.event == "message" || ev.event.is_empty())
                && let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&ev.data)
                && resp.id == Some(expected_id)
            {
                if let Some(err) = resp.error {
                    return Err(anyhow::anyhow!(
                        "MCP server error ({}): {}",
                        err.code,
                        err.message
                    ));
                }
                return resp
                    .result
                    .ok_or_else(|| anyhow::anyhow!("Empty result from MCP server"));
            }
        }
    }
    Err(anyhow::anyhow!("SSE stream ended without response"))
}

/// Send a JSON-RPC request via HTTP POST.
async fn post_jsonrpc(
    client: &reqwest::Client,
    url: &str,
    request: &JsonRpcRequest,
    auth_token: Option<&str>,
) -> anyhow::Result<()> {
    let mut req = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .json(request);

    if let Some(token) = auth_token
        && !token.is_empty()
    {
        req = req.header(AUTHORIZATION, format!("Bearer {}", token));
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "JSON-RPC POST failed ({}): {}",
            status,
            body
        ));
    }
    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sse_bytes_accumulates_within_cap() {
        let mut total = 0u64;
        assert!(add_sse_bytes(&mut total, 1024).is_ok());
        assert_eq!(total, 1024);
        assert!(add_sse_bytes(&mut total, 2048).is_ok());
        assert_eq!(total, 3072);
    }

    #[test]
    fn add_sse_bytes_rejects_over_cap() {
        // Success path: reaching the cap exactly is still accepted.
        let mut total = MAX_SSE_BYTES - 1;
        assert!(add_sse_bytes(&mut total, 1).is_ok());
        assert_eq!(total, MAX_SSE_BYTES);
        // Guard: one more byte must be rejected. Removing the cap check in
        // `add_sse_bytes` would make this assertion fail.
        assert!(add_sse_bytes(&mut total, 1).is_err());
    }

    #[test]
    fn sse_reader_parses_endpoint_event() {
        let mut reader = SseReader::new();
        let events = reader.feed(b"event: endpoint\ndata: /messages?x=1\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "endpoint");
        assert_eq!(events[0].data, "/messages?x=1");
    }
}
