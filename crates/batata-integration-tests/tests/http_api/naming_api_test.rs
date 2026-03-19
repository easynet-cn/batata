//! Service Discovery (Naming) API integration tests
//!
//! Tests for /nacos/v2/ns/instance and /nacos/v2/ns/service endpoints

use batata_integration_tests::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient, unique_service_name,
};

async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test instance registration
#[tokio::test]

async fn test_register_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("register");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.100"),
                ("port", "8080"),
                ("weight", "1.0"),
                ("healthy", "true"),
                ("enabled", "true"),
                ("ephemeral", "true"),
            ],
        )
        .await
        .expect("Failed to register instance");

    assert_eq!(response["code"], 0, "Registration should succeed");

    // Verify the instance exists by querying the instance list
    let list_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get instance list after registration");

    assert_eq!(
        list_response["code"], 0,
        "Instance list query should succeed"
    );
    let hosts = list_response["data"]["hosts"]
        .as_array()
        .expect("hosts should be an array");
    assert!(
        !hosts.is_empty(),
        "Instance list should not be empty after registration"
    );

    let registered = hosts
        .iter()
        .find(|h| h["ip"] == "192.168.1.100" && h["port"] == 8080);
    assert!(
        registered.is_some(),
        "Registered instance with ip=192.168.1.100 and port=8080 should be in the list"
    );
}

/// Test instance deregistration
#[tokio::test]

async fn test_deregister_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("deregister");

    // First register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.101"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Then deregister
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.101"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to deregister");

    assert_eq!(response["code"], 0, "Deregistration should succeed");

    // Verify instance is gone by querying the instance list
    let list_result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await;

    match list_result {
        Ok(list_response) => {
            let empty: Vec<serde_json::Value> = vec![];
            let hosts = list_response["data"]["hosts"].as_array().unwrap_or(&empty);
            let found = hosts
                .iter()
                .any(|h| h["ip"] == "192.168.1.101" && h["port"] == 8080);
            assert!(
                !found,
                "Deregistered instance should no longer appear in the instance list"
            );
        }
        Err(_) => {
            // Service not found after deregistration is acceptable
        }
    }
}

/// Test update instance
#[tokio::test]

async fn test_update_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("update");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.102"),
                ("port", "8080"),
                ("weight", "1.0"),
            ],
        )
        .await
        .expect("Failed to register");

    // Update weight
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.102"),
                ("port", "8080"),
                ("weight", "2.0"),
                ("healthy", "true"),
                ("enabled", "true"),
            ],
        )
        .await
        .expect("Failed to update");

    assert_eq!(response["code"], 0, "Update should succeed");

    // Verify updated weight by querying the instance
    let detail: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.102"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to get instance detail after update");

    assert_eq!(detail["code"], 0, "Get instance detail should succeed");
    let weight = detail["data"]["weight"]
        .as_f64()
        .expect("weight should be a number");
    assert!(
        (weight - 2.0).abs() < f64::EPSILON,
        "Weight should be updated to 2.0, got {}",
        weight
    );
}

/// Test get instance detail
#[tokio::test]

async fn test_get_instance() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("getdetail");

    // Register
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.103"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get detail
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

    assert_eq!(response["code"], 0, "Get should succeed");
    assert!(response["data"].is_object(), "Should return instance data");

    let data = &response["data"];
    assert_eq!(data["ip"], "192.168.1.103", "Instance IP should match");
    assert_eq!(data["port"], 8080, "Instance port should match");
    assert!(
        data["weight"].is_number(),
        "Instance should have a weight field"
    );
    assert!(
        data["healthy"].is_boolean(),
        "Instance should have a healthy field"
    );
}

/// Test get instance list
#[tokio::test]

