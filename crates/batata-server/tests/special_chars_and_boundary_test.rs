//! Special Characters and Boundary Conditions Tests
//!
//! Tests for special characters, unicode, and edge cases

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id, unique_service_name,
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

// ==================== Config Tests ====================

/// Test config with special characters in content
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_special_characters() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("special_chars");

    let special_content = "special.chars=!@#$%^&*()_+-=[]{}|;:,.<>?/~`\nquotes=\"\"''\nbackslash=\\\\";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", special_content),
            ],
        )
        .await
        .expect("Failed to publish special chars");

    assert_eq!(response["code"], 0, "Special chars should succeed");

    // Verify content is preserved
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(
        response["data"]["content"], special_content,
        "Special chars should be preserved"
    );
}

/// Test config with unicode characters
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_unicode() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("unicode");

    let unicode_content = "unicode.中文=你好世界\nemoji=✨🚀💻🎉\narabic=مرحبا\nrussian=Привет";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", unicode_content),
            ],
        )
        .await
        .expect("Failed to publish unicode");

    assert_eq!(response["code"], 0, "Unicode should succeed");

    // Verify unicode is preserved
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(
        response["data"]["content"], unicode_content,
        "Unicode should be preserved"
    );
}

/// Test config with very long dataId
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_long_data_id() {
    let client = authenticated_client().await;
    let long_data_id = "a".repeat(200);

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", long_data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish long dataId");

    assert_eq!(response["code"], 0, "Long dataId should succeed");
}

/// Test config with very long content
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_long_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("long_content");
    let long_content: String = (0..10000)
        .map(|i| format!("line{}=value{}\n", i, i))
        .collect();

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", long_content.as_str()),
            ],
        )
        .await
        .expect("Failed to publish long content");

    assert_eq!(response["code"], 0, "Long content should succeed");
}

/// Test config with empty content
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_empty_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("empty_content");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", ""),
            ],
        )
        .await
        .expect("Failed to publish empty content");

    assert_eq!(response["code"], 0, "Empty content should succeed");
}

/// Test config with whitespace only content
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_whitespace_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("whitespace");

    let whitespace_content = "   \n\t  \r\n   ";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", whitespace_content),
            ],
        )
        .await
        .expect("Failed to publish whitespace");

    assert_eq!(response["code"], 0, "Whitespace should succeed");
}

/// Test config with special characters in dataId
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_special_chars_data_id() {
    let client = authenticated_client().await;
    let special_data_id = "test-with_special.chars.123@#";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", special_data_id),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish special dataId");

    assert_eq!(response["code"], 0, "Special chars dataId should succeed");
}

/// Test config with very long group
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_long_group() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("long_group");
    let long_group = "G".repeat(200);

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", long_group.as_str()),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish long group");

    assert_eq!(response["code"], 0, "Long group should succeed");
}

/// Test config with special characters in group
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_special_chars_group() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("special_group");
    let special_group = "GROUP-WITH_SPECIAL.CHARS@";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", special_group),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish special group");

    assert_eq!(response["code"], 0, "Special chars group should succeed");
}

/// Test config with binary-like content
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_binary_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("binary");
    let binary_content = "binary.data=\\x00\\x01\\x02\\xff\\xfe";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", binary_content),
            ],
        )
        .await
        .expect("Failed to publish binary-like");

    assert_eq!(response["code"], 0, "Binary-like should succeed");
}

// ==================== Naming Tests ====================

/// Test service name with special characters
#[tokio::test]
#[ignore = "requires running server"]
async fn test_service_special_chars() {
    let client = authenticated_client().await;
    let service_name = format!("service-with_special.chars.{}", unique_test_id());

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.100",
                "port": 8080
            }),
        )
        .await
        .expect("Failed to register special service");

    assert_eq!(response["code"], 0, "Special chars service should succeed");
}

/// Test service name with unicode
#[tokio::test]
#[ignore = "requires running server"]
async fn test_service_unicode() {
    let client = authenticated_client().await;
    let service_name = format!("service-测试-service.{}", unique_test_id());

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.101",
                "port": 8080
            }),
        )
        .await
        .expect("Failed to register unicode service");

    assert_eq!(response["code"], 0, "Unicode service should succeed");
}

/// Test instance IP with edge values
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_edge_ip() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("edge_ip");

    // Test with IP 0.0.0.0
    let response1: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "0.0.0.0",
                "port": 8080
            }),
        )
        .await
        .expect("Failed to register 0.0.0.0");

    assert_eq!(response1["code"], 0, "0.0.0.0 should succeed");

    // Test with IP 255.255.255.255
    let response2: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": unique_service_name("edge_ip_2"),
                "ip": "255.255.255.255",
                "port": 8080
            }),
        )
        .await
        .expect("Failed to register 255.255.255.255");

    assert_eq!(response2["code"], 0, "255.255.255.255 should succeed");
}

