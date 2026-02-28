//! Server/Console separation integration tests
//!
//! Verifies that Nacos 3.x architecture is correctly implemented:
//! - Console Server (8081): serves V2/V3 console routes and auth
//! - Main Server (8848): serves V2 Open API, V3 Admin/Client, but NOT V2 console routes
//!
//! These tests require a running Batata server with default configuration.

use crate::common::{CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient};

// ============================================================================
// Helper functions
// ============================================================================

/// Create an authenticated client for the console server (port 8081)
async fn console_client() -> TestClient {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login to console server failed");
    client
}

/// Create an authenticated client for the main server (port 8848)
/// Uses cross-port auth: login on console, use token on main
async fn main_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Cross-port login failed");
    client
}

// ============================================================================
// Console Server (8081) - V2 console routes should be available
// ============================================================================

/// V2 namespace list should be served by console server
#[tokio::test]
async fn test_console_server_serves_v2_namespace_list() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v2/console/namespace/list")
        .await
        .expect("Console server should serve /v2/console/namespace/list");

    assert_eq!(
        response["code"], 0,
        "V2 namespace list on console server should return success"
    );
    assert!(
        response["data"].is_array(),
        "Should return array of namespaces"
    );
}

/// V2 health liveness should be served by console server
#[tokio::test]
async fn test_console_server_serves_v2_health_liveness() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v2/console/health/liveness")
        .await
        .expect("Console server should serve /v2/console/health/liveness");

    assert_eq!(
        response["code"], 0,
        "V2 health liveness on console server should return success"
    );
}

/// V2 health readiness should be served by console server
#[tokio::test]
async fn test_console_server_serves_v2_health_readiness() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v2/console/health/readiness")
        .await
        .expect("Console server should serve /v2/console/health/readiness");

    // readiness may return 200 (ok) or 500 (not ready), both are valid responses
    assert!(
        response["code"] == 0 || response["code"] == 500,
        "V2 health readiness should return a valid response, got: {:?}",
        response
    );
}

/// V3 console namespace list should be served by console server
#[tokio::test]
async fn test_console_server_serves_v3_namespace_list() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v3/console/core/namespace/list")
        .await
        .expect("Console server should serve /v3/console/core/namespace/list");

    assert_eq!(
        response["code"], 0,
        "V3 namespace list on console server should return success"
    );
}

/// V3 console health should be served by console server
#[tokio::test]
async fn test_console_server_serves_v3_health() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v3/console/health/liveness")
        .await
        .expect("Console server should serve /v3/console/health/liveness");

    assert_eq!(
        response["code"], 0,
        "V3 health liveness on console server should return success"
    );
}

/// Auth should be available on console server
#[tokio::test]
async fn test_console_server_serves_auth() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    let response = client
        .raw_post_form(
            "/v3/auth/user/login",
            &[("username", TEST_USERNAME), ("password", TEST_PASSWORD)],
        )
        .await
        .expect("Console server should serve /v3/auth/user/login");

    assert!(
        response.status().is_success(),
        "Auth login on console server should succeed"
    );
}

// ============================================================================
// Main Server (8848) - V2 console routes should NOT be available
// ============================================================================

/// V2 console namespace list should NOT be served by main server
#[tokio::test]
async fn test_main_server_does_not_serve_v2_console_namespace() {
    let client = main_client().await;

    let result = client
        .raw_get("/nacos/v2/console/namespace/list")
        .await
        .expect("Request should not fail at transport level");

    assert_eq!(
        result.status().as_u16(),
        404,
        "Main server should return 404 for /nacos/v2/console/namespace/list"
    );
}

/// V2 console health should NOT be served by main server
#[tokio::test]
async fn test_main_server_does_not_serve_v2_console_health() {
    let client = main_client().await;

    let result = client
        .raw_get("/nacos/v2/console/health/liveness")
        .await
        .expect("Request should not fail at transport level");

    assert_eq!(
        result.status().as_u16(),
        404,
        "Main server should return 404 for /nacos/v2/console/health/liveness"
    );
}

// ============================================================================
// Main Server (8848) - V2 Open API routes should still be available
// ============================================================================

