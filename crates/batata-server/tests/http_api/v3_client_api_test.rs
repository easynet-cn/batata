//! V3 Client API integration tests
//!
//! Tests for /nacos/v3/client/* endpoints (SDK-facing API)

use crate::common::{DEFAULT_GROUP, TestClient, unique_data_id, unique_service_name};

// ========== Client Instance Endpoints ==========

/// Test register instance via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_register_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3client_reg");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v3/client/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.1"),
                ("port", "8080"),
                ("weight", "1.0"),
                ("healthy", "true"),
                ("enabled", "true"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register instance");

    assert_eq!(response["code"], 0, "Register should succeed");
}

/// Test deregister instance via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_deregister_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3client_dereg");

    // Register first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/client/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.2"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Deregister
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/client/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.2"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to deregister");

    assert_eq!(response["code"], 0, "Deregister should succeed");
}

/// Test list instances via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_list_instances() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3client_list");

    // Register multiple instances
    for i in 1..=3 {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v3/client/ns/instance",
                &[
                    ("serviceName", service_name.as_str()),
                    ("ip", &format!("10.0.0.{}", 10 + i)),
                    ("port", "8080"),
                ],
            )
            .await
            .expect("Failed to register");
    }

    // List
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/client/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to list instances");

    assert_eq!(response["code"], 0, "List instances should succeed");
    assert!(response["data"].is_object(), "Should return instance list");
}

/// Test list instances with cluster filter via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_list_instances_with_cluster() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3client_cluster");

    // Register instance with cluster
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/client/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.20"),
                ("port", "8080"),
                ("clusterName", "cluster-a"),
            ],
        )
        .await
        .expect("Failed to register");

    // List with cluster filter
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/client/ns/instance/list",
            &[
                ("serviceName", service_name.as_str()),
                ("clusters", "cluster-a"),
            ],
        )
        .await
        .expect("Failed to list instances");

    assert_eq!(response["code"], 0, "List with cluster should succeed");
}

/// Test list instances with healthy only filter via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_list_instances_healthy_only() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3client_healthy");

    // Register healthy instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/client/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.30"),
                ("port", "8080"),
                ("healthy", "true"),
            ],
        )
        .await
        .expect("Failed to register");

    // List healthy only
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/client/ns/instance/list",
            &[
                ("serviceName", service_name.as_str()),
                ("healthyOnly", "true"),
            ],
        )
        .await
        .expect("Failed to list healthy instances");

    assert_eq!(response["code"], 0, "List healthy should succeed");
}

/// Test register instance with metadata via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_register_with_metadata() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3client_meta");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v3/client/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.40"),
                ("port", "8080"),
                ("metadata", r#"{"env":"test","version":"1.0"}"#),
            ],
        )
        .await
        .expect("Failed to register with metadata");

    assert_eq!(response["code"], 0, "Register with metadata should succeed");
}

/// Test register instance parameter validation
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_register_validation() {
    let client = TestClient::new("http://127.0.0.1:8848");

    // Missing required fields
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v3/client/ns/instance",
            &[("serviceName", ""), ("ip", ""), ("port", "0")],
        )
        .await
        .expect("Request should complete");

    assert_ne!(response["code"], 0, "Should fail with missing fields");
}

// ========== Client Config Endpoints ==========

/// Test get config via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_get_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3client_cfg");
    let content = "client.config=value";

    // Create config via V2 API first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", content),
            ],
        )
        .await
        .expect("Failed to create config");

    // Get via V3 Client API
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/client/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config via V3 client");

    assert_eq!(response["code"], 0, "Get config should succeed");
}

/// Test get config not found via V3 Client API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_client_get_config_not_found() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3client_notfound");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/client/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Request should complete");

    assert!(
        response["data"].is_null() || response["code"] != 0,
        "Should indicate config not found"
    );
}
