//! Performance and Load Tests
//!
//! Tests for performance characteristics and load handling

use batata_integration_tests::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_data_id, unique_service_name, unique_test_id,
};
use std::time::{Duration, Instant};

/// Create an authenticated test client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

// ==================== Config Performance Tests ====================

/// Benchmark config publish throughput
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn bench_config_publish_throughput() {
    let client = authenticated_client().await;
    let num_configs = 100;
    let start = Instant::now();

    let mut last_data_id = String::new();
    for i in 0..num_configs {
        let data_id = format!("bench_publish_{}", unique_test_id());
        if i == num_configs - 1 {
            last_data_id = data_id.clone();
        }
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("content", &format!("value={}", i)),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    let elapsed = start.elapsed();
    let throughput = num_configs as f64 / elapsed.as_secs_f64();

    println!("Config publish throughput: {:.2} configs/sec", throughput);
    assert!(throughput > 10.0, "Should publish at least 10 configs/sec");

    // Verify data integrity: retrieve the last published config
    let verify: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", last_data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to retrieve published config for verification");
    assert_eq!(
        verify["code"], 0,
        "Should be able to retrieve a published config after batch publish"
    );
    let content = verify["data"].as_str().unwrap_or("");
    assert!(
        content.starts_with("value="),
        "Retrieved config content should match published format, got: {}",
        content
    );
}

/// Benchmark config get throughput
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn bench_config_get_throughput() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("bench_get");
    let expected_content = "test.value=1";

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", expected_content),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Benchmark gets
    let num_gets = 1000;
    let start = Instant::now();

    for _ in 0..num_gets {
        let response: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/cs/config",
                &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
            )
            .await
            .expect("Failed to get config");

        // Verify returned content matches what was published
        assert_eq!(response["code"], 0, "Config get should succeed");
        assert_eq!(
            response["data"].as_str().unwrap_or(""),
            expected_content,
            "Returned config content should match published content"
        );
    }

    let elapsed = start.elapsed();
    let throughput = num_gets as f64 / elapsed.as_secs_f64();

    println!("Config get throughput: {:.2} gets/sec", throughput);
    assert!(throughput > 50.0, "Should get at least 50 configs/sec");
}

/// Benchmark config update throughput
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn bench_config_update_throughput() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("bench_update");

    // Publish initial config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "value=0"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Benchmark updates
    let num_updates = 100;
    let start = Instant::now();

    for i in 0..num_updates {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("content", &format!("value={}", i)),
                ],
            )
            .await
            .expect("Failed to update config");
    }

    let elapsed = start.elapsed();
    let throughput = num_updates as f64 / elapsed.as_secs_f64();

    println!("Config update throughput: {:.2} updates/sec", throughput);

    // Verify the last update persisted
    let verify: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to retrieve config after updates");
    assert_eq!(verify["code"], 0, "Config get should succeed after updates");
    let expected_final = format!("value={}", num_updates - 1);
    assert_eq!(
        verify["data"].as_str().unwrap_or(""),
        expected_final,
        "Config should contain the last update value"
    );
}

/// Benchmark config list throughput
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn bench_config_list_throughput() {
    let client = authenticated_client().await;

    // Publish multiple configs
    for i in 0..50 {
        let data_id = format!("bench_list_{}_{}", i, unique_test_id());
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("content", "test.value=1"),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Benchmark lists - use history/configs endpoint to list all configs in namespace
    let num_lists = 100;
    let start = Instant::now();

    for _ in 0..num_lists {
        let _: serde_json::Value = client
            .get_with_query("/nacos/v2/cs/history/configs", &[("namespaceId", "public")])
            .await
            .expect("Failed to list configs");
    }

    let elapsed = start.elapsed();
    let throughput = num_lists as f64 / elapsed.as_secs_f64();

    println!("Config list throughput: {:.2} lists/sec", throughput);
}

// ==================== Naming Performance Tests ====================

/// Benchmark instance registration throughput
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn bench_instance_register_throughput() {
    let client = authenticated_client().await;
    let num_instances = 100;
    let start = Instant::now();

    let mut last_service_name = String::new();
    let mut last_ip = String::new();
    let mut last_port: u32 = 0;
    for i in 0..num_instances {
        let service_name = unique_service_name("bench_register");
        let ip = format!("192.168.1.{}", i % 250);
        let port = 8080 + (i % 10);

        if i == num_instances - 1 {
            last_service_name = service_name.clone();
            last_ip = ip.clone();
            last_port = port;
        }

        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/ns/instance",
                &[
                    ("serviceName", service_name.as_str()),
                    ("ip", ip.as_str()),
                    ("port", &port.to_string()),
                    ("weight", "1.0"),
                ],
            )
            .await
            .expect("Failed to register instance");
    }

    let elapsed = start.elapsed();
    let throughput = num_instances as f64 / elapsed.as_secs_f64();

    println!(
        "Instance register throughput: {:.2} instances/sec",
        throughput
    );
    assert!(
        throughput > 10.0,
        "Should register at least 10 instances/sec"
    );

    // Verify data integrity: check the last registered instance exists
    let verify: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", last_service_name.as_str())],
        )
        .await
        .expect("Failed to query instances for verification");
    assert_eq!(
        verify["code"], 0,
        "Should be able to query registered service after batch register"
    );
    let hosts = verify["data"]["hosts"]
        .as_array()
        .expect("hosts should be an array");
    let found = hosts.iter().any(|h| {
        h["ip"].as_str() == Some(last_ip.as_str()) && h["port"].as_u64() == Some(last_port as u64)
    });
    assert!(
        found,
        "Last registered instance ({}:{}) should exist in the service",
        last_ip, last_port
    );
}

