//! Advanced Naming Service Functional Tests
//!
//! Tests advanced BatataNamingService scenarios: batch deregister, metadata,
//! weights, clusters, ephemeral flag, multiple subscriptions, concurrency,
//! multi-tenant, and pagination.
//! Requires a running Batata server.

mod common;

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

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

fn test_instance_with_cluster(ip: &str, port: i32, cluster: &str) -> Instance {
    Instance {
        cluster_name: cluster.to_string(),
        ..test_instance(ip, port)
    }
}

fn test_instance_with_weight(ip: &str, port: i32, weight: f64) -> Instance {
    Instance {
        weight,
        ..test_instance(ip, port)
    }
}

fn test_instance_with_metadata(ip: &str, port: i32, metadata: HashMap<String, String>) -> Instance {
    Instance {
        metadata,
        ..test_instance(ip, port)
    }
}

// ==================== Batch Deregister ====================

#[tokio::test]
async fn test_batch_deregister() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-batchdereg-{}", common::test_id());
    let instances: Vec<Instance> = (1..=3)
        .map(|i| test_instance(&format!("10.1.0.{}", i), 8080))
        .collect();

    // Register 3 instances
    svc.batch_register_instance(NS, GROUP, &service, instances.clone())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let result = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert_eq!(result.len(), 3, "Should have 3 registered instances before deregister");

    // Batch deregister all 3
    svc.batch_deregister_instance(NS, GROUP, &service, instances)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let result = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert!(
        result.is_empty(),
        "All instances should be deregistered, but found {}",
        result.len()
    );

    svc.shutdown().await;
}

// ==================== Instance Metadata ====================

#[tokio::test]
async fn test_instance_metadata() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-meta-{}", common::test_id());
    let mut metadata = HashMap::new();
    metadata.insert("env".to_string(), "production".to_string());
    metadata.insert("version".to_string(), "2.1.0".to_string());
    metadata.insert("region".to_string(), "us-east-1".to_string());

    let inst = test_instance_with_metadata("10.2.0.1", 9090, metadata.clone());
    svc.register_instance(NS, GROUP, &service, inst.clone())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let instances = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert_eq!(instances.len(), 1, "Should find one registered instance");

    let discovered = &instances[0];
    assert_eq!(discovered.ip, "10.2.0.1");
    assert_eq!(discovered.port, 9090);
    assert_eq!(
        discovered.metadata.get("env"),
        Some(&"production".to_string()),
        "Metadata 'env' should be preserved"
    );
    assert_eq!(
        discovered.metadata.get("version"),
        Some(&"2.1.0".to_string()),
        "Metadata 'version' should be preserved"
    );
    assert_eq!(
        discovered.metadata.get("region"),
        Some(&"us-east-1".to_string()),
        "Metadata 'region' should be preserved"
    );

    // Cleanup
    svc.deregister_instance(NS, GROUP, &service, inst)
        .await
        .unwrap();
    svc.shutdown().await;
}

// ==================== Instance Weight ====================

#[tokio::test]
async fn test_instance_weight() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-weight-{}", common::test_id());

    let inst_light = test_instance_with_weight("10.3.0.1", 8080, 0.5);
    let inst_heavy = test_instance_with_weight("10.3.0.2", 8080, 5.0);

    svc.register_instance(NS, GROUP, &service, inst_light.clone())
        .await
        .unwrap();
    svc.register_instance(NS, GROUP, &service, inst_heavy.clone())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let instances = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert_eq!(instances.len(), 2, "Should have 2 registered instances");

    // Find each instance by IP and verify its weight
    let light = instances.iter().find(|i| i.ip == "10.3.0.1");
    let heavy = instances.iter().find(|i| i.ip == "10.3.0.2");

    assert!(light.is_some(), "Should find the light-weight instance");
    assert!(heavy.is_some(), "Should find the heavy-weight instance");

    let light = light.unwrap();
    let heavy = heavy.unwrap();

    assert!(
        (light.weight - 0.5).abs() < 0.01,
        "Light instance weight should be ~0.5, got {}",
        light.weight
    );
    assert!(
        (heavy.weight - 5.0).abs() < 0.01,
        "Heavy instance weight should be ~5.0, got {}",
        heavy.weight
    );
    assert!(
        heavy.weight > light.weight,
        "Heavy instance should have greater weight than light"
    );

    // Cleanup
    svc.deregister_instance(NS, GROUP, &service, inst_light)
        .await
        .unwrap();
    svc.deregister_instance(NS, GROUP, &service, inst_heavy)
        .await
        .unwrap();
    svc.shutdown().await;
}

// ==================== Multiple Clusters ====================

