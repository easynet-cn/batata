//! Batata Client Integration Tests
//!
//! Integration tests for the Batata client SDK.
//! These tests require a running Batata server.
//!
//! To run these tests:
//! 1. Start a Batata server (e.g., using docker-compose.test.yml)
//! 2. Run: cargo test --test integration_test -- --ignored

use batata_client::{
    http::{BatataHttpClient, HttpClientConfig},
    grpc::{GrpcClient, GrpcClientConfig},
    config::BatataConfigService,
    naming::BatataNamingService,
};
use batata_api::naming::model::Instance;
use std::sync::Arc;

const TEST_SERVER_ADDR: &str = "http://127.0.0.1:8848";
const TEST_USERNAME: &str = "nacos";
const TEST_PASSWORD: &str = "nacos";
const TEST_NAMESPACE: &str = "";
const TEST_GROUP: &str = "DEFAULT_GROUP";

fn create_http_config() -> HttpClientConfig {
    HttpClientConfig::new(TEST_SERVER_ADDR)
        .with_auth(TEST_USERNAME, TEST_PASSWORD)
        .with_timeouts(5000, 30000)
}

fn create_grpc_config(module: &str) -> GrpcClientConfig {
    GrpcClientConfig {
        server_addrs: vec![TEST_SERVER_ADDR.replacen("http://", "", 1)],
        username: TEST_USERNAME.to_string(),
        password: TEST_PASSWORD.to_string(),
        module: module.to_string(),
        tenant: TEST_NAMESPACE.to_string(),
        labels: std::collections::HashMap::new(),
    }
}

// ============== HTTP Client Tests ==============

