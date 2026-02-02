//! Integration tests for Configuration service
//!
//! Tests configuration CRUD operations and related functionality.

use batata_server::service::config_fuzzy_watch::{ConfigFuzzyWatchManager, ConfigFuzzyWatchPattern};

// ============================================================================
// ConfigFuzzyWatchPattern Tests
// ============================================================================

#[test]
fn test_pattern_from_group_key_basic() {
    let pattern = ConfigFuzzyWatchPattern::from_group_key_pattern("public+DEFAULT_GROUP+app-*");
    assert!(pattern.is_some());

    let p = pattern.unwrap();
    assert_eq!(p.namespace, "public");
    assert_eq!(p.group_pattern, "DEFAULT_GROUP");
    assert_eq!(p.data_id_pattern, "app-*");
}

#[test]
fn test_pattern_from_group_key_wildcards() {
    let pattern = ConfigFuzzyWatchPattern::from_group_key_pattern("*+*+*");
    assert!(pattern.is_some());

    let p = pattern.unwrap();
    assert_eq!(p.namespace, "*");
    assert_eq!(p.group_pattern, "*");
    assert_eq!(p.data_id_pattern, "*");
}

#[test]
fn test_pattern_from_invalid_format() {
    // Single part - invalid
    let pattern = ConfigFuzzyWatchPattern::from_group_key_pattern("single");
    assert!(pattern.is_none());

    let pattern2 = ConfigFuzzyWatchPattern::from_group_key_pattern("");
    assert!(pattern2.is_none());
}

#[test]
fn test_pattern_from_two_parts() {
    // Two parts should work (dataId defaults to *)
    let pattern = ConfigFuzzyWatchPattern::from_group_key_pattern("public+DEFAULT_GROUP");
    assert!(pattern.is_some());

    let p = pattern.unwrap();
    assert_eq!(p.namespace, "public");
    assert_eq!(p.group_pattern, "DEFAULT_GROUP");
    assert_eq!(p.data_id_pattern, "*");
}

#[test]
fn test_pattern_matches_exact() {
    let pattern = ConfigFuzzyWatchPattern {
        namespace: "public".to_string(),
        group_pattern: "DEFAULT_GROUP".to_string(),
        data_id_pattern: "app.properties".to_string(),
        watch_type: String::new(),
    };

    assert!(pattern.matches("public", "DEFAULT_GROUP", "app.properties"));
    assert!(!pattern.matches("public", "DEFAULT_GROUP", "other.properties"));
    assert!(!pattern.matches("private", "DEFAULT_GROUP", "app.properties"));
}

#[test]
fn test_pattern_matches_wildcards() {
    let pattern = ConfigFuzzyWatchPattern {
        namespace: "public".to_string(),
        group_pattern: "*".to_string(),
        data_id_pattern: "app-*".to_string(),
        watch_type: String::new(),
    };

    assert!(pattern.matches("public", "DEFAULT_GROUP", "app-config"));
    assert!(pattern.matches("public", "OTHER_GROUP", "app-settings"));
    assert!(!pattern.matches("public", "DEFAULT_GROUP", "other-config"));
    assert!(!pattern.matches("private", "DEFAULT_GROUP", "app-config"));
}

#[test]
fn test_pattern_matches_all_wildcards() {
    let pattern = ConfigFuzzyWatchPattern {
        namespace: "*".to_string(),
        group_pattern: "*".to_string(),
        data_id_pattern: "*".to_string(),
        watch_type: String::new(),
    };

    assert!(pattern.matches("public", "DEFAULT_GROUP", "any-config"));
    assert!(pattern.matches("private", "CUSTOM_GROUP", "other-config"));
}

#[test]
fn test_build_group_key() {
    let key = ConfigFuzzyWatchPattern::build_group_key("public", "DEFAULT_GROUP", "app.properties");
    assert_eq!(key, "public+DEFAULT_GROUP+app.properties");
}

// ============================================================================
// ConfigFuzzyWatchManager Tests
// ============================================================================

#[test]
fn test_manager_register_watch() {
    let manager = ConfigFuzzyWatchManager::new();

    let registered = manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");
    assert!(registered);

    // Re-registering should also succeed (adds to existing)
    let re_registered = manager.register_watch("conn-1", "public+OTHER_GROUP+*", "update");
    assert!(re_registered);
}

#[test]
fn test_manager_register_invalid_pattern() {
    let manager = ConfigFuzzyWatchManager::new();

    let registered = manager.register_watch("conn-1", "invalid", "add");
    assert!(!registered);
}

