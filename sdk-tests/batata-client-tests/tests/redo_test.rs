//! Redo service state machine tests (no server required).
//!
//! These tests verify the InstanceRedoData and SubscriberRedoData state
//! machines, and the NamingGrpcRedoService behavior matching the Nacos
//! Java `NamingGrpcRedoService`.

use batata_api::naming::model::Instance;
use batata_client::naming::redo::{InstanceRedoData, NamingGrpcRedoService, SubscriberRedoData};

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

// --- InstanceRedoData state machine tests ---

#[test]
fn test_instance_redo_data_initial_needs_redo() {
    let data = InstanceRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        make_instance("10.0.0.1", 8080),
    );
    // Just created: not registered, expected registered → needs redo
    assert!(data.is_need_redo());
    assert!(!data.is_registered());
    assert!(!data.is_need_remove());
}

#[test]
fn test_instance_redo_data_after_successful_registration() {
    let data = InstanceRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        make_instance("10.0.0.1", 8080),
    );
    data.set_registered(true);

    assert!(!data.is_need_redo());
    assert!(data.is_registered());
    assert!(!data.is_need_remove());
}

#[test]
fn test_instance_redo_data_disconnect_marks_for_redo() {
    let data = InstanceRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        make_instance("10.0.0.1", 8080),
    );
    data.set_registered(true);
    assert!(!data.is_need_redo());

    // Simulate disconnect
    data.set_registered(false);
    assert!(data.is_need_redo());
}

#[test]
fn test_instance_redo_data_deregister_marks_for_removal() {
    let data = InstanceRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        make_instance("10.0.0.1", 8080),
    );
    data.set_registered(true);

    // Initiate deregistration
    data.set_unregistering();
    assert!(data.is_need_remove());
    assert!(!data.is_need_redo()); // Should NOT re-register
}

#[test]
fn test_instance_redo_data_not_expected_registered() {
    let data = InstanceRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        make_instance("10.0.0.1", 8080),
    );
    data.set_expected_registered(false);
    assert!(!data.is_need_redo()); // Not expected → no redo
}

// --- SubscriberRedoData state machine tests ---

#[test]
fn test_subscriber_redo_data_initial_needs_redo() {
    let data = SubscriberRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        "".into(),
    );
    assert!(data.is_need_redo());
    assert!(!data.is_registered());
}

#[test]
fn test_subscriber_redo_data_after_registration() {
    let data = SubscriberRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        "CLUSTER_A".into(),
    );
    data.set_registered(true);
    assert!(!data.is_need_redo());
    assert!(data.is_registered());
}

#[test]
fn test_subscriber_redo_data_disconnect() {
    let data = SubscriberRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        "".into(),
    );
    data.set_registered(true);
    assert!(!data.is_need_redo());

    data.set_registered(false);
    assert!(data.is_need_redo());
}

#[test]
fn test_subscriber_not_expected() {
    let data = SubscriberRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc".into(),
        "".into(),
    );
    data.set_expected_registered(false);
    assert!(!data.is_need_redo());
}

// --- NamingGrpcRedoService tests ---

#[test]
fn test_redo_service_empty() {
    let service = NamingGrpcRedoService::new();
    assert!(!service.is_connected());
    assert_eq!(service.instance_count(), 0);
    assert_eq!(service.subscriber_count(), 0);
    assert!(service.find_instance_redo_data().is_empty());
    assert!(service.find_subscriber_redo_data().is_empty());
}

#[test]
fn test_redo_service_instance_lifecycle() {
    let service = NamingGrpcRedoService::new();

    // Register
    let data = InstanceRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc-a".into(),
        make_instance("10.0.0.1", 8080),
    );
    service.cache_instance_for_redo("key1".into(), data);
    assert_eq!(service.instance_count(), 1);
    assert_eq!(service.find_instance_redo_data().len(), 1); // needs redo initially

    // Mark registered
    service.instance_registered("key1");
    assert!(service.find_instance_redo_data().is_empty()); // no redo needed

    // Deregister
    service.instance_deregister("key1");
    assert_eq!(service.find_instance_remove_data().len(), 1);

    // Remove
    service.remove_instance_for_redo("key1");
    assert_eq!(service.instance_count(), 0);
}