#[tokio::test]
#[ignore]
async fn test_http_client_basic_request() -> anyhow::Result<()> {
    // BatataHttpClient automatically authenticates on construction
    let config = create_http_config();
    let _client = BatataHttpClient::new(config).await?;

    println!("✓ HTTP client created and authenticated successfully");
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_http_client_namespace_list() -> anyhow::Result<()> {
    let config = create_http_config();
    let http_client = BatataHttpClient::new(config).await?;
    let api_client = batata_client::BatataApiClient::new(http_client);

    let namespaces = api_client.namespace_list().await?;
    println!("✓ Found {} namespaces", namespaces.len());
    assert!(!namespaces.is_empty());

    // Verify namespace structure
    let namespace = &namespaces[0];
    assert!(!namespace.namespace.is_empty());

    Ok(())
}

// ============== gRPC Client Tests ==============

#[tokio::test]
#[ignore]
async fn test_grpc_client_connect_config_module() -> anyhow::Result<()> {
    let config = create_grpc_config("config");
    let client = GrpcClient::new(config)?;

    client.connect().await?;
    println!("✓ gRPC client connected to config module");
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_grpc_client_connect_naming_module() -> anyhow::Result<()> {
    let config = create_grpc_config("naming");
    let client = GrpcClient::new(config)?;

    client.connect().await?;
    println!("✓ gRPC client connected to naming module");
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_grpc_client_multi_module() -> anyhow::Result<()> {
    // Test connecting to both modules
    let config = create_grpc_config("config,naming");
    let client = GrpcClient::new(config)?;

    client.connect().await?;
    println!("✓ gRPC client connected to config,naming modules");
    Ok(())
}

// ============== Config Service Tests ==============

#[tokio::test]
#[ignore]
async fn test_config_service_publish_and_get() -> anyhow::Result<()> {
    let grpc_config = create_grpc_config("config");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let config_service = BatataConfigService::new(grpc_client);

    let data_id = format!("test-config-{}", uuid::Uuid::new_v4());
    let content = "test.content=value";

    // Publish config
    let result = config_service
        .publish_config(&data_id, TEST_GROUP, TEST_NAMESPACE, content)
        .await?;
    assert!(result);
    println!("✓ Published config: {}", data_id);

    // Get config
    let retrieved = config_service
        .get_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;
    assert_eq!(retrieved, content);
    println!("✓ Retrieved config matches published content");

    // Remove config
    config_service
        .remove_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;
    println!("✓ Removed config: {}", data_id);

    // Verify removal
    let result = config_service.get_config(&data_id, TEST_GROUP, TEST_NAMESPACE).await;
    assert!(result.is_err());
    println!("✓ Config successfully removed");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_config_service_with_config_type() -> anyhow::Result<()> {
    let grpc_config = create_grpc_config("config");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let config_service = BatataConfigService::new(grpc_client);

    let data_id = format!("test-yaml-{}", uuid::Uuid::new_v4());
    let content = r#"
database:
  host: localhost
  port: 3306
  name: test_db
"#;

    // Publish with YAML type
    let result = config_service
        .publish_config_with_type(&data_id, TEST_GROUP, TEST_NAMESPACE, content, "yaml")
        .await?;
    assert!(result);
    println!("✓ Published YAML config: {}", data_id);

    // Get config
    let retrieved = config_service
        .get_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;
    assert_eq!(retrieved, content);
    println!("✓ Retrieved YAML config");

    // Cleanup
    config_service
        .remove_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_config_service_update_existing() -> anyhow::Result<()> {
    let grpc_config = create_grpc_config("config");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let config_service = BatataConfigService::new(grpc_client);

    let data_id = format!("test-update-{}", uuid::Uuid::new_v4());

    // First publish
    let content1 = "version=1";
    config_service
        .publish_config(&data_id, TEST_GROUP, TEST_NAMESPACE, content1)
        .await?;
    println!("✓ Published initial version");

    let retrieved = config_service
        .get_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;
    assert_eq!(retrieved, content1);

    // Update
    let content2 = "version=2";
    config_service
        .publish_config(&data_id, TEST_GROUP, TEST_NAMESPACE, content2)
        .await?;
    println!("✓ Updated config");

    let retrieved = config_service
        .get_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;
    assert_eq!(retrieved, content2);
    println!("✓ Retrieved updated version");

    // Cleanup
    config_service
        .remove_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_config_service_get_nonexistent() -> anyhow::Result<()> {
    let grpc_config = create_grpc_config("config");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let config_service = BatataConfigService::new(grpc_client);

    let data_id = format!("nonexistent-{}", uuid::Uuid::new_v4());

    // Get non-existent config should fail
    let result = config_service.get_config(&data_id, TEST_GROUP, TEST_NAMESPACE).await;
    assert!(result.is_err());
    println!("✓ Non-existent config correctly returns error");

    Ok(())
}

// ============== Naming Service Tests ==============

#[tokio::test]
#[ignore]
async fn test_naming_service_register_and_deregister() -> anyhow::Result<()> {
    let grpc_config = create_grpc_config("naming");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let naming_service = BatataNamingService::new(grpc_client);

    let service_name = format!("test-service-{}", uuid::Uuid::new_v4());

    let mut instance = Instance::default();
    instance.ip = "127.0.0.1".to_string();
    instance.port = 8080;
    instance.weight = 1.0;
    instance.healthy = true;
    instance.enabled = true;
    instance.ephemeral = true;
    instance.cluster_name = "DEFAULT".to_string();

    // Register instance
    naming_service
        .register_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance.clone())
        .await?;
    println!("✓ Registered instance for service: {}", service_name);

    // Get instances and verify
    let instances = naming_service
        .get_all_instances(TEST_NAMESPACE, TEST_GROUP, &service_name)
        .await?;
    assert!(!instances.is_empty());
    println!("✓ Found {} instance(s)", instances.len());

    // Deregister instance
    naming_service
        .deregister_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance)
        .await?;
    println!("✓ Deregistered instance");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_naming_service_subscribe_and_unsubscribe() -> anyhow::Result<()> {
    use batata_client::naming::listener::FnEventListener;

    let grpc_config = create_grpc_config("naming");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let naming_service = BatataNamingService::new(grpc_client);

    let service_name = format!("test-subscribe-{}", uuid::Uuid::new_v4());

    // Register an instance first
    let mut instance = Instance::default();
    instance.ip = "127.0.0.1".to_string();
    instance.port = 8080;
    instance.weight = 1.0;
    instance.healthy = true;
    instance.enabled = true;
    instance.ephemeral = true;
    instance.cluster_name = "DEFAULT".to_string();

    naming_service
        .register_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance.clone())
        .await?;

    // Create a simple listener
    let listener = FnEventListener::new(|_event| {
        // Event received
    });

    // Subscribe
    let instances = naming_service
        .subscribe(TEST_NAMESPACE, TEST_GROUP, &service_name, "DEFAULT", Arc::new(listener))
        .await?;
    assert!(!instances.is_empty());
    println!("✓ Subscribed to service: {}", service_name);

    // Unsubscribe
    naming_service
        .unsubscribe(TEST_NAMESPACE, TEST_GROUP, &service_name, "DEFAULT")
        .await?;
    println!("✓ Unsubscribed from service");

    // Cleanup
    naming_service
        .deregister_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_naming_service_multiple_instances() -> anyhow::Result<()> {
    let grpc_config = create_grpc_config("naming");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let naming_service = BatataNamingService::new(grpc_client);

    let service_name = format!("test-multi-{}", uuid::Uuid::new_v4());
    let mut instances = vec![];

    // Register multiple instances
    for i in 0..3 {
        let mut instance = Instance::default();
        instance.ip = "127.0.0.1".to_string();
        instance.port = 8080 + i as i32;
        instance.weight = 1.0;
        instance.healthy = true;
        instance.enabled = true;
        instance.ephemeral = true;
        instance.cluster_name = "DEFAULT".to_string();

        naming_service
            .register_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance.clone())
            .await?;
        instances.push(instance);
        println!("✓ Registered instance {}: 127.0.0.1:{}", i, 8080 + i);
    }

    // Get all instances
    let all_instances = naming_service
        .get_all_instances(TEST_NAMESPACE, TEST_GROUP, &service_name)
        .await?;
    assert_eq!(all_instances.len(), 3);
    println!("✓ Retrieved all {} instances", all_instances.len());

    // Deregister all instances
    for instance in instances {
        naming_service
            .deregister_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance)
            .await?;
    }
    println!("✓ Deregistered all instances");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_naming_service_get_instances_empty_service() -> anyhow::Result<()> {
    let grpc_config = create_grpc_config("naming");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;

    let naming_service = BatataNamingService::new(grpc_client);

    let service_name = format!("nonexistent-{}", uuid::Uuid::new_v4());

    // Get instances of non-existent service should return empty list
    let instances = naming_service
        .get_all_instances(TEST_NAMESPACE, TEST_GROUP, &service_name)
        .await?;
    assert!(instances.is_empty());
    println!("✓ Non-existent service returns empty instance list");

    Ok(())
}

// ============== Combined Tests ==============

#[tokio::test]
#[ignore]
async fn test_config_and_naming_services() -> anyhow::Result<()> {
    // Config operations
    let grpc_config = create_grpc_config("config");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;
    let config_service = BatataConfigService::new(grpc_client);

    let data_id = format!("test-combined-{}", uuid::Uuid::new_v4());
    let content = "config=value";

    config_service
        .publish_config(&data_id, TEST_GROUP, TEST_NAMESPACE, content)
        .await?;
    println!("✓ Published config");

    // Naming operations
    let grpc_config = create_grpc_config("naming");
    let grpc_client = Arc::new(GrpcClient::new(grpc_config)?);
    grpc_client.connect().await?;
    let naming_service = BatataNamingService::new(grpc_client);

    let service_name = format!("test-service-{}", uuid::Uuid::new_v4());
    let mut instance = Instance::default();
    instance.ip = "127.0.0.1".to_string();
    instance.port = 9090;
    instance.weight = 1.0;
    instance.healthy = true;
    instance.enabled = true;
    instance.ephemeral = true;
    instance.cluster_name = "DEFAULT".to_string();

    naming_service
        .register_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance.clone())
        .await?;
    println!("✓ Registered instance");

    // Verify both operations
    let retrieved_config = config_service
        .get_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;
    assert_eq!(retrieved_config, content);

    let instances = naming_service
        .get_all_instances(TEST_NAMESPACE, TEST_GROUP, &service_name)
        .await?;
    assert!(!instances.is_empty());

    // Cleanup
    config_service
        .remove_config(&data_id, TEST_GROUP, TEST_NAMESPACE)
        .await?;
    naming_service
        .deregister_instance(TEST_NAMESPACE, TEST_GROUP, &service_name, instance)
        .await?;

    println!("✓ All operations completed successfully");
    Ok(())
}