async fn test_get_instance_list() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("list");

    // Register multiple instances
    for i in 1..=3 {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/ns/instance",
                &[
                    ("serviceName", service_name.as_str()),
                    ("ip", &format!("192.168.1.{}", 110 + i)),
                    ("port", "8080"),
                ],
            )
            .await
            .expect("Failed to register");
    }

    // Get list
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get instance list");

    assert_eq!(response["code"], 0, "Get list should succeed");

    let hosts = response["data"]["hosts"]
        .as_array()
        .expect("hosts should be an array");
    assert!(
        hosts.len() >= 3,
        "Should have at least 3 hosts, got {}",
        hosts.len()
    );

    // Verify all registered IPs are present
    let ips: Vec<&str> = hosts.iter().filter_map(|h| h["ip"].as_str()).collect();
    for i in 1..=3 {
        let expected_ip = format!("192.168.1.{}", 110 + i);
        assert!(
            ips.contains(&expected_ip.as_str()),
            "Instance list should contain IP {}",
            expected_ip
        );
    }

    // Verify the service name in the response
    let response_name = response["data"]["name"].as_str().unwrap_or("");
    assert!(
        response_name.contains(&service_name) || !response_name.is_empty(),
        "Response should contain a service name"
    );
}

/// Test healthy only filter
#[tokio::test]

async fn test_instance_list_healthy_only() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("healthy");

    // Register a healthy instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.120"),
                ("port", "8080"),
                ("healthy", "true"),
            ],
        )
        .await
        .expect("Failed to register");

    // Get healthy only
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

    assert_eq!(response["code"], 0, "Get healthy should succeed");

    let hosts = response["data"]["hosts"]
        .as_array()
        .expect("hosts should be an array");
    assert!(
        !hosts.is_empty(),
        "Should have at least one healthy instance"
    );

    // All returned instances should be healthy
    for host in hosts {
        assert_eq!(
            host["healthy"], true,
            "All instances returned with healthyOnly=true should be healthy, got: {:?}",
            host
        );
    }

    // Verify our registered instance is present
    let found = hosts
        .iter()
        .any(|h| h["ip"] == "192.168.1.120" && h["port"] == 8080);
    assert!(
        found,
        "Healthy instance 192.168.1.120:8080 should be in the healthy-only list"
    );
}

/// Test batch update metadata
#[tokio::test]

async fn test_batch_update_metadata() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("batchmeta");

    // Register instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.130"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register");

    // Batch update metadata
    let instances = r#"[{"ip":"192.168.1.130","port":8080}]"#;
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/instance/metadata/batch",
            &[
                ("serviceName", service_name.as_str()),
                ("instances", instances),
                ("metadata", r#"{"env":"test"}"#),
            ],
        )
        .await
        .expect("Failed to batch update");

    assert_eq!(response["code"], 0, "Batch update should succeed");

    // Verify metadata was updated by querying the instance detail
    let detail: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.130"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to get instance detail after metadata update");

    assert_eq!(detail["code"], 0, "Get instance detail should succeed");
    let metadata = &detail["data"]["metadata"];
    assert!(metadata.is_object(), "Instance should have metadata object");
    assert_eq!(
        metadata["env"], "test",
        "Metadata 'env' should be 'test' after batch update"
    );
}

/// Test instance with weight
#[tokio::test]

async fn test_instance_with_weight() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("weighted");

    // Register with specific weight
    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.140"),
                ("port", "8080"),
                ("weight", "0.5"),
            ],
        )
        .await
        .expect("Failed to register");

    assert_eq!(
        response["code"], 0,
        "Registration with weight should succeed"
    );

    // Verify the weight was stored correctly
    let detail: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.140"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to get instance detail");

    assert_eq!(detail["code"], 0, "Get instance should succeed");
    let weight = detail["data"]["weight"]
        .as_f64()
        .expect("weight should be a number");
    assert!(
        (weight - 0.5).abs() < f64::EPSILON,
        "Weight should be 0.5, got {}",
        weight
    );
}

/// Test create service
#[tokio::test]

async fn test_create_service() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("create");

    let response: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[
                ("serviceName", service_name.as_str()),
                ("protectThreshold", "0.5"),
            ],
        )
        .await
        .expect("Failed to create service");

    assert_eq!(response["code"], 0, "Create service should succeed");

    // Verify service exists by querying it
    let get_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get created service");

    assert_eq!(
        get_response["code"], 0,
        "Get created service should succeed"
    );
    assert!(
        get_response["data"].is_object(),
        "Service data should be an object"
    );

    let svc_name = get_response["data"]["name"]
        .as_str()
        .or_else(|| get_response["data"]["serviceName"].as_str());
    assert!(
        svc_name.is_some(),
        "Service response should have a name or serviceName field"
    );
    assert_eq!(
        svc_name.unwrap(),
        service_name.as_str(),
        "Service name should match"
    );
}

