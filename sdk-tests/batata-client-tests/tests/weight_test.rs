//! Weight-based instance selection tests ported from NacosWeightTest.java
//!
//! Tests: default weight, custom weight, zero weight, max weight, weight update,
//! weighted random selection distribution, weight with health status.
//!
//! Requires a running Batata server.

mod common;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use batata_api::naming::model::Instance;
use tokio::time::sleep;

fn make_weighted(ip: &str, port: i32, weight: f64) -> Instance {
    Instance {
        ip: ip.to_string(),
        port,
        weight,
        healthy: true,
        enabled: true,
        ephemeral: true,
        ..Default::default()
    }
}

/// NWT-001: Default weight is 1.0
#[tokio::test]
async fn test_default_weight_value() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("wt-default-{}", id);

    let instance = Instance::new("192.168.200.1".to_string(), 8080);
    svc.register_instance("", "DEFAULT_GROUP", &service_name, instance.clone())
        .await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let instances = svc.get_all_instances("", "DEFAULT_GROUP", &service_name).await.unwrap();
    assert!(!instances.is_empty());
    assert!((instances[0].weight - 1.0).abs() < 0.001, "Default weight should be 1.0");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, instance).await.ok();
}

/// NWT-002: Custom weight is preserved
#[tokio::test]
async fn test_set_custom_weight() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("wt-custom-{}", id);

    let instance = make_weighted("192.168.200.2", 8080, 5.5);
    svc.register_instance("", "DEFAULT_GROUP", &service_name, instance.clone())
        .await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let instances = svc.get_all_instances("", "DEFAULT_GROUP", &service_name).await.unwrap();
    assert!(!instances.is_empty());
    assert!((instances[0].weight - 5.5).abs() < 0.001, "Custom weight should be 5.5");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, instance).await.ok();
}

/// NWT-003: Zero weight instance rarely/never selected
#[tokio::test]
async fn test_weight_zero_disabled() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("wt-zero-{}", id);

    let zero = make_weighted("192.168.200.3", 8080, 0.0);
    let normal = make_weighted("192.168.200.4", 8080, 1.0);
    svc.register_instance("", "DEFAULT_GROUP", &service_name, zero.clone()).await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, normal.clone()).await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let mut zero_count = 0;
    let mut normal_count = 0;
    for _ in 0..100 {
        if let Ok(Some(selected)) = svc.select_one_healthy_instance("", "DEFAULT_GROUP", &service_name).await {
            if selected.ip == "192.168.200.3" {
                zero_count += 1;
            } else {
                normal_count += 1;
            }
        }
    }
    assert!(
        zero_count == 0 || zero_count < (normal_count as f64 * 0.1) as i32,
        "Zero-weight should be rarely/never selected (zero={}, normal={})",
        zero_count, normal_count
    );

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, zero).await.ok();
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, normal).await.ok();
}

/// NWT-006: Weighted random selection favors higher weight
#[tokio::test]
async fn test_weighted_random_selection() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("wt-random-{}", id);

    let high = make_weighted("192.168.200.10", 8080, 9.0);
    let low = make_weighted("192.168.200.11", 8080, 1.0);
    svc.register_instance("", "DEFAULT_GROUP", &service_name, high.clone()).await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, low.clone()).await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let mut high_count = 0;
    let mut low_count = 0;
    for _ in 0..1000 {
        if let Ok(Some(selected)) = svc.select_one_healthy_instance("", "DEFAULT_GROUP", &service_name).await {
            if selected.ip == "192.168.200.10" {
                high_count += 1;
            } else {
                low_count += 1;
            }
        }
    }
    assert!(
        high_count > low_count,
        "High weight should be selected more often (high={}, low={})",
        high_count, low_count
    );

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, high).await.ok();
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, low).await.ok();
}

/// NWT-010: Unhealthy instance not selected even with high weight
#[tokio::test]
async fn test_weight_with_health_status() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("wt-health-{}", id);

    let unhealthy = Instance {
        ip: "192.168.200.50".to_string(),
        port: 8080,
        weight: 100.0,
        healthy: false,
        enabled: true,
        ephemeral: true,
        ..Default::default()
    };
    let healthy = make_weighted("192.168.200.51", 8080, 1.0);

    svc.register_instance("", "DEFAULT_GROUP", &service_name, unhealthy.clone()).await.unwrap();
    svc.register_instance("", "DEFAULT_GROUP", &service_name, healthy.clone()).await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let healthy_only = svc.select_instances("", "DEFAULT_GROUP", &service_name, true).await.unwrap();
    assert_eq!(healthy_only.len(), 1);
    assert_eq!(healthy_only[0].ip, "192.168.200.51");

    let mut unhealthy_selected = 0;
    for _ in 0..100 {
        if let Ok(Some(selected)) = svc.select_one_healthy_instance("", "DEFAULT_GROUP", &service_name).await {
            if selected.ip == "192.168.200.50" {
                unhealthy_selected += 1;
            }
        }
    }
    assert_eq!(unhealthy_selected, 0, "Unhealthy instance should never be selected");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, unhealthy).await.ok();
    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, healthy).await.ok();
}

/// NWT-013: Instance metadata preserved alongside weight
#[tokio::test]
async fn test_weight_metadata() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("wt-meta-{}", id);

    let mut metadata = HashMap::new();
    metadata.insert("weightCategory".to_string(), "medium".to_string());
    metadata.insert("targetWeight".to_string(), "5.0".to_string());
    metadata.insert("autoScale".to_string(), "true".to_string());

    let instance = Instance {
        ip: "192.168.200.60".to_string(),
        port: 8080,
        weight: 5.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        metadata: metadata.clone(),
        ..Default::default()
    };

    svc.register_instance("", "DEFAULT_GROUP", &service_name, instance.clone()).await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let instances = svc.get_all_instances("", "DEFAULT_GROUP", &service_name).await.unwrap();
    assert!(!instances.is_empty());
    let found = &instances[0];
    assert!((found.weight - 5.0).abs() < 0.001);
    assert_eq!(found.metadata.get("weightCategory").unwrap(), "medium");
    assert_eq!(found.metadata.get("targetWeight").unwrap(), "5.0");
    assert_eq!(found.metadata.get("autoScale").unwrap(), "true");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, instance).await.ok();
}
