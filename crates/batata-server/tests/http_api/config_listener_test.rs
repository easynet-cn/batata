//! Configuration Listener API integration tests
//!
//! Tests for config change listener and notification mechanism

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_NAMESPACE, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id,
};
use std::time::Duration;

/// Create an authenticated test client for the main API server
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test add config listener
#[tokio::test]
#[ignore = "requires running server"]
async fn test_add_listener() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener");

    // Publish config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = response["data"]["md5"].as_str().unwrap_or("").to_string();

    // Add listener
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}", data_id, DEFAULT_GROUP, md5, "30000").as_str(),
            )],
        )
        .await
        .expect("Failed to add listener");

    assert_eq!(response["code"], 0, "Add listener should succeed");
}

/// Test remove config listener
#[tokio::test]
#[ignore = "requires running server"]
async fn test_remove_listener() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_remove");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Add listener
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}", data_id, DEFAULT_GROUP, "placeholder", "30000").as_str(),
            )],
        )
        .await
        .expect("Failed to add listener");

    // Remove listener
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}", data_id, DEFAULT_GROUP, "", "0").as_str(),
            )],
        )
        .await
        .expect("Failed to remove listener");

    assert_eq!(response["code"], 0, "Remove listener should succeed");
}

/// Test listener receives config change notification
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_receives_notification() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_notify");

    // Publish initial config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "version=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = response["data"]["md5"].as_str().unwrap_or("").to_string();

    // Start listener in background
    let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies().clone());
    let data_id_clone = data_id.clone();

    let listener_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}", data_id_clone, DEFAULT_GROUP, md5, "30000").as_str(),
                )],
            )
            .await
            .expect("Failed to listen");
        response
    });

    // Wait a bit then update config
    tokio::time::sleep(Duration::from_millis(300)).await;

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "version=2"),
            ],
        )
        .await
        .expect("Failed to update config");

    // Wait for listener to receive notification
    let result = tokio::time::timeout(Duration::from_secs(10), listener_handle).await;

    assert!(result.is_ok(), "Listener should receive notification");
    let response = result.unwrap().expect("Listener task failed");
    assert_eq!(response["code"], 0, "Notification should succeed");
}

/// Test multiple configs listener
#[tokio::test]
#[ignore = "requires running server"]
async fn test_multiple_configs_listener() {
    let client = authenticated_client().await;
    let data_id1 = unique_data_id("listener_multi1");
    let data_id2 = unique_data_id("listener_multi2");

    // Publish multiple configs
    for data_id in &[&data_id1, &data_id2] {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", "value"),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Listen for multiple configs
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!(
                    "{}{}{}{}%02{}{}{}{}",
                    data_id1,
                    DEFAULT_GROUP,
                    "md5_1",
                    "30000",
                    data_id2,
                    DEFAULT_GROUP,
                    "md5_2",
                    "30000"
                )
                .as_str(),
            )],
        )
        .await
        .expect("Failed to listen for multiple configs");

    assert_eq!(response["code"], 0, "Multi-config listener should succeed");
}

/// Test listener with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_ns");

    // Publish config with namespace
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("namespaceId", TEST_NAMESPACE),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = response["data"]["md5"].as_str().unwrap_or("");

    // Listen with namespace
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}", data_id, DEFAULT_GROUP, md5, "30000").as_str(),
            )],
        )
        .await
        .expect("Failed to listen with namespace");

    assert_eq!(response["code"], 0, "Namespace listener should succeed");
}

/// Test listener with custom group
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_custom_group() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_group");

    // Publish config with custom group
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "TEST_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = response["data"]["md5"].as_str().unwrap_or("");

    // Listen with custom group
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}", data_id, "TEST_GROUP", md5, "30000").as_str(),
            )],
        )
        .await
        .expect("Failed to listen with custom group");

    assert_eq!(response["code"], 0, "Custom group listener should succeed");
}

/// Test listener with MD5 mismatch
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_md5_mismatch() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_md5");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Listen with wrong MD5 (should trigger immediate notification)
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!(
                    "{}{}{}{}",
                    data_id, DEFAULT_GROUP, "wrong_md5_value", "30000"
                )
                .as_str(),
            )],
        )
        .await
        .expect("Failed to listen with wrong MD5");

    assert_eq!(response["code"], 0, "MD5 mismatch should be handled");
}

/// Test concurrent listeners on same config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_concurrent_listeners() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_concurrent");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Add multiple concurrent listeners
    let mut handles = Vec::new();
    for i in 0..3 {
        let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies().clone());
        let data_id_clone = data_id.clone();

        handles.push(tokio::spawn(async move {
            let response: serde_json::Value = client_clone
                .post_form(
                    "/nacos/v2/cs/config/listener",
                    &[(
                        "Listening-Configs",
                        format!(
                            "{}{}{}{}",
                            data_id_clone, DEFAULT_GROUP, "md5_placeholder", "30000"
                        )
                        .as_str(),
                    )],
                )
                .await
                .expect(&format!("Failed on listener {}", i));
            response
        }));
    }

    // Wait for all listeners
    for handle in handles {
        let result = handle.await.expect("Listener task failed");
        assert_eq!(result["code"], 0, "Concurrent listener should succeed");
    }
}

