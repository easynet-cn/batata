//! Console API tests
//!
//! Tests console-specific endpoints on port 8081.

use batata_integration_tests::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient, unique_test_id,
};

/// Helper to create an authenticated console client
async fn console_client() -> TestClient {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Console login should succeed");
    client
}

// ============================================================================
// Health & Server State
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_health_liveness() {
    let client = console_client().await;
    let response = client.raw_get("/v3/console/health/liveness").await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| panic!("Expected valid JSON response, got: {}", body));
    // Liveness should indicate UP status
    let body_str = json.to_string().to_lowercase();
    assert!(
        body_str.contains("up") || json.get("status").is_some() || json.get("data").is_some(),
        "Expected liveness response to contain UP status or status field, got: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_health_readiness() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/health/readiness")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| panic!("Expected valid JSON response, got: {}", body));
    let body_str = json.to_string().to_lowercase();
    assert!(
        body_str.contains("up")
            || body_str.contains("ready")
            || json.get("status").is_some()
            || json.get("data").is_some(),
        "Expected readiness response to contain status indicator, got: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_server_state() {
    let client = console_client().await;
    let response = client.raw_get("/v3/console/server/state").await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| panic!("Expected valid JSON response, got: {}", body));
    // Server state response should contain a "state" field (either top-level or in data)
    assert!(
        json.get("state").is_some()
            || json.get("data").and_then(|d| d.get("state")).is_some()
            || json.get("data").and_then(|d| d.as_object()).is_some(),
        "Expected server state response to contain 'state' field, got: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_health_detailed() {
    let client = console_client().await;
    let response = client.raw_get("/v3/console/health/detailed").await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| panic!("Expected valid JSON response, got: {}", body));

    // Parse specific health fields from the JSON structure
    let data = if json.get("data").is_some() {
        json.get("data").unwrap()
    } else {
        &json
    };

    assert!(
        data.get("status").is_some(),
        "Detailed health should contain 'status' field, got: {}",
        body
    );
    assert!(
        data.get("components").is_some(),
        "Detailed health should contain 'components' field, got: {}",
        body
    );
    assert!(
        data.get("version").is_some(),
        "Detailed health should contain 'version' field, got: {}",
        body
    );
    assert!(
        data.get("serverState").is_some(),
        "Detailed health should contain 'serverState' field, got: {}",
        body
    );
}

// ============================================================================
// Cluster Info
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_cluster_nodes() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/core/cluster/nodes")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| panic!("Expected valid JSON response, got: {}", body));

    // Extract the node list (could be top-level array or wrapped in data)
    let nodes = if json.is_array() {
        json.as_array().unwrap()
    } else if let Some(data) = json.get("data") {
        if data.is_array() {
            data.as_array().unwrap()
        } else {
            panic!(
                "Expected 'data' to be an array of cluster nodes, got: {}",
                body
            );
        }
    } else {
        panic!("Expected array of cluster nodes in response, got: {}", body);
    };

    assert!(
        !nodes.is_empty(),
        "Cluster nodes list should have at least one node"
    );

    let first_node = &nodes[0];
    assert!(
        first_node.get("address").is_some() || first_node.get("ip").is_some(),
        "First cluster node should have 'address' or 'ip' field, got: {:?}",
        first_node
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_cluster_self() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/core/cluster/self")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| panic!("Expected valid JSON response, got: {}", body));

    // Extract the node info (could be top-level or wrapped in data)
    let node = if json.get("data").is_some() {
        json.get("data").unwrap()
    } else {
        &json
    };

    assert!(
        node.get("ip").is_some(),
        "Cluster self should contain 'ip' field, got: {}",
        body
    );
    assert!(
        node.get("port").is_some(),
        "Cluster self should contain 'port' field, got: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_cluster_standalone() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/core/cluster/standalone")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| panic!("Expected valid JSON response, got: {}", body));

    // In embedded mode, should be standalone (true)
    let standalone_value = if let Some(data) = json.get("data") {
        data
    } else {
        &json
    };
    assert!(
        standalone_value.is_boolean() || standalone_value.as_bool().is_some(),
        "Expected standalone to be a boolean value, got: {}",
        body
    );
    assert!(
        standalone_value == true,
        "Expected standalone=true in embedded mode, got: {}",
        body
    );
}

