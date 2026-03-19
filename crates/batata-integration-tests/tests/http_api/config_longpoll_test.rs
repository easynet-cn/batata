//! Config Long Polling API integration tests
//!
//! Tests for long polling mechanism used for real-time config updates

use batata_integration_tests::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_data_id,
};
use std::time::Duration;

/// Create an authenticated test client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test basic long polling for config changes
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_config_long_poll_basic() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_basic");

    // Publish config first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "initial.value=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let token = client.token().cloned().unwrap_or_default();

    // Start long poll task
    let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, token.clone());
    let data_id_clone = data_id.clone();

    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!(
                        "{}{}{}{}",
                        data_id_clone, DEFAULT_GROUP, "MD5_NOT_CARE_YET", "30000"
                    )
                    .as_str(),
                )],
            )
            .await
            .expect("Long poll failed");
        response
    });

    // Wait a bit then update config
    tokio::time::sleep(Duration::from_millis(500)).await;

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "updated.value=2"),
            ],
        )
        .await
        .expect("Failed to update config");

    // Wait for long poll to return
    let result = tokio::time::timeout(Duration::from_secs(35), poll_handle).await;

    assert!(result.is_ok(), "Long poll should complete");
    let response = result.unwrap().expect("Poll task failed");
    assert_eq!(response["code"], 0, "Poll should succeed");
    // The response should contain a list of changed configs
    let data = response["data"].to_string();
    assert!(
        data.contains(&data_id) || data.contains(DEFAULT_GROUP),
        "Long poll response should reference the changed config (dataId={} or group), got data: {}",
        data_id,
        data
    );
}

/// Test long poll timeout
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_config_long_poll_timeout() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_timeout");

    // Publish config and capture its MD5
    let publish_response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "timeout.test=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = publish_response["data"]["md5"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // Start long poll with short timeout and matching MD5 (no changes expected)
    let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies());
    let data_id_clone = data_id.clone();

    let start = std::time::Instant::now();
    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}", data_id_clone, DEFAULT_GROUP, md5, "2000").as_str(),
                )],
            )
            .await
            .expect("Long poll failed");
        response
    });

    // Long poll should timeout (no config change)
    let result = tokio::time::timeout(Duration::from_secs(10), poll_handle).await;

    assert!(result.is_ok(), "Long poll should complete on timeout");
    let response = result.unwrap().expect("Poll task failed");
    let elapsed = start.elapsed();

    assert_eq!(response["code"], 0, "Timeout poll should succeed");
    // Should have waited for at least part of the timeout period
    assert!(
        elapsed >= Duration::from_millis(500),
        "Long poll should wait for timeout period, but returned in {:?}",
        elapsed
    );
    // With matching MD5 and no changes, data should be empty
    if let Some(data_str) = response["data"].as_str() {
        assert!(
            data_str.is_empty(),
            "No changes expected on timeout with matching MD5, got: {}",
            data_str
        );
    } else if let Some(data_arr) = response["data"].as_array() {
        assert!(
            data_arr.is_empty(),
            "No changed configs expected on timeout, got: {:?}",
            data_arr
        );
    }
}

/// Test long poll with MD5 match
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_config_long_poll_md5_match() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_md5");

    // Publish config
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "md5.test=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let md5 = response["data"]["md5"].as_str().unwrap_or("");

    // Long poll with matching MD5 (should wait for change or timeout)
    let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies());
    let data_id_clone = data_id.clone();
    let md5_clone = md5.to_string();

    let start = std::time::Instant::now();
    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}", data_id_clone, DEFAULT_GROUP, md5_clone, "3000").as_str(),
                )],
            )
            .await
            .expect("Long poll failed");
        response
    });

    // Wait for timeout (no change)
    let result = tokio::time::timeout(Duration::from_secs(10), poll_handle).await;

    assert!(result.is_ok(), "Long poll should complete");
    let response = result.unwrap().expect("Poll task failed");
    let elapsed = start.elapsed();

    assert_eq!(response["code"], 0, "MD5 match poll should succeed");
    // With matching MD5, server should hold the connection until timeout
    assert!(
        elapsed >= Duration::from_millis(1000),
        "MD5 match should cause the server to hold the connection, but returned in {:?}",
        elapsed
    );
    // No changes should be returned when MD5 matches
    if let Some(data_str) = response["data"].as_str() {
        assert!(
            data_str.is_empty(),
            "No changes expected when MD5 matches, got: {}",
            data_str
        );
    } else if let Some(data_arr) = response["data"].as_array() {
        assert!(
            data_arr.is_empty(),
            "No changed configs expected when MD5 matches, got: {:?}",
            data_arr
        );
    }
}

