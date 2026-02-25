//! Service Discovery (Naming) API integration tests
//!
//! Tests for /nacos/v2/ns/instance and /nacos/v2/ns/service endpoints

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient, unique_service_name,
};

async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test instance registration
#[tokio::test]

async fn test_register_instance() {
    let client = authenticated_client().await;
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

async fn test_deregister_instance() {
    let client = authenticated_client().await;
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

async fn test_update_instance() {
    let client = authenticated_client().await;
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

async fn test_get_instance() {
    let client = authenticated_client().await;
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

async fn test_get_instance_list() {
    let client = authenticated_client().await;
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

async fn test_instance_list_healthy_only() {
    let client = authenticated_client().await;
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

async fn test_batch_update_metadata() {
    let client = authenticated_client().await;
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

async fn test_instance_with_weight() {
    let client = authenticated_client().await;
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

    assert_eq!(
        response["code"], 0,
        "Registration with weight should succeed"
    );
}

/// Test create service
#[tokio::test]

async fn test_create_service() {
    let client = authenticated_client().await;
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

async fn test_delete_service() {
    let client = authenticated_client().await;
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

async fn test_update_service() {
    let client = authenticated_client().await;
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

async fn test_get_service() {
    let client = authenticated_client().await;
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

async fn test_service_list() {
    let client = authenticated_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service/list",
            &[("pageNo", "1"), ("pageSize", "10")],
        )
        .await
        .expect("Failed to get service list");

    assert_eq!(response["code"], 0, "Get service list should succeed");
}

/// Test service not found
#[tokio::test]

async fn test_service_not_found() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("nonexistent");

    let result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await;

    // Should indicate service not found via HTTP 404 or error code
    match result {
        Ok(response) => {
            assert!(
                response["data"].is_null() || response["code"] != 0,
                "Should indicate service not found"
            );
        }
        Err(_) => {
            // HTTP 404 is expected for service not found
        }
    }
}

/// Test namespace isolation for naming - instances in different namespaces should not interfere
#[tokio::test]

async fn test_namespace_isolation_naming() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let client = authenticated_client().await;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Create two test namespaces via console API
    let ns_a = format!("test-ns-a-{}", timestamp);
    let ns_b = format!("test-ns-b-{}", timestamp);
    let service_name = "shared-service-name";
    let ip_a = "192.168.1.200";
    let ip_b = "192.168.1.201";

    // Create namespace A and B
    let console_client = || async {
        let mut client = TestClient::new(CONSOLE_BASE_URL);
        client
            .login(TEST_USERNAME, TEST_PASSWORD)
            .await
            .expect("Login failed");
        client
    };

    let console = console_client().await;
    let _: serde_json::Value = console
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_a.as_str()),
                ("namespaceName", "Test NS A for Naming"),
            ],
        )
        .await
        .expect("Failed to create namespace A");

    let _: serde_json::Value = console
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_b.as_str()),
                ("namespaceName", "Test NS B for Naming"),
            ],
        )
        .await
        .expect("Failed to create namespace B");

    // Register instance in namespace A
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_a),
                ("port", "8080"),
                ("namespaceId", ns_a.as_str()),
            ],
        )
        .await
        .expect("Failed to register instance in namespace A");

    // Register instance in namespace B with same service name
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_b),
                ("port", "8080"),
                ("namespaceId", ns_b.as_str()),
            ],
        )
        .await
        .expect("Failed to register instance in namespace B");

    // Get instances from namespace A - should only return instance A
    let response_a: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name),
                ("namespaceId", ns_a.as_str()),
            ],
        )
        .await
        .expect("Failed to get instances from namespace A");

    assert_eq!(response_a["code"], 0);
    let hosts_a = response_a["data"]["hosts"].as_array().unwrap();
    assert_eq!(hosts_a.len(), 1, "Namespace A should have 1 instance");
    assert_eq!(hosts_a[0]["ip"], ip_a, "Namespace A should have instance A");

    // Get instances from namespace B - should only return instance B
    let response_b: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name),
                ("namespaceId", ns_b.as_str()),
            ],
        )
        .await
        .expect("Failed to get instances from namespace B");

    assert_eq!(response_b["code"], 0);
    let hosts_b = response_b["data"]["hosts"].as_array().unwrap();
    assert_eq!(hosts_b.len(), 1, "Namespace B should have 1 instance");
    assert_eq!(hosts_b[0]["ip"], ip_b, "Namespace B should have instance B");

    // Get instances from default namespace - should not find this service
    let result_default = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name)],
        )
        .await;

    match result_default {
        Ok(response) => {
            // Service might not exist in default namespace
            let empty_hosts: Vec<serde_json::Value> = vec![];
            let hosts = response["data"]["hosts"].as_array().unwrap_or(&empty_hosts);
            assert!(
                hosts.is_empty() || response["code"] != 0,
                "Should not find service in default namespace"
            );
        }
        Err(_) => {
            // Service not found is acceptable
        }
    }

    // Cleanup: deregister instances and delete namespaces
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_a),
                ("port", "8080"),
                ("namespaceId", ns_a.as_str()),
            ],
        )
        .await
        .ok()
        .unwrap_or_default();

    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_b),
                ("port", "8080"),
                ("namespaceId", ns_b.as_str()),
            ],
        )
        .await
        .ok()
        .unwrap_or_default();

    let _: serde_json::Value = console
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_a.as_str())])
        .await
        .ok()
        .unwrap_or_default();

    let _: serde_json::Value = console
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_b.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}
