//! V3 Admin AI API integration tests
//!
//! Tests for /nacos/v3/admin/ai/* endpoints (MCP and A2A)
//!
//! The request/response shapes follow the upstream Nacos 3.x admin API:
//! - `endpointSpecification` is a JSON *string* carrying a Nacos
//!   `McpEndpointSpec` (`{"type":"direct","data":{"address":..,"port":..}}`).
//! - MCP get/delete use query params (`mcpName`), not path params.
//! - A2A get/delete use query params (`namespaceId` + `agentName`), not path params.

use batata_integration_tests::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient, unique_test_id,
};
use serde_json::json;

async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

// ========== MCP Server CRUD ==========

/// Test list MCP servers via V3 Admin API
#[tokio::test]

async fn test_v3_admin_list_mcp_servers() {
    let client = authenticated_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ai/mcp/list",
            &[("page", "1"), ("pageSize", "10")],
        )
        .await
        .expect("Failed to list MCP servers");

    assert_eq!(response["code"], 0, "List MCP servers should succeed");
}

/// Test create MCP server via V3 Admin API
#[tokio::test]

async fn test_v3_admin_create_mcp_server() {
    let client = authenticated_client().await;
    let server_name = format!("test-mcp-{}", unique_test_id());

    // `endpointSpecification` is a JSON string carrying a Nacos McpEndpointSpec.
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "mcpName": server_name,
                "endpointSpecification": r#"{"type":"direct","data":{"address":"localhost","port":9090}}"#,
                "description": "Test MCP server"
            }),
        )
        .await
        .expect("Failed to create MCP server");

    assert_eq!(response["code"], 0, "Create MCP server should succeed");

    // Cleanup (query params, matching Nacos)
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("namespaceId", "default"), ("mcpName", server_name.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test get MCP server via V3 Admin API
#[tokio::test]

async fn test_v3_admin_get_mcp_server() {
    let client = authenticated_client().await;
    let server_name = format!("get-mcp-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "mcpName": server_name,
                "endpointSpecification": r#"{"type":"direct","data":{"address":"localhost","port":9090}}"#
            }),
        )
        .await
        .expect("Failed to create MCP server");

    // Get (query params, matching Nacos: /mcp?mcpName=...)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("namespaceId", "default"), ("mcpName", server_name.as_str())],
        )
        .await
        .expect("Failed to get MCP server");

    assert_eq!(response["code"], 0, "Get MCP server should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("namespaceId", "default"), ("mcpName", server_name.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test delete MCP server via V3 Admin API
#[tokio::test]

async fn test_v3_admin_delete_mcp_server() {
    let client = authenticated_client().await;
    let server_name = format!("del-mcp-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ai/mcp",
            &json!({
                "mcpName": server_name,
                "endpointSpecification": r#"{"type":"direct","data":{"address":"localhost","port":9090}}"#
            }),
        )
        .await
        .expect("Failed to create MCP server");

    // Delete (query params, matching Nacos: /mcp?mcpName=...)
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/mcp",
            &[("namespaceId", "default"), ("mcpName", server_name.as_str())],
        )
        .await
        .expect("Failed to delete MCP server");

    assert_eq!(response["code"], 0, "Delete MCP server should succeed");
}

// ========== A2A Agent CRUD ==========

/// Test list A2A agents via V3 Admin API
#[tokio::test]

async fn test_v3_admin_list_a2a_agents() {
    let client = authenticated_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ai/a2a/list",
            &[("page", "1"), ("pageSize", "10")],
        )
        .await
        .expect("Failed to list A2A agents");

    assert_eq!(response["code"], 0, "List A2A agents should succeed");
}

/// Test register A2A agent via V3 Admin API
#[tokio::test]

async fn test_v3_admin_register_a2a_agent() {
    let client = authenticated_client().await;
    let agent_name = format!("test-agent-{}", unique_test_id());
    let agent_card = json!({
        "name": agent_name,
        "endpoint": "http://localhost:9100/a2a",
        "description": "Test A2A agent"
    })
    .to_string();

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ai/a2a",
            &[
            ("agentName", agent_name.as_str()),
            ("agentCard", agent_card.as_str()),
            ],
        )
        .await
        .expect("Failed to register A2A agent");

    assert_eq!(response["code"], 0, "Register agent should succeed");

    // Cleanup (query params, matching Nacos)
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/a2a",
            &[("namespaceId", "public"), ("agentName", agent_name.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test get A2A agent via V3 Admin API
#[tokio::test]

async fn test_v3_admin_get_a2a_agent() {
    let client = authenticated_client().await;
    let agent_name = format!("get-agent-{}", unique_test_id());
    let agent_card = json!({
        "name": agent_name,
        "endpoint": "http://localhost:9100/a2a",
        "description": "Test A2A agent"
    })
    .to_string();

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ai/a2a",
            &[
            ("agentName", agent_name.as_str()),
            ("agentCard", agent_card.as_str()),
            ],
        )
        .await
        .expect("Failed to register agent");

    // Get (query params, matching Nacos: /a2a?agentName=...)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ai/a2a",
            &[("namespaceId", "public"), ("agentName", agent_name.as_str())],
        )
        .await
        .expect("Failed to get agent");

    assert_eq!(response["code"], 0, "Get agent should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/a2a",
            &[("namespaceId", "public"), ("agentName", agent_name.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test delete A2A agent via V3 Admin API
#[tokio::test]

async fn test_v3_admin_delete_a2a_agent() {
    let client = authenticated_client().await;
    let agent_name = format!("del-agent-{}", unique_test_id());
    let agent_card = json!({
        "name": agent_name,
        "endpoint": "http://localhost:9100/a2a",
        "description": "Test A2A agent"
    })
    .to_string();

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ai/a2a",
            &[
            ("agentName", agent_name.as_str()),
            ("agentCard", agent_card.as_str()),
            ],
        )
        .await
        .expect("Failed to register agent");

    // Delete (query params, matching Nacos: /a2a?agentName=...)
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ai/a2a",
            &[("namespaceId", "public"), ("agentName", agent_name.as_str())],
        )
        .await
        .expect("Failed to delete agent");

    assert_eq!(response["code"], 0, "Delete agent should succeed");
}
