//! V3 Admin Config API integration tests
//!
//! Tests for /nacos/v3/admin/cs/* endpoints

use crate::common::{DEFAULT_GROUP, TestClient, unique_data_id};
use serde_json::json;

// ========== Config CRUD ==========

/// Test create config via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_create_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3admin_config");
    let content = "v3.admin.key=value";

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/cs/config",
            &json!({
                "dataId": data_id,
                "group": DEFAULT_GROUP,
                "content": content
            }),
        )
        .await
        .expect("Failed to create config");

    assert_eq!(response["code"], 0, "Create config should succeed");
}

/// Test get config via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_get_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3admin_get");
    let content = "v3.get.key=value";

    // Create first
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/cs/config",
            &json!({
                "dataId": data_id,
                "group": DEFAULT_GROUP,
                "content": content
            }),
        )
        .await
        .expect("Failed to create config");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get config should succeed");
}

/// Test update config via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_update_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3admin_upd");
    let content_v1 = "version=1";
    let content_v2 = "version=2";

    // Create
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/cs/config",
            &json!({
                "dataId": data_id,
                "group": DEFAULT_GROUP,
                "content": content_v1
            }),
        )
        .await
        .expect("Failed to create config");

    // Update
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v3/admin/cs/config",
            &json!({
                "dataId": data_id,
                "group": DEFAULT_GROUP,
                "content": content_v2
            }),
        )
        .await
        .expect("Failed to update config");

    assert_eq!(response["code"], 0, "Update config should succeed");

    // Verify update
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}

/// Test delete config via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_delete_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3admin_del");
    let content = "to_delete=true";

    // Create
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/cs/config",
            &json!({
                "dataId": data_id,
                "group": DEFAULT_GROUP,
                "content": content
            }),
        )
        .await
        .expect("Failed to create config");

    // Delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to delete config");

    assert_eq!(response["code"], 0, "Delete config should succeed");

    // Verify deleted
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Request should complete");

    assert!(
        response["data"].is_null() || response["code"] != 0,
        "Config should be deleted"
    );
}

/// Test config parameter validation
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_validation() {
    let client = TestClient::new("http://127.0.0.1:8848");

    // Missing dataId should fail
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/cs/config",
            &json!({
                "group": DEFAULT_GROUP,
                "content": "test"
            }),
        )
        .await
        .expect("Request should complete");

    assert_ne!(response["code"], 0, "Should fail with missing dataId");
}

// ========== Config History ==========

/// Test list config history via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_history_list() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3admin_hist");

    // Create config to generate history
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/cs/config",
            &json!({
                "dataId": data_id,
                "group": DEFAULT_GROUP,
                "content": "history=test"
            }),
        )
        .await
        .expect("Failed to create config");

    // List history
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/history/list",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to list history");

    assert_eq!(response["code"], 0, "List history should succeed");
}

// ========== Config Listener ==========

/// Test get listener state via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_listener() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("v3admin_listen");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/listener",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get listener state");

    assert_eq!(response["code"], 0, "Get listener should succeed");
}

// ========== Config Metrics ==========

/// Test get cluster metrics via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_cluster_metrics() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/cs/metrics/cluster")
        .await
        .expect("Failed to get cluster metrics");

    assert_eq!(response["code"], 0, "Get cluster metrics should succeed");
    assert!(response["data"].is_object(), "Should return metrics data");
}

/// Test get IP metrics via V3 Admin API
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_ip_metrics() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get_with_query("/nacos/v3/admin/cs/metrics/ip", &[("ip", "127.0.0.1")])
        .await
        .expect("Failed to get IP metrics");

    assert_eq!(response["code"], 0, "Get IP metrics should succeed");
}
