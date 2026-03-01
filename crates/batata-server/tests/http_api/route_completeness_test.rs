//! Route completeness integration tests
//!
//! Verifies that all documented API routes are accessible and return correct
//! status codes. These tests ensure no routes are accidentally dropped during
//! refactoring or crate migration.
//!
//! Route categories tested:
//! - V2 Open API dual-path aliases (/v2/ns/ops/*)
//! - V2 Core Ops (/v2/core/ops/*)
//! - V2 Console Health (/v2/console/health/*)
//! - V2 Health base path (PUT /v2/ns/health)
//! - V3 Admin Config Listener (/v3/admin/cs/listener)
//! - V3 Admin Naming Ops (/v3/admin/ns/ops/*)
//! - V3 Admin Service Cluster (/v3/admin/ns/service/cluster)

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_service_name,
};

async fn main_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

async fn console_client() -> TestClient {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

// ========== V2 Naming Health Base Path ==========

/// Test PUT /v2/ns/health (base path) - should work same as /v2/ns/health/instance
/// Nacos maps both "" and "/instance" to the same handler.
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_health_base_path() {
    let client = main_client().await;
    let service_name = unique_service_name("health_base");

    // Register an instance first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.1"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register instance");

    // PUT /v2/ns/health (base path, no /instance suffix)
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/health",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.1"),
                ("port", "8080"),
                ("healthy", "true"),
            ],
        )
        .await
        .expect("PUT /v2/ns/health base path should be accessible");

    assert_eq!(response["code"], 0, "Base health path should succeed");
}

/// Test PUT /v2/ns/health/instance - the alternative path
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_health_instance_path() {
    let client = main_client().await;
    let service_name = unique_service_name("health_instance");

    // Register an instance first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.2"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register instance");

    // PUT /v2/ns/health/instance
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/health/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.2"),
                ("port", "8080"),
                ("healthy", "true"),
            ],
        )
        .await
        .expect("PUT /v2/ns/health/instance should be accessible");

    assert_eq!(response["code"], 0, "Instance health path should succeed");
}

// ========== V2 Naming Operator Dual Paths ==========

/// Test GET /v2/ns/ops/switches (dual path alias of /v2/ns/operator/switches)
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_ops_switches_get() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v2/ns/ops/switches")
        .await
        .expect("GET /v2/ns/ops/switches should be accessible");

    assert_eq!(response["code"], 0, "ops/switches GET should succeed");
}

/// Test GET /v2/ns/operator/switches (original path)
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_operator_switches_get() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v2/ns/operator/switches")
        .await
        .expect("GET /v2/ns/operator/switches should be accessible");

    assert_eq!(response["code"], 0, "operator/switches GET should succeed");
}

/// Test GET /v2/ns/ops/metrics (dual path alias of /v2/ns/operator/metrics)
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_ops_metrics_get() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v2/ns/ops/metrics")
        .await
        .expect("GET /v2/ns/ops/metrics should be accessible");

    assert_eq!(response["code"], 0, "ops/metrics GET should succeed");
}

// ========== V2 Core Ops ==========

/// Test GET /v2/core/ops/ids
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_core_ops_ids() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v2/core/ops/ids")
        .await
        .expect("GET /v2/core/ops/ids should be accessible");

    assert_eq!(response["code"], 0, "core/ops/ids should succeed");
}

/// Test PUT /v2/core/ops/log
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_core_ops_log() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/core/ops/log",
            &[("logName", "root"), ("logLevel", "INFO")],
        )
        .await
        .expect("PUT /v2/core/ops/log should be accessible");

    assert_eq!(response["code"], 0, "core/ops/log should succeed");
}

// ========== V2 Console Health ==========

/// Test GET /v2/console/health/liveness
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_console_health_liveness() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v2/console/health/liveness")
        .await
        .expect("GET /v2/console/health/liveness should be accessible");

    // Liveness check returns a result (may or may not have code field)
    assert!(response.is_object(), "Liveness should return a JSON object");
}

/// Test GET /v2/console/health/readiness
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_console_health_readiness() {
    let client = console_client().await;

    let response: serde_json::Value = client
        .get("/v2/console/health/readiness")
        .await
        .expect("GET /v2/console/health/readiness should be accessible");

    assert!(
        response.is_object(),
        "Readiness should return a JSON object"
    );
}

// ========== V3 Admin Config Listener by IP ==========

/// Test GET /v3/admin/cs/listener (base path)
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_listener_base() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/listener",
            &[("dataId", "test"), ("groupName", DEFAULT_GROUP)],
        )
        .await
        .expect("GET /v3/admin/cs/listener should be accessible");

    assert_eq!(response["code"], 0, "Admin listener base should succeed");
    assert!(
        response["data"].is_object(),
        "Should return listener info object"
    );
}

/// Test GET /v3/admin/cs/listener/ip
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_listener_by_ip() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get_with_query("/nacos/v3/admin/cs/listener/ip", &[("ip", "127.0.0.1")])
        .await
        .expect("GET /v3/admin/cs/listener/ip should be accessible");

    assert_eq!(response["code"], 0, "Admin listener by IP should succeed");
    assert!(
        response["data"].is_object(),
        "Should return listener info object"
    );
}

/// Test GET /v3/admin/cs/listener/ip with all=true
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_listener_by_ip_all() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/listener/ip",
            &[("ip", "127.0.0.1"), ("all", "true")],
        )
        .await
        .expect("GET /v3/admin/cs/listener/ip?all=true should be accessible");

    assert_eq!(
        response["code"], 0,
        "Listener by IP with all should succeed"
    );
}

