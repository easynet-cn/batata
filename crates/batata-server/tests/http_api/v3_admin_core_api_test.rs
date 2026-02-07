//! V3 Admin Core API integration tests
//!
//! Tests for /nacos/v3/admin/core/* endpoints

use crate::common::{TestClient, unique_test_id};

// ========== Cluster Node Endpoints ==========

/// Test get self node info via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_cluster_get_self() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/core/cluster/node/self")
        .await
        .expect("Failed to get self node");

    assert_eq!(response["code"], 0, "Get self should succeed");
    assert!(response["data"].is_object(), "Should return node data");
    assert!(response["data"]["ip"].is_string(), "Should have IP field");
}

/// Test get self node health via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_cluster_self_health() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/core/cluster/node/self/health")
        .await
        .expect("Failed to get self health");

    assert_eq!(response["code"], 0, "Get health should succeed");
    assert!(
        response["data"]["healthy"].is_boolean(),
        "Should return healthy status"
    );
}

/// Test list cluster nodes via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_cluster_list_nodes() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/core/cluster/node/list")
        .await
        .expect("Failed to list nodes");

    assert_eq!(response["code"], 0, "List nodes should succeed");
    assert!(response["data"].is_array(), "Should return array of nodes");
}

/// Test list cluster nodes with keyword filter
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_cluster_list_nodes_with_keyword() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/core/cluster/node/list",
            &[("keyword", "127.0.0.1")],
        )
        .await
        .expect("Failed to list nodes with keyword");

    assert_eq!(response["code"], 0, "Filtered list should succeed");
}

// ========== Namespace Endpoints ==========

/// Test list namespaces via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_list_namespaces() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/core/namespace/list")
        .await
        .expect("Failed to list namespaces");

    assert_eq!(response["code"], 0, "List namespaces should succeed");
    assert!(
        response["data"].is_array(),
        "Should return array of namespaces"
    );
}

/// Test create namespace via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_create_namespace() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let ns_id = format!("v3admin-ns-{}", unique_test_id());

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/core/namespace",
            &[
                ("customNamespaceId", ns_id.as_str()),
                ("namespaceName", "V3 Admin Test NS"),
                ("namespaceDesc", "Created by V3 admin test"),
            ],
        )
        .await
        .expect("Failed to create namespace");

    assert_eq!(response["code"], 0, "Create namespace should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/core/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test get namespace via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_namespace() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let ns_id = format!("v3admin-get-ns-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/core/namespace",
            &[
                ("customNamespaceId", ns_id.as_str()),
                ("namespaceName", "V3 Admin Get NS"),
            ],
        )
        .await
        .expect("Failed to create namespace");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/core/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .expect("Failed to get namespace");

    assert_eq!(response["code"], 0, "Get namespace should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/core/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test update namespace via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_update_namespace() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let ns_id = format!("v3admin-upd-ns-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/core/namespace",
            &[
                ("customNamespaceId", ns_id.as_str()),
                ("namespaceName", "Original Name"),
            ],
        )
        .await
        .expect("Failed to create namespace");

    // Update
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v3/admin/core/namespace",
            &[
                ("namespaceId", ns_id.as_str()),
                ("namespaceName", "Updated Name"),
                ("namespaceDesc", "Updated via V3 admin"),
            ],
        )
        .await
        .expect("Failed to update namespace");

    assert_eq!(response["code"], 0, "Update namespace should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/core/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .ok()
        .unwrap_or_default();
}

/// Test delete namespace via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_delete_namespace() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let ns_id = format!("v3admin-del-ns-{}", unique_test_id());

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v3/admin/core/namespace",
            &[
                ("customNamespaceId", ns_id.as_str()),
                ("namespaceName", "To Delete"),
            ],
        )
        .await
        .expect("Failed to create namespace");

    // Delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/core/namespace",
            &[("namespaceId", ns_id.as_str())],
        )
        .await
        .expect("Failed to delete namespace");

    assert_eq!(response["code"], 0, "Delete namespace should succeed");
}

// ========== Core Ops ==========

/// Test get ID generator state via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_core_get_ids() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/core/ops/ids")
        .await
        .expect("Failed to get IDs");

    assert_eq!(response["code"], 0, "Get IDs should succeed");
}

// ========== Loader Endpoints ==========

/// Test get current server loader info
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_loader_current() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/core/loader/current")
        .await
        .expect("Failed to get current loader");

    assert_eq!(response["code"], 0, "Get current loader should succeed");
}

/// Test get cluster loader info
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_loader_cluster() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/core/loader/cluster")
        .await
        .expect("Failed to get cluster loader");

    assert_eq!(response["code"], 0, "Get cluster loader should succeed");
}