#[test]
fn test_manager_get_watchers_for_config() {
    let manager = ConfigFuzzyWatchManager::new();

    manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");
    manager.register_watch("conn-2", "public+*+*", "add");
    manager.register_watch("conn-3", "private+DEFAULT_GROUP+app-*", "add");

    let watchers = manager.get_watchers_for_config("public", "DEFAULT_GROUP", "app-config");
    assert_eq!(watchers.len(), 2);
    assert!(watchers.contains(&"conn-1".to_string()));
    assert!(watchers.contains(&"conn-2".to_string()));

    let watchers2 = manager.get_watchers_for_config("public", "OTHER_GROUP", "other-config");
    assert_eq!(watchers2.len(), 1);
    assert!(watchers2.contains(&"conn-2".to_string()));

    let watchers3 = manager.get_watchers_for_config("private", "DEFAULT_GROUP", "app-config");
    assert_eq!(watchers3.len(), 1);
    assert!(watchers3.contains(&"conn-3".to_string()));
}

#[test]
fn test_manager_unregister_connection() {
    let manager = ConfigFuzzyWatchManager::new();

    manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");
    manager.register_watch("conn-1", "public+OTHER_GROUP+*", "add");
    manager.register_watch("conn-2", "public+DEFAULT_GROUP+app-*", "add");

    manager.unregister_connection("conn-1");

    let watchers = manager.get_watchers_for_config("public", "DEFAULT_GROUP", "app-config");
    assert_eq!(watchers.len(), 1);
    assert!(!watchers.contains(&"conn-1".to_string()));
    assert!(watchers.contains(&"conn-2".to_string()));
}

#[test]
fn test_manager_mark_received() {
    let manager = ConfigFuzzyWatchManager::new();

    manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");

    let group_key = ConfigFuzzyWatchPattern::build_group_key("public", "DEFAULT_GROUP", "app-config");

    // Initially not received
    assert!(!manager.is_received("conn-1", &group_key));

    // Mark as received
    manager.mark_received("conn-1", &group_key);

    // Now should be received
    assert!(manager.is_received("conn-1", &group_key));
}

#[test]
fn test_manager_mark_received_batch() {
    let manager = ConfigFuzzyWatchManager::new();

    manager.register_watch("conn-1", "public+*+*", "add");

    let group_keys: std::collections::HashSet<String> = vec![
        ConfigFuzzyWatchPattern::build_group_key("public", "DEFAULT_GROUP", "app-config"),
        ConfigFuzzyWatchPattern::build_group_key("public", "OTHER_GROUP", "other-config"),
    ]
    .into_iter()
    .collect();

    manager.mark_received_batch("conn-1", &group_keys);

    for key in &group_keys {
        assert!(manager.is_received("conn-1", key));
    }
}

#[test]
fn test_manager_watcher_count() {
    let manager = ConfigFuzzyWatchManager::new();

    assert_eq!(manager.watcher_count(), 0);

    manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");
    manager.register_watch("conn-2", "public+*+*", "add");

    assert_eq!(manager.watcher_count(), 2);

    manager.unregister_connection("conn-1");

    assert_eq!(manager.watcher_count(), 1);
}

#[test]
fn test_manager_pattern_count() {
    let manager = ConfigFuzzyWatchManager::new();

    assert_eq!(manager.pattern_count(), 0);

    manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");
    manager.register_watch("conn-1", "public+OTHER_GROUP+*", "add");
    manager.register_watch("conn-2", "public+DEFAULT_GROUP+app-*", "add"); // Same pattern

    // 3 total patterns registered (even though 2 are the same pattern for different connections)
    assert_eq!(manager.pattern_count(), 3);
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[test]
fn test_manager_concurrent_registration() {
    use std::sync::Arc;
    use std::thread;

    let manager = Arc::new(ConfigFuzzyWatchManager::new());
    let mut handles = vec![];

    for i in 0..10 {
        let manager_clone = manager.clone();
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let conn_id = format!("conn-{}-{}", i, j);
                let pattern = format!("public+GROUP_{}+data-*", i);
                manager_clone.register_watch(&conn_id, &pattern, "add");
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have registered many watchers
    assert!(manager.watcher_count() > 0);
}

#[test]
fn test_manager_concurrent_read_write() {
    use std::sync::Arc;
    use std::thread;

    let manager = Arc::new(ConfigFuzzyWatchManager::new());

    // Pre-register some watchers
    for i in 0..5 {
        manager.register_watch(&format!("conn-{}", i), "public+*+*", "add");
    }

    let mut handles = vec![];

    // Writers
    for i in 0..5 {
        let manager_clone = manager.clone();
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let conn_id = format!("writer-{}-{}", i, j);
                manager_clone.register_watch(&conn_id, "public+DEFAULT_GROUP+app-*", "add");
            }
        }));
    }

    // Readers
    for _ in 0..5 {
        let manager_clone = manager.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = manager_clone.get_watchers_for_config("public", "DEFAULT_GROUP", "app-config");
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
