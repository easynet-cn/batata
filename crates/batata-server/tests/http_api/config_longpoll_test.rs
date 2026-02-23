//! Config Long Polling API integration tests
//!
//! Tests for long polling mechanism used for real-time config updates

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id,
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
                &[
                    (
                        "Listening-Configs",
                        format!("{}{}{}{}",
                            data_id_clone, DEFAULT_GROUP, "MD5_NOT_CARE_YET", "30000"
                        ).as_str()
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
    let result = tokio::time::timeout(
        Duration::from_secs(35),
        poll_handle
    ).await;

    assert!(result.is_ok(), "Long poll should complete");
    let response = result.unwrap().expect("Poll task failed");
    assert_eq!(response["code"], 0, "Poll should succeed");
}

/// Test long poll timeout
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_config_long_poll_timeout() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_timeout");

    // Publish config
    let _: serde_json::Value = client
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

    // Start long poll with short timeout
    let client_clone = TestClient::new_with_cookies(
        MAIN_BASE_URL,
        client.cookies()
    );
    let data_id_clone = data_id.clone();

    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}",
                        data_id_clone, DEFAULT_GROUP, "md5_placeholder", "1000").as_str()
                )],
            )
            .await
            .expect("Long poll failed");
        response
    });

    // Long poll should timeout (no config change)
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        poll_handle
    ).await;

    assert!(result.is_ok(), "Long poll should complete on timeout");
    let response = result.unwrap().expect("Poll task failed");
    // Timeout returns with no changes
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
    let client_clone = TestClient::new_with_cookies(
        MAIN_BASE_URL,
        client.cookies()
    );
    let data_id_clone = data_id.clone();
    let md5_clone = md5.to_string();

    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}",
                        data_id_clone, DEFAULT_GROUP, md5_clone, "30000").as_str()
                )],
            )
            .await
            .expect("Long poll failed");
        response
    });

    // Wait for timeout (no change)
    let result = tokio::time::timeout(
        Duration::from_secs(35),
        poll_handle
    ).await;

    assert!(result.is_ok(), "Long poll should complete");
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

    // Start two concurrent long polls
    let token = client.cookies();
    let data_id1_clone = data_id1.clone();
    let data_id2_clone = data_id2.clone();
    let token1 = token.clone();
    let token2 = token.clone();

    let poll1 = tokio::spawn(async move {
        let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, token1);
        let _: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}",
                        data_id1_clone, DEFAULT_GROUP, "md5_1", "30000").as_str()
                )],
            )
            .await
            .expect("Long poll failed");
    });

    let poll2 = tokio::spawn(async move {
        let client_clone = TestClient::new_with_cookies(MAIN_BASE_URL, token2);
        let _: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[("Listening-Configs", format!("{}{}{}{}",
                    data_id2_clone, DEFAULT_GROUP, "md5_2", "30000").as_str())],
            )
            .await
            .expect("Long poll failed");
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

    // Wait for both polls to complete
    let _ = poll1.await.expect("Poll 1 failed");
    let _ = poll2.await.expect("Poll 2 failed");
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

    // Add listener (long poll)
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/listener",
            &[(
                "Listening-Configs",
                format!("{}{}{}{}",
                    data_id, DEFAULT_GROUP, "md5_placeholder", "30000").as_str()
                )],
        )
        .await
        .expect("Failed to add listener");

    // Remove listener
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config/listener",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
            ],
        )
        .await
        .expect("Failed to remove listener");

    // Listener should be removed successfully
}

/// Test long poll with empty MD5 (initial poll)
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_long_poll_empty_md5() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("longpoll_empty_md5");

    // Publish config after starting long poll
    let client_clone = TestClient::new_with_cookies(
        MAIN_BASE_URL,
        client.cookies()
    );
    let data_id_clone = data_id.clone();

    let poll_handle = tokio::spawn(async move {
        let response: serde_json::Value = client_clone
            .post_form(
                "/nacos/v2/cs/config/listener",
                &[(
                    "Listening-Configs",
                    format!("{}{}{}{}",
                        data_id, DEFAULT_GROUP, "", "60000").as_str()
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
    let result = tokio::time::timeout(
        Duration::from_secs(35),
        poll_handle
    ).await;

    assert!(result.is_ok(), "Long poll should complete");
    let response = result.unwrap().expect("Poll task failed");
    assert_eq!(response["code"], 0, "Poll should succeed");
}
