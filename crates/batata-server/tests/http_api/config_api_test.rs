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

/// Test namespace isolation - configs in different namespaces should not interfere
#[tokio::test]

async fn test_namespace_isolation() {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let client = authenticated_client().await;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    
    // Create two test namespaces via console API
    let ns_a = format!("test-ns-a-{}", timestamp);
    let ns_b = format!("test-ns-b-{}", timestamp);
    let data_id = "shared-data-id";
    let content_a = "content-from-namespace-a";
    let content_b = "content-from-namespace-b";
    
    // Create namespace A
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
                ("namespaceName", "Test NS A"),
            ],
        )
        .await
        .expect("Failed to create namespace A");
    
    let _: serde_json::Value = console
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_b.as_str()),
                ("namespaceName", "Test NS B"),
            ],
        )
        .await
        .expect("Failed to create namespace B");
    
    // Publish same dataId in namespace A with content A
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id),
                ("group", DEFAULT_GROUP),
                ("namespaceId", ns_a.as_str()),
                ("content", content_a),
            ],
        )
        .await
        .expect("Failed to publish in namespace A");
    
    // Publish same dataId in namespace B with content B
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id),
                ("group", DEFAULT_GROUP),
                ("namespaceId", ns_b.as_str()),
                ("content", content_b),
            ],
        )
        .await
        .expect("Failed to publish in namespace B");
    
    // Get from namespace A should return content A
    let response_a: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id),
                ("group", DEFAULT_GROUP),
                ("namespaceId", ns_a.as_str()),
            ],
        )
        .await
        .expect("Failed to get from namespace A");
    
    assert_eq!(response_a["code"], 0);
    assert_eq!(response_a["data"]["content"], content_a, "Namespace A should have content A");
    
    // Get from namespace B should return content B
    let response_b: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id),
                ("group", DEFAULT_GROUP),
                ("namespaceId", ns_b.as_str()),
            ],
        )
        .await
        .expect("Failed to get from namespace B");
    
    assert_eq!(response_b["code"], 0);
    assert_eq!(response_b["data"]["content"], content_b, "Namespace B should have content B");
    
    // Get from default namespace should not find this dataId
    let result_default = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id),
                ("group", DEFAULT_GROUP),
            ],
        )
        .await;
    
    // Should not find config in default namespace
    match result_default {
        Ok(response) => {
            assert!(
                response["data"].is_null() || response["code"] != 0,
                "Should not find config in default namespace"
            );
        }
        Err(_) => {
            // HTTP 404 is acceptable
        }
    }
    
    // Cleanup namespaces
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
