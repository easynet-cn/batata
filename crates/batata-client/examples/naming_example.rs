//! Service discovery example for `batata-client` 1.0.0.
//!
//! Requires a running Batata server on `127.0.0.1:8848` with authentication
//! enabled (`nacos` / `nacos`).

use std::sync::Arc;
use std::time::Duration;

use batata_client::naming::listener::FnEventListener;
use batata_client::{BatataClient, ClientConfig, Instance};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ClientConfig::new("127.0.0.1:8848").with_auth("nacos", "nacos");
    let mut client = BatataClient::new(config).await?;
    let naming = client.naming_service().await?;

    let namespace = ""; // public namespace
    let group = "DEFAULT_GROUP";
    let service = "example-service";

    let mut instance = Instance::new("127.0.0.1".to_string(), 8080);
    instance.weight = 2.0;
    instance
        .metadata
        .insert("version".to_string(), "2.0.0".to_string());
    naming
        .register_instance(namespace, group, service, instance.clone())
        .await?;

    let listener = Arc::new(FnEventListener::new(|event| {
        println!(
            "{} now has {} instances",
            event.service_name,
            event.instances.len()
        );
    }));
    let initial = naming
        .subscribe(namespace, group, service, "", listener)
        .await?;
    println!("initial instances: {}", initial.len());

    let healthy = naming
        .select_instances(namespace, group, service, true)
        .await?;
    println!("healthy instances: {}", healthy.len());

    tokio::time::sleep(Duration::from_secs(30)).await;
    naming.unsubscribe(namespace, group, service, "").await?;
    naming
        .deregister_instance(namespace, group, service, instance)
        .await?;
    client.shutdown();
    Ok(())
}
