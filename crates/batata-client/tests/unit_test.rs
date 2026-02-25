//! Batata Client Unit Tests
//!
//! Unit tests for individual modules and components.
//! These tests can run without a live server.

use batata_client::{
    grpc::GrpcClientConfig,
    http::HttpClientConfig,
    limiter::SlidingWindowLimiter,
    metrics::{MetricsMonitor, SimpleCounter},
    model::*,
};
use serde_json::json;

// ============== HTTP Client Configuration Tests ==============

#[test]
fn test_http_config_default() {
    let config = HttpClientConfig::default();
    assert_eq!(config.server_addrs, vec!["http://127.0.0.1:8848"]);
    assert_eq!(config.username, "nacos");
    assert_eq!(config.password, "nacos");
    assert_eq!(config.connect_timeout_ms, 5000);
    assert_eq!(config.read_timeout_ms, 30000);
}

#[test]
fn test_http_config_builder() {
    let config = HttpClientConfig::new("http://example.com:8848")
        .with_auth("user", "pass")
        .with_timeouts(3000, 15000)
        .with_context_path("/nacos")
        .with_auth_endpoint("/custom/auth");

    assert_eq!(config.server_addrs, vec!["http://example.com:8848"]);
    assert_eq!(config.username, "user");
    assert_eq!(config.password, "pass");
    assert_eq!(config.connect_timeout_ms, 3000);
    assert_eq!(config.read_timeout_ms, 15000);
    assert_eq!(config.context_path, "/nacos");
    assert_eq!(config.auth_endpoint, "/custom/auth");
}

#[test]
fn test_http_config_multiple_servers() {
    let servers = vec![
        "http://server1:8848".to_string(),
        "http://server2:8848".to_string(),
        "http://server3:8848".to_string(),
    ];
    let config = HttpClientConfig::with_servers(servers);

    assert_eq!(config.server_addrs.len(), 3);
}

// ============== gRPC Client Configuration Tests ==============

#[test]
fn test_grpc_config_default() {
    let config = GrpcClientConfig::default();
    assert_eq!(config.server_addrs, vec!["127.0.0.1:8848"]);
    assert_eq!(config.module, "config");
    assert!(config.username.is_empty());
    assert!(config.password.is_empty());
    assert!(config.tenant.is_empty());
}

#[test]
fn test_grpc_config_custom() {
    let config = GrpcClientConfig {
        server_addrs: vec!["server1:8848".to_string(), "server2:8848".to_string()],
        username: "admin".to_string(),
        password: "admin123".to_string(),
        module: "config,naming".to_string(),
        tenant: "tenant1".to_string(),
        labels: {
            let mut map = std::collections::HashMap::new();
            map.insert("app".to_string(), "test".to_string());
            map.insert("version".to_string(), "1.0".to_string());
            map
        },
    };

    assert_eq!(config.server_addrs.len(), 2);
    assert_eq!(config.username, "admin");
    assert_eq!(config.module, "config,naming");
    assert_eq!(config.tenant, "tenant1");
    assert_eq!(config.labels.len(), 2);
}

// ============== Model Serialization Tests ==============

#[test]
fn test_namespace_serialization() {
    let ns = Namespace {
        namespace: "test-ns".to_string(),
        namespace_show_name: "Test Namespace".to_string(),
        namespace_desc: "A test namespace".to_string(),
        quota: 100,
        config_count: 5,
        type_: 0,
    };

    let json = serde_json::to_string(&ns).unwrap();
    // Note: The serialization uses camelCase, but field names might vary
    assert!(json.contains("test-ns"));
    assert!(json.contains("Test Namespace"));

    let deserialized: Namespace = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.namespace, ns.namespace);
    assert_eq!(deserialized.quota, ns.quota);
}

#[test]
fn test_config_basic_info_serialization() {
    let info = ConfigBasicInfo {
        id: 1,
        namespace_id: "public".to_string(),
        group_name: "DEFAULT_GROUP".to_string(),
        data_id: "test.yaml".to_string(),
        md5: "abc123".to_string(),
        r#type: "yaml".to_string(),
        app_name: "myapp".to_string(),
        create_time: 1234567890,
        modify_time: 1234567891,
    };

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: ConfigBasicInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.data_id, info.data_id);
    assert_eq!(deserialized.group_name, info.group_name);
    assert_eq!(deserialized.md5, info.md5);
}