/// Benchmark instance query throughput
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn bench_instance_query_throughput() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("bench_query");

    // Register instances
    for i in 0..10 {
        let ip = format!("192.168.1.{}", 250 + i);
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/ns/instance",
                &[
                    ("serviceName", service_name.as_str()),
                    ("ip", ip.as_str()),
                    ("port", "8080"),
                    ("weight", "1.0"),
                ],
            )
            .await
            .expect("Failed to register instance");
    }

    // Benchmark queries
    let num_queries = 1000;
    let start = Instant::now();

    for _ in 0..num_queries {
        let _: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/ns/instance/list",
                &[("serviceName", service_name.as_str())],
            )
            .await
            .expect("Failed to query instances");
    }

    let elapsed = start.elapsed();
    let throughput = num_queries as f64 / elapsed.as_secs_f64();

    println!("Instance query throughput: {:.2} queries/sec", throughput);
    assert!(throughput > 50.0, "Should query at least 50 instances/sec");
}

/// Benchmark service list throughput
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn bench_service_list_throughput() {
    let client = authenticated_client().await;

    // Register multiple services
    for i in 0..50 {
        let service_name = unique_service_name(&format!("bench_list_{}", i));
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/ns/instance",
                &[
                    ("serviceName", service_name.as_str()),
                    ("ip", "192.168.1.250"),
                    ("port", "8080"),
                    ("weight", "1.0"),
                ],
            )
            .await
            .expect("Failed to register instance");
    }

    // Benchmark service lists
    let num_lists = 100;
    let start = Instant::now();

    for _ in 0..num_lists {
        let _: serde_json::Value = client
            .get_with_query("/nacos/v2/ns/service/list", &[("groupName", DEFAULT_GROUP)])
            .await
            .expect("Failed to list services");
    }

    let elapsed = start.elapsed();
    let throughput = num_lists as f64 / elapsed.as_secs_f64();

    println!("Service list throughput: {:.2} lists/sec", throughput);
}

// ==================== Concurrent Operations Tests ====================

/// Test concurrent config operations
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn test_concurrent_config_operations() {
    let client = authenticated_client().await;
    let token = client.cookies();
    let num_tasks = 30;
    let start = Instant::now();
    let mut handles = Vec::new();
    let mut data_ids = Vec::new();

    for i in 0..num_tasks {
        let data_id = format!("concurrent_config_{}_{}", i, unique_test_id());
        data_ids.push(data_id.clone());
        let token_clone = token.clone();

        handles.push(tokio::spawn(async move {
            let client = TestClient::new_with_cookies(MAIN_BASE_URL, token_clone);

            let response: serde_json::Value = client
                .post_form(
                    "/nacos/v2/cs/config",
                    &[
                        ("dataId", data_id.as_str()),
                        ("group", DEFAULT_GROUP),
                        ("content", "concurrent.value=1"),
                    ],
                )
                .await
                .expect("Failed to publish config");
            response
        }));
    }

    for handle in handles {
        let result = handle.await.expect("Task failed");
        assert_eq!(result["code"], 0, "Concurrent operation should succeed");
    }

    let elapsed = start.elapsed();
    println!("Concurrent config operations time: {:?}", elapsed);

    // Verify data integrity: check that configs actually exist after concurrent publish
    for data_id in data_ids.iter().take(5) {
        let verify: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/cs/config",
                &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
            )
            .await
            .expect("Failed to verify config after concurrent publish");
        assert_eq!(
            verify["code"], 0,
            "Config '{}' should exist after concurrent publish",
            data_id
        );
        assert_eq!(
            verify["data"].as_str().unwrap_or(""),
            "concurrent.value=1",
            "Config '{}' content should match what was published",
            data_id
        );
    }
}

