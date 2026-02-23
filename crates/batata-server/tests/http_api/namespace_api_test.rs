//! Namespace API integration tests
//!
//! Tests for /v2/console/namespace endpoints on console server (port 8081)

use crate::common::{CONSOLE_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient, unique_test_id};

/// Create an authenticated client for the console server
async fn console_client() -> TestClient {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");
    client
}

/// Test create namespace
#[tokio::test]

async fn test_create_namespace() {
    let client = console_client().await;
    let ns_id = format!("test-ns-{}", unique_test_id());

    let response: serde_json::Value = client
        .post_form(
            "/v2/console/namespace",
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
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_id.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test namespace list
#[tokio::test]

async fn test_namespace_list() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v2/console/namespace/list")
        .await
        .expect("Get namespace list failed");

    assert_eq!(response["code"], 0, "Get namespace list should succeed");
    assert!(
        response["data"].is_array(),
        "Should return array of namespaces"
    );
}

/// Test get namespace
#[tokio::test]

async fn test_get_namespace() {
    let client = console_client().await;
    let ns_id = format!("get-ns-{}", unique_test_id());

    // Create first
    let _: serde_json::Value = client
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Get Test Namespace"),
            ],
        )
        .await
        .expect("Create namespace failed");

    // Get
    let response: serde_json::Value = client
        .get_with_query("/v2/console/namespace", &[("namespaceId", ns_id.as_str())])
        .await
        .expect("Get namespace failed");

    assert_eq!(response["code"], 0, "Get namespace should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_id.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test update namespace
#[tokio::test]

async fn test_update_namespace() {
    let client = console_client().await;
    let ns_id = format!("update-ns-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/v2/console/namespace",
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
            "/v2/console/namespace",
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
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_id.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test delete namespace
#[tokio::test]

async fn test_delete_namespace() {
    let client = console_client().await;
    let ns_id = format!("delete-ns-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "To Be Deleted"),
            ],
        )
        .await
        .expect("Create namespace failed");

    // Delete
    let response: serde_json::Value = client
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_id.as_str())])
        .await
        .expect("Delete namespace failed");

    assert_eq!(response["code"], 0, "Delete namespace should succeed");

    // Verify deleted - should return empty data or error
    let result = client
        .get_with_query::<serde_json::Value, _>(
            "/v2/console/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await;

    match result {
        Ok(response) => {
            assert!(
                response["data"].is_null() || response["code"] != 0,
                "Namespace should be deleted"
            );
        }
        Err(_) => {
            // Error is acceptable - namespace doesn't exist
        }
    }
}