#[test]
fn test_config_all_info_serialization() {
    let info = ConfigAllInfo {
        id: 1,
        data_id: "test.json".to_string(),
        group: "TEST_GROUP".to_string(),
        content: r#"{"key": "value"}"#.to_string(),
        md5: "def456".to_string(),
        tenant: "tenant1".to_string(),
        app_name: "testapp".to_string(),
        r#type: "json".to_string(),
        create_time: 1234567890,
        modify_time: 1234567891,
        create_user: "admin".to_string(),
        create_ip: "127.0.0.1".to_string(),
        desc: "Test config".to_string(),
        r#use: "production".to_string(),
        effect: "low".to_string(),
        schema: "".to_string(),
        config_tags: "tag1,tag2".to_string(),
        encrypted_data_key: "".to_string(),
    };

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: ConfigAllInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.data_id, info.data_id);
    assert_eq!(deserialized.content, info.content);
    assert_eq!(deserialized.config_tags, info.config_tags);
}

#[test]
fn test_member_serialization() {
    let member = Member {
        ip: "127.0.0.1".to_string(),
        port: 8848,
        state: "UP".to_string(),
        extend_info: {
            let mut map = std::collections::HashMap::new();
            map.insert("weight".to_string(), json!("1.0"));
            map
        },
        address: "127.0.0.1:8848".to_string(),
        fail_access_cnt: 0,
        abilities: NodeAbilities::default(),
        grpc_report_enabled: true,
    };

    let json = serde_json::to_string(&member).unwrap();
    assert!(json.contains("ip"));
    assert!(json.contains("state"));
    assert!(json.contains("grpcReportEnabled"));

    let deserialized: Member = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.ip, member.ip);
    assert_eq!(deserialized.port, member.port);
}

#[test]
fn test_page_serialization() {
    let items = vec![
        ConfigBasicInfo {
            id: 1,
            data_id: "config1.yaml".to_string(),
            group_name: "GROUP1".to_string(),
            ..Default::default()
        },
        ConfigBasicInfo {
            id: 2,
            data_id: "config2.yaml".to_string(),
            group_name: "GROUP1".to_string(),
            ..Default::default()
        },
    ];

    let page = Page {
        total_count: 100,
        page_number: 1,
        pages_available: 10,
        page_items: items,
    };

    let json = serde_json::to_string(&page).unwrap();
    assert!(json.contains("totalCount"));
    assert!(json.contains("pageNumber"));

    let deserialized: Page<ConfigBasicInfo> = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.total_count, 100);
    assert_eq!(deserialized.page_items.len(), 2);
}

#[test]
fn test_instance_info_serialization() {
    let instance = InstanceInfo {
        ip: "10.0.0.1".to_string(),
        port: 8080,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        service_name: "my-service".to_string(),
        metadata: {
            let mut map = std::collections::HashMap::new();
            map.insert("version".to_string(), "1.0".to_string());
            map
        },
        instance_heart_beat_interval: 5000,
        instance_heart_beat_timeout: 15000,
        ip_delete_timeout: 30000,
    };

    let json = serde_json::to_string(&instance).unwrap();
    assert!(json.contains("ip"));
    assert!(json.contains("healthy"));
    assert!(json.contains("metadata"));

    let deserialized: InstanceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.ip, instance.ip);
    assert_eq!(deserialized.port, instance.port);
    assert_eq!(deserialized.weight, instance.weight);
}

#[test]
fn test_api_response_generic() {
    // Test that ApiResponse can be deserialized correctly
    // Using a simple string as data type
    let json = r#"{
        "code": 200,
        "message": "Success",
        "data": "test-data"
    }"#;

    let deserialized: ApiResponse<String> = serde_json::from_str(json).unwrap();
    assert_eq!(deserialized.code, 200);
    assert_eq!(deserialized.message, "Success");
    assert_eq!(deserialized.data, "test-data");
}

// ============== Rate Limiter Tests ==============

#[tokio::test]
async fn test_rate_limiter_single_request() {
    let limiter = SlidingWindowLimiter::new(10, 1);
    assert!(limiter.try_allow().await);
}

#[tokio::test]
async fn test_rate_limiter_exceeds_limit() {
    let limiter = SlidingWindowLimiter::new(3, 1);

    // First 3 should succeed
    for _ in 0..3 {
        assert!(limiter.try_allow().await);
    }

    // 4th should fail
    assert!(!limiter.try_allow().await);
}

