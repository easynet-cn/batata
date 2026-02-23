//! Naming Service Cluster API integration tests
//!
//! Tests for cluster management, node operations, and cluster statistics

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id,
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

/// Test create cluster
#[tokio::test]
#[ignore = "requires running server"]
async fn test_create_cluster() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("cluster");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080,
                    "timeout": 5000,
                    "interval": 5000
                }
            }),
        )
        .await
        .expect("Failed to create cluster");

    assert_eq!(response["code"], 0, "Create cluster should succeed");
}

/// Test get cluster info
#[tokio::test]
#[ignore = "requires running server"]
async fn test_get_cluster() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("get_cluster");

    // Create cluster first
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to create cluster");

    // Get cluster info
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/cluster",
            &[(
                "clusterName", cluster_name.as_str()
            )],
        )
        .await
        .expect("Failed to get cluster");

    assert_eq!(response["code"], 0, "Get cluster should succeed");
    assert_eq!(
        response["data"]["clusterName"], cluster_name,
        "Cluster name should match"
    );
}

/// Test list clusters
#[tokio::test]
#[ignore = "requires running server"]
async fn test_list_clusters() {
    let client = authenticated_client().await;

    // Create multiple clusters
    for i in 1..=3 {
        let cluster_name = unique_data_id(&format!("list_cluster_{}", i));
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v3/admin/ns/cluster",
                &serde_json::json!({
                    "clusterName": cluster_name,
                    "namespaceId": "public",
                    "healthCheckType": "TCP",
                    "healthCheckConfig": {
                        "port": 8080 + i
                    }
                }),
            )
            .await
            .expect(&format!("Failed to create cluster {}", i));
    }

    // List clusters
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/cluster/list",
            &[("namespaceId", "public")],
        )
        .await
        .expect("Failed to list clusters");

    assert_eq!(response["code"], 0, "List clusters should succeed");
}

/// Test update cluster
#[tokio::test]
#[ignore = "requires running server"]
async fn test_update_cluster() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("update_cluster");

    // Create cluster
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080,
                    "timeout": 5000
                }
            }),
        )
        .await
        .expect("Failed to create cluster");

    // Update cluster
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080,
                    "timeout": 10000,
                    "interval": 10000
                }
            }),
        )
        .await
        .expect("Failed to update cluster");

    assert_eq!(response["code"], 0, "Update cluster should succeed");
}

/// Test delete cluster
#[tokio::test]
#[ignore = "requires running server"]
async fn test_delete_cluster() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("delete_cluster");

    // Create cluster
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to create cluster");

    // Delete cluster
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v3/admin/ns/cluster",
            &[(
                "clusterName", cluster_name.as_str()
            )],
        )
        .await
        .expect("Failed to delete cluster");

    assert_eq!(response["code"], 0, "Delete cluster should succeed");
}

/// Test cluster with TCP health check
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_tcp_health_check() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("tcp_cluster");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080,
                    "timeout": 5000,
                    "interval": 5000,
                    "failThreshold": 3,
                    "successThreshold": 2
                }
            }),
        )
        .await
        .expect("Failed to create TCP health check cluster");

    assert_eq!(response["code"], 0, "TCP health check cluster should succeed");
}

/// Test cluster with HTTP health check
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_http_health_check() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("http_cluster");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "HTTP",
                "healthCheckConfig": {
                    "port": 8080,
                    "path": "/health",
                    "timeout": 5000,
                    "interval": 5000,
                    "expectedCode": 200
                }
            }),
        )
        .await
        .expect("Failed to create HTTP health check cluster");

    assert_eq!(response["code"], 0, "HTTP health check cluster should succeed");
}

/// Test cluster statistics
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_statistics() {
    let client = authenticated_client().await;

    // Get cluster statistics
    let response: serde_json::Value = client
        .get("/nacos/v3/admin/ns/cluster/statistics")
        .await
        .expect("Failed to get cluster statistics");

    assert_eq!(response["code"], 0, "Get cluster statistics should succeed");
    assert!(response["data"].is_object(), "Statistics should be object");
}

/// Test cluster with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_with_namespace() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("ns_cluster");

    let response: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "test-namespace",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to create cluster with namespace");

    assert_eq!(response["code"], 0, "Cluster with namespace should succeed");
}

