//! Console API tests
//!
//! Tests console-specific endpoints on port 8081.

use crate::common::{
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
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_console_server_state() {
    let client = console_client().await;
    let response = client.raw_get("/v3/console/server/state").await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
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
    // In embedded mode, should be standalone
    assert!(
        body.contains("true") || body.contains("\"data\":true"),
        "Expected standalone=true: {}",
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

    // List namespaces
    let response = client
        .raw_get("/v3/console/core/namespace/list")
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);

    // Check namespace exists
    let response = client
        .raw_get(&format!(
            "/v3/console/core/namespace/exist?customNamespaceId={}",
            ns_id
        ))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);

    // Get namespace
    let response = client
        .raw_get(&format!("/v3/console/core/namespace?namespaceId={}", ns_id))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);

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

    // Publish config via console
    let response = client
        .raw_post_form(
            "/v3/console/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("groupName", "DEFAULT_GROUP"),
                ("namespaceId", ""),
                ("content", "console.test=true"),
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

    // Get config
    let response = client
        .raw_get(&format!(
            "/v3/console/cs/config?dataId={}&groupName=DEFAULT_GROUP&namespaceId=public",
            data_id
        ))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200, "Get config should succeed");

    // Search configs
    let response = client
        .raw_get(&format!(
            "/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId={}&groupName=&namespaceId=public",
            data_id
        ))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200, "Search should succeed");

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
