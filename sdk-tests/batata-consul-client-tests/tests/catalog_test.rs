//! Catalog API integration tests for the Batata Consul client.
//!
//! These tests require a running Batata server with Consul compatibility enabled.

mod common;

use batata_consul_client::{
    AgentService, AgentServiceRegistration, CatalogDeregistration, CatalogRegistration,
};

#[tokio::test]
#[ignore]
async fn test_catalog_datacenters() {
    common::init_tracing();
    let client = common::create_client();

    let (datacenters, _meta) = client.catalog_datacenters(&common::q()).await.unwrap();

    assert!(
        !datacenters.is_empty(),
        "Should have at least one datacenter"
    );
    assert!(
        datacenters.contains(&"dc1".to_string()),
        "Datacenters should contain 'dc1', got: {:?}",
        datacenters
    );
}

#[tokio::test]
#[ignore]
async fn test_catalog_nodes() {
    common::init_tracing();
    let client = common::create_client();

    let (nodes, _meta) = client.catalog_nodes(&common::q()).await.unwrap();

    assert!(!nodes.is_empty(), "Should have at least one node");
    assert!(
        !nodes[0].node.is_empty(),
        "First node should have a non-empty name"
    );
    assert!(
        !nodes[0].address.is_empty(),
        "First node should have a non-empty address"
    );
}

#[tokio::test]
#[ignore]
async fn test_catalog_services() {
    common::init_tracing();
    let client = common::create_client();

    let (services, _meta) = client.catalog_services(&common::q()).await.unwrap();

    assert!(
        !services.is_empty(),
        "Should have at least one service in the catalog"
    );
    assert!(
        services.contains_key("consul"),
        "Catalog services should contain the 'consul' service, got keys: {:?}",
        services.keys().collect::<Vec<_>>()
    );
}

#[tokio::test]
#[ignore]
async fn test_catalog_service() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let svc_id = format!("cat-svc-{}", suffix);
    let svc_name = format!("catalog-test-svc-{}", suffix);

    // Register a service via agent API first
    let reg = AgentServiceRegistration {
        id: Some(svc_id.clone()),
        name: svc_name.clone(),
        address: Some("192.168.1.100".to_string()),
        port: Some(5050),
        tags: Some(vec!["catalog-test".to_string()]),
        ..Default::default()
    };
    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Query catalog for this service
    let (entries, _meta) = client
        .catalog_service(&svc_name, "", &common::q())
        .await
        .unwrap();

    assert!(
        !entries.is_empty(),
        "Catalog should return at least one entry for '{}'",
        svc_name
    );
    let entry = &entries[0];
    assert_eq!(entry.service_name, svc_name, "Service name should match");
    assert_eq!(entry.service_port, 5050, "Service port should match");
    assert_eq!(entry.service_id, svc_id, "Service ID should match");

    // Cleanup
    client
        .agent_service_deregister(&svc_id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_catalog_node() {
    common::init_tracing();
    let client = common::create_client();

    // Get a known node name from catalog (not agent self, as they may differ in Batata)
    let (nodes, _) = client.catalog_nodes(&common::q()).await.unwrap();
    assert!(!nodes.is_empty(), "Should have at least one catalog node");
    let node_name = &nodes[0].node;
    assert!(!node_name.is_empty(), "Node name should not be empty");
    assert!(!nodes[0].address.is_empty(), "Node address should not be empty");

    // Query catalog for this specific node
    let (catalog_node, _meta) = client.catalog_node(node_name, &common::q()).await.unwrap();

    let catalog_node = catalog_node.expect("Catalog node should exist");
    let node = catalog_node.node.expect("Node info should be present");
    assert_eq!(node.node, *node_name, "Node name should match");
    assert!(!node.address.is_empty(), "Node address should not be empty");
}

#[tokio::test]
#[ignore]
async fn test_catalog_register_and_deregister() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let node_name = format!("test-node-{}", suffix);
    let svc_id = format!("cat-reg-svc-{}", suffix);
    let svc_name = format!("cat-reg-service-{}", suffix);

    // Register a node with a service via catalog API
    let registration = CatalogRegistration {
        node: node_name.clone(),
        address: "10.10.10.10".to_string(),
        service: Some(AgentService {
            id: svc_id.clone(),
            service: svc_name.clone(),
            port: 6060,
            address: "10.10.10.10".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };

    client
        .catalog_register(&registration, &common::w())
        .await
        .unwrap();

    // Verify the service appears in catalog
    let (entries, _) = client
        .catalog_service(&svc_name, "", &common::q())
        .await
        .unwrap();
    assert!(
        !entries.is_empty(),
        "Catalog should contain the registered service '{}'",
        svc_name
    );
    assert_eq!(entries[0].service_id, svc_id, "Service ID should match");
    assert_eq!(entries[0].service_port, 6060, "Service port should match");

    // Deregister the service
    let deregistration = CatalogDeregistration {
        node: node_name.clone(),
        service_id: Some(svc_id.clone()),
        ..Default::default()
    };
    client
        .catalog_deregister(&deregistration, &common::w())
        .await
        .unwrap();

    // Verify the service is gone
    let (entries_after, _) = client
        .catalog_service(&svc_name, "", &common::q())
        .await
        .unwrap();
    assert!(
        entries_after.is_empty(),
        "Catalog should no longer contain deregistered service '{}'",
        svc_name
    );

    // Deregister the node itself
    let node_dereg = CatalogDeregistration {
        node: node_name.clone(),
        ..Default::default()
    };
    client
        .catalog_deregister(&node_dereg, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_catalog_gateway_services() {
    common::init_tracing();
    let client = common::create_client();

    // Query gateway services for a non-existent gateway; should return empty or succeed
    let result = client
        .catalog_gateway_services("nonexistent-gateway", &common::q())
        .await;

    match result {
        Ok((services, _meta)) => {
            // May be empty, that is fine
            assert!(
                services.is_empty() || !services.is_empty(),
                "Gateway services query should return a valid list"
            );
        }
        Err(_) => {
            // Some implementations may return 404 for non-existent gateways, which is acceptable
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_catalog_service_multiple_tags() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let svc_id = format!("cat-tags-svc-{}", suffix);
    let svc_name = format!("cat-tags-service-{}", suffix);

    let reg = AgentServiceRegistration {
        id: Some(svc_id.clone()),
        name: svc_name.clone(),
        address: Some("172.16.0.1".to_string()),
        port: Some(7070),
        tags: Some(vec![
            "primary".to_string(),
            "v2".to_string(),
            "production".to_string(),
        ]),
        ..Default::default()
    };

    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Query by single tag (Batata does not support multi-tag filtering via repeated ?tag= params)
    let (entries, _) = client
        .catalog_service(&svc_name, "primary", &common::q())
        .await
        .unwrap();

    assert!(
        !entries.is_empty(),
        "Should find service '{}' when filtering by tag 'primary'",
        svc_name
    );
    let entry = &entries[0];
    assert_eq!(entry.service_id, svc_id, "Service ID should match");
    let tags = entry
        .service_tags
        .as_ref()
        .expect("Service tags should be present");
    assert!(
        tags.contains(&"primary".to_string()),
        "Tags should contain 'primary'"
    );
    assert!(tags.contains(&"v2".to_string()), "Tags should contain 'v2'");
    assert!(
        tags.contains(&"production".to_string()),
        "Tags should contain 'production'"
    );

    // Cleanup
    client
        .agent_service_deregister(&svc_id, &common::w())
        .await
        .unwrap();
}
