//! Shared test utilities for batata-maintainer functional tests.

use batata_client::{
    api::BatataApiClient,
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

pub fn test_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
}
