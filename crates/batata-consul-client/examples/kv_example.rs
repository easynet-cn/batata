//! KV, status and catalog smoke example for `batata-consul-client` 1.0.0.
//!
//! Expects a Consul-compatible endpoint. For a Batata server use
//! `ConsulClientConfig::new("http://127.0.0.1:8848")` instead.

use batata_consul_client::{ConsulClient, ConsulClientConfig, KVPair, QueryOptions, WriteOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConsulClientConfig::new("http://127.0.0.1:8500");
    let client = ConsulClient::new(config)?;

    let (leader, _) = client.status_leader(&QueryOptions::default()).await?;
    println!("leader: {leader}");

    // "key=value" in base64, matching Consul's JSON wire format.
    let pair = KVPair {
        key: "example/config".to_string(),
        value: Some("a2V5PXZhbHVl".to_string()),
        ..Default::default()
    };
    client.kv_put(&pair, &WriteOptions::default()).await?;

    let (stored, meta) = client
        .kv_get("example/config", &QueryOptions::default())
        .await?;
    println!(
        "value: {:?}, index: {}",
        stored.and_then(|p| p.value_str()),
        meta.last_index
    );

    let (keys, _) = client
        .kv_keys("example/", "", &QueryOptions::default())
        .await?;
    println!("keys under example/: {keys:?}");

    client
        .kv_delete("example/config", &WriteOptions::default())
        .await?;
    Ok(())
}