/// Test concurrent long polls
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_concurrent_long_polls() {
    let client = authenticated_client().await;
    let data_id1 = unique_data_id("longpoll_concurrent1");
    let data_id2 = unique_data_id("longpoll_concurrent2");

    // Publish two configs
    for data_id in [&data_id1, &data_id2] {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("content", "concurrent.value=1"),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Start two concurrent long polls with mismatched MD5s
    let token = client.cookies();
    let data_id1_clone = data_id1.clone();
    let data_id2_clone = data_id2.clone();
    let token1 = token.clone();
    let token2 = token.clone();

    let poll1 = tokio::spawn(async move {
        let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, token1);
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}", data_id1_clone, DEFAULT_GROUP, "md5_1", "30000").as_str(),
                )],
            )
            .await
            .expect("Long poll 1 failed");
        response
    });

    let poll2 = tokio::spawn(async move {
        let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, token2);
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}", data_id2_clone, DEFAULT_GROUP, "md5_2", "30000").as_str(),
                )],
            )
            .await
            .expect("Long poll 2 failed");
        response
    });

    // Update configs to trigger long polls
    tokio::time::sleep(Duration::from_millis(500)).await;

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id1.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "updated.concurrent=2"),
            ],
        )
        .await
        .expect("Failed to update config");

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id2.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "updated.concurrent=2"),
            ],
        )
        .await
        .expect("Failed to update config");

    // Wait for both polls to complete and verify responses
    let result1 = poll1.await.expect("Poll 1 task failed");
    let result2 = poll2.await.expect("Poll 2 task failed");

    assert_eq!(result1["code"], 0, "Concurrent poll 1 should succeed");
    assert_eq!(result2["code"], 0, "Concurrent poll 2 should succeed");

    // Each poll should return valid response data
    assert!(
        result1.get("data").is_some(),
        "Poll 1 response should contain data field"
    );
    assert!(
        result2.get("data").is_some(),
        "Poll 2 response should contain data field"
    );

    // With mismatched MD5s, each poll should report changes
    let data1 = result1["data"].to_string();
    let data2 = result2["data"].to_string();
    assert!(
        data1.contains(&data_id1) || data1.contains(DEFAULT_GROUP),
        "Poll 1 should reference changed config (dataId={}), got data: {}",
        data_id1,
        data1
    );
    assert!(
        data2.contains(&data_id2) || data2.contains(DEFAULT_GROUP),
        "Poll 2 should reference changed config (dataId={}), got data: {}",
        data_id2,
        data2
    );
}

/// Test long poll removal
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_long_poll_removal() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_remove");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "remove.test=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Add listener (long poll) with mismatched MD5
    let add_response: serde_json::Value = client
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
        .expect("Failed to add listener");

    assert_eq!(add_response["code"], 0, "Add listener should succeed");
    // With mismatched MD5, the config should be reported as changed
    let add_data = add_response["data"].to_string();
    assert!(
        add_data.contains(&data_id) || add_data.contains(DEFAULT_GROUP),
        "Listener with mismatched MD5 should report changed config, got data: {}",
        add_data
    );

    // Remove listener
    let remove_response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config/listener",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to remove listener");

    // Verify removal was acknowledged
    assert!(
        remove_response.get("code").is_some(),
        "Remove response should contain a code field"
    );
}

/// Test long poll with empty MD5 (initial poll)
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_long_poll_empty_md5() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_empty_md5");

    // Start long poll with empty MD5 before config exists
    let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, client.cookies());
    let data_id_clone = data_id.clone();

    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}", data_id, DEFAULT_GROUP, "", "60000").as_str(),
                )],
            )
            .await
            .expect("Long poll failed");
        response
    });

    // Publish config after a short delay
    tokio::time::sleep(Duration::from_millis(300)).await;

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id_clone.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "empty.md5=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Long poll should return with the new config
    let result = tokio::time::timeout(Duration::from_secs(35), poll_handle).await;

    assert!(result.is_ok(), "Long poll should complete");
    let response = result.unwrap().expect("Poll task failed");
    assert_eq!(response["code"], 0, "Poll should succeed");
    // Empty MD5 means the client has no cached version, so the server should
    // report the config as changed once it is published
    let data = response["data"].to_string();
    assert!(
        data.contains(&data_id_clone) || data.contains(DEFAULT_GROUP),
        "Empty MD5 poll should return the newly published config (dataId={}), got data: {}",
        data_id_clone,
        data
    );
}