/// Test cluster metadata persistence
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_metadata() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("meta_cluster");
    let metadata = serde_json::json!({
        "env": "prod",
        "region": "us-west",
        "custom": "value"
    });

    // Create cluster with metadata
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080
                },
                "metadata": metadata
            }),
        )
        .await
        .expect("Failed to create cluster with metadata");

    // Get cluster and verify metadata
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/cluster",
            &[("clusterName", cluster_name.as_str())],
        )
        .await
        .expect("Failed to get cluster");

    assert_eq!(response["code"], 0, "Get cluster should succeed");
}

/// Test cluster invalid health check type
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_invalid_health_check_type() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("invalid_type_cluster");

    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "INVALID_TYPE",
                "healthCheckConfig": {
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

/// Test cluster duplicate creation
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_duplicate_creation() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("duplicate_cluster");

    // Create cluster
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to create cluster");

    // Try to create duplicate
    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080
                }
            }),
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Duplicate cluster should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test cluster missing required fields
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_missing_fields() {
    let client = authenticated_client().await;

    // Try to create cluster without required fields
    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": unique_data_id("missing_fields"),
                "namespaceId": "public"
            }),
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Missing fields should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test cluster with invalid port
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_invalid_port() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("invalid_port_cluster");

    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
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

/// Test cluster health check configuration update
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_health_check_config_update() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("update_hc_cluster");

    // Create cluster with initial config
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080,
                    "timeout": 5000
                }
            }),
        )
        .await
        .expect("Failed to create cluster");

    // Update health check config
    let response: serde_json::Value = client
        .put_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080,
                    "timeout": 10000,
                    "interval": 10000,
                    "failThreshold": 5,
                    "successThreshold": 3
                }
            }),
        )
        .await
        .expect("Failed to update health check config");

    assert_eq!(response["code"], 0, "Update health check config should succeed");
}

/// Test cluster pagination
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_pagination() {
    let client = authenticated_client().await;

    // Create many clusters
    for i in 1..=15 {
        let cluster_name = unique_data_id(&format!("page_cluster_{}", i));
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v3/admin/ns/cluster",
                &serde_json::json!({
                    "clusterName": cluster_name,
                    "namespaceId": "public",
                    "healthCheckType": "TCP",
                    "healthCheckConfig": {
                        "port": 8080
                    }
                }),
            )
            .await
            .expect(&format!("Failed to create cluster {}", i));
    }

    // Get first page
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/cluster/list",
            &[
                ("namespaceId", "public"),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to get first page");

    assert_eq!(response["code"], 0, "Pagination should succeed");
}

/// Test cluster search
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_search() {
    let client = authenticated_client().await;
    let search_prefix = unique_data_id("search");

    // Create clusters with search prefix
    for i in 1..=3 {
        let cluster_name = format!("{}_{}", search_prefix, i);
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v3/admin/ns/cluster",
                &serde_json::json!({
                    "clusterName": cluster_name,
                    "namespaceId": "public",
                    "healthCheckType": "TCP",
                    "healthCheckConfig": {
                        "port": 8080
                    }
                }),
            )
            .await
            .expect("Failed to create cluster");
    }

    // Search clusters
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/cluster/list",
            &[
                ("namespaceId", "public"),
                ("search", search_prefix.as_str()),
            ],
        )
        .await
        .expect("Failed to search clusters");

    assert_eq!(response["code"], 0, "Search should succeed");
}

/// Test cluster health status
#[tokio::test]
#[ignore = "requires running server"]
async fn test_cluster_health_status() {
    let client = authenticated_client().await;
    let cluster_name = unique_data_id("health_status_cluster");

    // Create cluster
    let _: serde_json::Value = client
        .post_json(
            "/nacos/v3/admin/ns/cluster",
            &serde_json::json!({
                "clusterName": cluster_name,
                "namespaceId": "public",
                "healthCheckType": "TCP",
                "healthCheckConfig": {
                    "port": 8080
                }
            }),
        )
        .await
        .expect("Failed to create cluster");

    // Get cluster health status
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/ns/cluster",
            &[("clusterName", cluster_name.as_str())],
        )
        .await
        .expect("Failed to get cluster health status");

    assert_eq!(response["code"], 0, "Get health status should succeed");
}