#[tokio::test]
async fn test_rate_limiter_window_reset() {
    let limiter = SlidingWindowLimiter::new(2, 1);

    // Use up limit
    for _ in 0..2 {
        assert!(limiter.try_allow().await);
    }

    // Should be rate limited
    assert!(!limiter.try_allow().await);

    // Wait for window to expire
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

    // Should work again
    assert!(limiter.try_allow().await);
}

#[tokio::test]
async fn test_rate_limiter_concurrent() {
    let limiter = std::sync::Arc::new(SlidingWindowLimiter::new(100, 1));
    let mut handles = vec![];

    // Spawn 50 concurrent requests
    for _ in 0..50 {
        let limiter = limiter.clone();
        let handle = tokio::spawn(async move { limiter.try_allow().await });
        handles.push(handle);
    }

    let results: Vec<bool> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // All 50 should succeed within limit of 100
    assert!(results.iter().filter(|&&r| r).count() == 50);
}

// ============== Metrics Tests ==============

#[test]
fn test_simple_counter() {
    let counter = SimpleCounter::new();
    assert_eq!(counter.get(), 0);

    counter.increment();
    assert_eq!(counter.get(), 1);

    counter.add(5);
    assert_eq!(counter.get(), 6);

    counter.reset();
    assert_eq!(counter.get(), 0);
}

#[test]
fn test_metrics_monitor() {
    let monitor = MetricsMonitor::new().unwrap();

    monitor.record_latency("test_op", "success", std::time::Duration::from_millis(100));
    monitor.increment_success_request("test_op");

    let output = monitor.gather();
    assert!(output.contains("batata_request_latency_seconds"));
    assert!(output.contains("batata_success_requests_total"));
}

// ============== Utility Tests ==============

#[test]
fn test_build_service_key() {
    use batata_client::naming::service_info_holder::build_service_key;

    let key = build_service_key("DEFAULT_GROUP", "my-service");
    assert_eq!(key, "DEFAULT_GROUP@@my-service");

    let key = build_service_key("", "my-service");
    assert_eq!(key, "@@my-service");

    let key = build_service_key("group-with-@@", "service-with-@@");
    assert_eq!(key, "group-with-@@@@service-with-@@");
}

#[test]
fn test_build_cache_key() {
    use batata_client::config::cache::build_cache_key;

    let key = build_cache_key("config-id", "group-name", "tenant-id");
    assert_eq!(key, "config-id+group-name+tenant-id");

    let key = build_cache_key("config-id", "group-name", "");
    assert_eq!(key, "config-id+group-name");

    let key = build_cache_key("id", "g", "t");
    assert_eq!(key, "id+g+t");
}

#[test]
fn test_namespace_default() {
    let ns = Namespace::default();
    assert!(ns.namespace.is_empty());
    assert!(ns.namespace_show_name.is_empty());
    assert_eq!(ns.quota, 0);
    assert_eq!(ns.config_count, 0);
}

#[test]
fn test_config_basic_info_default() {
    let info = ConfigBasicInfo::default();
    assert_eq!(info.id, 0);
    assert!(info.namespace_id.is_empty());
    assert!(info.data_id.is_empty());
    assert_eq!(info.create_time, 0);
}

#[test]
fn test_page_default() {
    let page: Page<ConfigBasicInfo> = Page::default();
    assert_eq!(page.total_count, 0);
    assert_eq!(page.page_number, 0);
    assert!(page.page_items.is_empty());
}

// ============== Error Handling Tests ==============

#[test]
fn test_error_display() {
    use batata_client::ClientError;

    let error = ClientError::AuthFailed("Failed to connect".to_string());
    let error_str = format!("{}", error);
    assert!(error_str.contains("Failed to connect"));

    let error = ClientError::NotConnected;
    let error_str = format!("{}", error);
    assert!(error_str.contains("not ready"));
}

// ============== Concurrency Tests ==============

#[tokio::test]
async fn test_concurrent_counter() {
    use batata_client::SimpleCounter;
    use std::sync::Arc;

    let counter = Arc::new(SimpleCounter::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = counter.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..100 {
                counter.increment();
            }
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    assert_eq!(counter.get(), 1000);
}
