//! Naming Health Check API integration tests
//!
//! Tests for instance health check functionality

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_service_name,
};
use serde_json::json;

/// Create an authenticated test client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test TCP health check registration
#[tokio::test]

async fn test_tcp_health_check_registration() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("tcp_health");

    // Register instance with TCP health check
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.150",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "metadata": {
                    "healthCheckType": "TCP",
                    "healthCheckInterval": "5000",
                    "healthCheckTimeout": "3000"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test HTTP health check registration
#[tokio::test]

async fn test_http_health_check_registration() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("http_health");

    // Register instance with HTTP health check
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.151",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "metadata": {
                    "healthCheckType": "HTTP",
                    "healthCheckPath": "/health",
                    "healthCheckInterval": "5000",
                    "healthCheckTimeout": "3000",
                    "healthCheckExpectedCodes": "200"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test updating health check configuration
#[tokio::test]

async fn test_update_health_check_config() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("update_health");
    let ip = "192.168.1.152";

    // Register instance with initial health check
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "metadata": {
                    "healthCheckInterval": "5000"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Update health check config
    let _: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": 8080,
                "metadata": {
                    "healthCheckInterval": "10000",
                    "healthCheckTimeout": "5000"
                }
            }),
        )
        .await
        .expect("Failed to update health check");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test getting instance health status
#[tokio::test]

async fn test_get_instance_health_status() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("health_status");

    // Register healthy instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.153",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query all instances
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test getting only healthy instances
#[tokio::test]

async fn test_get_healthy_instances_only() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("healthy_only");

    // Register healthy instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.154",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": true,
                "enabled": true
            }),
        )
        .await
        .expect("Failed to register healthy instance");

    // Register unhealthy instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.155",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "healthy": false,
                "enabled": true
            }),
        )
        .await
        .expect("Failed to register unhealthy instance");

    // Query only healthy instances
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name.as_str()),
                ("healthyOnly", "true"),
            ],
        )
        .await
        .expect("Failed to query healthy instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test custom health check interval
#[tokio::test]

async fn test_custom_health_check_interval() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("custom_interval");

    // Register instance with custom interval
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.156",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "metadata": {
                    "healthCheckInterval": "10000"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test custom health check timeout
#[tokio::test]

async fn test_custom_health_check_timeout() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("custom_timeout");

    // Register instance with custom timeout
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.157",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "metadata": {
                    "healthCheckTimeout": "10000"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test health threshold configuration
#[tokio::test]

async fn test_health_threshold_config() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("health_threshold");

    // Register instance with health threshold
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.158",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "metadata": {
                    "healthCheckThreshold": "3"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test disabling health check
#[tokio::test]

async fn test_disable_health_check() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("disable_health");

    // Register instance with health check disabled
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.159",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "ephemeral": false,
                "metadata": {
                    "healthCheckEnabled": "false"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test HTTP health check with custom path
#[tokio::test]

async fn test_http_health_check_custom_path() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("http_custom_path");

    // Register instance with custom health check path
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.160",
                "port": 8080,
                "clusterName": DEFAULT_GROUP,
                "weight": 1.0,
                "metadata": {
                    "healthCheckType": "HTTP",
                    "healthCheckPath": "/api/health/custom",
                    "healthCheckExpectedCodes": "200,204"
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test batch health check updates
#[tokio::test]

async fn test_batch_health_check_updates() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("batch_health");

    // Register multiple instances
    for i in 0..3 {
        let ip = format!("192.168.1.{}", 170 + i);
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &json!({
                    "serviceName": service_name,
                    "ip": ip,
                    "port": 8080,
                    "clusterName": DEFAULT_GROUP,
                    "weight": 1.0,
                    "metadata": {
                        "healthCheckInterval": "5000"
                    }
                }),
            )
            .await
            .expect("Failed to register instance");
    }

    // Query all instances
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}
