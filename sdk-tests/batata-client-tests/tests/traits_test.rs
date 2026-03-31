//! SDK trait contract and typed response tests (no server required).
//!
//! Tests the ConfigService/NamingService trait definitions,
//! ConfigQueryResult, ListView types, and trait object safety.

use batata_client::traits::{ConfigQueryResult, ListView, DEFAULT_TIMEOUT_MS};

// ============================================================================
// ConfigQueryResult tests
// ============================================================================

#[test]
fn test_config_query_result_default() {
    let r = ConfigQueryResult::default();
    assert!(r.content.is_empty());
    assert!(r.md5.is_empty());
    assert!(r.config_type.is_empty());
    assert!(r.encrypted_data_key.is_empty());
}

#[test]
fn test_config_query_result_with_values() {
    let r = ConfigQueryResult {
        content: "server.port=8080\nserver.host=localhost".to_string(),
        md5: "a1b2c3d4e5f6".to_string(),
        config_type: "properties".to_string(),
        encrypted_data_key: "".to_string(),
    };
    assert_eq!(r.content, "server.port=8080\nserver.host=localhost");
    assert_eq!(r.md5, "a1b2c3d4e5f6");
    assert_eq!(r.config_type, "properties");
    assert!(r.encrypted_data_key.is_empty());
}

#[test]
fn test_config_query_result_clone() {
    let r = ConfigQueryResult {
        content: "data".to_string(),
        md5: "md5hash".to_string(),
        config_type: "yaml".to_string(),
        encrypted_data_key: "key123".to_string(),
    };
    let cloned = r.clone();
    assert_eq!(cloned.content, r.content);
    assert_eq!(cloned.md5, r.md5);
    assert_eq!(cloned.config_type, r.config_type);
    assert_eq!(cloned.encrypted_data_key, r.encrypted_data_key);
}

#[test]
fn test_config_query_result_debug() {
    let r = ConfigQueryResult {
        content: "test".to_string(),
        md5: "abc".to_string(),
        config_type: "json".to_string(),
        encrypted_data_key: "".to_string(),
    };
    let debug = format!("{:?}", r);
    assert!(debug.contains("ConfigQueryResult"));
    assert!(debug.contains("json"));
}

#[test]
fn test_config_query_result_with_encrypted_key() {
    let r = ConfigQueryResult {
        content: "encrypted content".to_string(),
        md5: "hash".to_string(),
        config_type: "text".to_string(),
        encrypted_data_key: "AES256-KEY-DATA".to_string(),
    };
    assert!(!r.encrypted_data_key.is_empty());
    assert_eq!(r.encrypted_data_key, "AES256-KEY-DATA");
}

// ============================================================================
// ListView tests
// ============================================================================

#[test]
fn test_list_view_default() {
    let v = ListView::<String>::default();
    assert_eq!(v.count, 0);
    assert!(v.data.is_empty());
}

#[test]
fn test_list_view_with_strings() {
    let v = ListView {
        count: 100,
        data: vec![
            "service-a".to_string(),
            "service-b".to_string(),
            "service-c".to_string(),
        ],
    };
    assert_eq!(v.count, 100); // Total count across all pages
    assert_eq!(v.data.len(), 3); // Items in this page
    assert_eq!(v.data[0], "service-a");
    assert_eq!(v.data[2], "service-c");
}

#[test]
fn test_list_view_with_integers() {
    let v = ListView {
        count: 5,
        data: vec![1, 2, 3, 4, 5],
    };
    assert_eq!(v.count, 5);
    assert_eq!(v.data.iter().sum::<i32>(), 15);
}

#[test]
fn test_list_view_clone() {
    let v = ListView {
        count: 3,
        data: vec!["a".to_string(), "b".to_string()],
    };
    let cloned = v.clone();
    assert_eq!(cloned.count, v.count);
    assert_eq!(cloned.data, v.data);
}

#[test]
fn test_list_view_pagination_semantics() {
    // Simulate page 2 of 3, page_size=2
    let page2 = ListView {
        count: 5, // total across all pages
        data: vec!["svc-3".to_string(), "svc-4".to_string()], // this page only
    };
    assert_eq!(page2.count, 5);
    assert_eq!(page2.data.len(), 2);
}

// ============================================================================
// Default timeout constant test
// ============================================================================

#[test]
fn test_default_timeout_ms() {
    assert_eq!(DEFAULT_TIMEOUT_MS, 3000);
    let timeout = std::time::Duration::from_millis(DEFAULT_TIMEOUT_MS);
    assert_eq!(timeout.as_secs(), 3);
}

// ============================================================================
// Trait object safety tests
// ============================================================================

// These tests verify that ConfigService and NamingService traits are object-safe
// (can be used as `dyn ConfigService` / `dyn NamingService`)

#[test]
fn test_config_service_is_object_safe() {
    // This compiles only if ConfigService is object-safe
    fn _accept_config_service(_svc: &dyn batata_client::ConfigService) {}
}

#[test]
fn test_naming_service_is_object_safe() {
    // This compiles only if NamingService is object-safe
    fn _accept_naming_service(_svc: &dyn batata_client::NamingService) {}
}

// ============================================================================
// Trait implementation verification (compile-time)
// ============================================================================

/// Verify BatataConfigService implements ConfigService trait
#[test]
fn test_batata_config_service_implements_trait() {
    fn _assert_impl<T: batata_client::ConfigService>() {}
    _assert_impl::<batata_client::BatataConfigService>();
}

/// Verify BatataNamingService implements NamingService trait
#[test]
fn test_batata_naming_service_implements_trait() {
    fn _assert_impl<T: batata_client::NamingService>() {}
    _assert_impl::<batata_client::BatataNamingService>();
}

/// Verify trait objects can be created from concrete types
#[test]
fn test_config_service_as_trait_object() {
    use std::sync::Arc;

    let config = batata_client::GrpcClientConfig::default();
    let client = Arc::new(batata_client::GrpcClient::new(config).unwrap());
    let svc = batata_client::BatataConfigService::new(client);

    // Should compile: concrete type → trait object
    let _trait_obj: &dyn batata_client::ConfigService = &svc;
}

#[test]
fn test_naming_service_as_trait_object() {
    use std::sync::Arc;

    let config = batata_client::GrpcClientConfig::default();
    let client = Arc::new(batata_client::GrpcClient::new(config).unwrap());
    let svc = batata_client::BatataNamingService::new(client);

    // Should compile: concrete type → trait object
    let _trait_obj: &dyn batata_client::NamingService = &svc;
}
