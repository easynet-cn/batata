//! Consul API compatibility tests
//!
//! Tests for Consul API endpoints compatibility

use crate::common::{TestClient, unique_service_name, unique_test_id};

/// Test service registration via Consul Agent API
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_service_register() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_id = format!("consul-svc-{}", unique_test_id());

    let service = serde_json::json!({
        "ID": service_id,
        "Name": "test-service",
        "Port": 8080,
        "Address": "192.168.1.100",
        "Tags": ["test", "integration"],
        "Meta": {
            "version": "1.0.0"
        }
    });

    let response = client
        .raw_post_form("/v1/agent/service/register", &service)
        .await;

    // Consul API returns 200 OK on success with empty body
    assert!(response.is_ok(), "Service registration should succeed");
}

/// Test service deregistration via Consul Agent API
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_service_deregister() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_id = format!("consul-dereg-{}", unique_test_id());

    // Register first
    let service = serde_json::json!({
        "ID": service_id,
        "Name": "to-deregister",
        "Port": 8080
    });
    let _ = client
        .raw_post_form("/v1/agent/service/register", &service)
        .await;

    // Deregister
    let response = client
        .raw_get(&format!("/v1/agent/service/deregister/{}", service_id))
        .await;

    assert!(response.is_ok(), "Service deregistration should succeed");
}

/// Test list services via Consul Agent API
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_list_services() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/v1/agent/services")
        .await
        .expect("List services should succeed");

    assert!(response.is_object(), "Should return services map");
}

/// Test health check via Consul Health API
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_health_service() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let service_id = format!("consul-health-{}", unique_test_id());

    // Register service
    let service = serde_json::json!({
        "ID": service_id,
        "Name": "health-test",
        "Port": 8080
    });
    let _ = client
        .raw_post_form("/v1/agent/service/register", &service)
        .await;

    // Query health
    let response: serde_json::Value = client
        .get("/v1/health/service/health-test")
        .await
        .expect("Health query should succeed");

    assert!(response.is_array(), "Should return health check results");
}

/// Test KV put operation
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_kv_put() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let key = format!("test/key/{}", unique_test_id());

    // PUT returns true on success
    let response = client
        .raw_post_form(&format!("/v1/kv/{}", key), &"test-value")
        .await;

    assert!(response.is_ok(), "KV put should succeed");
}

/// Test KV get operation
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_kv_get() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let key = format!("test/get/{}", unique_test_id());

    // Put first
    let _ = client
        .raw_post_form(&format!("/v1/kv/{}", key), &"get-test-value")
        .await;

    // Get
    let response: serde_json::Value = client
        .get(&format!("/v1/kv/{}", key))
        .await
        .expect("KV get should succeed");

    assert!(response.is_array(), "Should return KV pair array");
}

/// Test KV delete operation
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_kv_delete() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let key = format!("test/delete/{}", unique_test_id());

    // Put first
    let _ = client
        .raw_post_form(&format!("/v1/kv/{}", key), &"to-delete")
        .await;

    // Delete
    let response = client
        .delete::<serde_json::Value>(&format!("/v1/kv/{}", key))
        .await;

    assert!(response.is_ok(), "KV delete should succeed");
}

/// Test KV CAS (Compare-And-Swap) operation
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_kv_cas() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let key = format!("test/cas/{}", unique_test_id());

    // Initial put
    let _ = client
        .raw_post_form(&format!("/v1/kv/{}", key), &"initial")
        .await;

    // Get to obtain ModifyIndex
    let response: Vec<serde_json::Value> = client
        .get(&format!("/v1/kv/{}", key))
        .await
        .expect("KV get should succeed");

    let modify_index = response[0]["ModifyIndex"].as_u64().unwrap_or(0);

    // CAS update with correct index
    let response = client
        .raw_post_form(&format!("/v1/kv/{}?cas={}", key, modify_index), &"updated")
        .await;

    assert!(response.is_ok(), "CAS update should succeed");
}

/// Test catalog services endpoint
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_catalog_services() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/v1/catalog/services")
        .await
        .expect("Catalog services should succeed");

    assert!(response.is_object(), "Should return services map");
}

/// Test ACL token creation
#[tokio::test]
#[ignore = "requires running server with Consul plugin"]
async fn test_consul_acl_token() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let token = serde_json::json!({
        "Description": "Test token",
        "Policies": [],
        "Local": true
    });

    let response: serde_json::Value = client
        .post_json("/v1/acl/token", &token)
        .await
        .expect("ACL token creation should succeed");

    assert!(
        response["AccessorID"].is_string(),
        "Should return accessor ID"
    );
}
