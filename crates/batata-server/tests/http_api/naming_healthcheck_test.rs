//! Naming Service Health Check API integration tests
//!
//! Tests for health check functionality (TCP/HTTP)

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id, unique_service_name,
};

/// Create an authenticated test client for the main API server
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test register instance with TCP health check
#[tokio::test]
#[ignore = "requires running server"]
async fn test_register_instance_tcp_health_check() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("tcp_hc");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.100",
                "port": 8080,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": true,
                "metadata": {},
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080,
                    "interval": 5000,
                    "timeout": 5000
                }
            }),
        )
        .await
        .expect("Failed to register instance with TCP health check");

    assert_eq!(response["code"], 0, "TCP health check registration should succeed");
}

/// Test register instance with HTTP health check
#[tokio::test]
#[ignore = "requires running server"]
async fn test_register_instance_http_health_check() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("http_hc");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.101",
                "port": 8080,
                "weight": 1.0,
                "healthy": true,
                "enabled": true,
                "ephemeral": true,
                "metadata": {},
                "healthCheck": {
                    "type": "HTTP",
                    "port": 8080,
                    "path": "/health",
                    "interval": 5000,
                    "timeout": 5000,
                    "expectedCode": 200
                }
            }),
        )
        .await
        .expect("Failed to register instance with HTTP health check");

    assert_eq!(response["code"], 0, "HTTP health check registration should succeed");
}

/// Test update instance health check config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_update_instance_health_check() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("update_hc");

    // Register instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.102",
                "port": 8080,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080,
                    "interval": 5000
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Update health check config
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.102",
                "port": 8080,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080,
                    "interval": 10000,
                    "timeout": 10000,
                    "failThreshold": 3,
                    "successThreshold": 2
                }
            }),
        )
        .await
        .expect("Failed to update health check config");

    assert_eq!(response["code"], 0, "Update health check should succeed");
}

/// Test get instance health status
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_instance_health_status() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("health_status");

    // Register instance with health check
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.103",
                "port": 8080,
                "healthy": true,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080,
                    "interval": 5000
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Get instance detail
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.103"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to get instance");

    assert_eq!(response["code"], 0, "Get instance should succeed");
    assert_eq!(response["data"]["healthy"], true, "Instance should be healthy");
}

/// Test get healthy instances only
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_healthy_instances_only() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("healthy_only");

    // Register healthy instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.110",
                "port": 8080,
                "healthy": true,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to register healthy instance");

    // Register unhealthy instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.111",
                "port": 8080,
                "healthy": false,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to register unhealthy instance");

    // Get healthy instances only
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name.as_str()),
                ("healthyOnly", "true"),
            ],
        )
        .await
        .expect("Failed to get healthy instances");

    assert_eq!(response["code"], 0, "Get healthy only should succeed");
}

/// Test health check with custom interval
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_custom_interval() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("custom_interval");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.120",
                "port": 8080,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080,
                    "interval": 10000,
                    "timeout": 5000
                }
            }),
        )
        .await
        .expect("Failed to register with custom interval");

    assert_eq!(response["code"], 0, "Custom interval should succeed");
}

/// Test health check with thresholds
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_thresholds() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("thresholds");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.130",
                "port": 8080,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080,
                    "interval": 5000,
                    "timeout": 5000,
                    "failThreshold": 3,
                    "successThreshold": 2
                }
            }),
        )
        .await
        .expect("Failed to register with thresholds");

    assert_eq!(response["code"], 0, "Thresholds should succeed");
}

/// Test disable health check
#[tokio::test]
#[ignore = "requires running server"]
async fn test_disable_health_check() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("disable_hc");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.140",
                "port": 8080,
                "healthy": true,
                "enabled": true,
                "ephemeral": false,
                "healthCheck": {
                    "enabled": false
                }
            }),
        )
        .await
        .expect("Failed to register without health check");

    assert_eq!(response["code"], 0, "Disable health check should succeed");
}

/// Test health check timeout
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_timeout() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("timeout_hc");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.150",
                "port": 8080,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080,
                    "timeout": 10000,
                    "interval": 5000
                }
            }),
        )
        .await
        .expect("Failed to register with custom timeout");

    assert_eq!(response["code"], 0, "Custom timeout should succeed");
}

