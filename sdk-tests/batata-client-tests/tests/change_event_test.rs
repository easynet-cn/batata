//! Config Change Event Functional Tests
//!
//! Tests ConfigChangeEventListener for field-level diff notifications.

mod common;

use std::sync::{Arc, Mutex};
use batata_client::config::{
    BatataConfigService,
    change_parser::{ConfigChangeEvent, PropertyChangeType},
    listener::ConfigChangeEventListener,
};

const TENANT: &str = "";
const GROUP: &str = "DEFAULT_GROUP";

struct CaptureChangeListener {
    events: Arc<Mutex<Vec<ConfigChangeEvent>>>,
}

impl ConfigChangeEventListener for CaptureChangeListener {
    fn receive_config_change(&self, event: ConfigChangeEvent) {
        self.events.lock().unwrap().push(event);
    }
}

#[tokio::test]
#[ignore]
async fn test_change_event_added_property() {
    common::init_tracing();
    let grpc = common::create_config_grpc_client().await.unwrap();
    let svc = Arc::new(BatataConfigService::new(grpc));

    let data_id = format!("cfg-chg-add-{}", common::test_id());
    let events = Arc::new(Mutex::new(Vec::new()));

    // Publish initial
    svc.publish_config(&data_id, GROUP, TENANT, "key1=val1").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Add change event listener
    svc.add_change_event_listener(
        &data_id,
        GROUP,
        TENANT,
        "properties",
        Arc::new(CaptureChangeListener { events: events.clone() }),
    )
    .await
    .unwrap();

    // Update: add a property
    svc.publish_config(&data_id, GROUP, TENANT, "key1=val1\nkey2=val2")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let captured = events.lock().unwrap();
    assert!(!captured.is_empty(), "Should receive change event");
    if let Some(event) = captured.first() {
        let added = event.items.get("key2");
        assert!(added.is_some(), "key2 should be in change items");
        assert_eq!(added.unwrap().change_type, PropertyChangeType::Added);
        assert_eq!(added.unwrap().new_value.as_deref(), Some("val2"));
    }

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_change_event_modified_property() {
    common::init_tracing();
    let grpc = common::create_config_grpc_client().await.unwrap();
    let svc = Arc::new(BatataConfigService::new(grpc));

    let data_id = format!("cfg-chg-mod-{}", common::test_id());
    let events = Arc::new(Mutex::new(Vec::new()));

    svc.publish_config(&data_id, GROUP, TENANT, "key1=old").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    svc.add_change_event_listener(
        &data_id, GROUP, TENANT, "properties",
        Arc::new(CaptureChangeListener { events: events.clone() }),
    ).await.unwrap();

    // Modify
    svc.publish_config(&data_id, GROUP, TENANT, "key1=new").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let captured = events.lock().unwrap();
    if let Some(event) = captured.first() {
        let modified = event.items.get("key1");
        assert!(modified.is_some(), "key1 should be in change items");
        assert_eq!(modified.unwrap().change_type, PropertyChangeType::Modified);
        assert_eq!(modified.unwrap().old_value.as_deref(), Some("old"));
        assert_eq!(modified.unwrap().new_value.as_deref(), Some("new"));
    }

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}
