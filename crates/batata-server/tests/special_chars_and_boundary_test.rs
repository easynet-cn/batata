//! Special Characters and Boundary Conditions Tests
//!
//! Tests for handling special characters, Unicode, and edge cases

mod common;

use common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id, unique_service_name,
};
use serde_json::json;

/// Create an authenticated test client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

// ==================== Config Special Characters Tests ====================

/// Test config with special characters in content
#[tokio::test]

async fn test_config_special_characters_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("special_content");

    let special_content = r#"special=!@#$%^&*()_+-={}[]|\\:";'<>,.?/~`"#;
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", special_content),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Retrieve and verify
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    // Check if data field exists and is not empty
    if response["data"].is_object() {
        // V3 API format: data is an object with "content" field
        let content = response["data"]["content"].as_str().unwrap_or("");
        assert!(!content.is_empty(), "Content should not be empty");
    } else if response["data"].is_string() {
        // V2 API format: data is directly the content string
        let content = response["data"].as_str().unwrap_or("");
        assert!(!content.is_empty(), "Content should not be empty");
    } else {
        panic!("Unexpected response format: {:?}", response);
    }
}

/// Test config with Unicode content
#[tokio::test]

async fn test_config_unicode_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("unicode_content");

    let unicode_content = "unicode=测试测试🚀😊αβγδε";
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", unicode_content),
            ],
        )
        .await
        .expect("Failed to publish config");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}

/// Test config with very long dataId
#[tokio::test]

async fn test_config_very_long_dataid() {
    let client = authenticated_client().await;
    let long_data_id = "a".repeat(255);

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", long_data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "test.value=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", long_data_id.as_str()),
                ("group", DEFAULT_GROUP),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}

/// Test config with very long content
#[tokio::test]

async fn test_config_very_long_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("long_content");

    // Use a content size that fits within the URL encoding limit (16384 bytes)
    let long_content = format!("value={}", "x".repeat(10000));
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", long_content.as_str()),
            ],
        )
        .await
        .expect("Failed to publish config");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}

/// Test config with empty content - server should reject it
#[tokio::test]

async fn test_config_empty_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("empty_content");

    // Try to publish config with empty content - should fail with 400
    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", ""),
            ],
        )
        .await;

    assert!(result.is_err(), "Empty content should be rejected");
    if let Err(e) = result {
        assert!(e.to_string().contains("400") || e.to_string().contains("content"),
                "Error should be about missing content: {}", e);
    }
}

/// Test config with whitespace-only content
#[tokio::test]

async fn test_config_whitespace_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("whitespace_content");

    let whitespace_content = "   \t\n\r   ";
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", whitespace_content),
            ],
        )
        .await
        .expect("Failed to publish config");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}

// ==================== Naming Special Characters Tests ====================

/// Test service name with special characters
#[tokio::test]

async fn test_service_special_characters() {
    let client = authenticated_client().await;
    let service_name = "test-service_v1.0";

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", "192.168.1.200"),
                ("port", "8080"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register instance");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name)],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test instance with boundary IP values
#[tokio::test]

async fn test_instance_boundary_ip() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("boundary_ip");

    // Test minimum IP
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "0.0.0.0"),
                ("port", "8080"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register instance");

    // Test maximum IP
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "255.255.255.255"),
                ("port", "8081"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register instance");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test instance with boundary port values
#[tokio::test]

async fn test_instance_boundary_port() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("boundary_port");

    // Test minimum port
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.201"),
                ("port", "1"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register instance");

    // Test maximum port
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.202"),
                ("port", "65535"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register instance");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test instance with boundary weight values
#[tokio::test]

async fn test_instance_boundary_weight() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("boundary_weight");

    // Test minimum weight
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.203"),
                ("port", "8080"),
                ("weight", "0.1"),
            ],
        )
        .await
        .expect("Failed to register instance");

    // Test maximum weight
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.204"),
                ("port", "8081"),
                ("weight", "100.0"),
            ],
        )
        .await
        .expect("Failed to register instance");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test instance with special characters in metadata
#[tokio::test]

async fn test_instance_special_metadata() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("special_metadata");

    let metadata = json!({
        "special.chars.key": "special=value!@#",
        "unicode.key": "测试中文🚀",
        "empty.key": "",
        "null.key": null
    });

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.205"),
                ("port", "8080"),
                ("weight", "1.0"),
                ("metadata", metadata.to_string().as_str()),
            ],
        )
        .await
        .expect("Failed to register instance");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test config with very long group name
#[tokio::test]

async fn test_config_very_long_group() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("long_group");
    let long_group = "G".repeat(128);

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", long_group.as_str()),
                ("content", "test.value=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", long_group.as_str()),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}
