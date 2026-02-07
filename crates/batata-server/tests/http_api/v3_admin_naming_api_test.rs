//! V3 Admin Naming API integration tests
//!
//! Tests for /nacos/v3/admin/ns/* endpoints

use crate::common::{TestClient, unique_service_name};
use serde_json::json;

// ========== Service CRUD ==========

/// Test create service via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_create_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_create");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/service",
            &json!({
                "serviceName": service_name,
                "protectThreshold": 0.5
            }),
        )
        .await
        .expect("Failed to create service");

    assert_eq!(response["code"], 0, "Create service should succeed");
}

/// Test get service via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_get");

    // Create first
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/service",
            &json!({ "serviceName": service_name }),
        )
        .await
        .expect("Failed to create service");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get service");

    assert_eq!(response["code"], 0, "Get service should succeed");
    assert!(response["data"].is_object(), "Should return service data");
}

/// Test update service via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_update_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_update");

    // Create
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/service",
            &json!({
                "serviceName": service_name,
                "protectThreshold": 0.5
            }),
        )
        .await
        .expect("Failed to create service");

    // Update
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v3/admin/ns/service",
            &json!({
                "serviceName": service_name,
                "protectThreshold": 0.8,
                "metadata": "{\"env\":\"test\"}"
            }),
        )
        .await
        .expect("Failed to update service");

    assert_eq!(response["code"], 0, "Update service should succeed");
}

/// Test delete service via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_delete_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_delete");

    // Create
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/service",
            &json!({ "serviceName": service_name }),
        )
        .await
        .expect("Failed to create service");

    // Delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to delete service");

    assert_eq!(response["code"], 0, "Delete service should succeed");
}

/// Test list services via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_list_services() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/service/list",
            &[("pageNo", "1"), ("pageSize", "10")],
        )
        .await
        .expect("Failed to list services");

    assert_eq!(response["code"], 0, "List services should succeed");
    assert!(response["data"].is_object(), "Should return list data");
}

/// Test create service with duplicate name returns error
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_create_duplicate_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_dup");

    // Create first
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/service",
            &json!({ "serviceName": service_name }),
        )
        .await
        .expect("Failed to create service");

    // Create again - should fail
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/service",
            &json!({ "serviceName": service_name }),
        )
        .await
        .expect("Request should complete");

    assert_ne!(response["code"], 0, "Duplicate create should fail");
}

// ========== Instance CRUD ==========

/// Test register instance via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_register_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_inst_reg");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.1"),
                ("port", "8080"),
                ("weight", "1.0"),
                ("healthy", "true"),
                ("enabled", "true"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register instance");

    assert_eq!(response["code"], 0, "Register instance should succeed");
}

/// Test deregister instance via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_deregister_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_inst_dereg");

    // Register first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.2"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Deregister
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.2"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to deregister");

    assert_eq!(response["code"], 0, "Deregister should succeed");
}

/// Test update instance via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_update_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_inst_upd");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.3"),
                ("port", "8080"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register");

    // Update
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.3"),
                ("port", "8080"),
                ("weight", "2.0"),
                ("enabled", "true"),
            ],
        )
        .await
        .expect("Failed to update instance");

    assert_eq!(response["code"], 0, "Update instance should succeed");
}

/// Test list instances via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_list_instances() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_inst_list");

    // Register instances
    for i in 1..=2 {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v3/admin/ns/instance",
                &[
                    ("serviceName", service_name.as_str()),
                    ("ip", &format!("192.168.10.{}", 10 + i)),
                    ("port", "8080"),
                ],
            )
            .await
            .expect("Failed to register");
    }

    // List
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to list instances");

    assert_eq!(response["code"], 0, "List instances should succeed");
}

/// Test get single instance via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_instance() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_inst_get");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.20"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.20"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to get instance");

    assert_eq!(response["code"], 0, "Get instance should succeed");
}

/// Test update instance metadata via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_update_instance_metadata() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_inst_meta");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.30"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Update metadata
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v3/admin/ns/instance/metadata",
            &[
                ("serviceName", service_name.as_str()),
                ("instances", r#"[{"ip":"192.168.10.30","port":8080}]"#),
                ("metadata", r#"{"env":"staging"}"#),
            ],
        )
        .await
        .expect("Failed to update metadata");

    assert_eq!(response["code"], 0, "Update metadata should succeed");
}

// ========== Ops Endpoints ==========

/// Test get naming switches via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_naming_switches() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/ops/switches")
        .await
        .expect("Failed to get switches");

    assert_eq!(response["code"], 0, "Get switches should succeed");
    assert!(response["data"].is_object(), "Should return switches data");
}

/// Test get naming metrics via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_naming_metrics() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/ops/metrics")
        .await
        .expect("Failed to get metrics");

    assert_eq!(response["code"], 0, "Get metrics should succeed");
    assert!(response["data"].is_object(), "Should return metrics data");
}

// ========== Health Endpoints ==========

/// Test get health checkers via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_health_checkers() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/health/checkers")
        .await
        .expect("Failed to get health checkers");

    assert_eq!(response["code"], 0, "Get checkers should succeed");
}

/// Test update instance health via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_update_health() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_name = unique_service_name("v3admin_health");

    // Register instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.40"),
                ("port", "8080"),
                ("ephemeral", "false"),
            ],
        )
        .await
        .expect("Failed to register");

    // Update health
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v3/admin/ns/health/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.10.40"),
                ("port", "8080"),
                ("healthy", "false"),
            ],
        )
        .await
        .expect("Failed to update health");

    assert_eq!(response["code"], 0, "Update health should succeed");
}

// ========== Client Info Endpoints ==========

/// Test list clients via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_list_clients() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/client/list")
        .await
        .expect("Failed to list clients");

    assert_eq!(response["code"], 0, "List clients should succeed");
}
