//! Config fuzzy watch manager for pattern-based config watching
//!
//! This module provides in-memory storage for fuzzy watch patterns and
//! methods to match configs against registered patterns. It also provides
//! notification functionality for config changes.

use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Config fuzzy watch pattern
#[derive(Clone, Debug)]
pub struct ConfigFuzzyWatchPattern {
    pub namespace: String,
    pub group_pattern: String,
    pub data_id_pattern: String,
    pub watch_type: String,
}

impl ConfigFuzzyWatchPattern {
    /// Parse a groupKeyPattern in format: namespace+group+dataId
    /// The pattern can contain * as wildcard
    pub fn from_group_key_pattern(group_key_pattern: &str) -> Option<Self> {
        let parts: Vec<&str> = group_key_pattern.split('+').collect();
        if parts.len() >= 3 {
            Some(Self {
                namespace: parts[0].to_string(),
                group_pattern: parts[1].to_string(),
                data_id_pattern: parts[2..].join("+"), // Handle dataId with + in it
                watch_type: String::new(),
            })
        } else if parts.len() == 2 {
            // namespace+group (dataId pattern is *)
            Some(Self {
                namespace: parts[0].to_string(),
                group_pattern: parts[1].to_string(),
                data_id_pattern: "*".to_string(),
                watch_type: String::new(),
            })
        } else {
            None
        }
    }

    /// Check if a config matches this pattern
    pub fn matches(&self, namespace: &str, group: &str, data_id: &str) -> bool {
        if self.namespace != namespace && self.namespace != "*" {
            return false;
        }

        let group_matches = Self::glob_match(&self.group_pattern, group);
        let data_id_matches = Self::glob_match(&self.data_id_pattern, data_id);

        group_matches && data_id_matches
    }

    /// Simple glob pattern matching (* matches any sequence)
    fn glob_match(pattern: &str, text: &str) -> bool {
        if pattern.is_empty() || pattern == "*" {
            return true;
        }

        // Use cached regex matching from batata-common
        batata_common::glob_matches(pattern, text)
    }

    /// Build group key from config identifiers
    pub fn build_group_key(namespace: &str, group: &str, data_id: &str) -> String {
        format!("{}+{}+{}", namespace, group, data_id)
    }
}

/// Configuration change event types
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigChangeType {
    /// Configuration was added
    Add,
    /// Configuration was modified
    Modify,
    /// Configuration was deleted
    Delete,
}

impl ConfigChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigChangeType::Add => "add",
            ConfigChangeType::Modify => "modify",
            ConfigChangeType::Delete => "delete",
        }
    }
}

impl std::fmt::Display for ConfigChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration change event for notification
#[derive(Clone, Debug)]
pub struct ConfigChangeEvent {
    /// Namespace
    pub namespace: String,
    /// Group name
    pub group: String,
    /// Data ID
    pub data_id: String,
    /// Change type
    pub change_type: ConfigChangeType,
    /// Configuration MD5 (for change detection)
    pub md5: Option<String>,
    /// Timestamp of the change
    pub timestamp: i64,
}

