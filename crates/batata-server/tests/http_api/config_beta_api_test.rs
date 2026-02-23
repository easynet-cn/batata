//! Configuration Beta (Gray Release) API integration tests
//!
//! Tests for config beta distribution and release features

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_NAMESPACE, TEST_PASSWORD, TEST_USERNAME,
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

/// Test publish config with beta release
#[tokio::test]
#[ignore = "requires running server"]
async fn test_publish_beta_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta");

    // First publish main config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "main.config=value1"),
            ],
        )
        .await
        .expect("Failed to publish main config");

    // Publish beta config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta.config=value2"),
                ("betaIps", "192.168.1.100,192.168.1.101"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    assert_eq!(response["code"], 0, "Beta publish should succeed");

    // Get config from beta IP - should return beta version
    // Note: This test may require request from specific IP or using header
}

/// Test get beta config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_beta_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_get");

    // Publish main config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "main=value"),
            ],
        )
        .await
        .expect("Failed to publish main config");

    // Publish beta config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta=enabled"),
                ("betaIps", "127.0.0.1"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Get beta config (may require special query param or header)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get beta config");

    // Beta endpoint should return beta config info
    assert_eq!(response["code"], 0, "Get beta should succeed");
}

/// Test delete beta config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_delete_beta_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_delete");

    // Publish main and beta config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "main=value"),
            ],
        )
        .await
        .expect("Failed to publish main config");

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta=value"),
                ("betaIps", "192.168.1.1"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Delete beta config
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to delete beta config");

    assert_eq!(response["code"], 0, "Delete beta should succeed");

    // Verify main config still exists
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
        response["data"]["content"], "main=value",
        "Main config should remain"
    );
}

/// Test beta config with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_beta_config_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_ns");

    // Publish beta config with namespace
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta.ns=value"),
                ("betaIps", "192.168.1.200"),
                ("namespaceId", TEST_NAMESPACE),
            ],
        )
        .await
        .expect("Failed to publish beta config with namespace");

    assert_eq!(response["code"], 0, "Beta with namespace should succeed");
}

/// Test beta config with tags
#[tokio::test]
#[ignore = "requires running server"]
async fn test_beta_config_with_tags() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_tags");

    // Publish beta config with tags
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta.tags=value"),
                ("betaIps", "192.168.1.150"),
                ("tags", "env=prod,region=us-west"),
            ],
        )
        .await
        .expect("Failed to publish beta config with tags");

    assert_eq!(response["code"], 0, "Beta with tags should succeed");
}

/// Test beta config with multiple IPs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_beta_config_multiple_ips() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_multi");

    // Publish beta config with multiple IPs
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta.multi=value"),
                ("betaIps", "192.168.1.10,192.168.1.11,192.168.1.12,192.168.1.13"),
            ],
        )
        .await
        .expect("Failed to publish beta config with multiple IPs");

    assert_eq!(response["code"], 0, "Beta with multiple IPs should succeed");
}

/// Test beta config invalid IP format
#[tokio::test]
#[ignore = "requires running server"]
async fn test_beta_config_invalid_ip() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_invalid");

    // Try to publish with invalid IP
    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta=value"),
                ("betaIps", "invalid.ip.address"),
            ],
        )
        .await;

    match result {
        Ok(response) => {
            // Should return error code
            assert_ne!(response["code"], 0, "Invalid IP should be rejected");
        }
        Err(_) => {
            // HTTP error is also acceptable
        }
    }
}

/// Test beta config empty betaIps
#[tokio::test]
#[ignore = "requires running server"]
async fn test_beta_config_empty_ips() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_empty");

    // Try to publish without betaIps
    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta=value"),
            ],
        )
        .await;

    match result {
        Ok(response) => {
            // Should return error code - betaIps is required
            assert_ne!(response["code"], 0, "Empty betaIps should be rejected");
        }
        Err(_) => {
            // HTTP error is also acceptable
        }
    }
}

/// Test publish beta overwrites existing beta
#[tokio::test]
#[ignore = "requires running server"]
async fn test_publish_beta_overwrite() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_overwrite");

    // Publish main config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "main=value"),
            ],
        )
        .await
        .expect("Failed to publish main config");

    // Publish first beta
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta.v1=enabled"),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to publish first beta");

    // Publish second beta (overwrites first)
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta.v2=enabled"),
                ("betaIps", "192.168.1.200"),
            ],
        )
        .await
        .expect("Failed to publish second beta");

    assert_eq!(response["code"], 0, "Beta overwrite should succeed");
}

/// Test beta config release to all
#[tokio::test]
#[ignore = "requires running server"]
async fn test_beta_config_release_all() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_release");

    // Publish main config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "main=value"),
            ],
        )
        .await
        .expect("Failed to publish main config");

    // Publish beta config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta=enabled"),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Publish beta config to all IPs (release)
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta=enabled"),
            ],
        )
        .await
        .expect("Failed to release beta config");

    assert_eq!(response["code"], 0, "Release beta should succeed");

    // Verify main config now has beta content
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
        response["data"]["content"], "beta=enabled",
        "Main config should have beta content"
    );
}

/// Test get all beta configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_list_beta_configs() {
    let client = authenticated_client().await;
    let data_id1 = unique_data_id("beta_list1");
    let data_id2 = unique_data_id("beta_list2");

    // Publish multiple beta configs
    for data_id in &[&data_id1, &data_id2] {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", "main=value"),
                ],
            )
            .await
            .expect("Failed to publish main config");

        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config/beta",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", "beta=enabled"),
                    ("betaIps", "192.168.1.100"),
                ],
            )
            .await
            .expect("Failed to publish beta config");
    }

    // List all beta configs
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/beta/list",
            &[("group", "DEFAULT_GROUP")],
        )
        .await
        .expect("Failed to list beta configs");

    assert_eq!(response["code"], 0, "List beta should succeed");
}

/// Test beta config query with search
#[tokio::test]
#[ignore = "requires running server"]
async fn test_search_beta_configs() {
    let client = authenticated_client().await;
    let prefix = unique_data_id("beta_search");
    let data_id1 = format!("{}_one", prefix);
    let data_id2 = format!("{}_two", prefix);

    // Publish beta configs
    for data_id in &[&data_id1, &data_id2] {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", "main=value"),
                ],
            )
            .await
            .expect("Failed to publish main config");

        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config/beta",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", "beta=enabled"),
                    ("betaIps", "192.168.1.100"),
                ],
            )
            .await
            .expect("Failed to publish beta config");
    }

    // Search beta configs
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/beta/list",
            &[
                ("search", "accurate"),
                ("dataId", prefix.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to search beta configs");

    assert_eq!(response["code"], 0, "Search beta should succeed");
}
