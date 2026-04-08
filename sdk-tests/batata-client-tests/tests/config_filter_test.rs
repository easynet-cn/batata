// Config Filter Chain & Extended Methods Tests
//
// Tests get_config_with_result, filter_chain, select_healthy_instances,
// get_all_instances_with_subscribe.
// Requires a running Batata server.

mod common;

use std::sync::Arc;

const TENANT: &str = "";
const GROUP: &str = "DEFAULT_GROUP";

// ==================== get_config_with_result ====================

#[tokio::test]
async fn test_get_config_with_result() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-result-{}", common::test_id());
    let content = "result.key=value123";

    // Publish
    svc.publish_config(&data_id, GROUP, TENANT, content)
        .await
        .unwrap();

    // Get with result (returns ConfigQueryResult with content + metadata)
    let result = svc
        .get_config_with_result(&data_id, GROUP, TENANT)
        .await
        .unwrap();
    assert_eq!(
        result.content, content,
        "Content should match published value"
    );

    // Cleanup
    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

// ==================== select_healthy_instances ====================

#[tokio::test]
async fn test_select_healthy_instances() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("hlth-sel-{}", common::test_id());

    // Register a healthy instance
    let inst = batata_api::naming::model::Instance {
        ip: "10.0.70.1".to_string(),
        port: 8080,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        ..Default::default()
    };
    svc.register_instance("", GROUP, &service, inst)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // select_healthy_instances should only return healthy instances
    let healthy = svc
        .select_healthy_instances("", GROUP, &service)
        .await
        .unwrap();
    assert!(!healthy.is_empty(), "Should find healthy instances");
    assert_eq!(healthy[0].ip, "10.0.70.1");
    assert!(healthy[0].healthy, "Selected instances should be healthy");

    svc.shutdown().await;
}

// ==================== get_all_instances_with_subscribe ====================

#[tokio::test]
async fn test_get_all_instances_with_subscribe() {
    common::init_tracing();
    let svc = common::create_naming_service().await.unwrap();

    let service = format!("sub-all-{}", common::test_id());

    // Register instance
    let inst = batata_api::naming::model::Instance {
        ip: "10.0.80.1".to_string(),
        port: 9090,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        ..Default::default()
    };
    svc.register_instance("", GROUP, &service, inst)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Get with subscribe=true (enables push updates + uses cache)
    let instances = svc
        .get_all_instances_with_subscribe("", GROUP, &service, true)
        .await
        .unwrap();
    assert!(
        !instances.is_empty(),
        "Should find instances with subscribe=true"
    );
    assert_eq!(instances[0].ip, "10.0.80.1");
    assert_eq!(instances[0].port, 9090);

    // Get with subscribe=false (direct server query, no cache)
    let instances_no_sub = svc
        .get_all_instances_with_subscribe("", GROUP, &service, false)
        .await
        .unwrap();
    assert!(
        !instances_no_sub.is_empty(),
        "Should find instances with subscribe=false"
    );

    svc.shutdown().await;
}

// ==================== Filter Chain ====================

#[tokio::test]
async fn test_config_filter_chain() {
    common::init_tracing();

    use batata_client::config::filter::{
        ConfigFilterChainManager, ConfigRequest, ConfigResponse, IConfigFilter,
    };

    // Simple pass-through filter for testing
    struct TestFilter;

    #[async_trait::async_trait]
    impl IConfigFilter for TestFilter {
        fn order(&self) -> i32 {
            0
        }
        fn name(&self) -> &str {
            "test-filter"
        }
        async fn filter_publish(&self, _request: &mut ConfigRequest) -> anyhow::Result<()> {
            Ok(())
        }
        async fn filter_query(&self, _response: &mut ConfigResponse) -> anyhow::Result<()> {
            Ok(())
        }
    }

    let grpc = common::create_config_grpc_client().await.unwrap();
    let chain = ConfigFilterChainManager::new();
    let svc = Arc::new(batata_client::config::BatataConfigService::with_filter_chain(
        grpc, chain,
    ));
    svc.register_push_handlers();

    // Add filter dynamically
    svc.add_filter(Arc::new(TestFilter)).await;

    let data_id = format!("cfg-filter-{}", common::test_id());
    let content = "filtered.key=value";

    // Publish and get should work through filter chain
    svc.publish_config(&data_id, GROUP, TENANT, content)
        .await
        .unwrap();
    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(result, content);

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}
