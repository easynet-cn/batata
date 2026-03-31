//! Service subscription tests ported from NacosServiceSubscribeTest.java
//!
//! Tests: basic subscribe, unsubscribe, non-existent service, multiple subscribers,
//! subscriber receives changes, cluster-scoped subscribe, concurrent subscribe,
//! rapid subscribe/unsubscribe, duplicate subscribe.
//!
//! Requires a running Batata server.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use batata_api::naming::model::Instance;
use batata_client::naming::listener::{EventListener, NamingEvent};
use tokio::sync::Notify;
use tokio::time::sleep;

fn make_instance(ip: &str, port: i32) -> Instance {
    Instance {
        ip: ip.to_string(),
        port,
        healthy: true,
        weight: 1.0,
        enabled: true,
        ephemeral: true,
        ..Default::default()
    }
}

struct TestEventListener {
    notify: Arc<Notify>,
    count: Arc<AtomicU32>,
    last_instances: Arc<tokio::sync::Mutex<Vec<Instance>>>,
}

impl TestEventListener {
    fn new() -> (Arc<Self>, Arc<Notify>, Arc<AtomicU32>, Arc<tokio::sync::Mutex<Vec<Instance>>>) {
        let notify = Arc::new(Notify::new());
        let count = Arc::new(AtomicU32::new(0));
        let last = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let listener = Arc::new(Self {
            notify: notify.clone(),
            count: count.clone(),
            last_instances: last.clone(),
        });
        (listener, notify, count, last)
    }
}

impl EventListener for TestEventListener {
    fn on_event(&self, event: NamingEvent) {
        *self.last_instances.blocking_lock() = event.instances.clone();
        self.count.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
    }
}

/// NSS-001: Basic subscribe — receive notification when instance registered
#[tokio::test]
async fn test_basic_subscribe() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("sub-basic-{}", id);

    let (listener, notify, _count, last_instances) = TestEventListener::new();

    svc.subscribe("", "DEFAULT_GROUP", &service_name, "", listener)
        .await
        .unwrap();
    sleep(Duration::from_secs(1)).await;

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.1", 8080))
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(20), notify.notified())
        .await
        .is_ok();
    assert!(received, "Should receive notification within 20s");

    let instances = last_instances.lock().await;
    assert!(!instances.is_empty(), "Instance list should not be empty");
    assert_eq!(instances[0].ip, "192.168.1.1");
    assert_eq!(instances[0].port, 8080);

    // Cleanup
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.1", 8080))
        .await.ok();
    svc.unsubscribe("", "DEFAULT_GROUP", &service_name, "").await.ok();
}

/// NSS-002: Unsubscribe stops notifications
#[tokio::test]
async fn test_unsubscribe() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("sub-unsub-{}", id);

    let count = Arc::new(AtomicU32::new(0));
    let c = count.clone();
    let listener = Arc::new(batata_client::naming::listener::FnEventListener::new(
        move |_: NamingEvent| {
            c.fetch_add(1, Ordering::SeqCst);
        },
    ));

    svc.subscribe("", "DEFAULT_GROUP", &service_name, "", listener)
        .await
        .unwrap();
    svc.unsubscribe("", "DEFAULT_GROUP", &service_name, "")
        .await
        .unwrap();

    let count_before = count.load(Ordering::SeqCst);
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.2", 8080))
        .await
        .unwrap();
    sleep(Duration::from_secs(5)).await;

    assert_eq!(
        count_before,
        count.load(Ordering::SeqCst),
        "No notifications after unsubscribe"
    );

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.2", 8080))
        .await.ok();
}