/// Test listener timeout
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_timeout() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_timeout");

    // Publish config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = response["data"]["md5"].as_str().unwrap_or("").to_string();

    // Listen with short timeout (no changes expected)
    let start = std::time::Instant::now();
    let response: serde_json::Value = client
        .post_form_with_timeout(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}", data_id, DEFAULT_GROUP, md5, "1000").as_str(),
            )],
            Duration::from_millis(500),
        )
        .await
        .expect("Failed to listen with timeout");

    let elapsed = start.elapsed();
    assert_eq!(response["code"], 0, "Timeout listener should succeed");
    assert!(
        elapsed > Duration::from_millis(100),
        "Should wait for timeout"
    );
}

/// Test listener reconnection
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_reconnection() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_reconnect");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Multiple listener connections (simulating reconnection)
    for i in 0..3 {
        let response: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!(
                        "{}{}{}{}",
                        data_id, DEFAULT_GROUP, "md5_placeholder", "30000"
                    )
                    .as_str(),
                )],
            )
            .await
            .expect(&format!("Failed on listener attempt {}", i));

        assert_eq!(response["code"], 0, "Reconnection listener should succeed");
    }
}

/// Test listener on deleted config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_deleted_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_deleted");

    // Publish config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = response["data"]["md5"].as_str().unwrap_or("").to_string();

    // Start listener
    let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies().clone());
    let data_id_clone = data_id.clone();

    let listener_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}", data_id_clone, DEFAULT_GROUP, md5, "30000").as_str(),
                )],
            )
            .await
            .expect("Failed to listen");
        response
    });

    // Wait a bit then delete config
    tokio::time::sleep(Duration::from_millis(300)).await;

    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", "DEFAULT_GROUP")],
        )
        .await
        .expect("Failed to delete config");

    // Wait for listener to receive deletion notification
    let result = tokio::time::timeout(Duration::from_secs(10), listener_handle).await;

    assert!(result.is_ok(), "Listener should handle deletion");
    let response = result.unwrap().expect("Listener task failed");
    assert_eq!(response["code"], 0, "Deletion notification should succeed");
}

/// Test listener with empty configs list
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_empty_configs() {
    let client = authenticated_client().await;

    // Listen with empty configs (heartbeat)
    let response: serde_json::Value = client
        .post_form("/nacos/v2/cs/config/listener", &[("Listening-Configs", "")])
        .await
        .expect("Failed to listen with empty configs");

    assert_eq!(response["code"], 0, "Empty listener should succeed");
}

/// Test listener batch operation
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_batch_operation() {
    let client = authenticated_client().await;

    // Publish batch configs
    let mut configs = Vec::new();
    for i in 1..=5 {
        let data_id = unique_data_id(&format!("batch{}", i));
        let response: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", &format!("value={}", i)),
                ],
            )
            .await
            .expect("Failed to publish config");

        let md5 = response["data"]["md5"].as_str().unwrap_or("").to_string();
        configs.push((data_id, md5));
    }

    // Build batch listener string
    let listening_configs = configs
        .iter()
        .map(|(data_id, md5)| format!("{}{}{}{}", data_id, DEFAULT_GROUP, md5, "30000"))
        .collect::<Vec<_>>()
        .join("%02");

    // Batch listen
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[("Listening-Configs", listening_configs.as_str())],
        )
        .await
        .expect("Failed to batch listen");

    assert_eq!(response["code"], 0, "Batch listener should succeed");
}

/// Test listener persists across config updates
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_persists_across_updates() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_persist");

    // Publish initial config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "version=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Update config multiple times
    for i in 2..=5 {
        let current_md5 = response["data"]["md5"].as_str().unwrap_or("").to_string();

        // Listen for change
        let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies().clone());
        let data_id_clone = data_id.clone();

        let listener_handle = tokio::spawn(async move {
            let response: serde_json::Value = client_clone
                .post_form(
                    "/nacos/v2/cs/config/listener",
                    &[(
                        "Listening-Configs",
                        format!(
                            "{}{}{}{}",
                            data_id_clone, DEFAULT_GROUP, current_md5, "30000"
                        )
                        .as_str(),
                    )],
                )
                .await
                .expect("Failed to listen");
            response
        });

        // Update config
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", &format!("version={}", i)),
                ],
            )
            .await
            .expect(&format!("Failed to update config to version {}", i));

        // Wait for listener
        let result = tokio::time::timeout(Duration::from_secs(5), listener_handle).await;
        assert!(result.is_ok(), "Listener should handle update");
        let listener_response = result.unwrap().expect("Listener failed");
        assert_eq!(
            listener_response["code"], 0,
            "Update notification should succeed"
        );
    }
}

/// Test listener with tags
#[tokio::test]
#[ignore = "requires running server"]
async fn test_listener_with_tags() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("listener_tags");

    // Publish config with tags
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
                ("tags", "env=prod,region=us-west"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Listen (may use tags for filtering)
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!(
                    "{}{}{}{}",
                    data_id, DEFAULT_GROUP, "md5_placeholder", "30000"
                )
                .as_str(),
            )],
        )
        .await
        .expect("Failed to listen with tags");

    assert_eq!(response["code"], 0, "Tags listener should succeed");
}
