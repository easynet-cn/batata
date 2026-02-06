//! Service Discovery (Naming) API integration tests
//!
//! Tests for /nacos/v2/ns/instance and /nacos/v2/ns/service endpoints

use crate::common::{unique_service_name, TestClient, DEFAULT_GROUP, TEST_NAMESPACE};

/// Test instance registration
#[tokio::test]
#[ignore = "requires running server"]
async fn test_register_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("register");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.100"),
                ("port", "8080"),
                ("weight", "1.0"),
                ("healthy", "true"),
                ("enabled", "true"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register instance");

    assert_eq!(response["code"], 0, "Registration should succeed");
}

/// Test instance deregistration
#[tokio::test]
#[ignore = "requires running server"]
async fn test_deregister_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("deregister");

    // First register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.101"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Then deregister
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.101"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to deregister");

    assert_eq!(response["code"], 0, "Deregistration should succeed");
}

/// Test update instance
#[tokio::test]
#[ignore = "requires running server"]
async fn test_update_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("update");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.102"),
                ("port", "8080"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register");

    // Update weight
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.102"),
                ("port", "8080"),
                ("weight", "2.0"),
                ("healthy", "true"),
                ("enabled", "true"),
            ],
        )
        .await
        .expect("Failed to update");

    assert_eq!(response["code"], 0, "Update should succeed");
}

/// Test get instance detail
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("getdetail");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.103"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get detail
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.103"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to get instance");

    assert_eq!(response["code"], 0, "Get should succeed");
    assert!(response["data"].is_object(), "Should return instance data");
}

/// Test get instance list
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_instance_list() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("list");

    // Register multiple instances
    for i in 1..=3 {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/ns/instance",
                &[
                    ("serviceName", service_name.as_str()),
                    ("ip", &format!("192.168.1.{}", 110 + i)),
                    ("port", "8080"),
                ],
            )
            .await
            .expect("Failed to register");
    }

    // Get list
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get instance list");

    assert_eq!(response["code"], 0, "Get list should succeed");
}

/// Test healthy only filter
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_list_healthy_only() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("healthy");

    // Register instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.120"),
                ("port", "8080"),
                ("healthy", "true"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get healthy only
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name.as_str()),
                ("healthyOnly", "true"),
            ],
        )
        .await
        .expect("Failed to get healthy instances");

    assert_eq!(response["code"], 0, "Get healthy should succeed");
}

/// Test batch update metadata
#[tokio::test]
#[ignore = "requires running server"]
async fn test_batch_update_metadata() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("batchmeta");

    // Register instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.130"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Batch update metadata
    let instances = r#"[{"ip":"192.168.1.130","port":8080}]"#;
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance/metadata/batch",
            &[
                ("serviceName", service_name.as_str()),
                ("instances", instances),
                ("metadata", r#"{"env":"test"}"#),
            ],
        )
        .await
        .expect("Failed to batch update");

    assert_eq!(response["code"], 0, "Batch update should succeed");
}

/// Test instance with weight
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_with_weight() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("weighted");

    // Register with specific weight
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.140"),
                ("port", "8080"),
                ("weight", "0.5"),
            ],
        )
        .await
        .expect("Failed to register");

    assert_eq!(response["code"], 0, "Registration with weight should succeed");
}

/// Test create service
#[tokio::test]
#[ignore = "requires running server"]
async fn test_create_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("create");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[
                ("serviceName", service_name.as_str()),
                ("protectThreshold", "0.5"),
            ],
        )
        .await
        .expect("Failed to create service");

    assert_eq!(response["code"], 0, "Create service should succeed");
}

/// Test delete service
#[tokio::test]
#[ignore = "requires running server"]
async fn test_delete_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("delete");

    // Create first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to create");

    // Then delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to delete");

    assert_eq!(response["code"], 0, "Delete service should succeed");
}

/// Test update service
#[tokio::test]
#[ignore = "requires running server"]
async fn test_update_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("update_svc");

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[
                ("serviceName", service_name.as_str()),
                ("protectThreshold", "0.5"),
            ],
        )
        .await
        .expect("Failed to create");

    // Update
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/service",
            &[
                ("serviceName", service_name.as_str()),
                ("protectThreshold", "0.8"),
            ],
        )
        .await
        .expect("Failed to update");

    assert_eq!(response["code"], 0, "Update service should succeed");
}

/// Test get service detail
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("getservice");

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to create");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get service");

    assert_eq!(response["code"], 0, "Get service should succeed");
}

/// Test service list
#[tokio::test]
#[ignore = "requires running server"]
async fn test_service_list() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get_with_query("/nacos/v2/ns/service/list", &[("pageNo", "1"), ("pageSize", "10")])
        .await
        .expect("Failed to get service list");

    assert_eq!(response["code"], 0, "Get service list should succeed");
}

/// Test service not found
#[tokio::test]
#[ignore = "requires running server"]
async fn test_service_not_found() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("nonexistent");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Request should complete");

    // Should indicate service not found
    assert!(
        response["data"].is_null() || response["code"] != 0,
        "Should indicate service not found"
    );
}
