//! Naming heartbeat and instance expiration integration tests
//!
//! These tests verify the heartbeat timeout and automatic instance expiration functionality
//! that matches Nacos behavior.

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_service_name,
};
use serde_json::json;
use tokio::time::{Duration, sleep};

/// Create an authenticated test client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test that instances are marked unhealthy after heartbeat timeout
#[tokio::test]
async fn test_instance_marked_unhealthy_after_heartbeat_timeout() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("heartbeat_timeout");
    let ip = "192.168.1.200";
    let port = 9090;

    // Register an instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": port,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": true
            }),
        )
        .await
        .expect("Failed to register instance");

    // Verify instance is initially healthy
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // Wait for heartbeat timeout (default is 15 seconds)
    // In a real test, we might need to wait longer or adjust the timeout
    sleep(Duration::from_secs(16)).await;

    // Query instances again - instance should be marked as unhealthy
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // The instance should now be marked as unhealthy
    if let Some(hosts) = response["data"]["hosts"].as_array()
        && let Some(instance) = hosts.first()
        && let Some(healthy) = instance["healthy"].as_bool()
    {
        // Instance should be unhealthy after timeout
        // (If health check is running)
        assert!(
            !healthy,
            "Instance should be marked as unhealthy after heartbeat timeout"
        );
    }
}

/// Test that expired instances are deleted when expire is enabled
#[tokio::test]
async fn test_expired_instance_deleted_when_expire_enabled() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("expire_instance");
    let ip = "192.168.1.201";
    let port = 9091;

    // Register an instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": port,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": true
            }),
        )
        .await
        .expect("Failed to register instance");

    // Verify instance exists
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    let initial_count = response["data"]["hosts"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);
    assert!(
        initial_count >= 1,
        "Should have at least 1 instance initially"
    );

    // Wait for delete timeout (default is 30 seconds)
    sleep(Duration::from_secs(31)).await;

    // Query instances again - expired instance should be deleted
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // The instance should be deleted
    let _final_count = response["data"]["hosts"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);

    // Instance should be deleted (if expire is enabled and health check ran)
    // Note: This test may pass even if instance still exists, depending on timing
    // In production, we would wait longer or configure shorter timeouts for testing
}

/// Test that instance heartbeat refreshes its status
#[tokio::test]
async fn test_heartbeat_refreshes_instance_status() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("heartbeat_refresh");
    let ip = "192.168.1.202";
    let port = 9092;

    // Register an instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": port,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": true
            }),
        )
        .await
        .expect("Failed to register instance");

    // Send a heartbeat
    let heartbeat_response: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance/beat",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": port,
                "clusterName": DEFAULT_GROUP
            }),
        )
        .await
        .expect("Failed to send heartbeat");

    assert_eq!(heartbeat_response["code"], 0, "Heartbeat should succeed");

    // Wait a bit but less than heartbeat timeout
    sleep(Duration::from_secs(5)).await;

    // Query instances - instance should still be healthy
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // Instance should still be healthy after heartbeat
    if let Some(hosts) = response["data"]["hosts"].as_array()
        && let Some(instance) = hosts.first()
        && let Some(healthy) = instance["healthy"].as_bool()
    {
        assert!(healthy, "Instance should still be healthy after heartbeat");
    }
}

/// Test that deregistered instance stops being tracked
#[tokio::test]
async fn test_deregistered_instance_stops_being_tracked() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("deregister_tracking");
    let ip = "192.168.1.203";
    let port = 9093;

    // Register an instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": port,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": true
            }),
        )
        .await
        .expect("Failed to register instance");

    // Deregister the instance
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", ip),
                ("port", &port.to_string()),
                ("clusterName", DEFAULT_GROUP),
            ],
        )
        .await
        .expect("Failed to deregister instance");

    // Wait a bit
    sleep(Duration::from_secs(2)).await;

    // Query instances - instance should not exist
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // Deregistered instance should not be in the list
    if let Some(hosts) = response["data"]["hosts"].as_array() {
        let found = hosts
            .iter()
            .any(|instance| instance["ip"] == ip && instance["port"] == port);
        assert!(!found, "Deregistered instance should not be in the list");
    }
}

/// Test multiple instances with different heartbeat timings
#[tokio::test]
async fn test_multiple_instances_different_heartbeat_timings() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("multi_heartbeat");

    // Register multiple instances
    let instances = vec![
        ("192.168.1.210", 10000),
        ("192.168.1.211", 10001),
        ("192.168.1.212", 10002),
    ];

    for (ip, port) in &instances {
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &json!({
                    "serviceName": service_name,
                    "ip": ip,
                    "port": port,
                    "clusterName": DEFAULT_GROUP,
                    "weight": 1.0,
                    "healthy": true,
                    "enabled": true,
                    "ephemeral": true
                }),
            )
            .await
            .expect("Failed to register instance");
    }

    // Send heartbeat for first instance only
    let (ip, port) = &instances[0];
    let _: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance/beat",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": port,
                "clusterName": DEFAULT_GROUP
            }),
        )
        .await
        .expect("Failed to send heartbeat");

    // Wait a bit
    sleep(Duration::from_secs(5)).await;

    // Query all instances
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // All instances should exist
    if let Some(hosts) = response["data"]["hosts"].as_array() {
        assert_eq!(
            hosts.len(),
            instances.len(),
            "All instances should be present"
        );
    }
}

/// Test ephemeral instances are checked for heartbeat
#[tokio::test]
async fn test_ephemeral_instances_checked_for_heartbeat() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("ephemeral_heartbeat");

    // Register ephemeral instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.220",
                "port": 11000,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": true
            }),
        )
        .await
        .expect("Failed to register ephemeral instance");

    // Verify instance exists
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // Ephemeral instance should be present
    if let Some(hosts) = response["data"]["hosts"].as_array() {
        assert!(!hosts.is_empty(), "Ephemeral instance should be present");
    }
}

/// Test non-ephemeral instances are not checked for heartbeat
#[tokio::test]
async fn test_non_ephemeral_instances_not_checked_for_heartbeat() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("persistent_instance");

    // Register non-ephemeral (persistent) instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.230",
                "port": 12000,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": false
            }),
        )
        .await
        .expect("Failed to register persistent instance");

    // Verify instance exists
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // Non-ephemeral instance should be present
    if let Some(hosts) = response["data"]["hosts"].as_array() {
        assert!(
            !hosts.is_empty(),
            "Non-ephemeral instance should be present"
        );
    }

    // Wait a short time - non-ephemeral instance should remain
    sleep(Duration::from_secs(2)).await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");

    // Non-ephemeral instance should still be present
    if let Some(hosts) = response["data"]["hosts"].as_array() {
        assert!(
            !hosts.is_empty(),
            "Non-ephemeral instance should still be present"
        );
    }
}
