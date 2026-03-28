//! Shared test utilities for batata-consul-client tests.
//!
//! All tests require a running Batata server with Consul compatibility enabled.
//! Start with: ./scripts/start-embedded.sh (with --batata.plugin.consul.enabled=true)

use batata_consul_client::{ConsulClient, ConsulClientConfig, QueryOptions, WriteOptions};

pub const CONSUL_ADDR: &str = "http://127.0.0.1:8500";

/// Create a default Consul client for testing
pub fn create_client() -> ConsulClient {
    ConsulClient::new(ConsulClientConfig::new(CONSUL_ADDR)).unwrap()
}

/// Generate a unique test ID to avoid collisions
pub fn test_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

/// Default query options
pub fn q() -> QueryOptions {
    QueryOptions::default()
}

/// Default write options
pub fn w() -> WriteOptions {
    WriteOptions::default()
}

/// Init tracing for test output
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
}
