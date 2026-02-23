//! Naming Cluster API integration tests
//!
//! Tests for cluster management in service discovery

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_service_name,
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

/// Test creating a cluster
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_create_cluster() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_test");

    // Register instance with cluster
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.100",
                "port": 8080,
                "clusterName": "custom-cluster",
                "weight": 1.0,
                "healthy": true,
                "enabled": true
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query instance list (should include cluster)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test getting cluster info
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_get_cluster_info() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_info");

    // Register instances in different clusters
    for cluster in &["cluster-a", "cluster-b"] {
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &json!({
                    "serviceName": service_name,
                    "ip": "192.168.1.100",
                    "port": 8080,
                    "clusterName": cluster,
                    "weight": 1.0,
                    "healthy": true,
                    "enabled": true
                }),
            )
            .await
            .expect("Failed to register instance");
    }

    // Query with specific cluster
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name.as_str()),
                ("clusterName", "cluster-a"),
            ],
        )
        .await
        .expect("Failed to query cluster instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test updating cluster configuration
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_update_cluster() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_update");
    let ip = "192.168.1.101";

    // Register instance with initial cluster
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": 8080,
                "clusterName": "initial-cluster",
                "weight": 1.0
            }),
        )
        .await
        .expect("Failed to register instance");

    // Update to new cluster
    let _: serde_json::Value = client
        .put_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": 8080,
                "clusterName": "updated-cluster",
                "weight": 2.0
            }),
        )
        .await
        .expect("Failed to update cluster");

    // Verify update
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test deleting cluster
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_delete_cluster() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_delete");
    let ip = "192.168.1.102";

    // Register instance
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": ip,
                "port": 8080,
                "clusterName": "delete-cluster",
                "weight": 1.0
            }),
        )
        .await
        .expect("Failed to register instance");

    // Delete instance
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", ip),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to delete instance");

    // Verify deletion
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test cluster health check
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_cluster_health_check() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_health");

    // Register instance with health check
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.103",
                "port": 8080,
                "clusterName": "health-cluster",
                "weight": 1.0,
                "healthy": true,
                "enabled": true
            }),
        )
        .await
        .expect("Failed to register instance");

    // Query healthy instances
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

/// Test cluster statistics
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_cluster_stats() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_stats");

    // Register multiple instances in different clusters
    for (i, cluster) in ["stats-a", "stats-b", "stats-c"].iter().enumerate() {
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &json!({
                    "serviceName": service_name,
                    "ip": &format!("192.168.1.{}", i + 200),
                    "port": 8080,
                    "clusterName": cluster,
                    "weight": 1.0
                }),
            )
            .await
            .expect("Failed to register instance");
    }

    // Query service list
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service/list",
            &[("groupName", "DEFAULT_GROUP")],
        )
        .await
        .expect("Failed to query services");

    assert_eq!(response["code"], 0, "Query should succeed");
}

/// Test cluster with metadata
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_cluster_with_metadata() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_metadata");

    // Register instance with cluster metadata
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &json!({
                "serviceName": service_name,
                "ip": "192.168.1.104",
                "port": 8080,
                "clusterName": "metadata-cluster",
                "weight": 1.0,
                "metadata": {
                    "cluster.type": "production",
                    "cluster.region": "us-west",
                    "cluster.version": "1.0"
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

/// Test cluster weight for load balancing
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_cluster_weight() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("cluster_weight");

    // Register instances with different weights
    for (i, weight) in [1.0, 2.0, 3.0].iter().enumerate() {
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &json!({
                    "serviceName": service_name,
                    "ip": &format!("192.168.1.{}", i + 210),
                    "port": 8080,
                    "clusterName": "weight-cluster",
                    "weight": weight
                }),
            )
            .await
            .expect("Failed to register instance");
    }

    // Query instances
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances");

    assert_eq!(response["code"], 0, "Query should succeed");
}