/// Test delete service
#[tokio::test]

async fn test_delete_service() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("delete");

    // Create first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to create");

    // Then delete
    let response: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to delete");

    assert_eq!(response["code"], 0, "Delete service should succeed");

    // Verify service no longer exists
    let get_result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await;

    match get_result {
        Ok(resp) => {
            // If the API returns a response, the code should indicate not found
            assert_ne!(
                resp["code"], 0,
                "Querying a deleted service should return a non-zero error code"
            );
        }
        Err(_) => {
            // HTTP 404 or other error is expected for a deleted service
        }
    }
}

/// Test update service
#[tokio::test]

async fn test_update_service() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("update_svc");

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[
                ("serviceName", service_name.as_str()),
                ("protectThreshold", "0.5"),
            ],
        )
        .await
        .expect("Failed to create");

    // Update
    let response: serde_json::Value = client
        .put_form(
            "/nacos/v2/ns/service",
            &[
                ("serviceName", service_name.as_str()),
                ("protectThreshold", "0.8"),
            ],
        )
        .await
        .expect("Failed to update");

    assert_eq!(response["code"], 0, "Update service should succeed");

    // Verify protectThreshold was updated
    let get_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get service after update");

    assert_eq!(
        get_response["code"], 0,
        "Get updated service should succeed"
    );
    let threshold = get_response["data"]["protectThreshold"]
        .as_f64()
        .expect("protectThreshold should be a number");
    assert!(
        (threshold - 0.8).abs() < 0.01,
        "protectThreshold should be updated to 0.8, got {}",
        threshold
    );
}

/// Test get service detail
#[tokio::test]

async fn test_get_service() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("getservice");

    // Create
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to create");

    // Get
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to get service");

    assert_eq!(response["code"], 0, "Get service should succeed");
    assert!(
        response["data"].is_object(),
        "Service data should be an object"
    );

    let data = &response["data"];
    let svc_name = data["name"]
        .as_str()
        .or_else(|| data["serviceName"].as_str());
    assert!(
        svc_name.is_some(),
        "Service response should have a name or serviceName field"
    );
    assert_eq!(
        svc_name.unwrap(),
        service_name.as_str(),
        "Service name should match the created service"
    );

    // Verify groupName defaults to DEFAULT_GROUP
    let group = data["groupName"].as_str().unwrap_or("");
    assert_eq!(
        group, "DEFAULT_GROUP",
        "Default group should be DEFAULT_GROUP"
    );
}

/// Test service list
#[tokio::test]

async fn test_service_list() {
    let client = authenticated_client().await;

    // Create a known service so we can verify the list
    let service_name = unique_service_name("listcheck");
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to create service for list test");

    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/service/list",
            &[("pageNo", "1"), ("pageSize", "100")],
        )
        .await
        .expect("Failed to get service list");

    assert_eq!(response["code"], 0, "Get service list should succeed");
    assert!(
        response["data"].is_object() || response["data"].is_array(),
        "Service list data should be present"
    );

    // The response typically has a "services" or "doms" array, or count field
    let count = response["data"]["count"]
        .as_i64()
        .or_else(|| response["data"]["totalCount"].as_i64());
    if let Some(c) = count {
        assert!(c > 0, "Service count should be > 0, got {}", c);
    }

    // Check that our created service name appears in the list
    let services = response["data"]["services"]
        .as_array()
        .or_else(|| response["data"]["doms"].as_array());
    if let Some(svc_list) = services {
        let found = svc_list.iter().any(|s| {
            s.as_str() == Some(service_name.as_str())
                || s["name"].as_str() == Some(service_name.as_str())
                || s["serviceName"].as_str() == Some(service_name.as_str())
        });
        assert!(
            found,
            "Service list should contain the created service '{}'",
            service_name
        );
    }
}

/// Test service not found
#[tokio::test]