/// Test GET /v3/admin/cs/listener/ip with namespace filter
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_listener_by_ip_with_namespace() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/listener/ip",
            &[("ip", "127.0.0.1"), ("namespaceId", "public")],
        )
        .await
        .expect("GET /v3/admin/cs/listener/ip?namespaceId should be accessible");

    assert_eq!(
        response["code"], 0,
        "Listener by IP with namespace should succeed"
    );
}

// ========== V2 History Detail ==========

/// Test GET /v2/cs/history/detail
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v2_history_detail() {
    let client = main_client().await;

    // This may return empty/error for non-existent history, but the route should exist
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/history/detail",
            &[
                ("dataId", "nonexistent"),
                ("group", DEFAULT_GROUP),
                ("nid", "1"),
            ],
        )
        .await
        .expect("GET /v2/cs/history/detail should be accessible");

    // Route exists - the response code indicates the endpoint is registered
    assert!(
        response.get("code").is_some(),
        "Should return a valid API response"
    );
}

// ========== V3 Admin Naming Ops ==========

/// Test GET /v3/admin/ns/ops/switches
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_ops_switches() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/ops/switches")
        .await
        .expect("GET /v3/admin/ns/ops/switches should be accessible");

    assert_eq!(
        response["code"], 0,
        "Admin naming ops switches should succeed"
    );
}

/// Test GET /v3/admin/ns/ops/metrics
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_ops_metrics() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/ops/metrics")
        .await
        .expect("GET /v3/admin/ns/ops/metrics should be accessible");

    assert_eq!(
        response["code"], 0,
        "Admin naming ops metrics should succeed"
    );
}

// ========== V3 Admin Config Metrics ==========

/// Test GET /v3/admin/cs/metrics/cluster
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_metrics_cluster() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/cs/metrics/cluster")
        .await
        .expect("GET /v3/admin/cs/metrics/cluster should be accessible");

    assert_eq!(
        response["code"], 0,
        "Admin config metrics cluster should succeed"
    );
}

/// Test GET /v3/admin/cs/metrics/ip
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_metrics_ip() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/cs/metrics/ip")
        .await
        .expect("GET /v3/admin/cs/metrics/ip should be accessible");

    assert_eq!(
        response["code"], 0,
        "Admin config metrics ip should succeed"
    );
}

// ========== V3 Admin Config Capacity ==========

/// Test GET /v3/admin/cs/capacity
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_capacity_get() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/cs/capacity")
        .await
        .expect("GET /v3/admin/cs/capacity should be accessible");

    assert!(
        response.get("code").is_some(),
        "Should return a valid API response"
    );
}

// ========== V3 Admin Config Ops ==========

/// Test PUT /v3/admin/cs/ops/log
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_config_ops_log() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .put_form(
            "/nacos/v3/admin/cs/ops/log",
            &[("logName", "root"), ("logLevel", "INFO")],
        )
        .await
        .expect("PUT /v3/admin/cs/ops/log should be accessible");

    assert_eq!(response["code"], 0, "Admin config ops log should succeed");
}

// ========== V3 Admin Service Cluster ==========

/// Test PUT /v3/admin/ns/service/cluster
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_service_cluster_update() {
    let client = main_client().await;
    let service_name = unique_service_name("svc_cluster");

    // Create a service first
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/service",
            &serde_json::json!({
                "serviceName": service_name,
                "groupName": DEFAULT_GROUP,
            }),
        )
        .await
        .expect("Failed to create service");

    // Update cluster via service/cluster path
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v3/admin/ns/service/cluster",
            &serde_json::json!({
                "serviceName": service_name,
                "groupName": DEFAULT_GROUP,
                "clusterName": "DEFAULT",
                "healthChecker": {
                    "type": "TCP"
                }
            }),
        )
        .await
        .expect("PUT /v3/admin/ns/service/cluster should be accessible");

    assert_eq!(
        response["code"], 0,
        "Admin service cluster update should succeed"
    );
}

// ========== V3 Admin Naming Health ==========

/// Test PUT /v3/admin/ns/health/instance
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_health_instance() {
    let client = main_client().await;
    let service_name = unique_service_name("health_admin");

    // Register an instance first
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "10.0.0.10",
                "port": 8080,
            }),
        )
        .await
        .expect("Failed to register instance");

    // Update health via admin API
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v3/admin/ns/health/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "10.0.0.10"),
                ("port", "8080"),
                ("healthy", "true"),
            ],
        )
        .await
        .expect("PUT /v3/admin/ns/health/instance should be accessible");

    assert!(
        response.get("code").is_some(),
        "Should return a valid API response"
    );
}

/// Test GET /v3/admin/ns/health/checkers
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_health_checkers() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/health/checkers")
        .await
        .expect("GET /v3/admin/ns/health/checkers should be accessible");

    assert_eq!(response["code"], 0, "Admin health checkers should succeed");
}

// ========== V3 Admin Config History ==========

/// Test GET /v3/admin/cs/history/previous
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_history_previous() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get_with_query("/nacos/v3/admin/cs/history/previous", &[("id", "1")])
        .await
        .expect("GET /v3/admin/cs/history/previous should be accessible");

    // Route exists - may return error for non-existent history
    assert!(
        response.get("code").is_some(),
        "Should return a valid API response"
    );
}

/// Test GET /v3/admin/cs/history/configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_v3_admin_history_configs() {
    let client = main_client().await;

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/history/configs",
            &[("namespaceId", "public")],
        )
        .await
        .expect("GET /v3/admin/cs/history/configs should be accessible");

    assert_eq!(response["code"], 0, "Admin history configs should succeed");
}