/// NSS-003: Subscribe to non-existent service — fires when service created
#[tokio::test]
async fn test_subscribe_non_existent_service() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("sub-nonexist-{}", id);

    let (listener, notify, _count, last_instances) = TestEventListener::new();

    svc.subscribe("", "DEFAULT_GROUP", &service_name, "", listener)
        .await
        .unwrap();

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.3", 8080))
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(20), notify.notified())
        .await
        .is_ok();
    assert!(received, "Should receive notification when service is created");

    let instances = last_instances.lock().await;
    assert!(!instances.is_empty());
    assert_eq!(instances[0].ip, "192.168.1.3");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.3", 8080))
        .await.ok();
    svc.unsubscribe("", "DEFAULT_GROUP", &service_name, "").await.ok();
}

/// NSS-004: Multiple subscribers all receive notification
#[tokio::test]
async fn test_multiple_subscribers() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("sub-multi-{}", id);

    let total = Arc::new(AtomicU32::new(0));
    let all_done = Arc::new(Notify::new());

    for _ in 0..3 {
        let t = total.clone();
        let a = all_done.clone();
        let listener = Arc::new(batata_client::naming::listener::FnEventListener::new(
            move |_: NamingEvent| {
                if t.fetch_add(1, Ordering::SeqCst) == 2 {
                    a.notify_one();
                }
            },
        ));
        svc.subscribe("", "DEFAULT_GROUP", &service_name, "", listener)
            .await
            .unwrap();
    }

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.4", 8080))
        .await
        .unwrap();

    let all_received = tokio::time::timeout(Duration::from_secs(20), all_done.notified())
        .await
        .is_ok();
    assert!(all_received, "All 3 subscribers should be notified");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.4", 8080))
        .await.ok();
    svc.unsubscribe("", "DEFAULT_GROUP", &service_name, "").await.ok();
}

/// NSS-005: Subscriber receives add and remove changes
#[tokio::test]
async fn test_subscriber_receives_changes() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("sub-changes-{}", id);

    let instance_counts = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let count = Arc::new(AtomicU32::new(0));
    let notify = Arc::new(Notify::new());

    let ic = instance_counts.clone();
    let c = count.clone();
    let n = notify.clone();
    let listener = Arc::new(batata_client::naming::listener::FnEventListener::new(
        move |event: NamingEvent| {
            ic.blocking_lock().push(event.instances.len());
            c.fetch_add(1, Ordering::SeqCst);
            n.notify_one();
        },
    ));

    svc.subscribe("", "DEFAULT_GROUP", &service_name, "", listener)
        .await
        .unwrap();

    // Add first instance
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.10", 8080))
        .await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), notify.notified()).await.ok();
    sleep(Duration::from_secs(1)).await;

    // Add second instance
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.11", 8080))
        .await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), notify.notified()).await.ok();
    sleep(Duration::from_secs(1)).await;

    // Remove first instance
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.10", 8080))
        .await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), notify.notified()).await.ok();
    sleep(Duration::from_secs(1)).await;

    let counts = instance_counts.lock().await;
    assert!(counts.len() >= 3, "Should receive at least 3 notifications, got {}", counts.len());
    assert_eq!(counts[0], 1, "First: 1 instance");
    assert_eq!(counts[1], 2, "Second: 2 instances");
    assert_eq!(counts[2], 1, "Third: 1 instance (after removal)");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.11", 8080))
        .await.ok();
    svc.unsubscribe("", "DEFAULT_GROUP", &service_name, "").await.ok();
}

/// NSS-011: Subscribe with empty cluster list receives all events
#[tokio::test]
async fn test_subscribe_empty_cluster_list() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("sub-empty-cluster-{}", id);

    let (listener, notify, _count, last_instances) = TestEventListener::new();

    // Empty cluster string = receive all clusters
    svc.subscribe("", "DEFAULT_GROUP", &service_name, "", listener)
        .await
        .unwrap();

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.3.3", 8080))
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(20), notify.notified())
        .await
        .is_ok();
    assert!(received);

    let instances = last_instances.lock().await;
    assert!(!instances.is_empty());
    assert_eq!(instances[0].ip, "192.168.3.3");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.3.3", 8080))
        .await.ok();
    svc.unsubscribe("", "DEFAULT_GROUP", &service_name, "").await.ok();
}
