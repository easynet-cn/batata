//! Config fuzzy watch manager for pattern-based config watching
//!
//! This module provides in-memory storage for fuzzy watch patterns and
//! methods to match configs against registered patterns.

use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use regex::Regex;

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

        // Convert glob to regex: * -> .*, ? -> .
        let regex_pattern = format!(
            "^{}$",
            regex::escape(pattern)
                .replace("\\*", ".*")
                .replace("\\?", ".")
        );

        Regex::new(&regex_pattern)
            .map(|re| re.is_match(text))
            .unwrap_or(false)
    }

    /// Build group key from config identifiers
    pub fn build_group_key(namespace: &str, group: &str, data_id: &str) -> String {
        format!("{}+{}+{}", namespace, group, data_id)
    }
}

/// Manager for config fuzzy watch patterns
#[derive(Clone)]
pub struct ConfigFuzzyWatchManager {
    /// Key: connection_id, Value: list of watch patterns
    watchers: Arc<DashMap<String, Vec<ConfigFuzzyWatchPattern>>>,
    /// Key: connection_id, Value: set of received group keys (for deduplication)
    received_keys: Arc<DashMap<String, HashSet<String>>>,
}

impl Default for ConfigFuzzyWatchManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigFuzzyWatchManager {
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(DashMap::new()),
            received_keys: Arc::new(DashMap::new()),
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
}
