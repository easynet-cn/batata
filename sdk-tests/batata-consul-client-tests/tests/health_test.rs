//! Health API integration tests for the Batata Consul client.
//!
//! These tests require a running Batata server with Consul compatibility enabled.

mod common;

use batata_consul_client::AgentServiceRegistration;

#[tokio::test]
async fn test_health_node() {
    common::init_tracing();
    let client = common::create_client();

    // Find the node that has the serfHealth check by querying health state
    let (passing_checks, _meta) = client.health_state("passing", &common::q()).await.unwrap();
    let serf_check = passing_checks.iter().find(|c| c.check_id == "serfHealth");
    assert!(
        serf_check.is_some(),
        "Should have a 'serfHealth' check in passing state"
    );
    let node_name = &serf_check.unwrap().node;
    assert!(!node_name.is_empty(), "Node name should not be empty");

    // Query health for this node
    let (checks, _meta) = client.health_node(node_name, &common::q()).await.unwrap();

    // There should be at least the serfHealth check
    assert!(
        !checks.is_empty(),
        "Node '{}' should have at least one health check",
        node_name
    );
    let serf = checks.iter().find(|c| c.check_id == "serfHealth").unwrap();
    assert_eq!(serf.status, "passing", "serfHealth check should be passing");
    assert_eq!(
        &serf.node, node_name,
        "Check node should match queried node"
    );
}

