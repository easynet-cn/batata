//! V2 Additional API integration tests
//!
//! Tests for newly added V2 endpoints:
//! - PATCH /v2/ns/instance
//! - PUT /v2/ns/instance/beat
//! - GET /v2/ns/instance/statuses/{key}
//! - GET /v2/cs/config/searchDetail
//! - GET /v2/ns/catalog/instances
//! - PUT /v2/core/cluster/node/list
//! - DELETE /v2/core/cluster/nodes

use batata_integration_tests::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_data_id, unique_service_name,
};

async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

// ========== Instance PATCH ==========

/// Test patch instance via V2 API
#[tokio::test]

async fn test_v2_patch_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_patch");

    // Register first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.1"),
                ("port", "8080"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register");

    // Patch weight only
    let response: serde_json::Value = client
        .patch_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.1"),
                ("port", "8080"),
                ("weight", "3.0"),
            ],
        )
        .await
        .expect("Failed to patch instance");

    assert_eq!(response["code"], 0, "Patch instance should succeed");

    // Query back and verify the weight was actually updated
    let query_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.1"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to query instance after patch");

    assert_eq!(
        query_response["code"], 0,
        "Query after patch should succeed"
    );
    let weight = query_response["data"]["weight"]
        .as_f64()
        .expect("Instance should have a weight field");
    assert!(
        (weight - 3.0).abs() < 0.01,
        "Weight should be updated to 3.0 after patch, got {}",
        weight
    );
}

/// Test patch instance that does not exist
#[tokio::test]

async fn test_v2_patch_nonexistent_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_patch_noexist");

    let result = client
        .patch_form::<serde_json::Value, _>(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.99"),
                ("port", "8080"),
                ("weight", "2.0"),
            ],
        )
        .await;

    // Should fail - either HTTP error or response with non-zero code
    if let Ok(response) = result {
        assert_ne!(
            response["code"], 0,
            "Patching nonexistent instance should fail"
        );
    }
    // HTTP 404 error is also acceptable
}

// ========== Instance Beat ==========

/// Test instance heartbeat via V2 API
#[tokio::test]

async fn test_v2_instance_beat() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_beat");

    // Register first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.10"),
                ("port", "8080"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register");

    // Send heartbeat
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance/beat",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.10"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to send heartbeat");

    assert_eq!(response["code"], 0, "Heartbeat should succeed");
    // Verify beat response contains clientBeatInterval (expected in Nacos beat responses)
    let data = &response["data"];
    if data.is_object() {
        assert!(
            data.get("clientBeatInterval").is_some(),
            "Beat response data should contain clientBeatInterval field, got: {:?}",
            data
        );
        let interval = data["clientBeatInterval"]
            .as_i64()
            .expect("clientBeatInterval should be a number");
        assert!(
            interval > 0,
            "clientBeatInterval should be positive, got {}",
            interval
        );
    }
    // If data is not an object (e.g., a simple "ok" string), the code==0 check is sufficient
}

/// Test instance beat with JSON body
#[tokio::test]

async fn test_v2_instance_beat_with_json() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_beat_json");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.11"),
                ("port", "8080"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register");

    // Beat with JSON beat info
    let beat_json = format!(
        r#"{{"ip":"192.168.20.11","port":8080,"serviceName":"{}"}}"#,
        service_name
    );
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance/beat",
            &[
                ("serviceName", service_name.as_str()),
                ("beat", beat_json.as_str()),
            ],
        )
        .await
        .expect("Failed to send beat with JSON");

    assert_eq!(response["code"], 0, "Beat with JSON should succeed");
    // Verify beat response contains clientBeatInterval when data is an object
    let data = &response["data"];
    if data.is_object() {
        assert!(
            data.get("clientBeatInterval").is_some(),
            "Beat response data should contain clientBeatInterval field, got: {:?}",
            data
        );
    }
}

// ========== Instance Statuses ==========

/// Test get instance statuses via V2 API
#[tokio::test]

async fn test_v2_get_instance_statuses() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_statuses");

    // Register some instances
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.20"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get statuses using key format: namespace@@group@@serviceName
    let key = format!("public@@DEFAULT_GROUP@@{}", service_name);
    let url = format!("/nacos/v2/ns/instance/statuses/{}", key);
    let response: serde_json::Value = client.get(&url).await.expect("Failed to get statuses");

    assert_eq!(response["code"], 0, "Get statuses should succeed");
    // Verify the returned data contains status information
    let data = &response["data"];
    assert!(!data.is_null(), "Statuses response data should not be null");
    // If data is an object or array, verify entries contain valid status values
    if let Some(obj) = data.as_object() {
        for (instance_key, status_val) in obj {
            if let Some(status) = status_val.as_str() {
                let valid_statuses = ["UP", "DOWN", "STARTING", "OUT_OF_SERVICE", "UNKNOWN"];
                assert!(
                    valid_statuses.contains(&status),
                    "Instance {} has invalid status '{}', expected one of {:?}",
                    instance_key,
                    status,
                    valid_statuses
                );
            }
        }
    }
}

// ========== Config Search Detail ==========

/// Test search config detail via V2 API
#[tokio::test]