#[tokio::test]
async fn test_multiple_clusters() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-cluster-{}", common::test_id());

    let inst_a = test_instance_with_cluster("10.4.0.1", 8080, "CLUSTER_A");
    let inst_b = test_instance_with_cluster("10.4.0.2", 8080, "CLUSTER_B");
    let inst_c = test_instance_with_cluster("10.4.0.3", 8080, "CLUSTER_A");

    svc.register_instance(NS, GROUP, &service, inst_a.clone())
        .await
        .unwrap();
    svc.register_instance(NS, GROUP, &service, inst_b.clone())
        .await
        .unwrap();
    svc.register_instance(NS, GROUP, &service, inst_c.clone())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // All instances across all clusters
    let all = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert_eq!(all.len(), 3, "Should have 3 total instances across clusters");

    // Select only CLUSTER_A instances
    let cluster_a = svc
        .select_instances_with_clusters(
            NS,
            GROUP,
            &service,
            &["CLUSTER_A".to_string()],
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        cluster_a.len(),
        2,
        "CLUSTER_A should have 2 instances, got {}",
        cluster_a.len()
    );
    assert!(
        cluster_a.iter().all(|i| i.cluster_name == "CLUSTER_A"),
        "All selected instances should belong to CLUSTER_A"
    );

    // Select only CLUSTER_B instances
    let cluster_b = svc
        .select_instances_with_clusters(
            NS,
            GROUP,
            &service,
            &["CLUSTER_B".to_string()],
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        cluster_b.len(),
        1,
        "CLUSTER_B should have 1 instance, got {}",
        cluster_b.len()
    );
    assert_eq!(cluster_b[0].ip, "10.4.0.2");
    assert_eq!(cluster_b[0].cluster_name, "CLUSTER_B");

    // Cleanup
    svc.deregister_instance(NS, GROUP, &service, inst_a)
        .await
        .unwrap();
    svc.deregister_instance(NS, GROUP, &service, inst_b)
        .await
        .unwrap();
    svc.deregister_instance(NS, GROUP, &service, inst_c)
        .await
        .unwrap();
    svc.shutdown().await;
}

// ==================== Ephemeral Flag ====================

#[tokio::test]
async fn test_ephemeral_flag() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-ephemeral-{}", common::test_id());
    let inst = Instance {
        ip: "10.5.0.1".to_string(),
        port: 8080,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        service_name: String::new(),
        metadata: Default::default(),
        ..Default::default()
    };

    svc.register_instance(NS, GROUP, &service, inst.clone())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let instances = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert_eq!(instances.len(), 1, "Should find the ephemeral instance");
    assert!(
        instances[0].ephemeral,
        "Instance should be marked as ephemeral"
    );
    assert_eq!(instances[0].ip, "10.5.0.1");

    // Cleanup
    svc.deregister_instance(NS, GROUP, &service, inst)
        .await
        .unwrap();
    svc.shutdown().await;
}

// ==================== Multiple Subscriptions ====================

#[tokio::test]
async fn test_multiple_subscriptions() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let svc_name_1 = format!("svc-multisub1-{}", common::test_id());
    let svc_name_2 = format!("svc-multisub2-{}", common::test_id());

    let counter_1 = Arc::new(AtomicU32::new(0));
    let counter_2 = Arc::new(AtomicU32::new(0));

    struct CountingListener {
        counter: Arc<AtomicU32>,
    }
    impl EventListener for CountingListener {
        fn on_event(&self, _event: NamingEvent) {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    // Subscribe to both services
    svc.subscribe(
        NS,
        GROUP,
        &svc_name_1,
        "",
        Arc::new(CountingListener {
            counter: counter_1.clone(),
        }),
    )
    .await
    .unwrap();

    svc.subscribe(
        NS,
        GROUP,
        &svc_name_2,
        "",
        Arc::new(CountingListener {
            counter: counter_2.clone(),
        }),
    )
    .await
    .unwrap();

    // Verify both subscriptions are tracked
    let subscribed = svc.get_subscribe_services();
    assert!(
        subscribed.len() >= 2,
        "Should have at least 2 subscribed services, got {}",
        subscribed.len()
    );

    // Register instance to service 1 only
    svc.register_instance(NS, GROUP, &svc_name_1, test_instance("10.6.0.1", 8080))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    assert!(
        counter_1.load(Ordering::SeqCst) > 0,
        "Subscription 1 should have received at least one notification"
    );

    // Register instance to service 2
    svc.register_instance(NS, GROUP, &svc_name_2, test_instance("10.6.0.2", 8080))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    assert!(
        counter_2.load(Ordering::SeqCst) > 0,
        "Subscription 2 should have received at least one notification"
    );

    // Cleanup
    svc.unsubscribe(NS, GROUP, &svc_name_1, "").await.unwrap();
    svc.unsubscribe(NS, GROUP, &svc_name_2, "").await.unwrap();
    svc.shutdown().await;
}

// ==================== Concurrent Register ====================

#[tokio::test]
async fn test_concurrent_register() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("svc-concurrent-{}", common::test_id());

    // Register 10 instances concurrently
    let mut handles = Vec::new();
    for i in 1..=10 {
        let svc_clone = svc.clone();
        let service_clone = service.clone();
        handles.push(tokio::spawn(async move {
            let inst = test_instance(&format!("10.7.0.{}", i), 8080);
            svc_clone
                .register_instance(NS, GROUP, &service_clone, inst)
                .await
        }));
    }

    // Wait for all registrations to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let instances = svc.get_all_instances(NS, GROUP, &service).await.unwrap();
    assert_eq!(
        instances.len(),
        10,
        "Should have all 10 concurrently registered instances, got {}",
        instances.len()
    );

    // Verify all IPs are unique
    let mut ips: Vec<String> = instances.iter().map(|i| i.ip.clone()).collect();
    ips.sort();
    ips.dedup();
    assert_eq!(ips.len(), 10, "All 10 instances should have unique IPs");

    // Cleanup: batch deregister all
    let cleanup_instances: Vec<Instance> = (1..=10)
        .map(|i| test_instance(&format!("10.7.0.{}", i), 8080))
        .collect();
    svc.batch_deregister_instance(NS, GROUP, &service, cleanup_instances)
        .await
        .unwrap();
    svc.shutdown().await;
}

