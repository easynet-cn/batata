//! Agent API integration tests for the Batata Consul client.
//!
//! These tests require a running Batata server with Consul compatibility enabled.

mod common;

use std::collections::HashMap;

use batata_consul_client::{AgentCheckRegistration, AgentServiceRegistration};

#[tokio::test]
#[ignore]
async fn test_agent_self() {
    common::init_tracing();
    let client = common::create_client();

    let (info, meta) = client.agent_self(&common::q()).await.unwrap();

    // Verify the response contains a Config section with a NodeName
    let node_name = info
        .get("Config")
        .and_then(|c| c.get("NodeName"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    assert!(!node_name.is_empty(), "Node name should not be empty");
    assert!(
        meta.last_index > 0 || meta.last_index == 0,
        "QueryMeta should be returned"
    );
}

#[tokio::test]
#[ignore]
async fn test_agent_members() {
    common::init_tracing();
    let client = common::create_client();

    let (members, _meta) = client.agent_members(false, &common::q()).await.unwrap();

    assert!(!members.is_empty(), "Should have at least one member");
    assert!(
        !members[0].name.is_empty(),
        "First member should have a non-empty name"
    );
    assert!(
        !members[0].addr.is_empty(),
        "First member should have a non-empty address"
    );
}

#[tokio::test]
#[ignore]
async fn test_agent_service_register_and_deregister() {
    common::init_tracing();
    let client = common::create_client();
    let id = format!("test-svc-{}", common::test_id());

    let reg = AgentServiceRegistration {
        id: Some(id.clone()),
        name: format!("test-service-{}", common::test_id()),
        address: Some("127.0.0.1".to_string()),
        port: Some(9090),
        tags: Some(vec!["test".to_string(), "rust".to_string()]),
        ..Default::default()
    };

    // Register the service
    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Verify the service appears in agent services list
    let (services, _) = client.agent_services(&common::q()).await.unwrap();
    assert!(
        services.contains_key(&id),
        "Registered service '{}' should appear in agent services",
        id
    );
    let svc = &services[&id];
    assert_eq!(svc.port, 9090, "Service port should match");
    assert_eq!(svc.address, "127.0.0.1", "Service address should match");

    // Deregister the service
    client
        .agent_service_deregister(&id, &common::w())
        .await
        .unwrap();

    // Verify the service is gone
    let (services_after, _) = client.agent_services(&common::q()).await.unwrap();
    assert!(
        !services_after.contains_key(&id),
        "Deregistered service '{}' should no longer appear",
        id
    );
}

#[tokio::test]
#[ignore]
async fn test_agent_service_get() {
    common::init_tracing();
    let client = common::create_client();
    let id = format!("test-svc-get-{}", common::test_id());

    let reg = AgentServiceRegistration {
        id: Some(id.clone()),
        name: "test-service-get".to_string(),
        address: Some("10.0.0.1".to_string()),
        port: Some(8080),
        tags: Some(vec!["web".to_string()]),
        meta: Some(HashMap::from([("env".to_string(), "test".to_string())])),
        ..Default::default()
    };

    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Get service by ID
    let (svc, _) = client.agent_service(&id, &common::q()).await.unwrap();
    assert_eq!(svc.id, id, "Service ID should match");
    assert_eq!(svc.service, "test-service-get", "Service name should match");
    assert_eq!(svc.address, "10.0.0.1", "Service address should match");
    assert_eq!(svc.port, 8080, "Service port should match");
    let tags = svc.tags.as_ref().expect("Tags should be present");
    assert!(
        tags.contains(&"web".to_string()),
        "Tags should contain 'web'"
    );
    let meta = svc.meta.as_ref().expect("Meta should be present");
    assert_eq!(
        meta.get("env").map(|s| s.as_str()),
        Some("test"),
        "Meta 'env' should be 'test'"
    );

    // Cleanup
    client
        .agent_service_deregister(&id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_agent_check_register_and_deregister() {
    common::init_tracing();
    let client = common::create_client();
    let check_id = format!("test-check-{}", common::test_id());

    let check = AgentCheckRegistration {
        id: Some(check_id.clone()),
        name: "test-ttl-check".to_string(),
        ttl: Some("30s".to_string()),
        notes: Some("Integration test check".to_string()),
        ..Default::default()
    };

    // Register the check
    client
        .agent_check_register(&check, &common::w())
        .await
        .unwrap();

    // Verify it appears in agent checks
    let (checks, _) = client.agent_checks(&common::q()).await.unwrap();
    assert!(
        checks.contains_key(&check_id),
        "Registered check '{}' should appear in agent checks",
        check_id
    );
    let c = &checks[&check_id];
    assert_eq!(c.check_id, check_id, "Check ID should match");
    assert_eq!(c.name, "test-ttl-check", "Check name should match");

    // Deregister the check
    client
        .agent_check_deregister(&check_id, &common::w())
        .await
        .unwrap();

    // Verify it is gone
    let (checks_after, _) = client.agent_checks(&common::q()).await.unwrap();
    assert!(
        !checks_after.contains_key(&check_id),
        "Deregistered check '{}' should no longer appear",
        check_id
    );
}

#[tokio::test]
#[ignore]
async fn test_agent_check_pass_warn_fail() {
    common::init_tracing();
    let client = common::create_client();
    let check_id = format!("test-ttl-{}", common::test_id());

    let check = AgentCheckRegistration {
        id: Some(check_id.clone()),
        name: "test-ttl-status".to_string(),
        ttl: Some("60s".to_string()),
        ..Default::default()
    };

    client
        .agent_check_register(&check, &common::w())
        .await
        .unwrap();

    // Mark as passing
    client
        .agent_check_pass(&check_id, "all good", &common::w())
        .await
        .unwrap();
    let (checks, _) = client.agent_checks(&common::q()).await.unwrap();
    let c = checks.get(&check_id).expect("Check should exist");
    assert_eq!(
        c.status, "passing",
        "Check status should be 'passing' after pass"
    );

    // Mark as warning
    client
        .agent_check_warn(&check_id, "degraded", &common::w())
        .await
        .unwrap();
    let (checks, _) = client.agent_checks(&common::q()).await.unwrap();
    let c = checks.get(&check_id).expect("Check should exist");
    assert_eq!(
        c.status, "warning",
        "Check status should be 'warning' after warn"
    );

    // Mark as critical
    client
        .agent_check_fail(&check_id, "broken", &common::w())
        .await
        .unwrap();
    let (checks, _) = client.agent_checks(&common::q()).await.unwrap();
    let c = checks.get(&check_id).expect("Check should exist");
    assert_eq!(
        c.status, "critical",
        "Check status should be 'critical' after fail"
    );

    // Cleanup
    client
        .agent_check_deregister(&check_id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_agent_node_maintenance() {
    common::init_tracing();
    let client = common::create_client();

    // Enable node maintenance
    client
        .agent_enable_node_maintenance("test maintenance", &common::w())
        .await
        .unwrap();

    // Verify maintenance check appears
    let (checks, _) = client.agent_checks(&common::q()).await.unwrap();
    let maintenance_check = checks
        .values()
        .find(|c| c.check_id.contains("maintenance") || c.name.contains("Maintenance"));
    assert!(
        maintenance_check.is_some(),
        "A maintenance check should appear when node maintenance is enabled"
    );

    // Disable node maintenance
    client
        .agent_disable_node_maintenance(&common::w())
        .await
        .unwrap();

    // Verify maintenance check is removed
    let (checks_after, _) = client.agent_checks(&common::q()).await.unwrap();
    let maintenance_after = checks_after
        .values()
        .find(|c| c.check_id == "_node_maintenance");
    assert!(
        maintenance_after.is_none(),
        "Node maintenance check should be removed after disabling"
    );
}

#[tokio::test]
#[ignore]
async fn test_agent_service_maintenance() {
    common::init_tracing();
    let client = common::create_client();
    let id = format!("test-maint-svc-{}", common::test_id());

    let reg = AgentServiceRegistration {
        id: Some(id.clone()),
        name: "test-maint-service".to_string(),
        address: Some("127.0.0.1".to_string()),
        port: Some(7070),
        ..Default::default()
    };

    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();

    // Enable service maintenance
    client
        .agent_enable_service_maintenance(&id, "upgrading", &common::w())
        .await
        .unwrap();

    // Verify a maintenance check exists for this service
    let (checks, _) = client.agent_checks(&common::q()).await.unwrap();
    let maint_check = checks
        .values()
        .find(|c| c.service_id == id && c.check_id.contains("maintenance"));
    assert!(
        maint_check.is_some(),
        "A maintenance check should appear for service '{}'",
        id
    );

    // Disable service maintenance
    client
        .agent_disable_service_maintenance(&id, &common::w())
        .await
        .unwrap();

    // Cleanup
    client
        .agent_service_deregister(&id, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_agent_host() {
    common::init_tracing();
    let client = common::create_client();

    let (host_info, _meta) = client.agent_host(&common::q()).await.unwrap();

    // The host info should be a JSON object (non-null)
    assert!(
        host_info.is_object() || host_info.is_string() || !host_info.is_null(),
        "Host info should be a non-null response"
    );
}

#[tokio::test]
#[ignore]
async fn test_agent_version() {
    common::init_tracing();
    let client = common::create_client();

    let (version_info, _meta) = client.agent_version(&common::q()).await.unwrap();

    // The version info should contain some version string
    assert!(!version_info.is_null(), "Version info should not be null");
}

#[tokio::test]
#[ignore]
async fn test_agent_metrics() {
    common::init_tracing();
    let client = common::create_client();

    let (metrics, _meta) = client.agent_metrics(&common::q()).await.unwrap();

    assert!(!metrics.is_null(), "Metrics response should not be null");
    // Metrics should be a JSON object
    assert!(
        metrics.is_object() || metrics.is_array(),
        "Metrics should be a JSON object or array"
    );
}

#[tokio::test]
#[ignore]
async fn test_agent_services_with_filter() {
    common::init_tracing();
    let client = common::create_client();
    let suffix = common::test_id();
    let id1 = format!("test-filter-svc1-{}", suffix);
    let id2 = format!("test-filter-svc2-{}", suffix);
    let meta_value = format!("filter-group-{}", suffix);

    // Register two services with the same meta key/value
    let reg1 = AgentServiceRegistration {
        id: Some(id1.clone()),
        name: format!("filter-svc-{}", suffix),
        address: Some("10.0.0.1".to_string()),
        port: Some(8001),
        meta: Some(HashMap::from([("group".to_string(), meta_value.clone())])),
        ..Default::default()
    };
    let reg2 = AgentServiceRegistration {
        id: Some(id2.clone()),
        name: format!("filter-svc-{}", suffix),
        address: Some("10.0.0.2".to_string()),
        port: Some(8002),
        meta: Some(HashMap::from([("group".to_string(), meta_value.clone())])),
        ..Default::default()
    };

    client
        .agent_service_register(&reg1, &common::w())
        .await
        .unwrap();
    client
        .agent_service_register(&reg2, &common::w())
        .await
        .unwrap();

    // Filter by meta
    let filter = format!(r#"Meta["group"] == "{}""#, meta_value);
    let (filtered, _) = client
        .agent_services_with_filter(&filter, &common::q())
        .await
        .unwrap();

    assert!(
        filtered.len() >= 2,
        "Filter should return at least 2 services, got {}",
        filtered.len()
    );
    assert!(
        filtered.contains_key(&id1),
        "Filtered results should contain service '{}'",
        id1
    );
    assert!(
        filtered.contains_key(&id2),
        "Filtered results should contain service '{}'",
        id2
    );

    // Cleanup
    client
        .agent_service_deregister(&id1, &common::w())
        .await
        .unwrap();
    client
        .agent_service_deregister(&id2, &common::w())
        .await
        .unwrap();
}