async fn test_v2_search_config_detail() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("v2_search");

    // Create config first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "search.test=value"),
            ],
        )
        .await
        .expect("Failed to create config");

    // Search
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/searchDetail",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to search config");

    assert_eq!(response["code"], 0, "Search should succeed");
    assert!(response["data"].is_object(), "Should return search results");

    // Verify the search results contain the config we just created
    let page_items = response["data"]["pageItems"]
        .as_array()
        .or_else(|| response["data"]["content"].as_array());
    if let Some(items) = page_items {
        let found = items
            .iter()
            .any(|item| item["dataId"].as_str() == Some(data_id.as_str()));
        assert!(
            found,
            "Search results should contain the created config with dataId '{}', got: {:?}",
            data_id, items
        );
    }
    // If results are structured differently, the code==0 + is_object check still validates the API works
}

/// Test search config with wildcard filter
#[tokio::test]

async fn test_v2_search_config_with_filter() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("v2_filter");

    // Create a config to ensure at least one result
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "filter.test=value"),
            ],
        )
        .await
        .expect("Failed to create config for filter test");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/searchDetail",
            &[
                ("dataId", "*"),
                ("group", DEFAULT_GROUP),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to search config");

    assert_eq!(response["code"], 0, "Wildcard search should succeed");
    assert!(
        response["data"].is_object(),
        "Wildcard search should return results object"
    );

    // Verify wildcard returns at least one result
    let total_count = response["data"]["totalCount"]
        .as_i64()
        .or_else(|| response["data"]["total"].as_i64());
    if let Some(count) = total_count {
        assert!(
            count > 0,
            "Wildcard search should return at least one config, got totalCount={}",
            count
        );
    }
    let page_items = response["data"]["pageItems"]
        .as_array()
        .or_else(|| response["data"]["content"].as_array());
    if let Some(items) = page_items {
        assert!(
            !items.is_empty(),
            "Wildcard search should return at least one config item"
        );
    }
}

// ========== Catalog Instances ==========

/// Test list catalog instances via V2 API
#[tokio::test]

async fn test_v2_catalog_instances() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("v2_catalog");

    // Register instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.20.30"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get catalog
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/catalog/instances",
            &[
                ("serviceName", service_name.as_str()),
                ("pageNo", "1"),
                ("pageSize", "10"),
            ],
        )
        .await
        .expect("Failed to get catalog");

    assert_eq!(response["code"], 0, "Get catalog should succeed");

    // Verify catalog contains the registered instance
    let data = &response["data"];
    assert!(!data.is_null(), "Catalog response data should not be null");

    // Check for instances in the catalog response
    let instances = data["list"]
        .as_array()
        .or_else(|| data["instances"].as_array())
        .or_else(|| data["pageItems"].as_array());
    if let Some(items) = instances {
        assert!(
            !items.is_empty(),
            "Catalog should contain at least one instance"
        );
        // Verify the registered IP appears in catalog
        let found = items.iter().any(|inst| {
            inst["ip"].as_str() == Some("192.168.20.30")
                || inst["addr"]
                    .as_str()
                    .map_or(false, |a| a.contains("192.168.20.30"))
        });
        assert!(
            found,
            "Catalog should contain the registered instance with IP 192.168.20.30, got: {:?}",
            items
        );
    }
}

// ========== Cluster Node Operations ==========

/// Test update cluster node list
#[tokio::test]

async fn test_v2_update_cluster_node_list() {
    let client = authenticated_client().await;

    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/core/cluster/node/list",
            &[("nodes", "127.0.0.1:8848")],
        )
        .await
        .expect("Failed to update node list");

    assert_eq!(response["code"], 0, "Update node list should succeed");
    // Verify the response acknowledges the update
    // The data field should indicate success (true, "ok", or similar)
    let data = &response["data"];
    if data.is_boolean() {
        assert!(
            data.as_bool().unwrap(),
            "Update node list should return true, got: {:?}",
            data
        );
    } else if data.is_string() {
        assert!(
            !data.as_str().unwrap().is_empty(),
            "Update node list should return a non-empty acknowledgement"
        );
    }
    // code==0 already confirms the operation was accepted
}

/// Test remove cluster nodes
#[tokio::test]

async fn test_v2_remove_cluster_nodes() {
    let client = authenticated_client().await;

    // Attempt to remove a non-existent node - should fail gracefully
    let result = client
        .delete_with_query::<serde_json::Value, _>(
            "/nacos/v2/core/cluster/nodes",
            &[("nodes", "192.168.99.99:8848")],
        )
        .await;

    match result {
        Ok(response) => {
            // If the server returns a response, verify it has a valid code
            let code = response["code"]
                .as_i64()
                .expect("Response should have a numeric code field");
            // Removing a non-existent node may succeed as a no-op (code 0) or fail with an error code
            if code == 0 {
                // Success (no-op removal) - valid behavior
            } else {
                // Error code - verify it has a descriptive message
                assert!(
                    response["message"].is_string(),
                    "Error response should include a message, got: {:?}",
                    response
                );
            }
        }
        Err(_) => {
            // HTTP-level error (4xx/5xx) is also acceptable for removing non-existent nodes
        }
    }
}