/// Test HTTP health check with path
#[tokio::test]
#[ignore = "requires running server"]
async fn test_http_health_check_path() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("http_path");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.160",
                "port": 8080,
                "healthCheck": {
                    "type": "HTTP",
                    "port": 8080,
                    "path": "/api/health",
                    "interval": 5000,
                    "timeout": 5000,
                    "expectedCode": 200
                }
            }),
        )
        .await
        .expect("Failed to register with HTTP path");

    assert_eq!(response["code"], 0, "HTTP path health check should succeed");
}

/// Test HTTP health check with headers
#[tokio::test]
#[ignore = "requires running server"]
async fn test_http_health_check_headers() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("http_headers");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.170",
                "port": 8080,
                "healthCheck": {
                    "type": "HTTP",
                    "port": 8080,
                    "path": "/health",
                    "interval": 5000,
                    "timeout": 5000,
                    "expectedCode": 200,
                    "headers": {
                        "User-Agent": "Nacos-HealthCheck",
                        "X-Custom-Header": "test-value"
                    }
                }
            }),
        )
        .await
        .expect("Failed to register with HTTP headers");

    assert_eq!(response["code"], 0, "HTTP headers health check should succeed");
}

/// Test batch update health check
#[tokio::test]
#[ignore = "requires running server"]
async fn test_batch_update_health_check() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("batch_hc");

    // Register multiple instances
    for i in 0..=2 {
        let ip = format!("192.168.1.{}", 180 + i);
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &serde_json::json!({
                    "serviceName": service_name,
                    "ip": ip,
                    "port": 8080,
                    "healthCheck": {
                        "type": "TCP",
                        "port": 8080,
                        "interval": 5000
                    }
                }),
            )
            .await
            .expect("Failed to register instance");
    }

    // Batch update health check
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance/metadata/batch",
            &serde_json::json!({
                "serviceName": service_name,
                "instances": [
                    {"ip": "192.168.1.180", "port": 8080},
                    {"ip": "192.168.1.181", "port": 8080},
                    {"ip": "192.168.1.182", "port": 8080}
                ],
                "metadata": {
                    "healthCheckUpdated": "true"
                }
            }),
        )
        .await
        .expect("Failed to batch update health check");

    assert_eq!(response["code"], 0, "Batch update should succeed");
}

/// Test health check with non-default port
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_non_default_port() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("non_default_port");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.190",
                "port": 9090,
                "healthCheck": {
                    "type": "TCP",
                    "port": 9090,
                    "interval": 5000
                }
            }),
        )
        .await
        .expect("Failed to register with non-default port");

    assert_eq!(response["code"], 0, "Non-default port should succeed");
}

/// Test health check statistics
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_statistics() {
    let client = authenticated_client().await;

    // Get health check statistics
    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/health/statistics")
        .await
        .expect("Failed to get health check statistics");

    assert_eq!(response["code"], 0, "Get statistics should succeed");
    assert!(response["data"].is_object(), "Statistics should be object");
}

/// Test instance health status change
#[tokio::test]
#[ignore = "requires running server"]
async fn test_instance_health_status_change() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("status_change");

    // Register healthy instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.200",
                "port": 8080,
                "healthy": true,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to register instance");

    // Update to unhealthy
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.200",
                "port": 8080,
                "healthy": false,
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to update health status");

    assert_eq!(response["code"], 0, "Status change should succeed");

    // Verify status
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.200"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to get instance");

    assert_eq!(response["data"]["healthy"], false, "Status should be unhealthy");
}

/// Test health check with invalid type
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_invalid_type() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("invalid_type");

    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.210",
                "port": 8080,
                "healthCheck": {
                    "type": "INVALID_TYPE",
                    "port": 8080
                }
            }),
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Invalid type should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test health check with invalid port
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_invalid_port() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("invalid_port");

    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.220",
                "port": 8080,
                "healthCheck": {
                    "type": "TCP",
                    "port": 99999
                }
            }),
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Invalid port should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test health check heartbeat
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_heartbeat() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("heartbeat");

    let response: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance/beat",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.230",
                "port": 8080,
                "weight": 1.0,
                "ephemeral": true,
                "cluster": "DEFAULT"
            }),
        )
        .await
        .expect("Failed to send heartbeat");

    assert_eq!(response["code"], 0, "Heartbeat should succeed");
}

/// Test health check with custom metadata
#[tokio::test]
#[ignore = "requires running server"]
async fn test_health_check_metadata() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("hc_metadata");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.240",
                "port": 8080,
                "metadata": {
                    "healthCheckType": "TCP",
                    "customHealthCheck": "enabled"
                },
                "healthCheck": {
                    "type": "TCP",
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to register with metadata");

    assert_eq!(response["code"], 0, "Metadata health check should succeed");
}
