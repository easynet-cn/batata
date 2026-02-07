//! V3 Admin AI API integration tests
//!
//! Tests for /nacos/v3/admin/ai/* endpoints (MCP and A2A)

use crate::common::{TestClient, unique_test_id};
use serde_json::json;

// ========== MCP Server CRUD ==========

/// Test list MCP servers via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_list_mcp_servers() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ai/mcp/list",
            &[("pageNo", "1"), ("pageSize", "10")],
        )
        .await
        .expect("Failed to list MCP servers");

    assert_eq!(response["code"], 0, "List MCP servers should succeed");
}

/// Test create MCP server via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_create_mcp_server() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let server_name = format!("test-mcp-{}", unique_test_id());

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "serverName": server_name,
                "serverUrl": "http://localhost:9090/mcp",
                "description": "Test MCP server"
            }),
        )
        .await
        .expect("Failed to create MCP server");

    assert_eq!(response["code"], 0, "Create MCP server should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("serverName", server_name.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test get MCP server via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_mcp_server() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let server_name = format!("get-mcp-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "serverName": server_name,
                "serverUrl": "http://localhost:9090/mcp"
            }),
        )
        .await
        .expect("Failed to create MCP server");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("serverName", server_name.as_str())],
        )
        .await
        .expect("Failed to get MCP server");

    assert_eq!(response["code"], 0, "Get MCP server should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("serverName", server_name.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test update MCP server via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_update_mcp_server() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let server_name = format!("upd-mcp-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "serverName": server_name,
                "serverUrl": "http://localhost:9090/mcp"
            }),
        )
        .await
        .expect("Failed to create MCP server");

    // Update
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "serverName": server_name,
                "serverUrl": "http://localhost:9091/mcp",
                "description": "Updated MCP server"
            }),
        )
        .await
        .expect("Failed to update MCP server");

    assert_eq!(response["code"], 0, "Update MCP server should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("serverName", server_name.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test delete MCP server via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_delete_mcp_server() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let server_name = format!("del-mcp-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "serverName": server_name,
                "serverUrl": "http://localhost:9090/mcp"
            }),
        )
        .await
        .expect("Failed to create MCP server");

    // Delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("serverName", server_name.as_str())],
        )
        .await
        .expect("Failed to delete MCP server");

    assert_eq!(response["code"], 0, "Delete MCP server should succeed");
}

// ========== A2A Agent CRUD ==========

/// Test list A2A agents via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_list_a2a_agents() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ai/a2a/list",
            &[("pageNo", "1"), ("pageSize", "10")],
        )
        .await
        .expect("Failed to list A2A agents");

    assert_eq!(response["code"], 0, "List A2A agents should succeed");
}

/// Test register A2A agent via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_register_a2a_agent() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let agent_name = format!("test-agent-{}", unique_test_id());

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ai/a2a",
            &json!({
                "name": agent_name,
                "url": "http://localhost:9100/a2a",
                "description": "Test A2A agent"
            }),
        )
        .await
        .expect("Failed to register A2A agent");

    assert_eq!(response["code"], 0, "Register agent should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query("/nacos/v3/admin/ai/a2a", &[("name", agent_name.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test get A2A agent via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_a2a_agent() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let agent_name = format!("get-agent-{}", unique_test_id());

    // Register
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ai/a2a",
            &json!({
                "name": agent_name,
                "url": "http://localhost:9100/a2a"
            }),
        )
        .await
        .expect("Failed to register agent");

    // Get
    let response: serde_json::Value = client
        .get_with_query("/nacos/v3/admin/ai/a2a", &[("name", agent_name.as_str())])
        .await
        .expect("Failed to get agent");

    assert_eq!(response["code"], 0, "Get agent should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query("/nacos/v3/admin/ai/a2a", &[("name", agent_name.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test delete A2A agent via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_delete_a2a_agent() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let agent_name = format!("del-agent-{}", unique_test_id());

    // Register
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ai/a2a",
            &json!({
                "name": agent_name,
                "url": "http://localhost:9100/a2a"
            }),
        )
        .await
        .expect("Failed to register agent");

    // Delete
    let response: serde_json::Value = client
        .delete_with_query("/nacos/v3/admin/ai/a2a", &[("name", agent_name.as_str())])
        .await
        .expect("Failed to delete agent");

    assert_eq!(response["code"], 0, "Delete agent should succeed");
}