/// Test concurrent instance operations
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn test_concurrent_instance_operations() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("concurrent_instance");
    let token = client.cookies();
    let num_tasks = 30;
    let start = Instant::now();

    let mut handles = Vec::new();
    for i in 0..num_tasks {
        let service_name_clone = service_name.clone();
        let ip = format!("192.168.1.{}", i % 250);
        let token_clone = token.clone();

        handles.push(tokio::spawn(async move {
            let client = TestClient::new_with_cookies(MAIN_BASE_URL, token_clone);

            let response: serde_json::Value = client
                .post_form(
                    "/nacos/v2/ns/instance",
                    &[
                        ("serviceName", service_name_clone.as_str()),
                        ("ip", ip.as_str()),
                        ("port", "8080"),
                        ("weight", "1.0"),
                    ],
                )
                .await
                .expect("Failed to register instance");
            response
        }));
    }

    for handle in handles {
        let result = handle.await.expect("Task failed");
        assert_eq!(result["code"], 0, "Concurrent registration should succeed");
    }

    let elapsed = start.elapsed();
    println!("Concurrent instance operations time: {:?}", elapsed);

    // Verify data integrity: check that instances exist after concurrent registration
    let verify: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/ns/instance/list",
            &[("serviceName", service_name.as_str())],
        )
        .await
        .expect("Failed to query instances after concurrent registration");
    assert_eq!(
        verify["code"], 0,
        "Instance list query should succeed after concurrent registration"
    );
    let hosts = verify["data"]["hosts"]
        .as_array()
        .expect("hosts should be an array");
    assert!(
        !hosts.is_empty(),
        "Service should have registered instances after concurrent registration"
    );
    assert!(
        hosts.len() >= 2,
        "Service should have multiple registered instances, got {}",
        hosts.len()
    );
}

/// Test mixed concurrent operations
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn test_mixed_concurrent_operations() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("mixed_concurrent");
    let service_name = unique_service_name("mixed_concurrent");
    let token = client.cookies();

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "mixed.value=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Register instance
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", service_name.as_str()),
                ("ip", "192.168.1.100"),
                ("port", "8080"),
            ],
        )
        .await
        .expect("Failed to register instance");

    let num_tasks = 15;
    let start = Instant::now();
    let mut handles = Vec::new();

    // Mix config and naming operations
    for i in 0..num_tasks {
        let data_id_clone = data_id.clone();
        let service_name_clone = service_name.clone();
        let token_clone = token.clone();

        handles.push(tokio::spawn(async move {
            let client = TestClient::new_with_cookies(MAIN_BASE_URL, token_clone);

            // Alternate between config and naming operations
            if i % 2 == 0 {
                let response: serde_json::Value = client
                    .get_with_query(
                        "/nacos/v2/cs/config",
                        &[("dataId", data_id_clone.as_str()), ("group", DEFAULT_GROUP)],
                    )
                    .await
                    .expect("Failed to get config");
                response
            } else {
                let response: serde_json::Value = client
                    .get_with_query(
                        "/nacos/v2/ns/instance/list",
                        &[("serviceName", service_name_clone.as_str())],
                    )
                    .await
                    .expect("Failed to query instances");
                response
            }
        }));
    }

    for handle in handles {
        let result = handle.await.expect("Task failed");
        assert_eq!(result["code"], 0, "Mixed operation should succeed");
    }

    let elapsed = start.elapsed();
    println!("Mixed concurrent operations time: {:?}", elapsed);
}

/// Test sustained load over time
#[tokio::test]
#[ignore = "performance tests require specialized setup"]

async fn test_sustained_load() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("sustained_load");
    let duration = Duration::from_secs(10);
    let start = Instant::now();

    let mut ops = 0;
    while start.elapsed() < duration {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("content", &format!("value={}", ops)),
                ],
            )
            .await
            .expect("Failed to update config");

        let _: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/cs/config",
                &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
            )
            .await
            .expect("Failed to get config");

        ops += 1;
    }

    let elapsed = start.elapsed();
    let ops_per_sec = ops as f64 / elapsed.as_secs_f64();

    println!(
        "Sustained load: {:.2} ops/sec over {:?}",
        ops_per_sec, elapsed
    );
    assert!(ops_per_sec > 5.0, "Should handle at least 5 ops/sec");

    // Verify data integrity after sustained operations
    let verify: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to retrieve config after sustained load");
    assert_eq!(
        verify["code"], 0,
        "Config should be retrievable after sustained load"
    );
    let expected_final = format!("value={}", ops - 1);
    assert_eq!(
        verify["data"].as_str().unwrap_or(""),
        expected_final,
        "Config should contain the last written value after sustained load"
    );
}
