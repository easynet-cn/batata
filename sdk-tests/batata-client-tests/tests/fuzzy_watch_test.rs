//! Fuzzy Watch Functional Tests
//!
//! Tests config and naming fuzzy watch with initial key sync.

mod common;

use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use batata_client::config::{
    BatataConfigService,
    fuzzy_watch::{ConfigFuzzyWatchEvent, ConfigFuzzyWatchListener, ConfigFuzzyWatchService},
};

const TENANT: &str = "public";
const GROUP: &str = "DEFAULT_GROUP";

struct CountFuzzyListener {
    count: Arc<AtomicU32>,
}

impl ConfigFuzzyWatchListener for CountFuzzyListener {
    fn on_change(&self, _event: ConfigFuzzyWatchEvent) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn test_config_fuzzy_watch_with_keys() {
    common::init_tracing();
    let grpc = common::create_config_grpc_client().await.unwrap();
    let config_svc = BatataConfigService::new(grpc.clone());
    let fuzzy_svc = Arc::new(ConfigFuzzyWatchService::new(grpc.clone()));
    fuzzy_svc.register_push_handlers();

    let id = common::test_id();
    let data_id1 = format!("fz-cfg1-{}", id);
    let data_id2 = format!("fz-cfg2-{}", id);

    // Pre-create configs
    config_svc.publish_config(&data_id1, GROUP, TENANT, "v1").await.unwrap();
    config_svc.publish_config(&data_id2, GROUP, TENANT, "v2").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let count = Arc::new(AtomicU32::new(0));

    // Watch with pattern — should return initial matching keys
    let pattern = format!("{}>>{}>>*", TENANT, GROUP);
    let keys = fuzzy_svc
        .watch_with_keys(
            &pattern,
            Arc::new(CountFuzzyListener { count }),
            std::time::Duration::from_secs(10),
        )
        .await
        .unwrap();

    // Should contain our test configs (and possibly others)
    assert!(!keys.is_empty(), "Should return initial matched keys");

    // Cleanup
    config_svc.remove_config(&data_id1, GROUP, TENANT).await.unwrap();
    config_svc.remove_config(&data_id2, GROUP, TENANT).await.unwrap();
    config_svc.shutdown().await;
}
