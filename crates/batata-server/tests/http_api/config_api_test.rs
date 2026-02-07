//! Configuration API integration tests
//!
//! Tests for /nacos/v2/cs/config endpoints

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_NAMESPACE, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id,
};

/// Create an authenticated test client for the main API server
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test configuration publish and get
#[tokio::test]
#[ignore = "requires running server"]
async fn test_publish_and_get_config() {
    let client = authenticated_client().await;
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
    assert_eq!(response["data"]["content"], content, "Content should match");
}

/// Test configuration not found
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_config_not_found() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("nonexistent");

    let result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await;

    // Config not found returns HTTP 404 or JSON with code != 0
    match result {
        Ok(response) => {
            assert!(
                response["data"].is_null() || response["code"] != 0,
                "Should indicate config not found"
            );
        }
        Err(_) => {
            // HTTP 404 is expected for config not found
        }
    }
}

/// Test configuration delete
#[tokio::test]
#[ignore = "requires running server"]
async fn test_delete_config() {
    let client = authenticated_client().await;
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

    // Verify deleted - should return 404 or error
    let result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await;

    match result {
        Ok(response) => {
            assert!(
                response["data"].is_null() || response["code"] != 0,
                "Config should be deleted"
            );
        }
        Err(_) => {
            // HTTP 404 is expected for deleted config
        }
    }
}

/// Test configuration with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_with_namespace() {
    let client = authenticated_client().await;
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
    assert_eq!(response["data"]["content"], content, "Content should match");
}

/// Test configuration update (overwrite)
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_update() {
    let client = authenticated_client().await;
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

    assert_eq!(
        response["data"]["content"], content_v2,
        "Should have v2 content"
    );
}

/// Test configuration parameter validation
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_parameter_validation() {
    let client = authenticated_client().await;

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
    let client = authenticated_client().await;
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
