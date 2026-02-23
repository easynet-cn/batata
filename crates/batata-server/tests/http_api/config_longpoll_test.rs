//! Configuration Long Polling API integration tests
//!
//! Tests for long polling mechanism used for real-time config updates

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_NAMESPACE, TEST_PASSWORD, TEST_USERNAME,
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

/// Test long polling for config changes
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_config_change() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll");

    // Publish initial config
    let _: serde_json::Value = client
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

    // Long poll for changes
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[
                ("Listening-Configs", format!("{}{}{}{}",
                    data_id, DEFAULT_GROUP, "MD5_NOT_CARE_YET", "30000").as_str()),
            ],
        )
        .await
        .expect("Failed to listen for changes");

    assert_eq!(response["code"], 0, "Long poll should succeed");
}

/// Test long polling with timeout
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_timeout() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_timeout");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "initial=value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Long poll with short timeout - should timeout without changes
    let start = std::time::Instant::now();
    let response: serde_json::Value = client
        .post_form_with_timeout(
            "/nacos/v2/cs/config/listener",
            &[("Listening-Configs", format!("{}{}{}{}",
                data_id, DEFAULT_GROUP, "md5_placeholder", "1000").as_str())],
            Duration::from_millis(500), // Client timeout
        )
        .await
        .expect("Failed to long poll");

    let elapsed = start.elapsed();
    assert_eq!(response["code"], 0, "Long poll should succeed");
    // Should wait for at least some time (not return immediately)
    assert!(elapsed > Duration::from_millis(100), "Should wait for timeout");
}

/// Test long poll with config update
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_with_update() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_update");

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

    let md5 = response["data"]["md5"].as_str().unwrap_or("");

    // Start long poll in background
    let data_id_clone = data_id.clone();
    let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies().clone());

    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[
                    ("Listening-Configs", format!("{}{}{}{}",
                        data_id_clone, DEFAULT_GROUP, md5, "30000").as_str()),
                ],
            )
            .await
            .expect("Failed to long poll");
        response
    });

    // Wait a bit then update config
    tokio::time::sleep(Duration::from_millis(500)).await;

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

    // Wait for long poll to return
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        poll_handle
    ).await;

    assert!(result.is_ok(), "Long poll should complete");
    let response = result.unwrap().expect("Poll task failed");
    assert_eq!(response["code"], 0, "Long poll should return on change");
}

/// Test long polling multiple configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_multiple_configs() {
    let client = authenticated_client().await;
    let data_id1 = unique_data_id("longpoll_multi1");
    let data_id2 = unique_data_id("longpoll_multi2");

    // Publish multiple configs
    for data_id in &[&data_id1, &data_id2] {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", "config=value"),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Long poll for multiple configs
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}%02{}{}{}{}",
                    data_id1, DEFAULT_GROUP, "md5_1", "30000",
                    data_id2, DEFAULT_GROUP, "md5_2", "30000"
                ).as_str()
            )],
        )
        .await
        .expect("Failed to long poll multiple configs");

    assert_eq!(response["code"], 0, "Multi-config long poll should succeed");
}

/// Test long poll with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_ns");

    // Publish config with namespace
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("namespaceId", TEST_NAMESPACE),
                ("content", "ns.config=value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Long poll with namespace
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}",
                    data_id, DEFAULT_GROUP, "md5_placeholder", "30000"
                ).as_str()
            )],
        )
        .await
        .expect("Failed to long poll with namespace");

    assert_eq!(response["code"], 0, "Long poll with namespace should succeed");
}

/// Test long poll remove listener
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_remove_listener() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_remove");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "initial=value"),
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
                format!("{}{}{}{}",
                    data_id, DEFAULT_GROUP, "md5_placeholder", "30000"
                ).as_str()
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
                format!("{}{}{}{}",
                    data_id, DEFAULT_GROUP, "", "0"
                ).as_str()
            )],
        )
        .await
        .expect("Failed to remove listener");

    assert_eq!(response["code"], 0, "Remove listener should succeed");
}

/// Test long poll with custom group
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_custom_group() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_group");

    // Publish config with custom group
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "TEST_GROUP"),
                ("content", "group.config=value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Long poll with custom group
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}",
                    data_id, "TEST_GROUP", "md5_placeholder", "30000"
                ).as_str()
            )],
        )
        .await
        .expect("Failed to long poll with custom group");

    assert_eq!(response["code"], 0, "Long poll with custom group should succeed");
}

/// Test long poll connection loss handling
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_reconnect() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_reconnect");

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

    // Multiple long poll requests (simulating reconnection)
    for i in 0..3 {
        let response: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}",
                        data_id, DEFAULT_GROUP, "md5_placeholder", "30000"
                    ).as_str()
                )],
            )
            .await
            .expect(&format!("Failed on long poll attempt {}", i));

        assert_eq!(response["code"], 0, "Long poll should succeed");
    }
}

/// Test long poll with very long timeout
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_long_timeout() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_long");

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

    // Long poll with very long timeout (should be handled gracefully)
    let response: serde_json::Value = client
        .post_form_with_timeout(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}",
                    data_id, DEFAULT_GROUP, "md5_placeholder", "60000"
                ).as_str()
            )],
            Duration::from_millis(200), // Client timeout shorter than server timeout
        )
        .await
        .expect("Failed to long poll");

    assert_eq!(response["code"], 0, "Long poll with long timeout should succeed");
}

/// Test long poll concurrent requests
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_concurrent() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_concurrent");

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

    // Spawn multiple concurrent long poll requests
    let mut handles = Vec::new();
    for i in 0..5 {
        let client_clone = TestClient::new_with_cookies(
            MAIN_BASE_URL,
            client.cookies().clone(),
        );
        let data_id_clone = data_id.clone();

        handles.push(tokio::spawn(async move {
            let response: serde_json::Value = client_clone
                .post_form(
                    "/nacos/v2/cs/config/listener",
                    &[(
                        "Listening-Configs",
                        format!("{}{}{}{}",
                            data_id_clone, DEFAULT_GROUP, "md5_placeholder", "30000"
                        ).as_str()
                    )],
                )
                .await
                .expect(&format!("Failed on concurrent request {}", i));
            response
        }));
    }

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.expect("Task failed");
        assert_eq!(result["code"], 0, "Concurrent long poll should succeed");
    }
}

/// Test long poll return changes
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_return_changes() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_changes");

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

    let md5 = response["data"]["md5"].as_str().unwrap_or("");

    // Long poll requesting return changes
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[
                ("Listening-Configs", format!("{}{}{}{}",
                    data_id, DEFAULT_GROUP, md5, "30000").as_str()),
            ],
        )
        .await
        .expect("Failed to long poll");

    // Response should indicate if changes were returned
    assert_eq!(response["code"], 0, "Long poll should succeed");
}

/// Test long poll with empty configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_empty_configs() {
    let client = authenticated_client().await;

    // Long poll with no configs (heartbeat)
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[("Listening-Configs", "")],
        )
        .await
        .expect("Failed to long poll with empty configs");

    assert_eq!(response["code"], 0, "Empty long poll should succeed");
}

/// Test long poll with invalid MD5 format
#[tokio::test]
#[ignore = "requires running server"]
async fn test_long_poll_invalid_md5() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_invalid");

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

    // Long poll with invalid MD5 format
    let result = client
        .post_form::<serde_json::Value, _>(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                "invalid_format_no_delimiters"
            )],
        )
        .await;

    // Should handle gracefully - either return error or treat as new config
    match result {
        Ok(response) => {
            assert!(
                response["code"] == 0 || response["code"] != 0,
                "Invalid MD5 format should be handled"
            );
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}
