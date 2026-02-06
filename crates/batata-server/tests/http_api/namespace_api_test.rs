//! Namespace API integration tests
//!
//! Tests for /nacos/v2/console/namespace endpoints

use crate::common::{unique_test_id, TestClient, TEST_USERNAME, TEST_PASSWORD};

/// Test create namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_create_namespace() {
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let ns_id = format!("test-ns-{}", unique_test_id());

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Test Namespace"),
                ("namespaceDesc", "Created by integration test"),
            ],
        )
        .await
        .expect("Create namespace failed");

    assert_eq!(response["code"], 0, "Create namespace should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/console/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test namespace list
#[tokio::test]
#[ignore = "requires running server"]
async fn test_namespace_list() {
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let response: serde_json::Value = client
        .get("/nacos/v2/console/namespace/list")
        .await
        .expect("Get namespace list failed");

    assert_eq!(response["code"], 0, "Get namespace list should succeed");
    assert!(response["data"].is_array(), "Should return array of namespaces");
}

/// Test get namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_namespace() {
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let ns_id = format!("get-ns-{}", unique_test_id());

    // Create first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Get Test Namespace"),
            ],
        )
        .await
        .expect("Create namespace failed");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/console/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .expect("Get namespace failed");

    assert_eq!(response["code"], 0, "Get namespace should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/console/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test update namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_update_namespace() {
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let ns_id = format!("update-ns-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Original Name"),
            ],
        )
        .await
        .expect("Create namespace failed");

    // Update
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Updated Name"),
                ("namespaceDesc", "Updated description"),
            ],
        )
        .await
        .expect("Update namespace failed");

    assert_eq!(response["code"], 0, "Update namespace should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/console/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test delete namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_delete_namespace() {
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let ns_id = format!("delete-ns-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "To Be Deleted"),
            ],
        )
        .await
        .expect("Create namespace failed");

    // Delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/console/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .expect("Delete namespace failed");

    assert_eq!(response["code"], 0, "Delete namespace should succeed");

    // Verify deleted
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/console/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .expect("Get request should complete");

    assert!(
        response["data"].is_null() || response["code"] != 0,
        "Namespace should be deleted"
    );
}
