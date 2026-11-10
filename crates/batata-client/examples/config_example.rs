//! Configuration management example for `batata-client` 1.0.0.
//!
//! Requires a running Batata server on `127.0.0.1:8848` with authentication
//! enabled (`nacos` / `nacos`).

use std::sync::Arc;
use std::time::Duration;

use batata_client::config::listener::FnConfigChangeListener;
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ClientConfig::new("127.0.0.1:8848").with_auth("nacos", "nacos");
    let mut client = BatataClient::new(config).await?;
    let config_svc = client.config_service().await?;

    let data_id = "example.yaml";
    let group = "DEFAULT_GROUP";
    let tenant = ""; // empty tenant = public namespace

    config_svc
        .publish_config(data_id, group, tenant, "key=value")
        .await?;
    let content = config_svc.get_config(data_id, group, tenant).await?;
    println!("config content: {content}");

    let listener = Arc::new(FnConfigChangeListener::new(|info| {
        println!("{} changed: {}", info.data_id, info.content);
    }));
    config_svc
        .add_listener(data_id, group, tenant, listener)
        .await?;

    // Keep the process alive so server push notifications can arrive.
    tokio::time::sleep(Duration::from_secs(30)).await;
    config_svc.remove_listener(data_id, group, tenant).await?;
    client.shutdown();
    Ok(())
}
