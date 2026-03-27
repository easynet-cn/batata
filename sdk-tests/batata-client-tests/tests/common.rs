//! Shared test utilities for batata-client functional tests.
//!
//! All tests require a running Batata server.
//! Start with: ./scripts/start-embedded.sh
//! Init admin: ./scripts/init-admin.sh

use std::collections::HashMap;
use std::sync::Arc;

use batata_client::{
    grpc::{GrpcClient, GrpcClientConfig},
    http::{BatataHttpClient, HttpClientConfig},
};

pub const SERVER_ADDR: &str = "127.0.0.1:8848";
pub const SERVER_URL: &str = "http://127.0.0.1:8848";
pub const USERNAME: &str = "nacos";
pub const PASSWORD: &str = "nacos";

/// Create an HTTP client with auth
pub async fn create_http_client() -> anyhow::Result<BatataHttpClient> {
    let config = HttpClientConfig::new(SERVER_URL)
        .with_auth(USERNAME, PASSWORD)
        .with_timeouts(5000, 30000);
    BatataHttpClient::new(config).await
}

/// Create a gRPC client for config module
pub async fn create_config_grpc_client() -> anyhow::Result<Arc<GrpcClient>> {
    let config = GrpcClientConfig {
        server_addrs: vec![SERVER_ADDR.to_string()],
        username: USERNAME.to_string(),
        password: PASSWORD.to_string(),
        module: "config".to_string(),
        labels: HashMap::new(),
        ..Default::default()
    };
    let client = Arc::new(GrpcClient::new(config)?);
    client.connect().await?;
    Ok(client)
}

/// Create a gRPC client for naming module
pub async fn create_naming_grpc_client() -> anyhow::Result<Arc<GrpcClient>> {
    let config = GrpcClientConfig {
        server_addrs: vec![SERVER_ADDR.to_string()],
        username: USERNAME.to_string(),
        password: PASSWORD.to_string(),
        module: "naming".to_string(),
        labels: HashMap::new(),
        ..Default::default()
    };
    let client = Arc::new(GrpcClient::new(config)?);
    client.connect().await?;
    Ok(client)
}

/// Generate a unique test ID to avoid collisions
pub fn test_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

/// Init tracing for test output
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("batata_client=debug,info")
        .with_test_writer()
        .try_init();
}
