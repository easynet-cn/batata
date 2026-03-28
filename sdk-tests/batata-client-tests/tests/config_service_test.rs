//! Config Service Functional Tests
//!
//! Tests BatataConfigService: CRUD, CAS, listeners, server status.
//! Requires a running Batata server.

mod common;

use std::sync::{Arc, atomic::{AtomicBool, AtomicU32, Ordering}};
use batata_client::config::listener::{ConfigChangeListener, ConfigResponse};

const TENANT: &str = "";
const GROUP: &str = "DEFAULT_GROUP";

// ==================== Config CRUD ====================

#[tokio::test]
#[ignore] // Requires running server
async fn test_publish_and_get_config() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-crud-{}", common::test_id());
    let content = "server.port=8080";

    // Publish
    let ok = svc.publish_config(&data_id, GROUP, TENANT, content).await.unwrap();
    assert!(ok);

    // Get
    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(result, content);

    // Cleanup
    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_publish_config_with_type() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-type-{}.yaml", common::test_id());
    let content = "server:\n  port: 8080";

    let ok = svc.publish_config_with_type(&data_id, GROUP, TENANT, content, "yaml").await.unwrap();
    assert!(ok);

    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(result, content);

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_remove_config() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-rm-{}", common::test_id());

    svc.publish_config(&data_id, GROUP, TENANT, "v1").await.unwrap();
    let ok = svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    assert!(ok);

    // Get should fail or return empty
    let result = svc.get_config(&data_id, GROUP, TENANT).await;
    assert!(result.is_err() || result.unwrap().is_empty());

    svc.shutdown().await;
}

// ==================== Config CAS ====================

#[tokio::test]
#[ignore]
async fn test_publish_config_cas_success() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-cas-ok-{}", common::test_id());

    // Initial publish
    svc.publish_config(&data_id, GROUP, TENANT, "v1").await.unwrap();

    // Get MD5
    let content = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    let md5 = format!("{:x}", md5::compute(content.as_bytes()));

    // CAS update with correct MD5
    let ok = svc.publish_config_cas(&data_id, GROUP, TENANT, "v2", &md5).await.unwrap();
    assert!(ok);

    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(result, "v2");

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_publish_config_cas_conflict() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-cas-fail-{}", common::test_id());

    svc.publish_config(&data_id, GROUP, TENANT, "v1").await.unwrap();

    // CAS with wrong MD5 — should fail
    let result = svc.publish_config_cas(&data_id, GROUP, TENANT, "v2", "wrong_md5").await;
    assert!(result.is_err(), "CAS with wrong MD5 should fail");

    // Content should still be v1
    let content = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(content, "v1");

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

// ==================== Config Listener ====================

#[tokio::test]
#[ignore]
async fn test_config_listener() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-listen-{}", common::test_id());
    let received = Arc::new(AtomicBool::new(false));
    let received_clone = received.clone();

    struct TestListener { received: Arc<AtomicBool> }
    impl ConfigChangeListener for TestListener {
        fn receive_config_info(&self, _info: ConfigResponse) {
            self.received.store(true, Ordering::SeqCst);
        }
    }

    // Publish initial
    svc.publish_config(&data_id, GROUP, TENANT, "initial").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Add listener
    svc.add_listener(&data_id, GROUP, TENANT, Arc::new(TestListener { received: received_clone }))
        .await.unwrap();

    // Update config — should trigger listener
    svc.publish_config(&data_id, GROUP, TENANT, "updated").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    assert!(received.load(Ordering::SeqCst), "Listener should have been notified");

    svc.remove_listener(&data_id, GROUP, TENANT).await.unwrap();
    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_get_config_and_sign_listener() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-sign-{}", common::test_id());
    let notify_count = Arc::new(AtomicU32::new(0));
    let count_clone = notify_count.clone();

    struct CountListener { count: Arc<AtomicU32> }
    impl ConfigChangeListener for CountListener {
        fn receive_config_info(&self, _info: ConfigResponse) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    svc.publish_config(&data_id, GROUP, TENANT, "v1").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Atomic get + listen
    let content = svc.get_config_and_sign_listener(
        &data_id, GROUP, TENANT,
        Arc::new(CountListener { count: count_clone }),
    ).await.unwrap();
    assert_eq!(content, "v1");

    // Update — should trigger
    svc.publish_config(&data_id, GROUP, TENANT, "v2").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    assert!(notify_count.load(Ordering::SeqCst) >= 1, "Listener should be notified at least once");

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

// ==================== Server Status ====================

#[tokio::test]
#[ignore]
async fn test_get_server_status() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let status = svc.get_server_status().await;
    assert_eq!(status, "UP");

    svc.shutdown().await;
}
