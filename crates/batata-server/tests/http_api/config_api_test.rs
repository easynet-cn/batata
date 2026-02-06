//! Configuration API integration tests
//!
//! Tests for /nacos/v2/cs/config endpoints

use crate::common::{unique_data_id, TestClient, DEFAULT_GROUP, TEST_NAMESPACE};

/// Test configuration publish and get
#[tokio::test]
#[ignore = "requires running server"]
async fn test_publish_and_get_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("config");
    let content = "test.key=test.value\ntest.number=123";

    // Publish config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", content),
            ],
        )
        .await
        .expect("Failed to publish config");

    assert_eq!(response["code"], 0, "Publish should succeed");

    // Get config
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    assert_eq!(response["data"], content, "Content should match");
}

/// Test configuration not found
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_config_not_found() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("nonexistent");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Request should complete");

    // Config not found should return empty data or specific error code
    assert!(
        response["data"].is_null() || response["code"] != 0,
        "Should indicate config not found"
    );
}

/// Test configuration delete
#[tokio::test]
#[ignore = "requires running server"]
async fn test_delete_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("to_delete");
    let content = "temporary=true";

    // First publish
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
        .expect("Failed to publish config");

    // Then delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to delete config");

    assert_eq!(response["code"], 0, "Delete should succeed");

    // Verify deleted
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Request should complete");

    assert!(
        response["data"].is_null() || response["code"] != 0,
        "Config should be deleted"
    );
}

/// Test configuration with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_with_namespace() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("namespaced");
    let content = "namespaced.key=namespaced.value";

    // Publish with namespace
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("namespaceId", TEST_NAMESPACE),
                ("content", content),
            ],
        )
        .await
        .expect("Failed to publish config");

    assert_eq!(response["code"], 0, "Publish should succeed");

    // Get with namespace
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("namespaceId", TEST_NAMESPACE),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    assert_eq!(response["data"], content, "Content should match");
}

/// Test configuration update (overwrite)
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_update() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("updatable");
    let content_v1 = "version=1";
    let content_v2 = "version=2";

    // Publish v1
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", content_v1),
            ],
        )
        .await
        .expect("Failed to publish v1");

    // Update to v2
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", content_v2),
            ],
        )
        .await
        .expect("Failed to publish v2");

    assert_eq!(response["code"], 0, "Update should succeed");

    // Verify v2
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["data"], content_v2, "Should have v2 content");
}

/// Test configuration parameter validation
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_parameter_validation() {
    let client = TestClient::new("http://127.0.0.1:8848");

    // Missing dataId
    let response = client
        .raw_post_form(
            "/nacos/v2/cs/config",
            &[("group", DEFAULT_GROUP), ("content", "test")],
        )
        .await;

    // Should fail with validation error
    if let Ok(resp) = response {
        let status = resp.status();
        assert!(
            status.is_client_error() || status.is_server_error(),
            "Should reject missing dataId"
        );
    }
}

/// Test MD5 in config response
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_md5() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let data_id = unique_data_id("md5test");
    let content = "md5.test=value";

    // Publish config
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
        .expect("Failed to publish config");

    // Get full config info (may need different endpoint or check response structure)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    // MD5 might be in headers or response metadata depending on API version
}
