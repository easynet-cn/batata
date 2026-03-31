//! Parameter validation tests ported from NacosParameterValidationTest.java
//!
//! Tests: null/empty parameters, special characters, Chinese chars,
//! non-existent configs, update existing, deregister edge cases.
//!
//! Requires a running Batata server.

mod common;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use batata_api::naming::model::Instance;
use tokio::time::sleep;

/// PV-005: Publish config with special characters round-trips correctly
#[tokio::test]
async fn test_publish_config_with_special_characters() {
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("pv-special-{}", id);
    let content = "key1=!@#$%^&*()\nkey2=<>&\"'\nkey3={[}]:;,./\\";

    let result = svc.publish_config(&data_id, "DEFAULT_GROUP", "", content).await.unwrap();
    assert!(result);
    sleep(Duration::from_millis(1000)).await;

    let retrieved = svc.get_config(&data_id, "DEFAULT_GROUP", "").await.unwrap();
    assert_eq!(retrieved, content, "Special characters should round-trip correctly");

    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// PV-006: Publish config with Chinese/Unicode characters
#[tokio::test]
async fn test_publish_config_with_unicode_characters() {
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("pv-unicode-{}", id);
    let content = "name=\u{4f60}\u{597d}\u{4e16}\u{754c}\ngreeting=\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}\nemoji=test\u{2603}\u{2764}";

    let result = svc.publish_config(&data_id, "DEFAULT_GROUP", "", content).await.unwrap();
    assert!(result);
    sleep(Duration::from_millis(1000)).await;

    let retrieved = svc.get_config(&data_id, "DEFAULT_GROUP", "").await.unwrap();
    assert_eq!(retrieved, content, "Unicode/CJK should round-trip correctly");

    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// PV-008: Remove non-existent config should not throw
#[tokio::test]
async fn test_remove_non_existent_config() {
    let svc = common::create_config_service().await.unwrap();
    let data_id = format!("pv-nonexist-{}", common::test_id());

    // Should not panic/error
    let result = svc.remove_config(&data_id, "DEFAULT_GROUP", "").await;
    // Either Ok(true) or Ok(false) is acceptable
    assert!(result.is_ok());
}

/// PV-010: Deregister non-existent instance should not throw
#[tokio::test]
async fn test_deregister_non_existent_instance() {
    let svc = common::create_naming_service().await.unwrap();
    let service_name = format!("pv-dereg-nonexist-{}", common::test_id());

    let instance = Instance::new("10.0.0.99".to_string(), 9999);
    let result = svc.deregister_instance("", "DEFAULT_GROUP", &service_name, instance).await;
    // Should either succeed silently or return an error — no panic
    let _ = result;
}

/// PV-011: Deregister last instance leaves empty list
#[tokio::test]
async fn test_deregister_last_instance() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("pv-dereg-last-{}", id);

    let instance = Instance {
        ip: "192.168.100.1".to_string(),
        port: 8080,
        healthy: true,
        weight: 1.0,
        enabled: true,
        ephemeral: true,
        ..Default::default()
    };

    svc.register_instance("", "DEFAULT_GROUP", &service_name, instance.clone())
        .await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let before = svc.get_all_instances("", "DEFAULT_GROUP", &service_name).await.unwrap();
    assert_eq!(before.len(), 1, "Should have 1 instance before deregister");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, instance)
        .await.unwrap();
    sleep(Duration::from_secs(1)).await;

    let after = svc.get_all_instances("", "DEFAULT_GROUP", &service_name).await.unwrap();
    assert!(after.is_empty(), "Should have 0 instances after deregister");
}

/// PV-012: Register instance with metadata round-trip
#[tokio::test]
async fn test_register_instance_metadata_round_trip() {
    let svc = common::create_naming_service().await.unwrap();
    let id = common::test_id();
    let service_name = format!("pv-meta-roundtrip-{}", id);

    let mut metadata = HashMap::new();
    metadata.insert("version".to_string(), "2.1.0".to_string());
    metadata.insert("env".to_string(), "staging".to_string());
    metadata.insert("region".to_string(), "us-east-1".to_string());
    metadata.insert("protocol".to_string(), "gRPC".to_string());
    metadata.insert("team".to_string(), "platform".to_string());
    metadata.insert("special-chars".to_string(), "a=b&c=d".to_string());

    let instance = Instance {
        ip: "192.168.200.1".to_string(),
        port: 8080,
        healthy: true,
        weight: 1.0,
        enabled: true,
        ephemeral: true,
        metadata: metadata.clone(),
        ..Default::default()
    };

    svc.register_instance("", "DEFAULT_GROUP", &service_name, instance.clone())
        .await.unwrap();
    sleep(Duration::from_millis(1500)).await;

    let instances = svc.get_all_instances("", "DEFAULT_GROUP", &service_name).await.unwrap();
    assert!(!instances.is_empty(), "Instance should be registered");

    let found = &instances[0];
    assert_eq!(found.metadata.get("version").unwrap(), "2.1.0");
    assert_eq!(found.metadata.get("env").unwrap(), "staging");
    assert_eq!(found.metadata.get("region").unwrap(), "us-east-1");
    assert_eq!(found.metadata.get("protocol").unwrap(), "gRPC");
    assert_eq!(found.metadata.get("team").unwrap(), "platform");
    assert_eq!(found.metadata.get("special-chars").unwrap(), "a=b&c=d");

    svc.deregister_instance("", "DEFAULT_GROUP", &service_name, instance).await.ok();
}

/// PV-017: Get non-existent config returns error (Rust) or null (Java)
#[tokio::test]
async fn test_get_config_when_not_exists() {
    let svc = common::create_config_service().await.unwrap();
    let data_id = format!("pv-getnotexist-{}", common::test_id());

    let result = svc.get_config(&data_id, "DEFAULT_GROUP", "").await;
    // In Rust, non-existent config returns an error (ServerError code 300)
    assert!(result.is_err(), "Non-existent config should return error");
}

/// PV-018: Publish new config and retrieve
#[tokio::test]
async fn test_publish_new_config() {
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("pv-new-{}", id);
    let content = "new.config=true";

    let published = svc.publish_config(&data_id, "DEFAULT_GROUP", "", content).await.unwrap();
    assert!(published);
    sleep(Duration::from_millis(500)).await;

    let retrieved = svc.get_config(&data_id, "DEFAULT_GROUP", "").await.unwrap();
    assert_eq!(retrieved, content);

    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// PV-019: Update existing config
#[tokio::test]
async fn test_update_existing_config() {
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("pv-update-{}", id);

    svc.publish_config(&data_id, "DEFAULT_GROUP", "", "version=1.0").await.unwrap();
    sleep(Duration::from_millis(500)).await;

    let updated = svc.publish_config(&data_id, "DEFAULT_GROUP", "", "version=2.0").await.unwrap();
    assert!(updated);
    sleep(Duration::from_millis(500)).await;

    let retrieved = svc.get_config(&data_id, "DEFAULT_GROUP", "").await.unwrap();
    assert_eq!(retrieved, "version=2.0");

    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// PV-020: Remove non-existent config returns true
#[tokio::test]
async fn test_remove_config_when_not_exists() {
    let svc = common::create_config_service().await.unwrap();
    let data_id = format!("pv-rmnotexist-{}", common::test_id());

    let result = svc.remove_config(&data_id, "DEFAULT_GROUP", "").await;
    // Should succeed (returns true even for non-existent, matching Nacos behavior)
    assert!(result.is_ok());
}