// ==================== Multi-Tenant Naming ====================

#[tokio::test]
async fn test_multi_tenant_naming() {
    common::init_tracing();

    // Use two separate naming service clients for different namespaces
    let svc_public = common::create_naming_service().await.unwrap();
    let svc_custom = common::create_naming_service().await.unwrap();

    let service = format!("svc-tenant-{}", common::test_id());
    let custom_ns = format!("custom_ns_{}", common::test_id());

    let inst_public = test_instance("10.8.0.1", 8080);
    let inst_custom = test_instance("10.8.0.2", 9090);

    // Register in public namespace (empty string)
    svc_public
        .register_instance("", GROUP, &service, inst_public.clone())
        .await
        .unwrap();

    // Register in custom namespace
    svc_custom
        .register_instance(&custom_ns, GROUP, &service, inst_custom.clone())
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Query public namespace
    let public_instances = svc_public
        .get_all_instances("", GROUP, &service)
        .await
        .unwrap();
    assert!(
        !public_instances.is_empty(),
        "Public namespace should have at least one instance"
    );
    assert!(
        public_instances.iter().any(|i| i.ip == "10.8.0.1"),
        "Public namespace should contain the public instance (10.8.0.1)"
    );

    // Query custom namespace
    let custom_instances = svc_custom
        .get_all_instances(&custom_ns, GROUP, &service)
        .await
        .unwrap();
    assert!(
        !custom_instances.is_empty(),
        "Custom namespace should have at least one instance"
    );
    assert!(
        custom_instances.iter().any(|i| i.ip == "10.8.0.2"),
        "Custom namespace should contain the custom instance (10.8.0.2)"
    );

    // Instances should be isolated: public should NOT contain custom instance
    assert!(
        !public_instances.iter().any(|i| i.ip == "10.8.0.2"),
        "Public namespace should NOT contain the custom namespace instance"
    );

    // Cleanup
    svc_public
        .deregister_instance("", GROUP, &service, inst_public)
        .await
        .unwrap();
    svc_custom
        .deregister_instance(&custom_ns, GROUP, &service, inst_custom)
        .await
        .unwrap();

    svc_public.shutdown().await;
    svc_custom.shutdown().await;
}

// ==================== Service List Pagination ====================

#[tokio::test]
async fn test_service_list_pagination() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let tag = common::test_id();

    // Create 5 distinct services by registering an instance in each
    let service_names: Vec<String> = (1..=5)
        .map(|i| format!("svc-page-{}-{}", tag, i))
        .collect();

    for name in &service_names {
        svc.register_instance(NS, GROUP, name, test_instance("10.9.0.1", 8080))
            .await
            .unwrap();
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Request page 1 with page_size=3
    let (total_count, page1) = svc.list_services(NS, GROUP, 1, 3).await.unwrap();
    assert!(
        total_count >= 5,
        "Total count should be at least 5 (our created services), got {}",
        total_count
    );
    assert!(
        page1.len() <= 3,
        "Page 1 with page_size=3 should return at most 3 services, got {}",
        page1.len()
    );

    // Request page 2
    let (_total_count_2, page2) = svc.list_services(NS, GROUP, 2, 3).await.unwrap();
    assert!(
        !page2.is_empty(),
        "Page 2 should have results when total > page_size"
    );

    // Pages should not overlap
    for svc_name in &page1 {
        assert!(
            !page2.contains(svc_name),
            "Service '{}' appears in both page 1 and page 2 -- pagination overlap",
            svc_name
        );
    }

    // Cleanup: deregister all created instances
    for name in &service_names {
        svc.deregister_instance(NS, GROUP, name, test_instance("10.9.0.1", 8080))
            .await
            .unwrap();
    }

    svc.shutdown().await;
}
