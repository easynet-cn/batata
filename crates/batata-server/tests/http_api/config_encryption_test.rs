//! Configuration Encryption API integration tests
//!
//! Tests for config encryption and decryption functionality

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
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

/// Test publish encrypted config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_publish_encrypted_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "sensitive.password=secret123"),
                ("encrypt", "true"),
                ("encryptionAlgorithm", "AES"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config");

    assert_eq!(response["code"], 0, "Publish encrypted config should succeed");
}

/// Test get and decrypt config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_decrypted_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("decrypted");

    // Publish encrypted config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "api.key=sk_test_12345"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config");

    // Get config (should be decrypted)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get decrypted config");

    assert_eq!(response["code"], 0, "Get decrypted config should succeed");
    assert_eq!(
        response["data"]["content"], "api.key=sk_test_12345",
        "Content should be decrypted"
    );
}

/// Test update encrypted config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_update_encrypted_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("update_encrypted");

    // Publish encrypted config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "password=oldpass123"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config");

    // Update encrypted config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "password=newpass456"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to update encrypted config");

    assert_eq!(response["code"], 0, "Update encrypted config should succeed");

    // Verify update
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get updated config");

    assert_eq!(
        response["data"]["content"], "password=newpass456",
        "Updated content should be decrypted"
    );
}

/// Test encrypted config with custom key
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_custom_key() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("custom_key");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "custom.secret=value"),
                ("encrypt", "true"),
                ("encryptionKey", "my-custom-encryption-key-123"),
            ],
        )
        .await
        .expect("Failed to publish with custom key");

    assert_eq!(response["code"], 0, "Publish with custom key should succeed");
}

/// Test delete encrypted config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_delete_encrypted_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("delete_encrypted");

    // Publish encrypted config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "to.be.deleted=value"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config");

    // Delete encrypted config
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to delete encrypted config");

    assert_eq!(response["code"], 0, "Delete encrypted config should succeed");

    // Verify deletion
    let result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await;

    match result {
        Ok(response) => {
            assert!(
                response["data"].is_null() || response["code"] != 0,
                "Encrypted config should be deleted"
            );
        }
        Err(_) => {
            // HTTP 404 is expected
        }
    }
}

/// Test encrypted config MD5 calculation
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_md5() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("md5_encrypted");

    // Publish encrypted config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "md5.test=value"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config");

    assert_eq!(response["code"], 0, "Publish should succeed");
    // MD5 should be calculated on decrypted content
    assert!(response["data"]["md5"].is_string(), "Should have MD5");
}

/// Test mix encrypted and unencrypted configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_mixed_encrypted_unencrypted() {
    let client = authenticated_client().await;

    // Publish encrypted config
    let data_id_encrypted = unique_data_id("encrypted_mix");
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id_encrypted.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "secret.value=123"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config");

    // Publish unencrypted config
    let data_id_normal = unique_data_id("normal_mix");
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id_normal.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "normal.value=456"),
            ],
        )
        .await
        .expect("Failed to publish normal config");

    // Get both configs
    let response_encrypted: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id_encrypted.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get encrypted config");

    let response_normal: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id_normal.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get normal config");

    assert_eq!(response_encrypted["code"], 0, "Encrypted get should succeed");
    assert_eq!(response_normal["code"], 0, "Normal get should succeed");
    assert_eq!(
        response_encrypted["data"]["content"], "secret.value=123",
        "Encrypted should be decrypted"
    );
    assert_eq!(
        response_normal["data"]["content"], "normal.value=456",
        "Normal content should be unchanged"
    );
}

/// Test encrypted config with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_ns");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("namespaceId", "test-namespace"),
                ("content", "ns.secret=value"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config with namespace");

    assert_eq!(response["code"], 0, "Encrypted with namespace should succeed");
}

/// Test encrypted config special characters
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_special_chars() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_special");

    let special_content = "special.chars=!@#$%^&*()_+-=[]{}|;:,.<>?/~`";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", special_content),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted special chars");

    assert_eq!(response["code"], 0, "Special chars encrypted should succeed");

    // Verify decryption preserves special characters
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get decrypted special chars");

    assert_eq!(
        response["data"]["content"], special_content,
        "Special characters should be preserved"
    );
}

/// Test encrypted config unicode
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_unicode() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_unicode");

    let unicode_content = "unicode.测试=你好世界🌍\nemoji=✨🚀💻";

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", unicode_content),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted unicode");

    assert_eq!(response["code"], 0, "Unicode encrypted should succeed");

    // Verify decryption preserves unicode
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get decrypted unicode");

    assert_eq!(
        response["data"]["content"], unicode_content,
        "Unicode should be preserved"
    );
}

/// Test encrypted config with different algorithms
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_algorithms() {
    let client = authenticated_client().await;

    // Test AES encryption
    let data_id_aes = unique_data_id("aes_encrypted");
    let response_aes: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id_aes.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "aes.test=value"),
                ("encrypt", "true"),
                ("encryptionAlgorithm", "AES"),
            ],
        )
        .await
        .expect("Failed to publish AES encrypted");

    assert_eq!(response_aes["code"], 0, "AES encryption should succeed");
}

/// Test encrypted config long content
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_long_content() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_long");

    // Generate long content
    let long_content: String = (0..1000)
        .map(|i| format!("line{}=secret_value_{}\n", i, i))
        .collect();

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", long_content.as_str()),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish long encrypted config");

    assert_eq!(response["code"], 0, "Long encrypted should succeed");
}

/// Test encrypted config import/export
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_export() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_export");

    // Publish encrypted config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "export.secret=value"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted config");

    // Export encrypted config (should export decrypted content)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/export",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to export encrypted config");

    assert_eq!(response["code"], 0, "Export encrypted should succeed");
}

/// Test encrypted config beta
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_beta() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_beta");

    // Publish main config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "main.secret=value1"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish main encrypted");

    // Publish beta config (also encrypted)
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta.secret=value2"),
                ("encrypt", "true"),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to publish beta encrypted");

    assert_eq!(response["code"], 0, "Beta encrypted should succeed");
}

/// Test encrypted config with tags
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_tags() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_tags");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "tagged.secret=value"),
                ("encrypt", "true"),
                ("tags", "env=prod,region=us-west,sensitive=true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted with tags");

    assert_eq!(response["code"], 0, "Encrypted with tags should succeed");
}

/// Test encrypted config invalid algorithm
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_invalid_algorithm() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_invalid_algo");

    // Try to publish with invalid encryption algorithm
    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
                ("encrypt", "true"),
                ("encryptionAlgorithm", "INVALID_ALGO"),
            ],
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Invalid algorithm should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test encrypted config without key (should use default)
#[tokio::test]
#[ignore = "requires running server"]
async fn test_encrypted_config_default_key() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("encrypted_default_key");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "default.key=value"),
                ("encrypt", "true"),
            ],
        )
        .await
        .expect("Failed to publish encrypted with default key");

    assert_eq!(response["code"], 0, "Default key encryption should succeed");

    // Verify decryption works
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get decrypted");

    assert_eq!(
        response["data"]["content"], "default.key=value",
        "Should decrypt with default key"
    );
}
