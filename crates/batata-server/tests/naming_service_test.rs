// Integration tests for NamingService
// Tests service registration, discovery, and subscription functionality

use batata_server::api::naming::model::Instance;
use batata_server::service::naming::NamingService;
use std::collections::HashMap;

fn create_test_instance(ip: &str, port: i32, cluster: &str) -> Instance {
    Instance {
        instance_id: format!("{}#{}#{}#test-service", ip, port, cluster),
        ip: ip.to_string(),
        port,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: cluster.to_string(),
        service_name: String::new(),
        metadata: HashMap::new(),
    }
}

#[test]
fn test_register_and_get_instances() {
    let naming = NamingService::new();

    // Register multiple instances
    let instance1 = create_test_instance("192.168.1.1", 8080, "DEFAULT");
    let instance2 = create_test_instance("192.168.1.2", 8080, "DEFAULT");
    let instance3 = create_test_instance("192.168.1.3", 8080, "CLUSTER_A");

    naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
    naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);
    naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance3);

    // Get all instances
    let all = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
    assert_eq!(all.len(), 3);

    // Get instances by cluster
    let default_cluster =
        naming.get_instances("public", "DEFAULT_GROUP", "test-service", "DEFAULT", false);
    assert_eq!(default_cluster.len(), 2);

    let cluster_a = naming.get_instances(
        "public",
        "DEFAULT_GROUP",
        "test-service",
        "CLUSTER_A",
        false,
    );
    assert_eq!(cluster_a.len(), 1);
}

#[test]
fn test_healthy_only_filter() {
    let naming = NamingService::new();

    let healthy = create_test_instance("192.168.1.1", 8080, "DEFAULT");
    let mut unhealthy = create_test_instance("192.168.1.2", 8080, "DEFAULT");
    unhealthy.healthy = false;

    naming.register_instance("public", "DEFAULT_GROUP", "test-service", healthy);
    naming.register_instance("public", "DEFAULT_GROUP", "test-service", unhealthy);

    let all = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
    assert_eq!(all.len(), 2);

    let healthy_only = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", true);
    assert_eq!(healthy_only.len(), 1);
    assert!(healthy_only[0].healthy);
}

#[test]
fn test_deregister_instance() {
    let naming = NamingService::new();

    let instance = create_test_instance("192.168.1.1", 8080, "DEFAULT");
    naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance.clone());

    let before = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
    assert_eq!(before.len(), 1);

    naming.deregister_instance("public", "DEFAULT_GROUP", "test-service", &instance);

    let after = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
    assert!(after.is_empty());
}

#[test]
fn test_subscription_management() {
    let naming = NamingService::new();

    // Subscribe multiple connections
    naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "service-a");
    naming.subscribe("conn-2", "public", "DEFAULT_GROUP", "service-a");
    naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "service-b");

    // Get subscribers for service-a
    let subs_a = naming.get_subscribers("public", "DEFAULT_GROUP", "service-a");
    assert_eq!(subs_a.len(), 2);
    assert!(subs_a.contains(&"conn-1".to_string()));
    assert!(subs_a.contains(&"conn-2".to_string()));

    // Unsubscribe
    naming.unsubscribe("conn-1", "public", "DEFAULT_GROUP", "service-a");
    let subs_a_after = naming.get_subscribers("public", "DEFAULT_GROUP", "service-a");
    assert_eq!(subs_a_after.len(), 1);
    assert!(!subs_a_after.contains(&"conn-1".to_string()));

    // Remove subscriber completely
    naming.remove_subscriber("conn-1");
    let subs_b = naming.get_subscribers("public", "DEFAULT_GROUP", "service-b");
    assert!(subs_b.is_empty());
}

#[test]
fn test_list_services_pagination() {
    let naming = NamingService::new();

    // Register 5 services
    for i in 1..=5 {
        let instance = create_test_instance("192.168.1.1", 8080 + i, "DEFAULT");
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            &format!("service-{}", i),
            instance,
        );
    }

    // Test pagination
    let (total, page1) = naming.list_services("public", "DEFAULT_GROUP", 1, 2);
    assert_eq!(total, 5);
    assert_eq!(page1.len(), 2);

    let (_, page2) = naming.list_services("public", "DEFAULT_GROUP", 2, 2);
    assert_eq!(page2.len(), 2);

    let (_, page3) = naming.list_services("public", "DEFAULT_GROUP", 3, 2);
    assert_eq!(page3.len(), 1);
}

#[test]
fn test_get_service_info() {
    let naming = NamingService::new();

    let instance1 = create_test_instance("192.168.1.1", 8080, "DEFAULT");
    let mut instance2 = create_test_instance("192.168.1.2", 8081, "DEFAULT");
    instance2.healthy = false;

    naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
    naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);

    let service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);

    assert_eq!(service.name, "test-service");
    assert_eq!(service.group_name, "DEFAULT_GROUP");
    assert_eq!(service.hosts.len(), 1); // Only healthy
    assert!(service.all_ips); // Has instances (even unhealthy)
}

#[test]
fn test_batch_operations() {
    let naming = NamingService::new();

    let instances = vec![
        create_test_instance("192.168.1.1", 8080, "DEFAULT"),
        create_test_instance("192.168.1.2", 8081, "DEFAULT"),
        create_test_instance("192.168.1.3", 8082, "DEFAULT"),
    ];

    // Batch register
    naming.batch_register_instances("public", "DEFAULT_GROUP", "test-service", instances.clone());

    let registered = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
    assert_eq!(registered.len(), 3);

    // Batch deregister
    naming.batch_deregister_instances("public", "DEFAULT_GROUP", "test-service", &instances);

    let after = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
    assert!(after.is_empty());
}

#[test]
fn test_namespace_isolation() {
    let naming = NamingService::new();

    let instance = create_test_instance("192.168.1.1", 8080, "DEFAULT");

    naming.register_instance(
        "namespace-a",
        "DEFAULT_GROUP",
        "test-service",
        instance.clone(),
    );
    naming.register_instance("namespace-b", "DEFAULT_GROUP", "test-service", instance);

    let ns_a = naming.get_instances("namespace-a", "DEFAULT_GROUP", "test-service", "", false);
    let ns_b = naming.get_instances("namespace-b", "DEFAULT_GROUP", "test-service", "", false);
    let ns_c = naming.get_instances("namespace-c", "DEFAULT_GROUP", "test-service", "", false);

    assert_eq!(ns_a.len(), 1);
    assert_eq!(ns_b.len(), 1);
    assert!(ns_c.is_empty());
}