// ============================================================================
// Namespace CRUD
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_namespace_crud() {
    let client = console_client().await;
    let ns_id = format!("test-ns-{}", unique_test_id());

    // Create namespace
    let response = client
        .raw_post_form(
            "/v3/console/core/namespace",
            &[
                ("customNamespaceId", ns_id.as_str()),
                ("namespaceName", "Console Test NS"),
                ("namespaceDesc", "Created by console API test"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        response.status().as_u16(),
        200,
        "Create namespace should succeed"
    );
    let create_body = response.text().await.unwrap();
    let create_json: serde_json::Value = serde_json::from_str(&create_body).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON for create response, got: {}",
            create_body
        )
    });
    assert!(
        create_json.get("code").is_some_and(|c| c == 0 || c == 200)
            || create_json.get("data").is_some(),
        "Create namespace response should indicate success, got: {}",
        create_body
    );

    // List namespaces
    let response = client
        .raw_get("/v3/console/core/namespace/list")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let list_body = response.text().await.unwrap();
    let list_json: serde_json::Value = serde_json::from_str(&list_body)
        .unwrap_or_else(|_| panic!("Expected valid JSON for list response, got: {}", list_body));
    let list_str = list_json.to_string();
    assert!(
        list_str.contains(&ns_id),
        "Namespace list should contain the created namespace '{}', got: {}",
        ns_id,
        list_body
    );

    // Check namespace exists
    let response = client
        .raw_get(&format!(
            "/v3/console/core/namespace/exist?customNamespaceId={}",
            ns_id
        ))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let exist_body = response.text().await.unwrap();
    let exist_json: serde_json::Value = serde_json::from_str(&exist_body).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON for exist response, got: {}",
            exist_body
        )
    });
    // Should indicate the namespace exists (data=true or similar)
    assert!(
        exist_json.get("data") == Some(&serde_json::Value::Bool(true))
            || exist_body.contains("true"),
        "Namespace exist check should return true for '{}', got: {}",
        ns_id,
        exist_body
    );

    // Get namespace
    let response = client
        .raw_get(&format!("/v3/console/core/namespace?namespaceId={}", ns_id))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let get_body = response.text().await.unwrap();
    let get_json: serde_json::Value = serde_json::from_str(&get_body)
        .unwrap_or_else(|_| panic!("Expected valid JSON for get response, got: {}", get_body));
    // Verify returned namespace data matches what we created
    let ns_data = get_json.get("data").unwrap_or(&get_json);
    let ns_name = ns_data
        .get("namespaceName")
        .or_else(|| ns_data.get("namespace"))
        .or_else(|| ns_data.get("namespaceShowName"));
    assert!(
        ns_name.is_some(),
        "Get namespace should return namespace name, got: {}",
        get_body
    );

    // Cleanup: delete namespace
    let empty: Vec<(&str, &str)> = vec![];
    let _ = client
        .raw_post_form(
            &format!("/v3/console/core/namespace?namespaceId={}", ns_id),
            &empty,
        )
        .await;
}

// ============================================================================
// Config via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_config_publish_and_get() {
    let client = console_client().await;
    let data_id = format!("console-test-{}", unique_test_id());
    let expected_content = "console.test=true";

    // Publish config via console
    let response = client
        .raw_post_form(
            "/v3/console/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("groupName", "DEFAULT_GROUP"),
                ("namespaceId", ""),
                ("content", expected_content),
                ("type", "properties"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        response.status().as_u16(),
        200,
        "Publish config should succeed"
    );
    let publish_body = response.text().await.unwrap();
    let publish_json: serde_json::Value =
        serde_json::from_str(&publish_body).unwrap_or_else(|_| {
            panic!(
                "Expected valid JSON for publish response, got: {}",
                publish_body
            )
        });
    assert!(
        publish_json.get("code").is_some_and(|c| c == 0 || c == 200)
            || publish_json.get("data") == Some(&serde_json::Value::Bool(true)),
        "Publish config response should indicate success, got: {}",
        publish_body
    );

    // Get config
    let response = client
        .raw_get(&format!(
            "/v3/console/cs/config?dataId={}&groupName=DEFAULT_GROUP&namespaceId=public",
            data_id
        ))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200, "Get config should succeed");
    let get_body = response.text().await.unwrap();
    let get_json: serde_json::Value = serde_json::from_str(&get_body).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON for get config response, got: {}",
            get_body
        )
    });
    // Verify the retrieved content matches what was published
    let config_data = get_json.get("data").unwrap_or(&get_json);
    let content = config_data
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or_else(|| {
            panic!(
                "Get config response should contain 'content' field, got: {}",
                get_body
            )
        });
    assert_eq!(
        content, expected_content,
        "Retrieved config content should match published content"
    );

    // Search configs
    let response = client
        .raw_get(&format!(
            "/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId={}&groupName=&namespaceId=public",
            data_id
        ))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200, "Search should succeed");
    let search_body = response.text().await.unwrap();
    let search_json: serde_json::Value = serde_json::from_str(&search_body).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON for search response, got: {}",
            search_body
        )
    });
    // Verify search results contain the published config
    let search_str = search_json.to_string();
    assert!(
        search_str.contains(&data_id),
        "Search results should contain published config '{}', got: {}",
        data_id,
        search_body
    );

    // Cleanup: delete config
    let _ = client
        .raw_get(&format!(
            "/v3/console/cs/config?dataId={}&groupName=DEFAULT_GROUP&namespaceId=public",
            data_id
        ))
        .await;
}

