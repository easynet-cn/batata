//! Fuzzy watch pattern operations

use std::collections::HashSet;

use super::{FuzzyWatchPattern, NamingService, parse_service_key};

impl NamingService {
    /// Register a fuzzy watch pattern for a connection
    pub fn register_fuzzy_watch(
        &self,
        connection_id: &str,
        namespace: &str,
        group_pattern: &str,
        service_pattern: &str,
        watch_type: &str,
    ) {
        let pattern = FuzzyWatchPattern {
            namespace: namespace.to_string(),
            group_pattern: group_pattern.to_string(),
            service_pattern: service_pattern.to_string(),
            watch_type: watch_type.to_string(),
        };

        self.fuzzy_watchers
            .entry(connection_id.to_string())
            .or_default()
            .push(pattern);
    }

    /// Unregister fuzzy watch patterns for a connection
    pub fn unregister_fuzzy_watch(&self, connection_id: &str) {
        self.fuzzy_watchers.remove(connection_id);
    }

    /// Get all services matching a fuzzy watch pattern
    pub fn get_services_by_pattern(
        &self,
        namespace: &str,
        group_pattern: &str,
        service_pattern: &str,
    ) -> Vec<String> {
        let pattern = FuzzyWatchPattern {
            namespace: namespace.to_string(),
            group_pattern: group_pattern.to_string(),
            service_pattern: service_pattern.to_string(),
            watch_type: String::new(),
        };

        self.services
            .iter()
            .filter_map(|entry| {
                let key = entry.key();
                if let Some((ns, group, service)) = parse_service_key(key)
                    && pattern.matches(ns, group, service)
                {
                    return Some(key.clone());
                }
                None
            })
            .collect()
    }

    /// Get all service keys that a connection is fuzzy-watching
    pub fn get_fuzzy_watched_services(&self, connection_id: &str) -> Vec<String> {
        // Clone patterns to release fuzzy_watchers lock before iterating services
        let patterns: Vec<FuzzyWatchPattern> = match self.fuzzy_watchers.get(connection_id) {
            Some(entry) => entry.clone(),
            None => return vec![],
        };
        // --- fuzzy_watchers lock released ---

        // Snapshot service keys to release services lock before pattern matching
        let service_keys: Vec<String> = self
            .services
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        // --- services locks released ---

        // Pattern matching on owned data (no locks held)
        let mut matched_services = HashSet::new();
        for pattern in &patterns {
            for key in &service_keys {
                if let Some((ns, group, service)) = parse_service_key(key)
                    && pattern.matches(ns, group, service)
                {
                    matched_services.insert(key.clone());
                }
            }
        }

        matched_services.into_iter().collect()
    }

    /// Get connections watching a specific service through fuzzy patterns
    pub fn get_fuzzy_watchers_for_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<String> {
        self.fuzzy_watchers
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .iter()
                    .any(|pattern| pattern.matches(namespace, group_name, service_name))
            })
            .map(|entry| entry.key().clone())
            .collect()
    }
}
