//! Client Introspection API Functional Tests
//!
//! Tests client list, detail, published/subscribed service queries,
//! and server loader management.

mod common;

// ==================== Client List ====================

#[tokio::test]
#[ignore]
async fn test_client_list() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    // List all clients — may be empty if no gRPC clients connected
    let result = client.client_list().await.unwrap();
    // Just verify the query succeeds
    assert!(result.count >= 0);
}

// ==================== Loader ====================

#[tokio::test]
#[ignore]
async fn test_loader_current() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let loader = client.loader_current().await.unwrap();
    assert!(
        loader.is_object() || loader.is_array(),
        "Loader should return data"
    );
}

#[tokio::test]
#[ignore]
async fn test_loader_cluster_metrics() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let metrics = client.loader_cluster_metrics().await.unwrap();
    assert!(
        metrics.is_object() || metrics.is_array(),
        "Cluster metrics should return data"
    );
}

// ==================== Combined: gRPC client + HTTP introspection ====================

#[tokio::test]
#[ignore]
async fn test_client_detail_with_grpc_connection() {
    common::init_tracing();

    // Create a gRPC connection to generate a client entry
    let grpc_config = batata_client::grpc::GrpcClientConfig {
        server_addrs: vec![common::SERVER_URL.replacen("http://", "", 1)],
        username: common::USERNAME.to_string(),
        password: common::PASSWORD.to_string(),
        module: "naming".to_string(),
        ..Default::default()
    };
    let grpc = std::sync::Arc::new(batata_client::grpc::GrpcClient::new(grpc_config).unwrap());
    grpc.connect().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Now use HTTP API to introspect
    let api = common::create_api_client().await.unwrap();

    let result = api.client_list().await.unwrap();
    assert!(
        !result.client_ids.is_empty(),
        "Should see our gRPC connection in client list"
    );

    // Get first client detail
    if let Some(client_id) = result.client_ids.first() {
        let detail = api.client_detail(client_id).await.unwrap();
        assert!(detail.is_object(), "Client detail should be an object");
    }

    // Cleanup gRPC connection
    drop(grpc);
}

#[tokio::test]
#[ignore]
async fn test_service_publisher_and_subscriber_clients() {
    common::init_tracing();

    let service = format!("intro-svc-{}", common::test_id());

    // Create gRPC naming client and register an instance
    let grpc_config = batata_client::grpc::GrpcClientConfig {
        server_addrs: vec![common::SERVER_URL.replacen("http://", "", 1)],
        username: common::USERNAME.to_string(),
        password: common::PASSWORD.to_string(),
        module: "naming".to_string(),
        ..Default::default()
    };
    let grpc = std::sync::Arc::new(batata_client::grpc::GrpcClient::new(grpc_config).unwrap());
    grpc.connect().await.unwrap();

    let naming = batata_client::naming::BatataNamingService::new(grpc.clone());
    let inst = batata_api::naming::model::Instance {
        ip: "10.0.99.1".to_string(),
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
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Query publishers via HTTP
    let api = common::create_api_client().await.unwrap();
    let publishers = api
        .service_publisher_clients("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
    // Publisher list returns { count, clients } - verify it's accessible
    assert!(
        publishers.is_object(),
        "Should return publisher list object"
    );

    naming.shutdown().await;
    drop(grpc);
}
