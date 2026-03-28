//! Cluster & Server Management Extended Tests
//!
//! Tests cluster management, loader, selector types, and naming metrics.
//! Covers API methods not tested in core_admin_test.

mod common;

// ==================== Cluster Info ====================

#[tokio::test]
#[ignore]
async fn test_cluster_healthy_members() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let members = client.cluster_healthy_members().await.unwrap();
    assert!(!members.is_empty(), "Should have at least one healthy member");
    assert_eq!(members[0].state, "UP", "Healthy member should be UP");
}

#[tokio::test]
#[ignore]
async fn test_cluster_member_by_address() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    // Get self address first
    let self_info = client.cluster_self().await.unwrap();
    let address = &self_info.address;
    assert!(!address.is_empty());

    // Get member by address
    let member = client.cluster_member(address).await.unwrap();
    assert!(member.is_some(), "Should find member by address");
    let m = member.unwrap();
    assert_eq!(m.address, *address, "Address should match");
    assert_eq!(m.state, "UP");
}

#[tokio::test]
#[ignore]
async fn test_cluster_member_count() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let count = client.cluster_member_count().await.unwrap();
    assert!(count >= 1, "Should have at least 1 member");
}

#[tokio::test]
#[ignore]
async fn test_cluster_is_standalone() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let standalone = client.cluster_is_standalone().await.unwrap();
    // Embedded mode is standalone
    assert!(standalone, "Embedded mode should be standalone");
}

#[tokio::test]
#[ignore]
async fn test_cluster_health() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let health = client.cluster_health().await.unwrap();
    assert!(health.member_count >= 1, "Should have at least 1 member");
    assert!(health.healthy_count >= 1, "Should have at least 1 healthy member");
    assert_eq!(health.server_status, "UP", "Server should be UP");
}

#[tokio::test]
#[ignore]
async fn test_cluster_refresh_self() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let ok = client.cluster_refresh_self().await.unwrap();
    assert!(ok, "Refresh self should succeed");
}

// ==================== Loader Management ====================

#[tokio::test]
#[ignore]
async fn test_loader_current_detail() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let loader = client.loader_current().await.unwrap();
    assert!(
        loader.is_object() || loader.is_array(),
        "Loader current should return data"
    );
}

#[tokio::test]
#[ignore]
async fn test_loader_cluster_metrics_detail() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let metrics = client.loader_cluster_metrics().await.unwrap();
    assert!(
        metrics.is_object() || metrics.is_array(),
        "Cluster metrics should return data"
    );
}

// ==================== Service Selector Types ====================

#[tokio::test]
#[ignore]
async fn test_service_selector_types() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let result = client.service_selector_types().await;
    // May or may not return data, just verify the endpoint works
    assert!(result.is_ok(), "Selector types query should succeed");
}

// ==================== Naming Metrics Detail ====================

#[tokio::test]
#[ignore]
async fn test_naming_metrics_keys() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let metrics = client.naming_metrics().await.unwrap();
    assert!(metrics.is_object(), "Naming metrics should be an object");
    // Verify some expected metric fields exist
    assert!(
        metrics.get("serviceCount").is_some()
            || metrics.get("instanceCount").is_some()
            || metrics.as_object().map(|m| !m.is_empty()).unwrap_or(false),
        "Naming metrics should contain some data"
    );
}

// ==================== Client Introspection Extended ====================

#[tokio::test]
#[ignore]
async fn test_client_published_services() {
    common::init_tracing();

    // Create a gRPC naming connection and register an instance
    let grpc_config = batata_client::grpc::GrpcClientConfig {
        server_addrs: vec![common::SERVER_URL.replacen("http://", "", 1)],
        username: common::USERNAME.to_string(),
        password: common::PASSWORD.to_string(),
        module: "naming".to_string(),
        ..Default::default()
    };
    let grpc = std::sync::Arc::new(batata_client::grpc::GrpcClient::new(grpc_config).unwrap());
    grpc.connect().await.unwrap();

    let naming = std::sync::Arc::new(batata_client::naming::BatataNamingService::new(grpc.clone()));
    naming.register_push_handlers();

    let service = format!("pub-svc-{}", common::test_id());
    let inst = batata_api::naming::model::Instance {
        ip: "10.0.50.1".to_string(),
        port: 8080,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        ..Default::default()
    };
    naming
        .register_instance("public", "DEFAULT_GROUP", &service, inst)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Query via HTTP
    let api = common::create_api_client().await.unwrap();
    let clients = api.client_list().await.unwrap();

    if !clients.client_ids.is_empty() {
        let client_id = &clients.client_ids[0];
        let published = api.client_published_services(client_id).await;
        assert!(published.is_ok(), "Published services query should succeed");
    }

    naming.shutdown().await;
    drop(grpc);
}

#[tokio::test]
#[ignore]
async fn test_client_subscribed_services() {
    common::init_tracing();

    let grpc_config = batata_client::grpc::GrpcClientConfig {
        server_addrs: vec![common::SERVER_URL.replacen("http://", "", 1)],
        username: common::USERNAME.to_string(),
        password: common::PASSWORD.to_string(),
        module: "naming".to_string(),
        ..Default::default()
    };
    let grpc = std::sync::Arc::new(batata_client::grpc::GrpcClient::new(grpc_config).unwrap());
    grpc.connect().await.unwrap();

    let naming = std::sync::Arc::new(batata_client::naming::BatataNamingService::new(grpc.clone()));
    naming.register_push_handlers();

    let service = format!("sub-svc-{}", common::test_id());

    struct NoopListener;
    impl batata_client::naming::listener::EventListener for NoopListener {
        fn on_event(&self, _: batata_client::naming::listener::NamingEvent) {}
    }

    naming
        .subscribe("public", "DEFAULT_GROUP", &service, "", std::sync::Arc::new(NoopListener))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let api = common::create_api_client().await.unwrap();
    let clients = api.client_list().await.unwrap();

    if !clients.client_ids.is_empty() {
        let client_id = &clients.client_ids[0];
        let subscribed = api.client_subscribed_services(client_id).await;
        assert!(subscribed.is_ok(), "Subscribed services query should succeed");
    }

    naming.shutdown().await;
    drop(grpc);
}
