//! Performance and Load Tests
//!
//! Tests for performance characteristics and load handling

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME,
    TestClient, unique_data_id, unique_service_name,
};
use std::time::{Duration, Instant};

/// Create an authenticated test client for the main API server
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
#[ignore = "requires running server"]
async fn bench_config_publish_throughput() {
    let client = authenticated_client().await;
    let num_configs = 100;
    let start = Instant::now();

    for i in 0..num_configs {
        let data_id = format!("bench_publish_{}_{}", i, unique_test_id());
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
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
}

/// Benchmark config get throughput
#[tokio::test]
#[ignore = "requires running server"]
async fn bench_config_get_throughput() {
    let client = authenticated_client().await;

    // Publish test config
    let data_id = unique_data_id("bench_get");
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let num_requests = 1000;
    let start = Instant::now();

    for _ in 0..num_requests {
        let _: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                ],
            )
            .await
            .expect("Failed to get config");
    }

    let elapsed = start.elapsed();
    let throughput = num_requests as f64 / elapsed.as_secs_f64();
    let avg_latency = elapsed.as_millis() as f64 / num_requests as f64;

    println!("Config get throughput: {:.2} requests/sec", throughput);
    println!("Config get average latency: {:.2} ms", avg_latency);
    assert!(throughput > 100.0, "Should handle at least 100 requests/sec");
}

/// Benchmark config update throughput
#[tokio::test]
#[ignore = "requires running server"]
async fn bench_config_update_throughput() {
    let client = authenticated_client().await;

    // Publish test config
    let data_id = unique_data_id("bench_update");
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "version=0"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let num_updates = 50;
    let start = Instant::now();

    for i in 0..num_updates {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", &format!("version={}", i)),
                ],
            )
            .await
            .expect("Failed to update config");
    }

    let elapsed = start.elapsed();
    let throughput = num_updates as f64 / elapsed.as_secs_f64();

    println!("Config update throughput: {:.2} updates/sec", throughput);
    assert!(throughput > 10.0, "Should handle at least 10 updates/sec");
}

/// Benchmark config list performance
#[tokio::test]
#[ignore = "requires running server"]
async fn bench_config_list_performance() {
    let client = authenticated_client().await;

    // Publish test configs
    for i in 0..50 {
        let data_id = format!("bench_list_{}_{}", i, unique_test_id());
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", "value"),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    let num_requests = 100;
    let start = Instant::now();

    for _ in 0..num_requests {
        let _: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/cs/config/list",
                &[
                    ("search", "accurate"),
                    ("dataId", "bench_list"),
                    ("group", "DEFAULT_GROUP"),
                ],
            )
            .await
            .expect("Failed to list configs");
    }

    let elapsed = start.elapsed();
    let throughput = num_requests as f64 / elapsed.as_secs_f64();

    println!("Config list throughput: {:.2} requests/sec", throughput);
    assert!(throughput > 20.0, "Should handle at least 20 list requests/sec");
}

// ==================== Naming Performance Tests ====================

/// Benchmark instance registration throughput
#[tokio::test]
#[ignore = "requires running server"]
async fn bench_instance_register_throughput() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("bench_register");
    let num_instances = 100;
    let start = Instant::now();

    for i in 0..num_instances {
        let ip = format!("192.168.1.{}", i % 250);
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &serde_json::json!({
                    "serviceName": service_name,
                    "ip": ip,
                    "port": 8080 + i,
                    "weight": 1.0
                }),
            )
            .await
            .expect("Failed to register instance");
    }

    let elapsed = start.elapsed();
    let throughput = num_instances as f64 / elapsed.as_secs_f64();

    println!("Instance register throughput: {:.2} instances/sec", throughput);
    assert!(throughput > 20.0, "Should register at least 20 instances/sec");
}

/// Benchmark instance query throughput
#[tokio::test]
#[ignore = "requires running server"]
async fn bench_instance_query_throughput() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("bench_query");

    // Register test instances
    for i in 0..10 {
        let ip = format!("192.168.1.{}", i);
        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &serde_json::json!({
                    "serviceName": service_name,
                    "ip": ip,
                    "port": 8080,
                    "weight": 1.0
                }),
            )
            .await
            .expect("Failed to register instance");
    }

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
    let avg_latency = elapsed.as_millis() as f64 / num_queries as f64;

    println!("Instance query throughput: {:.2} requests/sec", throughput);
    println!("Instance query average latency: {:.2} ms", avg_latency);
    assert!(throughput > 200.0, "Should handle at least 200 query requests/sec");
}

