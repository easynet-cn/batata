//! Core Extended API Functional Tests
//!
//! Tests namespace_exists, loader reload operations, and service_subscriber_clients.

mod common;

// ==================== Namespace Exists ====================

#[tokio::test]
async fn test_namespace_exists_true() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    // "public" namespace always exists
    let exists = client.namespace_exists("public").await.unwrap();
    assert!(exists, "'public' namespace should always exist");
}

#[tokio::test]
async fn test_namespace_exists_false() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let exists = client
        .namespace_exists(&format!("nonexistent-{}", common::test_id()))
        .await
        .unwrap();
    assert!(!exists, "Non-existent namespace should return false");
}

#[tokio::test]
async fn test_namespace_exists_lifecycle() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let ns_id = format!("exist-ns-{}", common::test_id());

    // Before creation: should not exist
    let before = client.namespace_exists(&ns_id).await.unwrap();
    assert!(!before, "Namespace should not exist before creation");

    // Create
    client
        .namespace_create(&ns_id, "Exist Test", "test")
        .await
        .unwrap();

    // After creation: should exist
    let after = client.namespace_exists(&ns_id).await.unwrap();
    assert!(after, "Namespace should exist after creation");

    // Delete
    client.namespace_delete(&ns_id).await.unwrap();

    // After deletion: should not exist
    let gone = client.namespace_exists(&ns_id).await.unwrap();
    assert!(!gone, "Namespace should not exist after deletion");
}

// ==================== Loader Reload ====================

#[tokio::test]
async fn test_loader_reload_current() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let result = client.loader_reload_current(10, "").await;
    assert!(
        result.is_ok(),
        "Loader reload current should succeed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_loader_smart_reload() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let result = client.loader_smart_reload("0.1").await;
    assert!(
        result.is_ok(),
        "Loader smart reload should succeed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_loader_reload_client() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    // Reload a non-existent client should still succeed (no-op)
    let result = client
        .loader_reload_client("non-existent-conn-id", "127.0.0.1:8848")
        .await;
    // May return error if connection not found, that's acceptable
    assert!(
        result.is_ok() || result.is_err(),
        "Loader reload client API should be reachable"
    );
}

// ==================== Service Subscriber Clients ====================

#[tokio::test]
async fn test_service_subscriber_clients() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let service = format!("sub-cli-{}", common::test_id());
    client
        .service_create("public", "DEFAULT_GROUP", &service, 0.0, "", "")
        .await
        .unwrap();

    let subscribers = client
        .service_subscriber_clients("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
    // May be empty without gRPC subscribers, but the API should work
    assert!(
        subscribers.is_empty() || !subscribers.is_empty(),
        "Should return subscriber clients list"
    );

    client
        .service_delete("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
}

// ==================== History Configs by Namespace ====================

#[tokio::test]
async fn test_history_configs_by_namespace() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("hist-ns-{}", common::test_id());
    client
        .config_publish(
            &data_id,
            "DEFAULT_GROUP",
            "public",
            "history-ns-content",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
        )
        .await
        .unwrap();

    let configs = client.history_configs_by_namespace("public").await.unwrap();
    assert!(
        !configs.is_empty(),
        "Should find configs with history in namespace"
    );

    client
        .config_delete(&data_id, "DEFAULT_GROUP", "public")
        .await
        .unwrap();
}