/// V2 Open API config routes should still work on main server
#[tokio::test]
async fn test_main_server_still_serves_v2_config() {
    let client = main_client().await;

    // GET config with missing params returns an API error, but not 404
    let result = client
        .raw_get("/nacos/v2/cs/config")
        .await
        .expect("Request should not fail at transport level");

    // Should get a response (even if it's a 400 for missing params), not 404
    assert_ne!(
        result.status().as_u16(),
        404,
        "Main server should still serve /nacos/v2/cs/config"
    );
}

/// V2 Open API cluster routes should still work on main server
#[tokio::test]
async fn test_main_server_still_serves_v2_cluster() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v2/core/cluster/node/self")
        .await
        .expect("Main server should serve /nacos/v2/core/cluster/node/self");

    assert_eq!(
        response["code"], 0,
        "V2 cluster node/self on main server should return success"
    );
}

/// Auth should also be available on main server (for SDK authentication)
#[tokio::test]
async fn test_main_server_serves_auth() {
    let client = TestClient::new(MAIN_BASE_URL);

    let response = client
        .raw_post_form(
            "/nacos/v3/auth/user/login",
            &[("username", TEST_USERNAME), ("password", TEST_PASSWORD)],
        )
        .await
        .expect("Main server should serve /nacos/v3/auth/user/login");

    assert!(
        response.status().is_success(),
        "Auth login on main server should succeed"
    );
}

// ============================================================================
// Console Server (8081) - should NOT serve Main Server routes
// ============================================================================

/// V2 Open API config routes should NOT be on console server
#[tokio::test]
async fn test_console_server_does_not_serve_v2_open_api() {
    let client = console_client().await;

    let result = client
        .raw_get("/v2/cs/config")
        .await
        .expect("Request should not fail at transport level");

    assert_eq!(
        result.status().as_u16(),
        404,
        "Console server should return 404 for /v2/cs/config (Open API)"
    );
}

/// V3 Admin routes should NOT be on console server
#[tokio::test]
async fn test_console_server_does_not_serve_v3_admin() {
    let client = console_client().await;

    let result = client
        .raw_get("/v3/admin/core/cluster/node/self")
        .await
        .expect("Request should not fail at transport level");

    assert_eq!(
        result.status().as_u16(),
        404,
        "Console server should return 404 for /v3/admin/* routes"
    );
}

// ============================================================================
// V2 Console CRUD on Console Server (full lifecycle)
// ============================================================================

/// Full namespace CRUD lifecycle on console server via V2 API
#[tokio::test]
async fn test_v2_namespace_crud_on_console_server() {
    let client = console_client().await;
    let ns_id = format!(
        "sep-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // 1. Create namespace
    let create_response: serde_json::Value = client
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Separation Test NS"),
                ("namespaceDesc", "Testing console/server separation"),
            ],
        )
        .await
        .expect("Create namespace on console server should succeed");

    assert_eq!(
        create_response["code"], 0,
        "Create namespace should return success"
    );

    // 2. Get namespace
    let get_response: serde_json::Value = client
        .get_with_query("/v2/console/namespace", &[("namespaceId", ns_id.as_str())])
        .await
        .expect("Get namespace on console server should succeed");

    assert_eq!(
        get_response["code"], 0,
        "Get namespace should return success"
    );

    // 3. Update namespace
    let update_response: serde_json::Value = client
        .put_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Updated Sep Test NS"),
                ("namespaceDesc", "Updated description"),
            ],
        )
        .await
        .expect("Update namespace on console server should succeed");

    assert_eq!(
        update_response["code"], 0,
        "Update namespace should return success"
    );

    // 4. Verify in list
    let list_response: serde_json::Value = client
        .get("/v2/console/namespace/list")
        .await
        .expect("List namespaces should succeed");

    assert_eq!(list_response["code"], 0);
    let namespaces = list_response["data"]
        .as_array()
        .expect("data should be array");
    let found = namespaces
        .iter()
        .any(|ns| ns["namespace"].as_str() == Some(ns_id.as_str()));
    assert!(found, "Created namespace should appear in the list");

    // 5. Delete namespace
    let delete_response: serde_json::Value = client
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_id.as_str())])
        .await
        .expect("Delete namespace on console server should succeed");

    assert_eq!(
        delete_response["code"], 0,
        "Delete namespace should return success"
    );
}