/// Benchmark service query throughput
#[tokio::test]
#[ignore = "requires running server"]
async fn bench_service_query_throughput() {
    let client = authenticated_client().await;

    let num_queries = 500;
    let start = Instant::now();

    for _ in 0..num_queries {
        let _: serde_json::Value = client
            .get_with_query(
                "/nacos/v2/ns/service/list",
                &[("pageNo", "1"), ("pageSize", "10")],
            )
            .await
            .expect("Failed to list services");
    }

    let elapsed = start.elapsed();
    let throughput = num_queries as f64 / elapsed.as_secs_f64();

    println!("Service query throughput: {:.2} requests/sec", throughput);
    assert!(throughput > 100.0, "Should handle at least 100 service queries/sec");
}

/// Benchmark heartbeat throughput
#[tokio::test]
#[ignore = "requires running server"]
async fn bench_heartbeat_throughput() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("bench_heartbeat");

    let num_heartbeats = 500;
    let start = Instant::now();

    for i in 0..num_heartbeats {
        let ip = format!("192.168.1.{}", i % 250);
        let _: serde_json::Value = client
            .put_json(
                "/nacos/v2/ns/instance/beat",
                &serde_json::json!({
                    "serviceName": service_name,
                    "ip": ip,
                    "port": 8080,
                    "weight": 1.0,
                    "ephemeral": true
                }),
            )
            .await
            .expect("Failed to send heartbeat");
    }

    let elapsed = start.elapsed();
    let throughput = num_heartbeats as f64 / elapsed.as_secs_f64();

    println!("Heartbeat throughput: {:.2} heartbeats/sec", throughput);
    assert!(throughput > 50.0, "Should handle at least 50 heartbeats/sec");
}

// ==================== Concurrency Tests ====================