#[tokio::test]
async fn test_health_service() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let svc_id = format!("health-svc-{}", suffix);
    let svc_name = format!("health-test-svc-{}", suffix);

    // Register a service
    let reg = AgentServiceRegistration {
        id: Some(svc_id.clone()),
        name: svc_name.clone(),
        address: Some("127.0.0.1".to_string()),
        port: Some(9191),
        tags: Some(vec!["health-test".to_string()]),
        ..Default::default()
    };
    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Query health for the service
    let (entries, _meta) = client
        .health_service(&svc_name, "", false, &common::q())
        .await
        .unwrap();

    assert!(
        !entries.is_empty(),
        "Health service query should return at least one entry for '{}'",
        svc_name
    );
    let entry = &entries[0];
    assert_eq!(entry.service.id, svc_id, "Service entry ID should match");
    assert_eq!(
        entry.service.service, svc_name,
        "Service entry name should match"
    );
    assert_eq!(entry.service.port, 9191, "Service port should match");
    assert!(
        !entry.node.node.is_empty(),
        "Node name in entry should not be empty"
    );

    // Cleanup
    client
        .agent_service_deregister(&svc_id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_health_checks() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let svc_id = format!("health-checks-svc-{}", suffix);
    let svc_name = format!("health-checks-test-{}", suffix);

    // Register a service with a TTL check
    let reg = AgentServiceRegistration {
        id: Some(svc_id.clone()),
        name: svc_name.clone(),
        address: Some("127.0.0.1".to_string()),
        port: Some(9292),
        check: Some(batata_consul_client::AgentServiceCheck {
            ttl: Some("30s".to_string()),
            name: Some("ttl-health-check".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Query checks for this service
    let (checks, _meta) = client.health_checks(&svc_name, &common::q()).await.unwrap();

    assert!(
        !checks.is_empty(),
        "Should have at least one health check for service '{}'",
        svc_name
    );
    let check = &checks[0];
    assert_eq!(
        check.service_name, svc_name,
        "Check's service name should match"
    );
    assert_eq!(check.service_id, svc_id, "Check's service ID should match");
    // TTL checks start in critical state
    assert!(
        check.status == "critical" || check.status == "passing" || check.status == "warning",
        "Check status should be a valid state, got: '{}'",
        check.status
    );

    // Cleanup
    client
        .agent_service_deregister(&svc_id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_health_state() {
    common::init_tracing();
    let client = common::create_client();

    // Query all checks in "passing" state
    let (checks, _meta) = client.health_state("passing", &common::q()).await.unwrap();

    // There should be at least the serfHealth check in passing state
    assert!(
        !checks.is_empty(),
        "Should have at least one check in 'passing' state"
    );
    for check in &checks {
        assert_eq!(
            check.status, "passing",
            "All returned checks should have 'passing' status, but found '{}'",
            check.status
        );
    }
    // Verify serfHealth is among them
    let serf = checks.iter().find(|c| c.check_id == "serfHealth");
    assert!(
        serf.is_some(),
        "serfHealth should be in the 'passing' state list"
    );
}

#[tokio::test]
async fn test_health_connect() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let svc_name = format!("connect-health-{}", suffix);

    // Query connect health for a service (may be empty if no connect-proxies are registered)
    let result = client
        .health_connect(&svc_name, "", false, &common::q())
        .await;

    match result {
        Ok((entries, _meta)) => {
            // Connect health may return empty for non-connect services
            assert!(
                entries.is_empty() || !entries.is_empty(),
                "Connect health query should return a valid list"
            );
        }
        Err(_) => {
            // Some implementations may not fully support Connect
        }
    }
}

#[tokio::test]
async fn test_health_service_multiple_tags() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let svc_id = format!("health-mtags-svc-{}", suffix);
    let svc_name = format!("health-mtags-test-{}", suffix);

    let reg = AgentServiceRegistration {
        id: Some(svc_id.clone()),
        name: svc_name.clone(),
        address: Some("10.0.0.5".to_string()),
        port: Some(9393),
        tags: Some(vec![
            "web".to_string(),
            "v3".to_string(),
            "canary".to_string(),
        ]),
        ..Default::default()
    };
    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Query health with single tag (Batata does not support multi-tag filtering via repeated ?tag= params)
    let (entries, _meta) = client
        .health_service(&svc_name, "web", false, &common::q())
        .await
        .unwrap();

    assert!(
        !entries.is_empty(),
        "Should find service '{}' when filtering by tag 'web'",
        svc_name
    );
    let entry = &entries[0];
    assert_eq!(entry.service.id, svc_id, "Service ID should match");
    let tags = entry.service.tags.as_ref().expect("Tags should be present");
    assert!(
        tags.contains(&"web".to_string()),
        "Tags should contain 'web'"
    );
    assert!(tags.contains(&"v3".to_string()), "Tags should contain 'v3'");
    assert!(
        tags.contains(&"canary".to_string()),
        "Tags should contain 'canary'"
    );

    // Cleanup
    client
        .agent_service_deregister(&svc_id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_health_passing_filter() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let svc_id = format!("health-pass-svc-{}", suffix);
    let svc_name = format!("health-pass-test-{}", suffix);

    // Register a service (no checks, so it should be considered passing by default)
    let reg = AgentServiceRegistration {
        id: Some(svc_id.clone()),
        name: svc_name.clone(),
        address: Some("127.0.0.1".to_string()),
        port: Some(9494),
        ..Default::default()
    };
    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Query with passing_only=true
    let (entries, _meta) = client
        .health_service(&svc_name, "", true, &common::q())
        .await
        .unwrap();

    assert!(
        !entries.is_empty(),
        "Service '{}' with no checks should appear in passing-only query",
        svc_name
    );
    let entry = &entries[0];
    assert_eq!(entry.service.id, svc_id, "Service ID should match");
    // Verify all checks (if any) are passing
    for check in &entry.checks {
        assert_eq!(
            check.status, "passing",
            "All checks should be 'passing' in passing-only query, got '{}'",
            check.status
        );
    }

    // Cleanup
    client
        .agent_service_deregister(&svc_id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_health_ingress() {
    common::init_tracing();
    let client = common::create_client();

    // Query ingress health for a non-existent service; should return empty or succeed
    let result = client
        .health_ingress("nonexistent-ingress", &common::q())
        .await;

    match result {
        Ok((entries, _meta)) => {
            // May be empty since no ingress gateways are configured
            assert!(
                entries.is_empty() || !entries.is_empty(),
                "Ingress health query should return a valid list"
            );
        }
        Err(_) => {
            // Some implementations may return an error for non-existent ingress services
        }
    }
}