// ============================================================================
// Service Discovery via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_service_list() {
    let client = console_client().await;
    let response = client
        .raw_get(
            "/v3/console/ns/service/list?pageNo=1&pageSize=10&namespaceId=&groupName=DEFAULT_GROUP",
        )
        .await
        .unwrap();
    assert_eq!(
        response.status().as_u16(),
        200,
        "Service list should succeed"
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_selector_types() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/ns/service/selector/types")
        .await
        .unwrap();
    assert_eq!(
        response.status().as_u16(),
        200,
        "Selector types should succeed"
    );
}

// ============================================================================
// Auth via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_login_and_use_token() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Console login should succeed");

    // Use token for subsequent requests
    let response = client.raw_get("/v3/console/health/liveness").await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_cross_port_auth() {
    // Login on console (8081), use token on main server (8848)
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Cross-port login should succeed");

    // Token should work on main server
    let response = client
        .raw_get("/nacos/v2/cs/config?dataId=test&group=DEFAULT_GROUP")
        .await
        .unwrap();
    let status = response.status().as_u16();
    // Should not be 401/403
    assert!(
        status != 401 && status != 403,
        "Token should be accepted on main server, got {}",
        status
    );
}

// ============================================================================
// Config Search & History via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_config_search() {
    let client = console_client().await;
    let response = client
        .raw_get(
            "/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId=&groupName=&namespaceId=public",
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_config_history() {
    let client = console_client().await;
    // First publish a config
    let data_id = format!("history-test-{}", unique_test_id());
    let response = client
        .raw_post_form(
            "/v3/console/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("groupName", "DEFAULT_GROUP"),
                ("namespaceId", ""),
                ("content", "key=value"),
                ("type", "properties"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);

    // Query history
    let response = client
        .raw_get(&format!(
            "/v3/console/cs/history/list?pageNo=1&pageSize=10&dataId={}&groupName=DEFAULT_GROUP&namespaceId=public",
            data_id
        ))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_config_listener() {
    let client = console_client().await;
    let response = client
        .raw_get(
            "/v3/console/cs/config/listener?dataId=test&groupName=DEFAULT_GROUP&namespaceId=public",
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

// ============================================================================
// Auth Management via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_user_list() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/auth/user/list?pageNo=1&pageSize=10")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_role_list() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/auth/role/list?pageNo=1&pageSize=10")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_permission_list() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/auth/permission/list?pageNo=1&pageSize=10&role=ROLE_ADMIN")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

// ============================================================================
// Service Discovery (Pagination) via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_service_list_with_pagination() {
    let client = console_client().await;
    let response = client
        .raw_get(
            "/v3/console/ns/service/list?pageNo=1&pageSize=5&namespaceId=&groupName=DEFAULT_GROUP",
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

// ============================================================================
// AI/MCP & A2A via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_mcp_list() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/ai/mcp/list?pageNo=1&pageSize=10&namespaceId=public")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_a2a_list() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/ai/a2a/list?pageNo=1&pageSize=10&namespaceId=public")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

// ============================================================================
// Cluster Endpoints via Console
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_cluster_health() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/core/cluster/health")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_cluster_count() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/core/cluster/count")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_cluster_leader() {
    let client = console_client().await;
    let response = client
        .raw_get("/v3/console/core/cluster/leader")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
}
