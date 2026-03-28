//! Config Admin API Functional Tests
//!
//! Tests config CRUD via HTTP admin API, metadata, gray, history.

mod common;

// ==================== Config CRUD ====================

#[tokio::test]
#[ignore]
async fn test_config_publish_and_get() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("adm-cfg-{}", common::test_id());
    let group = "DEFAULT_GROUP";
    let ns = "public";

    let ok = client
        .config_publish(
            &data_id, group, ns, "key=value", "", "", "", "", "", "properties", "", "",
        )
        .await
        .unwrap();
    assert!(ok);

    let config = client.config_get(&data_id, group, ns).await.unwrap();
    assert!(config.is_some(), "Config should exist after publish");
    assert_eq!(config.unwrap().content, "key=value");

    client.config_delete(&data_id, group, ns).await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_config_list_paginated() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("adm-list-{}", common::test_id());
    client
        .config_publish(&data_id, "DEFAULT_GROUP", "public", "v1", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    let page = client
        .config_list(1, 10, "public", "", "DEFAULT_GROUP", "", "", "", "")
        .await
        .unwrap();
    assert!(page.total_count > 0, "Should find configs");

    client
        .config_delete(&data_id, "DEFAULT_GROUP", "public")
        .await
        .unwrap();
}

// ==================== Config Metadata ====================

#[tokio::test]
#[ignore]
async fn test_config_update_metadata() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("adm-meta-{}", common::test_id());
    client
        .config_publish(&data_id, "DEFAULT_GROUP", "public", "v1", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    let ok = client
        .config_update_metadata(&data_id, "DEFAULT_GROUP", "public", "Updated desc", "tag1,tag2")
        .await
        .unwrap();
    assert!(ok);

    client
        .config_delete(&data_id, "DEFAULT_GROUP", "public")
        .await
        .unwrap();
}

// ==================== Config Gray ====================

#[tokio::test]
#[ignore]
async fn test_config_gray_publish_and_stop() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("adm-gray-{}", common::test_id());

    // First publish formal config
    client
        .config_publish(&data_id, "DEFAULT_GROUP", "public", "formal", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    // Publish gray (beta) with betaIps
    client
        .config_gray_publish(
            &data_id,
            "DEFAULT_GROUP",
            "public",
            "gray-content",
            "127.0.0.1",
            "",
            "",
        )
        .await
        .unwrap();

    // Query gray
    let gray = client
        .config_gray_get(&data_id, "DEFAULT_GROUP", "public")
        .await;
    assert!(gray.is_ok(), "Gray config should exist");

    // Stop gray
    client
        .config_gray_delete(&data_id, "DEFAULT_GROUP", "public")
        .await
        .unwrap();

    client
        .config_delete(&data_id, "DEFAULT_GROUP", "public")
        .await
        .unwrap();
}

// ==================== Config History ====================

#[tokio::test]
#[ignore]
async fn test_config_history_list() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("adm-hist-{}", common::test_id());
    client
        .config_publish(&data_id, "DEFAULT_GROUP", "public", "v1", "", "", "", "", "", "", "", "")
        .await
        .unwrap();
    client
        .config_publish(&data_id, "DEFAULT_GROUP", "public", "v2", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    let history = client
        .history_list(&data_id, "DEFAULT_GROUP", "public", 1, 10)
        .await
        .unwrap();
    assert!(
        history.total_count >= 1,
        "Should have at least one history entry"
    );

    client
        .config_delete(&data_id, "DEFAULT_GROUP", "public")
        .await
        .unwrap();
}

// ==================== Config Listeners ====================

#[tokio::test]
#[ignore]
async fn test_config_listeners() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    // This queries server-side listener info (may be empty without active gRPC subscribers)
    let listeners = client
        .config_listeners("test-data-id", "DEFAULT_GROUP", "public")
        .await;
    assert!(listeners.is_ok(), "Query should succeed even if empty");
}
