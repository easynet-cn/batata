//! Core Admin API Functional Tests
//!
//! Tests server state, liveness, readiness, namespace CRUD, cluster info.

mod common;

// ==================== Server Health ====================

#[tokio::test]
#[ignore]
async fn test_server_state() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let state = client.server_state().await.unwrap();
    assert!(state.is_object(), "Server state should be a JSON object");
}

#[tokio::test]
#[ignore]
async fn test_liveness() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let result = client.liveness().await;
    assert!(result.is_ok(), "Liveness should succeed");
}

#[tokio::test]
#[ignore]
async fn test_readiness() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let result = client.readiness().await;
    assert!(result.is_ok(), "Readiness should succeed");
}

// ==================== Namespace CRUD ====================

#[tokio::test]
#[ignore]
async fn test_namespace_lifecycle() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let ns_id = format!("test-ns-{}", common::test_id());
    let ns_name = format!("Test NS {}", &ns_id[..4]);

    // Create
    let created = client.namespace_create(&ns_id, &ns_name, "test desc").await.unwrap();
    assert!(created, "Namespace should be created");

    // Get
    let ns = client.namespace_get(&ns_id).await.unwrap();
    assert_eq!(ns.namespace, ns_id);
    assert_eq!(ns.namespace_show_name, ns_name);

    // Exists
    let exists = client.namespace_exists(&ns_id).await.unwrap();
    assert!(exists, "Namespace should exist");

    // Update
    let updated = client
        .namespace_update(&ns_id, "Updated Name", "updated desc")
        .await
        .unwrap();
    assert!(updated);

    // List
    let list = client.namespace_list().await.unwrap();
    assert!(list.iter().any(|n| n.namespace == ns_id), "Should find our namespace");

    // Delete
    let deleted = client.namespace_delete(&ns_id).await.unwrap();
    assert!(deleted);

    let exists_after = client.namespace_exists(&ns_id).await.unwrap();
    assert!(!exists_after, "Namespace should be deleted");
}

// ==================== Cluster ====================

#[tokio::test]
#[ignore]
async fn test_cluster_members() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let members = client.cluster_members().await.unwrap();
    assert!(!members.is_empty(), "Should have at least one cluster member");
}

#[tokio::test]
#[ignore]
async fn test_cluster_self() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let self_info = client.cluster_self().await.unwrap();
    assert!(!self_info.member.address.is_empty(), "Self member should have address");
}

// ==================== Log Level ====================

#[tokio::test]
#[ignore]
async fn test_update_log_level() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let result = client.update_log_level("ROOT", "INFO").await;
    assert!(result.is_ok(), "Update log level should succeed");
}
