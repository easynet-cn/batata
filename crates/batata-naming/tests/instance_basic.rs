//! Baseline in-process semantics for instance registration and lookup.
//!
//! These are the invariants every higher-level suite (AP/CP routing, distro
//! sharding, gRPC handlers) builds on: a write must be visible to the next
//! read on the same node, deregistration must remove it, and registering the
//! same target twice must not create a duplicate entry.

mod common;

use batata_api::naming::model::Instance;
use common::{GROUP, NS, endpoints, instance, persistent_instance, query, service, with_instances};

#[test]
fn register_is_immediately_visible_to_query() {
    let svc = service();
    assert!(svc.register_instance(NS, GROUP, "svc", instance("10.0.0.1", 8080)));

    let got = query(&svc, "svc", "", false);
    assert_eq!(endpoints(&got), vec!["10.0.0.1:8080"]);
    assert!(svc.service_exists(NS, GROUP, "svc"));
}

#[test]
fn deregister_removes_the_instance_and_keeps_the_service() {
    let svc = service();
    let inst = instance("10.0.0.1", 8080);
    svc.register_instance(NS, GROUP, "svc", inst.clone());
    assert!(svc.deregister_instance(NS, GROUP, "svc", &inst));

    assert!(query(&svc, "svc", "", false).is_empty());
    // Nacos keeps the (now empty) service until the empty-service reaper runs.
    assert!(svc.service_exists(NS, GROUP, "svc"));
}

#[test]
fn registering_the_same_target_twice_does_not_duplicate() {
    let svc = service();
    svc.register_instance(NS, GROUP, "svc", instance("10.0.0.1", 8080));
    svc.register_instance(NS, GROUP, "svc", instance("10.0.0.1", 8080));

    assert_eq!(svc.get_instance_count(NS, GROUP, "svc"), 1);
    assert_eq!(
        endpoints(&query(&svc, "svc", "", false)),
        vec!["10.0.0.1:8080"]
    );
}

#[test]
fn multiple_instances_are_all_returned() {
    let svc = with_instances(5);
    let got = query(&svc, "svc", "", false);

    assert_eq!(got.len(), 5);
    assert_eq!(
        endpoints(&got),
        vec![
            "10.0.0.1:8080",
            "10.0.0.2:8080",
            "10.0.0.3:8080",
            "10.0.0.4:8080",
            "10.0.0.5:8080"
        ]
    );
}

#[test]
fn cluster_defaults_are_normalized_on_register() {
    let svc = service();
    let mut anonymous = Instance::new("10.0.0.1".to_string(), 8080);
    anonymous.cluster_name = String::new();
    svc.register_instance(NS, GROUP, "svc", anonymous);

    let got = query(&svc, "svc", "", false);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].cluster_name, "DEFAULT");
}

#[test]
fn healthy_only_filters_unhealthy_instances() {
    let svc = service();
    let mut unhealthy = instance("10.0.0.2", 8080);
    unhealthy.healthy = false;
    svc.register_instance(NS, GROUP, "svc", instance("10.0.0.1", 8080));
    svc.register_instance(NS, GROUP, "svc", unhealthy);

    assert_eq!(query(&svc, "svc", "", false).len(), 2);
    assert_eq!(
        endpoints(&query(&svc, "svc", "", true)),
        vec!["10.0.0.1:8080"]
    );
}

#[test]
fn persistent_instances_share_the_same_lookup_path() {
    let svc = service();
    svc.register_instance(NS, GROUP, "svc", persistent_instance("10.0.0.1", 8080));

    let got = query(&svc, "svc", "", false);
    assert_eq!(got.len(), 1);
    assert!(!got[0].ephemeral);
}