#[test]
fn test_redo_service_subscriber_lifecycle() {
    let service = NamingGrpcRedoService::new();

    let data = SubscriberRedoData::new(
        "public".into(),
        "DEFAULT_GROUP".into(),
        "svc-b".into(),
        "CLUSTER_A".into(),
    );
    service.cache_subscriber_for_redo("sub1".into(), data);
    assert_eq!(service.subscriber_count(), 1);
    assert_eq!(service.find_subscriber_redo_data().len(), 1);

    service.subscriber_registered("sub1");
    assert!(service.find_subscriber_redo_data().is_empty());

    service.remove_subscriber_for_redo("sub1");
    assert_eq!(service.subscriber_count(), 0);
}

#[test]
fn test_redo_service_disconnect_marks_all_for_redo() {
    let service = NamingGrpcRedoService::new();

    // Add and register multiple items
    for i in 0..5 {
        let data = InstanceRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            format!("svc-{}", i),
            make_instance("10.0.0.1", 8080 + i),
        );
        let key = format!("inst-{}", i);
        service.cache_instance_for_redo(key.clone(), data);
        service.instance_registered(&key);
    }

    for i in 0..3 {
        let data = SubscriberRedoData::new(
            "public".into(),
            "DEFAULT_GROUP".into(),
            format!("svc-{}", i),
            "".into(),
        );
        let key = format!("sub-{}", i);
        service.cache_subscriber_for_redo(key.clone(), data);
        service.subscriber_registered(&key);
    }

    // Everything registered — nothing needs redo
    assert!(service.find_instance_redo_data().is_empty());
    assert!(service.find_subscriber_redo_data().is_empty());

    // Disconnect
    service.on_disconnected();
    assert!(!service.is_connected());

    // ALL items now need redo
    assert_eq!(service.find_instance_redo_data().len(), 5);
    assert_eq!(service.find_subscriber_redo_data().len(), 3);
}

#[test]
fn test_redo_service_connect_disconnect_cycle() {
    let service = NamingGrpcRedoService::new();

    let data = InstanceRedoData::new(
        "ns".into(),
        "g".into(),
        "s".into(),
        make_instance("1.1.1.1", 80),
    );
    service.cache_instance_for_redo("k".into(), data);
    service.instance_registered("k");

    // Connect
    service.on_connected();
    assert!(service.is_connected());

    // Disconnect
    service.on_disconnected();
    assert!(!service.is_connected());
    assert_eq!(service.find_instance_redo_data().len(), 1);

    // Re-connect
    service.on_connected();
    assert!(service.is_connected());
    // Items still need redo (on_connected doesn't auto-register)
    assert_eq!(service.find_instance_redo_data().len(), 1);
}

#[test]
fn test_redo_service_shutdown_clears_all() {
    let service = NamingGrpcRedoService::new();

    service.cache_instance_for_redo(
        "k1".into(),
        InstanceRedoData::new(
            "ns".into(),
            "g".into(),
            "s1".into(),
            make_instance("1.1.1.1", 80),
        ),
    );
    service.cache_subscriber_for_redo(
        "s1".into(),
        SubscriberRedoData::new("ns".into(), "g".into(), "s1".into(), "".into()),
    );

    assert_eq!(service.instance_count(), 1);
    assert_eq!(service.subscriber_count(), 1);

    service.shutdown();
    assert!(service.is_shutdown());
    assert_eq!(service.instance_count(), 0);
    assert_eq!(service.subscriber_count(), 0);
}

#[test]
fn test_redo_service_find_mixed_states() {
    let service = NamingGrpcRedoService::new();

    // Instance 1: registered (no redo)
    let d1 = InstanceRedoData::new(
        "ns".into(),
        "g".into(),
        "s1".into(),
        make_instance("1.1.1.1", 80),
    );
    service.cache_instance_for_redo("k1".into(), d1);
    service.instance_registered("k1");

    // Instance 2: not registered (needs redo)
    let d2 = InstanceRedoData::new(
        "ns".into(),
        "g".into(),
        "s2".into(),
        make_instance("2.2.2.2", 80),
    );
    service.cache_instance_for_redo("k2".into(), d2);

    // Instance 3: deregistering (needs removal, not redo)
    let d3 = InstanceRedoData::new(
        "ns".into(),
        "g".into(),
        "s3".into(),
        make_instance("3.3.3.3", 80),
    );
    service.cache_instance_for_redo("k3".into(), d3);
    service.instance_registered("k3");
    service.instance_deregister("k3");

    // Only instance 2 needs redo
    let redo = service.find_instance_redo_data();
    assert_eq!(redo.len(), 1);
    assert_eq!(redo[0].instance.ip, "2.2.2.2");

    // Only instance 3 needs removal
    let remove = service.find_instance_remove_data();
    assert_eq!(remove.len(), 1);
    assert_eq!(remove[0].1.instance.ip, "3.3.3.3");
}
