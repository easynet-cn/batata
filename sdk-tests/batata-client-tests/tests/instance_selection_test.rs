//! Instance selection tests ported from NacosInstanceSelectionTest.java
//!
//! Tests: select healthy/unhealthy, cluster filtering, weighted selection,
//! empty cluster returns all, multiple clusters, default cluster behavior.
//!
//! Requires a running Batata server.

mod common;

use std::collections::HashSet;
use std::sync::Arc;

use batata_api::naming::model::Instance;
use tokio::time::{Duration, sleep};

fn make_instance(ip: &str, port: i32, healthy: bool) -> Instance {
    Instance {
        ip: ip.to_string(),
        port,
        healthy,
        weight: 1.0,
        enabled: true,
        ephemeral: true,
        ..Default::default()
    }
}

fn make_instance_with_cluster(ip: &str, port: i32, cluster: &str) -> Instance {
    Instance {
        ip: ip.to_string(),
        port,
        healthy: true,
        weight: 1.0,
        enabled: true,
        ephemeral: true,
        cluster_name: cluster.to_string(),
        ..Default::default()
    }
}

fn make_weighted_instance(ip: &str, port: i32, weight: f64) -> Instance {
    Instance {
        ip: ip.to_string(),
        port,
        healthy: true,
        weight,
        enabled: true,
        ephemeral: true,
        ..Default::default()
    }
}

/// NIS-001: Select healthy instances only
#[tokio::test]
async fn test_select_instances_healthy_only() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("select-healthy-{}", id);

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.1", 8080, true))
        .await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.2", 8080, false))
        .await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let healthy = svc.select_instances("", "DEFAULT_GROUP", &service_name, true).await.unwrap();
    assert!(!healthy.is_empty());
    assert!(healthy.iter().any(|i| i.ip == "192.168.1.1"), "Healthy instance should be present");

    // Cleanup
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.1", 8080, true)).await.ok();
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance("192.168.1.2", 8080, false)).await.ok();
}

/// NIS-004: Select instances with cluster filter
#[tokio::test]
async fn test_select_instances_with_clusters() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("select-cluster-{}", id);

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.4.1", 8080, "cluster-A"))
        .await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.4.2", 8080, "cluster-B"))
        .await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let clusters = vec!["cluster-A".to_string()];
    let filtered = svc.select_instances_with_clusters("", "DEFAULT_GROUP", &service_name, &clusters, true).await.unwrap();
    assert!(!filtered.is_empty());
    for inst in &filtered {
        if !inst.cluster_name.is_empty() {
            assert_eq!(inst.cluster_name, "cluster-A");
        }
    }

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.4.1", 8080, "cluster-A")).await.ok();
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.4.2", 8080, "cluster-B")).await.ok();
}

/// NIS-007: Select one healthy instance
#[tokio::test]
async fn test_select_one_healthy_instance() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("select-one-{}", id);

    for i in 1..=3 {
        svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance(&format!("192.168.7.{}", i), 8080, true))
            .await.unwrap();
    }
    sleep(Duration::from_secs(1)).await;

    let selected = svc.select_one_healthy_instance("", "DEFAULT_GROUP", &service_name).await.unwrap();
    assert!(selected.is_some(), "Should select one healthy instance");
    assert!(selected.unwrap().healthy);

    for i in 1..=3 {
        svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance(&format!("192.168.7.{}", i), 8080, true)).await.ok();
    }
}

/// NIS-011: Weighted selection favors higher weight
#[tokio::test]
async fn test_weighted_selection() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("select-weight-{}", id);

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_weighted_instance("192.168.11.1", 8080, 10.0))
        .await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_weighted_instance("192.168.11.2", 8080, 1.0))
        .await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let mut high_count = 0;
    let mut low_count = 0;
    for _ in 0..100 {
        let selected = svc.select_one_healthy_instance("", "DEFAULT_GROUP", &service_name).await.unwrap();
        assert!(selected.is_some());
        if selected.unwrap().ip == "192.168.11.1" {
            high_count += 1;
        } else {
            low_count += 1;
        }
    }
    assert!(high_count > low_count, "Weight=10 should be selected more than weight=1 (high={}, low={})", high_count, low_count);

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_weighted_instance("192.168.11.1", 8080, 10.0)).await.ok();
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_weighted_instance("192.168.11.2", 8080, 1.0)).await.ok();
}

/// NIS-014: Empty cluster list returns all instances
#[tokio::test]
async fn test_empty_cluster_returns_all() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("select-empty-cluster-{}", id);

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.14.1", 8080, "cluster-X"))
        .await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.14.2", 8080, "cluster-Y"))
        .await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.14.3", 8080, "cluster-Z"))
        .await.unwrap();
    sleep(Duration::from_secs(2)).await;

    // Empty clusters = all instances
    let all = svc.select_instances_full("", "DEFAULT_GROUP", &service_name, &[], true, false).await.unwrap();
    assert!(all.len() >= 3, "Empty cluster should return all instances, got {}", all.len());

    let ips: HashSet<String> = all.iter().map(|i| i.ip.clone()).collect();
    assert!(ips.contains("192.168.14.1"));
    assert!(ips.contains("192.168.14.2"));
    assert!(ips.contains("192.168.14.3"));

    for i in 1..=3 {
        svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster(&format!("192.168.14.{}", i), 8080, &format!("cluster-{}", ["X","Y","Z"][i-1]))).await.ok();
    }
}

/// NIS-015: Multiple clusters filter returns union
#[tokio::test]
async fn test_select_multiple_clusters() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("select-multi-cluster-{}", id);

    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.15.1", 8080, "alpha"))
        .await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.15.2", 8080, "beta"))
        .await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster("192.168.15.3", 8080, "gamma"))
        .await.unwrap();
    sleep(Duration::from_secs(2)).await;

    let clusters = vec!["alpha".to_string(), "gamma".to_string()];
    let filtered = svc.select_instances_full("", "DEFAULT_GROUP", &service_name, &clusters, true, false).await.unwrap();

    assert_eq!(filtered.len(), 2, "Should get alpha + gamma only");
    let ips: HashSet<String> = filtered.iter().map(|i| i.ip.clone()).collect();
    assert!(ips.contains("192.168.15.1"), "alpha should be present");
    assert!(ips.contains("192.168.15.3"), "gamma should be present");
    assert!(!ips.contains("192.168.15.2"), "beta should NOT be present");

    for (i, c) in [("192.168.15.1","alpha"),("192.168.15.2","beta"),("192.168.15.3","gamma")] {
        svc.deregister_instance("", "DEFAULT_GROUP", &service_name, make_instance_with_cluster(i, 8080, c)).await.ok();
    }
}