/// Test concurrent config operations
#[tokio::test]
#[ignore = "requires running server"]
async fn test_concurrent_config_operations() {
    let client = authenticated_client().await;
    let num_tasks = 20;
    let start = Instant::now();

    let mut handles = Vec::new();
    for i in 0..num_tasks {
        let client_clone = TestClient::new_with_cookies(
            MAIN_BASE_URL,
            client.cookies().clone(),
        );
        let data_id = format!("concurrent_{}_{}", i, unique_test_id());

        handles.push(tokio::spawn(async move {
            let response: serde_json::Value = client_clone
                .post_form(
                    "/nacos/v2/cs/config",
                    &[
                        ("dataId", data_id.as_str()),
                        ("group", "DEFAULT_GROUP"),
                        ("content", "value"),
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
}

/// Test concurrent instance operations
#[tokio::test]
#[ignore = "requires running server"]
async fn test_concurrent_instance_operations() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("concurrent_instance");
    let num_tasks = 30;
    let start = Instant::now();

    let mut handles = Vec::new();
    for i in 0..num_tasks {
        let client_clone = TestClient::new_with_cookies(
            MAIN_BASE_URL,
            client.cookies().clone(),
        );
        let service_name_clone = service_name.clone();
        let ip = format!("192.168.1.{}", i % 250);

        handles.push(tokio::spawn(async move {
            let response: serde_json::Value = client_clone
                .post_json(
                    "/nacos/v2/ns/instance",
                    &serde_json::json!({
                        "serviceName": service_name_clone,
                        "ip": ip,
                        "port": 8080,
                        "weight": 1.0
                    }),
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
}

/// Test mixed concurrent operations
#[tokio::test]
#[ignore = "requires running server"]
async fn test_mixed_concurrent_operations() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("mixed_concurrent");
    let data_id = unique_data_id("mixed");

    // Publish config and register instance first
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    let _: serde_json::Value = client
        .post_json(
            "/nacos/v2/ns/instance",
            &serde_json::json!({
                "serviceName": service_name,
                "ip": "192.168.1.100",
                "port": 8080
            }),
        )
        .await
        .expect("Failed to register instance");

    let num_tasks = 15;
    let start = Instant::now();
    let mut handles = Vec::new();

    // Mix config and naming operations
    for i in 0..num_tasks {
        let client_clone = TestClient::new_with_cookies(
            MAIN_BASE_URL,
            client.cookies().clone(),
        );
        let data_id_clone = data_id.clone();
        let service_name_clone = service_name.clone();

        handles.push(tokio::spawn(async move {
            // Alternate between config and naming operations
            if i % 2 == 0 {
                let response: serde_json::Value = client_clone
                    .get_with_query(
                        "/nacos/v2/cs/config",
                        &[
                            ("dataId", data_id_clone.as_str()),
                            ("group", "DEFAULT_GROUP"),
                        ],
                    )
                    .await
                    .expect("Failed to get config");
                response
            } else {
                let response: serde_json::Value = client_clone
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

// ==================== Load Tests ====================

/// Test sustained load - config operations
#[tokio::test]
#[ignore = "requires running server"]
async fn test_sustained_load_config() {
    let client = authenticated_client().await;
    let num_iterations = 5;
    let operations_per_iteration = 20;

    for iter in 0..num_iterations {
        let start = Instant::now();

        for i in 0..operations_per_iteration {
            let data_id = format!("load_{}_{}_{}", iter, i, unique_test_id());
            let _: serde_json::Value = client
                .post_form(
                    "/nacos/v2/cs/config",
                    &[
                        ("dataId", data_id.as_str()),
                        ("group", "DEFAULT_GROUP"),
                        ("content", "value"),
                    ],
                )
                .await
                .expect("Failed to publish config");
        }

        let elapsed = start.elapsed();
        println!(
            "Iteration {}: {} operations in {:?}",
            iter, operations_per_iteration, elapsed
        );

        // Small delay between iterations
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Test sustained load - naming operations
#[tokio::test]
#[ignore = "requires running server"]
async fn test_sustained_load_naming() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("load_naming");
    let num_iterations = 5;
    let operations_per_iteration = 20;

    for iter in 0..num_iterations {
        let start = Instant::now();

        for i in 0..operations_per_iteration {
            let ip = format!("192.168.{}.{}", iter % 2, i % 250);
            let port = 8080 + i;

            let _: serde_json::Value = client
                .post_json(
                    "/nacos/v2/ns/instance",
                    &serde_json::json!({
                        "serviceName": service_name,
                        "ip": ip,
                        "port": port,
                        "weight": 1.0
                    }),
                )
                .await
                .expect("Failed to register instance");
        }

        let elapsed = start.elapsed();
        println!(
            "Iteration {}: {} operations in {:?}",
            iter, operations_per_iteration, elapsed
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Test memory behavior with many configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_memory_many_configs() {
    let client = authenticated_client().await;
    let num_configs = 200;

    println!("Publishing {} configs...", num_configs);
    let start = Instant::now();

    for i in 0..num_configs {
        let data_id = format!("memory_{}_{}", i, unique_test_id());
        let content = format!("config.value.{}={}", i, "x".repeat(100));

        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", content.as_str()),
                ],
            )
            .await
            .expect("Failed to publish config");

        if i % 50 == 0 {
            println!("Published {} configs", i);
        }
    }

    let elapsed = start.elapsed();
    println!("Published {} configs in {:?}", num_configs, elapsed);

    // Query all configs
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/list",
            &[("pageNo", "1"), ("pageSize", "1000")],
        )
        .await
        .expect("Failed to list configs");

    assert_eq!(response["code"], 0, "List should succeed");
}

/// Test memory behavior with many instances
#[tokio::test]
#[ignore = "requires running server"]
async fn test_memory_many_instances() {
    let client = authenticated_client().await;
    let service_name = unique_service_name("memory_instances");
    let num_instances = 200;

    println!("Registering {} instances...", num_instances);
    let start = Instant::now();

    for i in 0..num_instances {
        let ip = format!("192.168.{}.{}", i % 10, i % 250);
        let port = 8000 + (i % 1000);

        let _: serde_json::Value = client
            .post_json(
                "/nacos/v2/ns/instance",
                &serde_json::json!({
                    "serviceName": service_name,
                    "ip": ip,
                    "port": port,
                    "weight": 1.0,
                    "metadata": {
                        "index": i
                    }
                }),
            )
            .await
            .expect("Failed to register instance");

        if i % 50 == 0 {
            println!("Registered {} instances", i);
        }
    }

    let elapsed = start.elapsed();
    println!("Registered {} instances in {:?}", num_instances, elapsed);

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

// Helper function
fn unique_test_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test_{}", timestamp)
}
