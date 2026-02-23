//! Config Beta (Canary Release) API integration tests
//!
//! Tests for Beta configuration feature used for canary releases

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id,
};

/// Create an authenticated test client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test publishing beta configuration
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_publish_beta_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta");

    // Publish beta config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "beta.value=1"),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Get config (should return beta value)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    assert!(response["data"].as_str().unwrap().contains("beta.value=1"));
}

/// Test deleting beta configuration
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_delete_beta_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_delete");

    // Publish beta config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "value=1"),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Delete beta config
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to delete beta config");

    // Verify deletion
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
    // Config should not exist
}

/// Test beta config with namespace
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_beta_config_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_ns");
    let namespace = "test-namespace";

    // Publish beta config with namespace
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("tenant", namespace),
                ("content", "ns.value=1"),
                ("betaIps", "192.168.1.101"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Get beta config with namespace
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("tenant", namespace),
                ("betaIps", "192.168.1.101"),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}

/// Test beta config with tags
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_beta_config_with_tags() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_tag");

    // Publish beta config with tags
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "tagged.value=1"),
                ("betaIps", "192.168.1.102"),
                ("tags", "canary,version-1.0"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Verify beta config published
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("betaIps", "192.168.1.102"),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
}

/// Test multiple beta IPs
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_beta_config_multiple_ips() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("beta_multi_ip");

    // Publish beta config with multiple IPs
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "multi.beta=1"),
                ("betaIps", "192.168.1.200,192.168.1.201"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Get beta config for each IP
    for ip in ["192.168.1.200", "192.168.1.201"] {
        let response: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("betaIps", ip),
                ],
            )
            .await
            .expect("Failed to get config");

        assert_eq!(response["code"], 0, "Get should succeed");
    }
}
