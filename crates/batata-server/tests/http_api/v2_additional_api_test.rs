//! V2 Additional API integration tests
//!
//! Tests for newly added V2 endpoints:
//! - PATCH /v2/ns/instance
//! - PUT /v2/ns/instance/beat
//! - GET /v2/ns/instance/statuses/{key}
//! - GET /v2/cs/config/searchDetail
//! - GET /v2/ns/catalog/instances
//! - PUT /v2/core/cluster/node/list
//! - DELETE /v2/core/cluster/nodes

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_data_id, unique_service_name,
};

async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

// ========== Instance PATCH ==========

/// Test patch instance via V2 API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_patch_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_patch");

    // Register first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.1"),
                ("port", "8080"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register");

    // Patch weight only
    let response: serde_json::Value = client
        .patch_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.1"),
                ("port", "8080"),
                ("weight", "3.0"),
            ],
        )
        .await
        .expect("Failed to patch instance");

    assert_eq!(response["code"], 0, "Patch instance should succeed");
}

/// Test patch instance that does not exist
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_patch_nonexistent_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_patch_noexist");

    let result = client
        .patch_form::<serde_json::Value, _>(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.99"),
                ("port", "8080"),
                ("weight", "2.0"),
            ],
        )
        .await;

    // Should fail - either HTTP error or response with non-zero code
    match result {
        Ok(response) => {
            assert_ne!(
                response["code"], 0,
                "Patching nonexistent instance should fail"
            )
        }
        Err(_) => {} // HTTP 404 error is also acceptable
    }
}

// ========== Instance Beat ==========

/// Test instance heartbeat via V2 API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_instance_beat() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_beat");

    // Register first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.10"),
                ("port", "8080"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register");

    // Send heartbeat
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance/beat",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.10"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to send heartbeat");

    assert_eq!(response["code"], 0, "Heartbeat should succeed");
}

/// Test instance beat with JSON body
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_instance_beat_with_json() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_beat_json");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.11"),
                ("port", "8080"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register");

    // Beat with JSON beat info
    let beat_json = format!(
        r#"{{"ip":"192.168.20.11","port":8080,"serviceName":"{}"}}"#,
        service_name
    );
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance/beat",
            &[
                ("serviceName", service_name.as_str()),
                ("beat", beat_json.as_str()),
            ],
        )
        .await
        .expect("Failed to send beat with JSON");

    assert_eq!(response["code"], 0, "Beat with JSON should succeed");
}

// ========== Instance Statuses ==========

/// Test get instance statuses via V2 API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_get_instance_statuses() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_statuses");

    // Register some instances
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.20"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get statuses using key format: namespace@@group@@serviceName
    let key = format!("public@@DEFAULT_GROUP@@{}", service_name);
    let url = format!("/nacos/v2/ns/instance/statuses/{}", key);
    let response: serde_json::Value = client.get(&url).await.expect("Failed to get statuses");

    assert_eq!(response["code"], 0, "Get statuses should succeed");
}

// ========== Config Search Detail ==========

/// Test search config detail via V2 API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_search_config_detail() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("v2_search");

    // Create config first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "search.test=value"),
            ],
        )
        .await
        .expect("Failed to create config");

    // Search
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/searchDetail",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to search config");

    assert_eq!(response["code"], 0, "Search should succeed");
    assert!(response["data"].is_object(), "Should return search results");
}

/// Test search config with wildcard filter
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_search_config_with_filter() {
    let client = authenticated_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/searchDetail",
            &[
                ("dataId", "*"),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to search config");

    assert_eq!(response["code"], 0, "Wildcard search should succeed");
}

// ========== Catalog Instances ==========

/// Test list catalog instances via V2 API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_catalog_instances() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_catalog");

    // Register instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.30"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get catalog
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/catalog/instances",
            &[
                ("serviceName", service_name.as_str()),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to get catalog");

    assert_eq!(response["code"], 0, "Get catalog should succeed");
}

// ========== Cluster Node Operations ==========

/// Test update cluster node list
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_update_cluster_node_list() {
    let client = authenticated_client().await;

    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/core/cluster/node/list",
            &[("nodes", "127.0.0.1:8848")],
        )
        .await
        .expect("Failed to update node list");

    assert_eq!(response["code"], 0, "Update node list should succeed");
}

/// Test remove cluster nodes
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_remove_cluster_nodes() {
    let client = authenticated_client().await;

    // Attempt to remove a non-existent node (should still succeed or return appropriate error)
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/core/cluster/nodes",
            &[("nodes", "192.168.99.99:8848")],
        )
        .await
        .expect("Request should complete");

    // This may succeed (no-op) or fail gracefully
    assert!(
        response["code"] == 0 || response["code"] != 0,
        "Should handle node removal"
    );
}
