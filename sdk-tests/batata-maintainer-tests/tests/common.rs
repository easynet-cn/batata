//! Shared test utilities for batata-maintainer functional tests.
#![allow(dead_code)] // Each test binary only uses part of this shared module.

use std::sync::Arc;

use batata_client::{
    api::BatataApiClient,
    grpc::GrpcClient,
    http::{BatataHttpClient, HttpClientConfig},
};

pub const SERVER_URL: &str = "http://127.0.0.1:8848";
pub const USERNAME: &str = "nacos";
pub const PASSWORD: &str = "nacos";

pub async fn create_api_client() -> anyhow::Result<BatataApiClient> {
    let config = HttpClientConfig::new(SERVER_URL)
        .with_auth(USERNAME, PASSWORD)
        .with_context_path("/nacos")
        .with_auth_endpoint("/nacos/v3/auth/user/login")
        .with_timeouts(5000, 30000);
    let http = BatataHttpClient::new(config).await?;
    Ok(BatataApiClient::new(http))
}

/// Return the server-assigned id of a connected gRPC client.
pub async fn grpc_client_id(grpc: &Arc<GrpcClient>) -> String {
    let guard = grpc.connection_ref().read().await;
    guard
        .as_ref()
        .expect("gRPC client must be connected before introspection")
        .connection_id()
        .to_string()
}

/// Wait until our own gRPC connection is visible in the admin client list.
///
/// Tests in this binary run in parallel and other clients may disconnect while
/// we introspect, so picking `client_ids[0]` can target a stale client. Always
/// query the id that belongs to the connection held by this test.
pub async fn wait_for_visible_client(api: &BatataApiClient, grpc: &Arc<GrpcClient>) -> String {
    let client_id = grpc_client_id(grpc).await;
    for _ in 0..20 {
        let listed = api.client_list().await.unwrap();
        if listed.client_ids.iter().any(|id| id == &client_id) {
            return client_id;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    panic!("gRPC client {client_id} did not appear in client_list within 10s");
}

pub fn test_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
}
