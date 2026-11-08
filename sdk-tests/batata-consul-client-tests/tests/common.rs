//! Shared test utilities for batata-consul-client tests.
//!
//! All tests require a running Batata server with Consul compatibility enabled.
//! Start with: ./scripts/startup.sh -m standalone -p embedded (add --batata.plugin.consul.enabled=true)

// Each integration test file compiles common.rs as its own module, so Rust
// warns about helpers that a given test happens not to use. Suppress those
// false positives — the helpers ARE used across the test suite as a whole.
#![allow(dead_code)]

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

/// Poll until a KV key is visible (or absent) after a write.
///
/// Cluster-mode writes are replicated through Raft; a read issued immediately
/// after the write can legitimately land before the entry is applied locally.
/// The Go SDK suite already uses this pattern (`waitForKV` / `waitForKVAbsent`).
pub async fn wait_for_kv(client: &ConsulClient, key: &str, present: bool) -> bool {
    for _ in 0..50 {
        if let Ok((pair, _)) = client.kv_get(key, &q()).await {
            if pair.is_some() == present {
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}

/// Poll until `/v1/status/leader` reports a non-empty leader.
pub async fn wait_for_leader(client: &ConsulClient) -> bool {
    for _ in 0..50 {
        if let Ok((leader, _)) = client
            .raw_query::<serde_json::Value>("/v1/status/leader", &q())
            .await
        {
            if leader.as_str().map(|s| !s.is_empty()).unwrap_or(false) {
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}

/// Init tracing for test output
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
}