/// Test instance with edge port values
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_edge_port() {
    let client = authenticated_client().await;

    // Test with port 1
    let response1: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": unique_service_name("port_1"),
                "ip": "192.168.1.110",
                "port": 1
            }),
        )
        .await
        .expect("Failed to register port 1");

    assert_eq!(response1["code"], 0, "Port 1 should succeed");

    // Test with port 65535
    let response2: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": unique_service_name("port_65535"),
                "ip": "192.168.1.111",
                "port": 65535
            }),
        )
        .await
        .expect("Failed to register port 65535");

    assert_eq!(response2["code"], 0, "Port 65535 should succeed");
}

/// Test instance with edge weight values
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_edge_weight() {
    let client = authenticated_client().await;

    // Test with weight 0.0
    let response1: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": unique_service_name("weight_0"),
                "ip": "192.168.1.120",
                "port": 8080,
                "weight": 0.0
            }),
        )
        .await
        .expect("Failed to register weight 0");

    assert_eq!(response1["code"], 0, "Weight 0 should succeed");

    // Test with weight 1.0
    let response2: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": unique_service_name("weight_1"),
                "ip": "192.168.1.121",
                "port": 8080,
                "weight": 1.0
            }),
        )
        .await
        .expect("Failed to register weight 1");

    assert_eq!(response2["code"], 0, "Weight 1 should succeed");

    // Test with weight 100.0
    let response3: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": unique_service_name("weight_100"),
                "ip": "192.168.1.122",
                "port": 8080,
                "weight": 100.0
            }),
        )
        .await
        .expect("Failed to register weight 100");

    assert_eq!(response3["code"], 0, "Weight 100 should succeed");
}

/// Test instance metadata with special characters
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_metadata_special_chars() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("metadata_special");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.130",
                "port": 8080,
                "metadata": {
                    "key.with.special-chars": "value!@#$%",
                    "unicode": "你好世界",
                    "emoji": "✨🚀"
                }
            }),
        )
        .await
        .expect("Failed to register with special metadata");

    assert_eq!(response["code"], 0, "Special metadata should succeed");
}

/// Test instance with very long metadata
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_long_metadata() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("long_metadata");

    let mut metadata = serde_json::Map::new();
    for i in 0..100 {
        metadata.insert(format!("key{}", i), format!("value{}-{}", i, "x".repeat(50)));
    }

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.140",
                "port": 8080,
                "metadata": metadata
            }),
        )
        .await
        .expect("Failed to register with long metadata");

    assert_eq!(response["code"], 0, "Long metadata should succeed");
}

// ==================== Invalid Input Tests ====================

/// Test config with missing dataId
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_missing_data_id() {
    let client = authenticated_client().await;

    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("group", "DEFAULT_GROUP"), ("content", "value")],
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Missing dataId should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test config with missing group
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_missing_group() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("missing_group");

    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("content", "value")],
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Missing group should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test instance with invalid IP
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_invalid_ip() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("invalid_ip");

    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "invalid.ip.address",
                "port": 8080
            }),
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Invalid IP should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test instance with invalid port
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_invalid_port() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("invalid_port");

    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.150",
                "port": 99999
            }),
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Invalid port should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test instance with negative weight
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_negative_weight() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("negative_weight");

    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.160",
                "port": 8080,
                "weight": -1.0
            }),
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Negative weight should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test config exceeding max size
#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_exceeds_max_size() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("oversized");
    let oversized_content = "x".repeat(10_000_000); // 10MB

    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", oversized_content.as_str()),
            ],
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Oversized config should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test namespace with special characters
#[tokio::test]
#[ignore = "requires running server"]
async fn test_namespace_special_chars() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("ns_special");
    let special_namespace = "namespace-with_special.chars-123";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("namespaceId", special_namespace),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish with special namespace");

    assert_eq!(response["code"], 0, "Special namespace should succeed");
}

/// Test null values in JSON
#[tokio::test]
#[ignore = "requires running server"]
async fn test_json_null_values() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("null_values");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.170",
                "port": 8080,
                "weight": null,
                "healthy": null,
                "enabled": null,
                "ephemeral": null
            }),
        )
        .await
        .expect("Failed to register with null values");

    assert_eq!(response["code"], 0, "Null values should be handled");
}

// Helper function
fn unique_test_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test_{}", timestamp)
}
