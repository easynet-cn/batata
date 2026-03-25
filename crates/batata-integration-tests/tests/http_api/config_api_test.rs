//! Configuration API integration tests
//!
//! Tests for /nacos/v2/cs/config endpoints

use batata_integration_tests::{
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

    match result {
        Ok(response) => {
            // Should return error code 20004 (config not found) or similar non-zero code
            let code = response["code"].as_i64().unwrap_or(-1);
            assert_ne!(code, 0, "Config not found should return non-zero code");
            // Nacos uses 20004 for "config data not exist"
            assert_eq!(
                code, 20004,
                "Config not found should return error code 20004, got {}",
                code
            );
        }
        Err(e) => {
            // HTTP 404 is also acceptable for config not found
            let err_str = format!("{}", e);
            assert!(
                err_str.contains("404") || err_str.contains("Not found"),
                "Expected 404/Not found error, got: {}",
                err_str
            );
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

    // Verify deleted - GET should return config not found
    let result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await;

    match result {
        Ok(response) => {
            let code = response["code"].as_i64().unwrap_or(-1);
            assert_ne!(code, 0, "Deleted config should not return success");
            assert_eq!(
                code, 20004,
                "Deleted config should return 20004 (not found), got {}",
                code
            );
        }
        Err(e) => {
            // HTTP 404 is also acceptable
            let err_str = format!("{}", e);
            assert!(
                err_str.contains("404") || err_str.contains("Not found"),
                "Expected 404/Not found after delete, got: {}",
                err_str
            );
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

    // Missing dataId - use raw request to inspect status and body
    let response = client
        .raw_post_form(
            "/nacos/v2/cs/config",
            &[("group", DEFAULT_GROUP), ("content", "test")],
        )
        .await
        .expect("HTTP request itself should not fail");

    let status = response.status();
    let body = response.text().await.expect("Should read response body");

    // Should fail with validation error - either HTTP 400 or JSON error code
    if status.is_client_error() {
        assert_eq!(
            status.as_u16(),
            400,
            "Validation error should return HTTP 400, got {}",
            status
        );
    } else {
        // Some APIs return 200 with an error code in the JSON body
        let json: serde_json::Value =
            serde_json::from_str(&body).expect("Response should be valid JSON");
        let code = json["code"].as_i64().unwrap_or(0);
        assert_ne!(
            code, 0,
            "Missing dataId should return a non-zero error code"
        );
        // Nacos uses error codes in the 2xxxx or 4xxxx range for parameter errors
        assert!(
            code == 400 || (20000..30000).contains(&code) || (40000..50000).contains(&code),
            "Expected a parameter validation error code, got {}",
            code
        );
    }
}

/// Test that config update generates history records
///
/// Verifies the fix: both create AND update must generate history entries.
/// Previously, updates with unchanged content would skip history recording.
/// Nacos unconditionally records history on every create/update/delete.
#[tokio::test]

async fn test_config_update_generates_history() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("history_on_update");
    let content_v1 = "version=1";
    let content_v2 = "version=2";

    // Step 1: Create config (should generate history with op_type "I")
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", content_v1),
            ],
        )
        .await
        .expect("Failed to publish config v1");

    assert_eq!(response["code"], 0, "Publish v1 should succeed");

    // Step 2: Query history - should have 1 entry (create)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/history/list",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "100"),
            ],
        )
        .await
        .expect("Failed to list history after create");

    assert_eq!(response["code"], 0, "History list should succeed");
    let total_after_create = response["data"]["totalCount"].as_i64().unwrap_or(0);
    assert!(
        total_after_create >= 1,
        "Should have at least 1 history entry after create, got {}",
        total_after_create
    );

    // Step 3: Update config with new content (should generate history with op_type "U")
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
        .expect("Failed to publish config v2");

    assert_eq!(response["code"], 0, "Publish v2 should succeed");

    // Step 4: Query history again - should have 2 entries (create + update)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/history/list",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "100"),
            ],
        )
        .await
        .expect("Failed to list history after update");

    assert_eq!(response["code"], 0, "History list should succeed");
    let total_after_update = response["data"]["totalCount"].as_i64().unwrap_or(0);
    assert!(
        total_after_update >= 2,
        "Should have at least 2 history entries after update, got {}",
        total_after_update
    );

    // Cleanup
    let _ = client
        .delete_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await;
}

/// Test that config update with same content still generates history
///
/// Even when content doesn't change, Nacos records a history entry.
/// This ensures the fix handles the "no-op update" case correctly.
#[tokio::test]

async fn test_config_update_same_content_generates_history() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("history_same_content");
    let content = "unchanged.key=unchanged.value";

    // Step 1: Create config
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

    // Step 2: Update with SAME content (this was the bug - no history was generated)
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
        .expect("Failed to re-publish config");

    assert_eq!(response["code"], 0, "Re-publish should succeed");

    // Step 3: Query history - should have 2 entries even though content didn't change
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/history/list",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "100"),
            ],
        )
        .await
        .expect("Failed to list history");

    assert_eq!(response["code"], 0, "History list should succeed");
    let total = response["data"]["totalCount"].as_i64().unwrap_or(0);
    assert!(
        total >= 2,
        "Should have at least 2 history entries even when content unchanged, got {}",
        total
    );

    // Cleanup
    let _ = client
        .delete_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await;
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

    // Get config and check for MD5 field in the response
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("showBeta", "false"),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    assert_eq!(
        response["data"]["content"], content,
        "Content should match published value"
    );

    // Check MD5 field in the response data
    let md5_value = &response["data"]["md5"];
    assert!(
        md5_value.is_string(),
        "Response should contain an md5 field in data, got: {:?}",
        response["data"]
    );

    let md5_str = md5_value.as_str().unwrap();

    // MD5 should be a valid 32-character hex string
    assert_eq!(
        md5_str.len(),
        32,
        "MD5 should be 32 characters long, got {} chars: '{}'",
        md5_str.len(),
        md5_str
    );
    assert!(
        md5_str.chars().all(|c| c.is_ascii_hexdigit()),
        "MD5 should only contain hex digits, got: '{}'",
        md5_str
    );

    // Verify MD5 matches the expected value for the content
    let expected_md5 = format!("{:x}", md5::compute(content.as_bytes()));
    assert_eq!(
        md5_str, expected_md5,
        "MD5 should match md5 of content '{}': expected '{}', got '{}'",
        content, expected_md5, md5_str
    );
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
    assert_eq!(
        response_a["data"]["content"], content_a,
        "Namespace A should have content A"
    );

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
    assert_eq!(
        response_b["data"]["content"], content_b,
        "Namespace B should have content B"
    );

    // Get from default namespace should not find this dataId
    let result_default = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("dataId", data_id), ("group", DEFAULT_GROUP)],
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