impl ConfigChangeEvent {
    /// Create a new config change event
    pub fn new(
        namespace: &str,
        group: &str,
        data_id: &str,
        change_type: ConfigChangeType,
    ) -> Self {
        Self {
            namespace: namespace.to_string(),
            group: group.to_string(),
            data_id: data_id.to_string(),
            change_type,
            md5: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create an add event
    pub fn add(namespace: &str, group: &str, data_id: &str) -> Self {
        Self::new(namespace, group, data_id, ConfigChangeType::Add)
    }

    /// Create a modify event
    pub fn modify(namespace: &str, group: &str, data_id: &str) -> Self {
        Self::new(namespace, group, data_id, ConfigChangeType::Modify)
    }

    /// Create a delete event
    pub fn delete(namespace: &str, group: &str, data_id: &str) -> Self {
        Self::new(namespace, group, data_id, ConfigChangeType::Delete)
    }

    /// Set MD5 hash
    pub fn with_md5(mut self, md5: &str) -> Self {
        self.md5 = Some(md5.to_string());
        self
    }

    /// Build group key
    pub fn group_key(&self) -> String {
        ConfigFuzzyWatchPattern::build_group_key(&self.namespace, &self.group, &self.data_id)
    }
}

/// Notification to send to a connection
#[derive(Clone, Debug)]
pub struct ConfigWatchNotification {
    /// Connection ID to notify
    pub connection_id: String,
    /// The change event
    pub event: ConfigChangeEvent,
}

/// Manager for config fuzzy watch patterns
#[derive(Clone)]
pub struct ConfigFuzzyWatchManager {
    /// Key: connection_id, Value: list of watch patterns
    watchers: Arc<DashMap<String, Vec<ConfigFuzzyWatchPattern>>>,
    /// Key: connection_id, Value: set of received group keys (for deduplication)
    received_keys: Arc<DashMap<String, HashSet<String>>>,
    /// Broadcast channel for config change events
    event_sender: broadcast::Sender<ConfigChangeEvent>,
}

impl Default for ConfigFuzzyWatchManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigFuzzyWatchManager {
    /// Default channel capacity for config change events
    const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CHANNEL_CAPACITY)
    }

