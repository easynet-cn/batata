//! Naming fuzzy watch manager for pattern-based service watching
//!
//! This module provides in-memory storage for fuzzy watch patterns and
//! methods to match services against registered patterns.

use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;

/// Naming fuzzy watch pattern
#[derive(Clone, Debug)]
pub struct NamingFuzzyWatchPattern {
    pub namespace: String,
    pub group_pattern: String,
    pub service_name_pattern: String,
    pub watch_type: String,
}

impl NamingFuzzyWatchPattern {
    /// Parse a groupKeyPattern in format: namespace+group+serviceName
    /// The pattern can contain * as wildcard
    pub fn from_group_key_pattern(group_key_pattern: &str) -> Option<Self> {
        let parts: Vec<&str> = group_key_pattern.split('+').collect();
        if parts.len() >= 3 {
            Some(Self {
                namespace: parts[0].to_string(),
                group_pattern: parts[1].to_string(),
                service_name_pattern: parts[2..].join("+"), // Handle serviceName with + in it
                watch_type: String::new(),
            })
        } else if parts.len() == 2 {
            // namespace+group (serviceName pattern is *)
            Some(Self {
                namespace: parts[0].to_string(),
                group_pattern: parts[1].to_string(),
                service_name_pattern: "*".to_string(),
                watch_type: String::new(),
            })
        } else {
            None
        }
    }

    /// Check if a service matches this pattern
    pub fn matches(&self, namespace: &str, group: &str, service_name: &str) -> bool {
        if self.namespace != namespace && self.namespace != "*" {
            return false;
        }

        let group_matches = Self::glob_match(&self.group_pattern, group);
        let service_name_matches = Self::glob_match(&self.service_name_pattern, service_name);

        group_matches && service_name_matches
    }

    /// Simple glob pattern matching (* matches any sequence)
    fn glob_match(pattern: &str, text: &str) -> bool {
        if pattern.is_empty() || pattern == "*" {
            return true;
        }

        // Use cached regex matching from batata-common
        batata_common::glob_matches(pattern, text)
    }

    /// Build group key from service identifiers
    pub fn build_group_key(namespace: &str, group: &str, service_name: &str) -> String {
        format!("{}+{}+{}", namespace, group, service_name)
    }
}

/// Manager for naming fuzzy watch patterns
#[derive(Clone)]
pub struct NamingFuzzyWatchManager {
    /// Key: connection_id, Value: list of watch patterns
    watchers: Arc<DashMap<String, Vec<NamingFuzzyWatchPattern>>>,
    /// Key: connection_id, Value: set of received group keys (for deduplication)
    received_keys: Arc<DashMap<String, HashSet<String>>>,
}

impl Default for NamingFuzzyWatchManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NamingFuzzyWatchManager {
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
        if let Some(mut pattern) = NamingFuzzyWatchPattern::from_group_key_pattern(group_key_pattern)
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

    /// Get connections watching a specific service
    pub fn get_watchers_for_service(
        &self,
        namespace: &str,
        group: &str,
        service_name: &str,
    ) -> Vec<String> {
        self.watchers
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .iter()
                    .any(|pattern| pattern.matches(namespace, group, service_name))
            })
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all patterns for a connection
    pub fn get_patterns(&self, connection_id: &str) -> Vec<NamingFuzzyWatchPattern> {
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
    fn test_pattern_from_group_key() {
        let pattern = NamingFuzzyWatchPattern::from_group_key_pattern("public+DEFAULT_GROUP+test-service");
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.namespace, "public");
        assert_eq!(p.group_pattern, "DEFAULT_GROUP");
        assert_eq!(p.service_name_pattern, "test-service");
    }

    #[test]
    fn test_pattern_from_group_key_with_wildcard() {
        let pattern = NamingFuzzyWatchPattern::from_group_key_pattern("public+DEFAULT_GROUP+*");
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.namespace, "public");
        assert_eq!(p.group_pattern, "DEFAULT_GROUP");
        assert_eq!(p.service_name_pattern, "*");
    }

    #[test]
    fn test_pattern_matches() {
        let pattern = NamingFuzzyWatchPattern::from_group_key_pattern("public+DEFAULT_GROUP+*").unwrap();
        assert!(pattern.matches("public", "DEFAULT_GROUP", "service1"));
        assert!(pattern.matches("public", "DEFAULT_GROUP", "service2"));
        assert!(!pattern.matches("dev", "DEFAULT_GROUP", "service1"));
    }

    #[test]
    fn test_pattern_wildcard_namespace() {
        let pattern = NamingFuzzyWatchPattern::from_group_key_pattern("*+*+*").unwrap();
        assert!(pattern.matches("public", "DEFAULT_GROUP", "service1"));
        assert!(pattern.matches("dev", "TEST_GROUP", "service2"));
    }

    #[test]
    fn test_build_group_key() {
        let key = NamingFuzzyWatchPattern::build_group_key("public", "DEFAULT_GROUP", "my-service");
        assert_eq!(key, "public+DEFAULT_GROUP+my-service");
    }

    #[test]
    fn test_manager_register_and_get() {
        let manager = NamingFuzzyWatchManager::new();
        assert!(manager.register_watch("conn1", "public+DEFAULT_GROUP+*", "watch1"));
        assert!(manager.register_watch("conn1", "public+*+my-service", "watch2"));

        let patterns = manager.get_patterns("conn1");
        assert_eq!(patterns.len(), 2);
    }

    #[test]
    fn test_manager_get_watchers() {
        let manager = NamingFuzzyWatchManager::new();
        manager.register_watch("conn1", "public+DEFAULT_GROUP+*", "watch1");
        manager.register_watch("conn2", "public+TEST_GROUP+*", "watch1");

        let watchers = manager.get_watchers_for_service("public", "DEFAULT_GROUP", "my-service");
        assert_eq!(watchers.len(), 1);
        assert!(watchers.contains(&"conn1".to_string()));

        let watchers = manager.get_watchers_for_service("public", "TEST_GROUP", "my-service");
        assert_eq!(watchers.len(), 1);
        assert!(watchers.contains(&"conn2".to_string()));
    }

    #[test]
    fn test_manager_unregister() {
        let manager = NamingFuzzyWatchManager::new();
        manager.register_watch("conn1", "public+DEFAULT_GROUP+*", "watch1");
        manager.register_watch("conn2", "public+*+my-service", "watch2");

        manager.unregister_connection("conn1");

        let patterns = manager.get_patterns("conn1");
        assert_eq!(patterns.len(), 0);

        let patterns = manager.get_patterns("conn2");
        assert_eq!(patterns.len(), 1);
    }
}
