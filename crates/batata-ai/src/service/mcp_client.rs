//! MCP SSE Client for tool discovery
//!
//! Connects to a remote MCP server via the SSE transport protocol,
//! performs the initialize handshake, and calls tools/list to discover available tools.
//! Follows the same behavior as Nacos's importToolsFromMcp.

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
// MCP protocol types
// =============================================================================

#[derive(Deserialize)]
struct ListToolsResult {
    #[serde(default)]
    tools: Vec<McpToolSpec>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolSpec {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    input_schema: serde_json::Value,
}

// =============================================================================
// Public API
// =============================================================================

/// Import tools from a remote MCP server via SSE transport.
///
/// Connects to `{base_url}{endpoint}`, performs the MCP initialize handshake,
/// then calls `tools/list` to discover available tools.
///
/// Only the `mcp-sse` transport type is supported (matching Nacos behavior).
pub async fn import_tools_from_mcp_sse(
    base_url: &str,
    endpoint: &str,
    auth_token: Option<&str>,
    timeout: Duration,
) -> anyhow::Result<Vec<McpTool>> {
    tokio::time::timeout(timeout, import_tools_inner(base_url, endpoint, auth_token))
        .await
        .map_err(|_| anyhow::anyhow!("Timed out connecting to MCP server"))?
}

async fn import_tools_inner(
    base_url: &str,
    endpoint: &str,
    auth_token: Option<&str>,
) -> anyhow::Result<Vec<McpTool>> {
    let sse_url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);

    // Build HTTP client with optional auth
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(ACCEPT, "text/event-stream".parse()?);
    if let Some(token) = auth_token {
        if !token.is_empty() {
            headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse()?);
        }
    }

    let client = reqwest::Client::builder()
        .default_headers(headers.clone())
        .build()?;

    // Step 1: Connect to SSE endpoint and wait for the "endpoint" event
    let mut es = reqwest_eventsource::EventSource::new(client.get(&sse_url))?;

    let post_url = wait_for_endpoint_event(&mut es, base_url).await?;

    tracing::debug!(post_url = %post_url, "Received MCP endpoint URL");

    // Step 2: Send initialize request
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

    post_jsonrpc(&client, &post_url, &init_request, auth_token).await?;
    wait_for_response(&mut es, 1).await?;

    // Step 3: Send initialized notification (no response expected)
    let initialized_notification = JsonRpcRequest {
        jsonrpc: "2.0",
        id: None,
        method: "notifications/initialized",
        params: None,
    };
    post_jsonrpc(&client, &post_url, &initialized_notification, auth_token).await?;

    // Step 4: Send tools/list request
    let list_tools_request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: Some(2),
        method: "tools/list",
        params: None,
    };
    post_jsonrpc(&client, &post_url, &list_tools_request, auth_token).await?;
    let result_value = wait_for_response(&mut es, 2).await?;

    // Step 5: Parse tools
    let list_result: ListToolsResult = serde_json::from_value(result_value)
        .map_err(|e| anyhow::anyhow!("Failed to parse tools/list result: {}", e))?;

    let tools = list_result
        .tools
        .into_iter()
        .map(|t| McpTool {
            name: t.name,
            description: t.description.unwrap_or_default(),
            input_schema: t.input_schema,
        })
        .collect();

    // Close SSE stream
    es.close();

    Ok(tools)
}

// =============================================================================
// Helper functions
// =============================================================================

/// Wait for the SSE "endpoint" event that tells us where to POST JSON-RPC messages.
async fn wait_for_endpoint_event(
    es: &mut reqwest_eventsource::EventSource,
    base_url: &str,
) -> anyhow::Result<String> {
    while let Some(event) = es.next().await {
        match event {
            Ok(reqwest_eventsource::Event::Open) => {
                tracing::debug!("SSE connection opened");
            }
            Ok(reqwest_eventsource::Event::Message(msg)) => {
                if msg.event == "endpoint" {
                    let endpoint_path = msg.data.trim().to_string();
                    // Resolve relative URL against base_url
                    if endpoint_path.starts_with("http://") || endpoint_path.starts_with("https://")
                    {
                        return Ok(endpoint_path);
                    }
                    let base = base_url.trim_end_matches('/');
                    return Ok(format!("{}{}", base, endpoint_path));
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("SSE connection error: {}", e));
            }
        }
    }
    Err(anyhow::anyhow!(
        "SSE stream ended without receiving endpoint event"
    ))
}

/// Wait for a JSON-RPC response with the given id on the SSE stream.
async fn wait_for_response(
    es: &mut reqwest_eventsource::EventSource,
    expected_id: u64,
) -> anyhow::Result<serde_json::Value> {
    while let Some(event) = es.next().await {
        match event {
            Ok(reqwest_eventsource::Event::Message(msg)) => {
                if msg.event == "message" || msg.event.is_empty() {
                    if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&msg.data) {
                        if resp.id == Some(expected_id) {
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
            }
            Ok(reqwest_eventsource::Event::Open) => {}
            Err(e) => {
                return Err(anyhow::anyhow!("SSE error waiting for response: {}", e));
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

    if let Some(token) = auth_token {
        if !token.is_empty() {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "MCP server returned HTTP {}: {}",
            status,
            body
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id: Some(1),
            method: "initialize",
            params: Some(serde_json::json!({"protocolVersion": "2024-11-05"})),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_jsonrpc_notification_no_id() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id: None,
            method: "notifications/initialized",
            params: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"id\""));
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn test_jsonrpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_jsonrpc_error_deserialization() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn test_list_tools_result_deserialization() {
        let json = r#"{
            "tools": [
                {
                    "name": "get_weather",
                    "description": "Get weather info",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "city": {"type": "string"}
                        }
                    }
                },
                {
                    "name": "search",
                    "inputSchema": {}
                }
            ]
        }"#;
        let result: ListToolsResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.tools.len(), 2);
        assert_eq!(result.tools[0].name, "get_weather");
        assert_eq!(
            result.tools[0].description,
            Some("Get weather info".to_string())
        );
        assert_eq!(result.tools[1].name, "search");
        assert!(result.tools[1].description.is_none());
    }

    #[test]
    fn test_mcp_tool_conversion() {
        let spec = McpToolSpec {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let tool = McpTool {
            name: spec.name,
            description: spec.description.unwrap_or_default(),
            input_schema: spec.input_schema,
        };
        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, "A test tool");
    }

    #[test]
    fn test_empty_tools_list() {
        let json = r#"{"tools": []}"#;
        let result: ListToolsResult = serde_json::from_str(json).unwrap();
        assert!(result.tools.is_empty());
    }

    #[test]
    fn test_url_construction() {
        // Absolute URL
        let base = "http://localhost:3000";
        let endpoint = "/sse";
        let url = format!("{}{}", base.trim_end_matches('/'), endpoint);
        assert_eq!(url, "http://localhost:3000/sse");

        // Base URL with trailing slash
        let base = "http://localhost:3000/";
        let url = format!("{}{}", base.trim_end_matches('/'), endpoint);
        assert_eq!(url, "http://localhost:3000/sse");
    }

    #[tokio::test]
    async fn test_timeout() {
        // Connecting to a non-existent server should timeout
        let result = import_tools_from_mcp_sse(
            "http://127.0.0.1:19999",
            "/sse",
            None,
            Duration::from_millis(100),
        )
        .await;
        assert!(result.is_err());
    }
}