async fn test_service_not_found() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("nonexistent");

    let result = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/ns/service",
            &[("serviceName", service_name.as_str())],
        )
        .await;

    // Should indicate service not found via HTTP 404 or error code
    match result {
        Ok(response) => {
            // API returns a non-zero error code for not found (typically 20404 or 21008)
            let code = response["code"].as_i64().unwrap_or(0);
            assert_ne!(
                code, 0,
                "Non-existent service should return a non-zero error code, got response: {:?}",
                response
            );
        }
        Err(_) => {
            // HTTP 404 is expected for service not found
        }
    }
}

/// Test namespace isolation for naming - instances in different namespaces should not interfere
#[tokio::test]

async fn test_namespace_isolation_naming() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let client = authenticated_client().await;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Create two test namespaces via console API
    let ns_a = format!("test-ns-a-{}", timestamp);
    let ns_b = format!("test-ns-b-{}", timestamp);
    let service_name = "shared-service-name";
    let ip_a = "192.168.1.200";
    let ip_b = "192.168.1.201";

    // Create namespace A and B
    let console_client = || async {
        let mut client = TestClient::new(CONSOLE_BASE_URL);
        client
            .login(TEST_USERNAME, TEST_PASSWORD)
            .await
            .expect("Login failed");
        client
    };

    let console = console_client().await;
    let _: serde_json::Value = console
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_a.as_str()),
                ("namespaceName", "Test NS A for Naming"),
            ],
        )
        .await
        .expect("Failed to create namespace A");

    let _: serde_json::Value = console
        .post_form(
            "/v2/console/namespace",
            &[
                ("namespaceId", ns_b.as_str()),
                ("namespaceName", "Test NS B for Naming"),
            ],
        )
        .await
        .expect("Failed to create namespace B");

    // Register instance in namespace A
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_a),
                ("port", "8080"),
                ("namespaceId", ns_a.as_str()),
            ],
        )
        .await
        .expect("Failed to register instance in namespace A");

    // Register instance in namespace B with same service name
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_b),
                ("port", "8080"),
                ("namespaceId", ns_b.as_str()),
            ],
        )
        .await
        .expect("Failed to register instance in namespace B");

    // Get instances from namespace A - should only return instance A
    let response_a: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name),
                ("namespaceId", ns_a.as_str()),
            ],
        )
        .await
        .expect("Failed to get instances from namespace A");

    assert_eq!(response_a["code"], 0);
    let hosts_a = response_a["data"]["hosts"].as_array().unwrap();
    assert_eq!(hosts_a.len(), 1, "Namespace A should have 1 instance");
    assert_eq!(hosts_a[0]["ip"], ip_a, "Namespace A should have instance A");

    // Get instances from namespace B - should only return instance B
    let response_b: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[
                ("serviceName", service_name),
                ("namespaceId", ns_b.as_str()),
            ],
        )
        .await
        .expect("Failed to get instances from namespace B");

    assert_eq!(response_b["code"], 0);
    let hosts_b = response_b["data"]["hosts"].as_array().unwrap();
    assert_eq!(hosts_b.len(), 1, "Namespace B should have 1 instance");
    assert_eq!(hosts_b[0]["ip"], ip_b, "Namespace B should have instance B");

    // Get instances from default namespace - should not find this service
    let result_default = client
        .get_with_query::<serde_json::Value, _>(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name)],
        )
        .await;

    match result_default {
        Ok(response) => {
            // Service might not exist in default namespace
            let empty_hosts: Vec<serde_json::Value> = vec![];
            let hosts = response["data"]["hosts"].as_array().unwrap_or(&empty_hosts);
            assert!(
                hosts.is_empty() || response["code"] != 0,
                "Should not find service in default namespace"
            );
        }
        Err(_) => {
            // Service not found is acceptable
        }
    }

    // Cleanup: deregister instances and delete namespaces
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_a),
                ("port", "8080"),
                ("namespaceId", ns_a.as_str()),
            ],
        )
        .await
        .ok()
        .unwrap_or_default();

    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name),
                ("ip", ip_b),
                ("port", "8080"),
                ("namespaceId", ns_b.as_str()),
            ],
        )
        .await
        .ok()
        .unwrap_or_default();

    let _: serde_json::Value = console
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_a.as_str())])
        .await
        .ok()
        .unwrap_or_default();

    let _: serde_json::Value = console
        .delete_with_query("/v2/console/namespace", &[("namespaceId", ns_b.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}
