//! Naming Service Functional Tests
//!
//! Tests BatataNamingService: register, discover, select, subscribe.
//! Requires a running Batata server.

mod common;

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use batata_api::naming::model::Instance;
use batata_client::naming::listener::{EventListener, NamingEvent};

const NS: &str = "";
const GROUP: &str = "DEFAULT_GROUP";

fn test_instance(ip: &str, port: i32) -> Instance {
    Instance {
        ip: ip.to_string(),
        port,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        service_name: String::new(),
        metadata: Default::default(),
        ..Default::default()
    }
}

// ==================== Register & Discover ====================

#[tokio::test]
#[ignore]
async fn test_register_and_discover() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-reg-{}", common::test_id());
    let inst = test_instance("10.0.0.1", 8080);

    svc.register_instance(NS, GROUP, &service, inst.clone()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let instances = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert!(!instances.is_empty(), "Should find registered instance");
    assert_eq!(instances[0].ip, "10.0.0.1");
    assert_eq!(instances[0].port, 8080);

    svc.deregister_instance(NS, GROUP, &service, inst).await.unwrap();
    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_deregister_instance() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-dereg-{}", common::test_id());
    let inst = test_instance("10.0.0.2", 8080);

    svc.register_instance(NS, GROUP, &service, inst.clone()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    svc.deregister_instance(NS, GROUP, &service, inst).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let instances = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert!(instances.is_empty(), "Instance should be deregistered");

    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_batch_register() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-batch-{}", common::test_id());
    let instances: Vec<Instance> = (1..=5)
        .map(|i| test_instance(&format!("10.0.1.{}", i), 8080))
        .collect();

    svc.batch_register_instance(NS, GROUP, &service, instances).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let result = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert_eq!(result.len(), 5, "Should have 5 registered instances");

    svc.shutdown().await;
}

// ==================== Select Instances ====================

#[tokio::test]
#[ignore]
async fn test_select_instances_healthy() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-sel-h-{}", common::test_id());
    svc.register_instance(NS, GROUP, &service, test_instance("10.0.2.1", 8080)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let healthy = svc.select_instances(NS, GROUP, &service, true).await.unwrap();
    assert!(!healthy.is_empty(), "Should find healthy instances");
    assert!(healthy.iter().all(|i| i.healthy), "All should be healthy");

    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_select_one_healthy_instance() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-sel1-{}", common::test_id());
    for i in 1..=3 {
        svc.register_instance(NS, GROUP, &service, test_instance(&format!("10.0.3.{}", i), 8080))
            .await.unwrap();
    }
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let one = svc.select_one_healthy_instance(NS, GROUP, &service).await.unwrap();
    assert!(one.is_some(), "Should select one instance");
    assert!(one.unwrap().healthy, "Selected instance should be healthy");

    svc.shutdown().await;
}

// ==================== Subscribe ====================

#[tokio::test]
#[ignore]
async fn test_subscribe_and_notify() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-sub-{}", common::test_id());
    let notified = Arc::new(AtomicBool::new(false));
    let notified_clone = notified.clone();

    struct TestEventListener { notified: Arc<AtomicBool> }
    impl EventListener for TestEventListener {
        fn on_event(&self, _event: NamingEvent) {
            self.notified.store(true, Ordering::SeqCst);
        }
    }

    // Subscribe first
    svc.subscribe(NS, GROUP, &service, "",
        Arc::new(TestEventListener { notified: notified_clone }))
        .await.unwrap();

    // Register instance — should trigger notification
    svc.register_instance(NS, GROUP, &service, test_instance("10.0.4.1", 8080)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    assert!(notified.load(Ordering::SeqCst), "Subscriber should be notified");

    svc.unsubscribe(NS, GROUP, &service, "").await.unwrap();
    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_get_subscribe_services() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let svc1 = format!("svc-gsub1-{}", common::test_id());
    let svc2 = format!("svc-gsub2-{}", common::test_id());

    struct NoopListener;
    impl EventListener for NoopListener {
        fn on_event(&self, _: NamingEvent) {}
    }

    svc.subscribe(NS, GROUP, &svc1, "", Arc::new(NoopListener)).await.unwrap();
    svc.subscribe(NS, GROUP, &svc2, "", Arc::new(NoopListener)).await.unwrap();

    let subscribed = svc.get_subscribe_services();
    assert!(subscribed.len() >= 2, "Should have at least 2 subscribed services");

    svc.shutdown().await;
}

// ==================== List Services ====================

#[tokio::test]
#[ignore]
async fn test_list_services() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-list-{}", common::test_id());
    svc.register_instance(NS, GROUP, &service, test_instance("10.0.5.1", 8080)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let (count, _services) = svc.list_services(NS, GROUP, 1, 10).await.unwrap();
    assert!(count > 0, "Should find at least one service");

    svc.shutdown().await;
}

// ==================== Server Status ====================

#[tokio::test]
#[ignore]
async fn test_naming_server_status() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let status = svc.get_server_status().await;
    assert_eq!(status, "UP");

    svc.shutdown().await;
}