    /// Create a new manager with custom channel capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let (event_sender, _) = broadcast::channel(capacity);
        Self {
            watchers: Arc::new(DashMap::new()),
            received_keys: Arc::new(DashMap::new()),
            event_sender,
        }
    }

    /// Register a fuzzy watch pattern for a connection
    pub fn register_watch(
        &self,
        connection_id: &str,
        group_key_pattern: &str,
        watch_type: &str,
    ) -> bool {
        if let Some(mut pattern) = ConfigFuzzyWatchPattern::from_group_key_pattern(group_key_pattern)
        {
            pattern.watch_type = watch_type.to_string();

            self.watchers
                .entry(connection_id.to_string())
                .or_default()
                .push(pattern);
            true
        } else {
            false
        }
    }

    /// Unregister all watch patterns for a connection
    pub fn unregister_connection(&self, connection_id: &str) {
        self.watchers.remove(connection_id);
        self.received_keys.remove(connection_id);
    }

    /// Mark a group key as received by a connection
    pub fn mark_received(&self, connection_id: &str, group_key: &str) {
        self.received_keys
            .entry(connection_id.to_string())
            .or_default()
            .insert(group_key.to_string());
    }

    /// Mark multiple group keys as received
    pub fn mark_received_batch(&self, connection_id: &str, group_keys: &HashSet<String>) {
        if let Some(mut entry) = self.received_keys.get_mut(connection_id) {
            entry.extend(group_keys.iter().cloned());
        } else {
            self.received_keys
                .insert(connection_id.to_string(), group_keys.clone());
        }
    }

    /// Check if a group key is already received by a connection
    pub fn is_received(&self, connection_id: &str, group_key: &str) -> bool {
        self.received_keys
            .get(connection_id)
            .map(|keys| keys.contains(group_key))
            .unwrap_or(false)
    }

    /// Get connections watching a specific config
    pub fn get_watchers_for_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
    ) -> Vec<String> {
        self.watchers
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .iter()
                    .any(|pattern| pattern.matches(namespace, group, data_id))
            })
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all patterns for a connection
    pub fn get_patterns(&self, connection_id: &str) -> Vec<ConfigFuzzyWatchPattern> {
        self.watchers
            .get(connection_id)
            .map(|entry| entry.value().clone())
            .unwrap_or_default()
    }

    /// Get total watcher count
    pub fn watcher_count(&self) -> usize {
        self.watchers.len()
    }

    /// Get total pattern count across all connections
    pub fn pattern_count(&self) -> usize {
        self.watchers
            .iter()
            .map(|entry| entry.value().len())
            .sum()
    }

    // ========== Notification Methods ==========

    /// Subscribe to config change events
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.event_sender.subscribe()
    }

    /// Notify about a config change
    ///
    /// This will:
    /// 1. Broadcast the event to all subscribers
    /// 2. Return the list of connection IDs that match the pattern
    pub fn notify_change(&self, event: ConfigChangeEvent) -> Vec<String> {
        let group_key = event.group_key();
        let watchers =
            self.get_watchers_for_config(&event.namespace, &event.group, &event.data_id);

        debug!(
            "Config change notification: {} {} - {} watchers",
            event.change_type,
            group_key,
            watchers.len()
        );

        // Broadcast the event
        if let Err(e) = self.event_sender.send(event) {
            warn!("Failed to broadcast config change event: {}", e);
        }

        watchers
    }

    /// Notify about a config add
    pub fn notify_add(&self, namespace: &str, group: &str, data_id: &str) -> Vec<String> {
        let event = ConfigChangeEvent::add(namespace, group, data_id);
        self.notify_change(event)
    }

    /// Notify about a config modification
    pub fn notify_modify(&self, namespace: &str, group: &str, data_id: &str) -> Vec<String> {
        let event = ConfigChangeEvent::modify(namespace, group, data_id);
        self.notify_change(event)
    }

    /// Notify about a config deletion
    pub fn notify_delete(&self, namespace: &str, group: &str, data_id: &str) -> Vec<String> {
        let event = ConfigChangeEvent::delete(namespace, group, data_id);
        self.notify_change(event)
    }

    /// Get all notifications for matching connections
    ///
    /// This returns the list of notifications that should be sent to each connection
    /// that matches the given config change event
    pub fn get_notifications(&self, event: &ConfigChangeEvent) -> Vec<ConfigWatchNotification> {
        let watchers =
            self.get_watchers_for_config(&event.namespace, &event.group, &event.data_id);

        watchers
            .into_iter()
            .map(|connection_id| ConfigWatchNotification {
                connection_id,
                event: event.clone(),
            })
            .collect()
    }

    /// Check if a config change should be notified to a specific connection
    pub fn should_notify(&self, connection_id: &str, event: &ConfigChangeEvent) -> bool {
        if let Some(patterns) = self.watchers.get(connection_id) {
            patterns
                .iter()
                .any(|p| p.matches(&event.namespace, &event.group, &event.data_id))
        } else {
            false
        }
    }

    /// Get the number of active event subscribers
    pub fn subscriber_count(&self) -> usize {
        self.event_sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_from_group_key_pattern() {
        let pattern = ConfigFuzzyWatchPattern::from_group_key_pattern("public+DEFAULT_GROUP+*");
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.namespace, "public");
        assert_eq!(p.group_pattern, "DEFAULT_GROUP");
        assert_eq!(p.data_id_pattern, "*");
    }

    #[test]
    fn test_pattern_matches() {
        let pattern = ConfigFuzzyWatchPattern {
            namespace: "public".to_string(),
            group_pattern: "DEFAULT_GROUP".to_string(),
            data_id_pattern: "app-*".to_string(),
            watch_type: String::new(),
        };

        assert!(pattern.matches("public", "DEFAULT_GROUP", "app-config"));
        assert!(pattern.matches("public", "DEFAULT_GROUP", "app-settings"));
        assert!(!pattern.matches("public", "DEFAULT_GROUP", "other-config"));
        assert!(!pattern.matches("private", "DEFAULT_GROUP", "app-config"));
    }

    #[test]
    fn test_manager_register_and_get_watchers() {
        let manager = ConfigFuzzyWatchManager::new();

        manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");
        manager.register_watch("conn-2", "public+*+*", "add");

        let watchers = manager.get_watchers_for_config("public", "DEFAULT_GROUP", "app-config");
        assert_eq!(watchers.len(), 2);

        let watchers2 = manager.get_watchers_for_config("public", "OTHER_GROUP", "other-config");
        assert_eq!(watchers2.len(), 1);
        assert!(watchers2.contains(&"conn-2".to_string()));
    }

    #[test]
    fn test_manager_unregister() {
        let manager = ConfigFuzzyWatchManager::new();

        manager.register_watch("conn-1", "public+DEFAULT_GROUP+*", "add");
        assert_eq!(manager.watcher_count(), 1);

        manager.unregister_connection("conn-1");
        assert_eq!(manager.watcher_count(), 0);
    }

    #[test]
    fn test_received_keys_tracking() {
        let manager = ConfigFuzzyWatchManager::new();

        let group_key = "public+DEFAULT_GROUP+app.yaml";
        assert!(!manager.is_received("conn-1", group_key));

        manager.mark_received("conn-1", group_key);
        assert!(manager.is_received("conn-1", group_key));
    }

    // Notification tests

    #[test]
    fn test_config_change_event() {
        let event = ConfigChangeEvent::add("public", "DEFAULT_GROUP", "app.yaml");
        assert_eq!(event.namespace, "public");
        assert_eq!(event.group, "DEFAULT_GROUP");
        assert_eq!(event.data_id, "app.yaml");
        assert_eq!(event.change_type, ConfigChangeType::Add);
        assert_eq!(event.group_key(), "public+DEFAULT_GROUP+app.yaml");
    }

    #[test]
    fn test_config_change_event_with_md5() {
        let event = ConfigChangeEvent::modify("public", "DEFAULT_GROUP", "app.yaml")
            .with_md5("abc123");
        assert_eq!(event.md5, Some("abc123".to_string()));
    }

    #[test]
    fn test_notify_change_returns_watchers() {
        let manager = ConfigFuzzyWatchManager::new();

        manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");
        manager.register_watch("conn-2", "public+*+*", "add");
        manager.register_watch("conn-3", "private+*+*", "add");

        let watchers = manager.notify_add("public", "DEFAULT_GROUP", "app-config");
        assert_eq!(watchers.len(), 2);
        assert!(watchers.contains(&"conn-1".to_string()));
        assert!(watchers.contains(&"conn-2".to_string()));
    }

    #[test]
    fn test_get_notifications() {
        let manager = ConfigFuzzyWatchManager::new();

        manager.register_watch("conn-1", "public+DEFAULT_GROUP+*", "add");
        manager.register_watch("conn-2", "public+*+*", "add");

        let event = ConfigChangeEvent::modify("public", "DEFAULT_GROUP", "app.yaml");
        let notifications = manager.get_notifications(&event);

        assert_eq!(notifications.len(), 2);
        for n in &notifications {
            assert_eq!(n.event.namespace, "public");
            assert_eq!(n.event.data_id, "app.yaml");
        }
    }

    #[test]
    fn test_should_notify() {
        let manager = ConfigFuzzyWatchManager::new();

        manager.register_watch("conn-1", "public+DEFAULT_GROUP+app-*", "add");

        let matching_event = ConfigChangeEvent::add("public", "DEFAULT_GROUP", "app-config");
        let non_matching_event = ConfigChangeEvent::add("public", "DEFAULT_GROUP", "other-config");

        assert!(manager.should_notify("conn-1", &matching_event));
        assert!(!manager.should_notify("conn-1", &non_matching_event));
        assert!(!manager.should_notify("conn-2", &matching_event)); // conn-2 not registered
    }

    #[tokio::test]
    async fn test_subscribe_receives_events() {
        let manager = ConfigFuzzyWatchManager::new();
        let mut receiver = manager.subscribe();

        manager.register_watch("conn-1", "public+*+*", "add");

        // Notify about a change
        manager.notify_add("public", "DEFAULT_GROUP", "app.yaml");

        // Should receive the event
        let event = receiver.try_recv();
        assert!(event.is_ok());
        let event = event.unwrap();
        assert_eq!(event.namespace, "public");
        assert_eq!(event.data_id, "app.yaml");
        assert_eq!(event.change_type, ConfigChangeType::Add);
    }

    #[test]
    fn test_change_type_display() {
        assert_eq!(ConfigChangeType::Add.as_str(), "add");
        assert_eq!(ConfigChangeType::Modify.as_str(), "modify");
        assert_eq!(ConfigChangeType::Delete.as_str(), "delete");
        assert_eq!(format!("{}", ConfigChangeType::Add), "add");
    }
}
