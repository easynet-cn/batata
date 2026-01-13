//! Config subscriber management
//!
//! Tracks which connections are listening to which configurations.
//! Used by both gRPC handlers and console API endpoints.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Key for a configuration: (dataId, group, tenant/namespace)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConfigKey {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
}

impl ConfigKey {
    pub fn new(data_id: &str, group: &str, tenant: &str) -> Self {
        Self {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
        }
    }

    /// Create a unique key string for internal storage
    pub fn to_key_string(&self) -> String {
        format!("{}@@{}@@{}", self.tenant, self.group, self.data_id)
    }
}

/// Information about a config listener/subscriber
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigSubscriber {
    /// Connection ID of the subscriber
    pub connection_id: String,
    /// Client IP address
    pub client_ip: String,
    /// MD5 checksum the client has
    pub md5: String,
}

/// Manages config subscriptions across all connections
#[derive(Clone)]
pub struct ConfigSubscriberManager {
    /// Map from config key string to set of subscribers
    /// Key: "tenant@@group@@dataId", Value: Map of connection_id -> ConfigSubscriber
    config_subscribers: Arc<DashMap<String, HashMap<String, ConfigSubscriber>>>,

    /// Map from connection_id to set of config keys
    /// Used for efficient cleanup when a connection disconnects
    connection_configs: Arc<DashMap<String, HashSet<String>>>,
}

impl Default for ConfigSubscriberManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigSubscriberManager {
    pub fn new() -> Self {
        Self {
            config_subscribers: Arc::new(DashMap::new()),
            connection_configs: Arc::new(DashMap::new()),
        }
    }

    /// Register a subscription for a configuration
    pub fn subscribe(
        &self,
        connection_id: &str,
        client_ip: &str,
        config_key: &ConfigKey,
        md5: &str,
    ) {
        let key_string = config_key.to_key_string();

        // Add subscriber to config
        self.config_subscribers
            .entry(key_string.clone())
            .or_default()
            .insert(
                connection_id.to_string(),
                ConfigSubscriber {
                    connection_id: connection_id.to_string(),
                    client_ip: client_ip.to_string(),
                    md5: md5.to_string(),
                },
            );

        // Track which configs this connection is subscribed to
        self.connection_configs
            .entry(connection_id.to_string())
            .or_default()
            .insert(key_string);
    }

    /// Unsubscribe from a specific configuration
    pub fn unsubscribe(&self, connection_id: &str, config_key: &ConfigKey) {
        let key_string = config_key.to_key_string();

        // Remove subscriber from config
        if let Some(mut subscribers) = self.config_subscribers.get_mut(&key_string) {
            subscribers.remove(connection_id);
            // Clean up empty entries
            if subscribers.is_empty() {
                drop(subscribers);
                self.config_subscribers.remove(&key_string);
            }
        }

        // Remove config from connection's subscription list
        if let Some(mut configs) = self.connection_configs.get_mut(connection_id) {
            configs.remove(&key_string);
        }
    }

    /// Unsubscribe from all configurations for a connection (called on disconnect)
    pub fn unsubscribe_all(&self, connection_id: &str) {
        // Get all configs this connection was subscribed to
        if let Some((_, config_keys)) = self.connection_configs.remove(connection_id) {
            // Remove this connection from each config's subscriber list
            for key_string in config_keys {
                if let Some(mut subscribers) = self.config_subscribers.get_mut(&key_string) {
                    subscribers.remove(connection_id);
                    if subscribers.is_empty() {
                        drop(subscribers);
                        self.config_subscribers.remove(&key_string);
                    }
                }
            }
        }
    }

    /// Get all subscribers for a specific configuration
    pub fn get_subscribers(&self, config_key: &ConfigKey) -> Vec<ConfigSubscriber> {
        let key_string = config_key.to_key_string();

        self.config_subscribers
            .get(&key_string)
            .map(|subscribers| subscribers.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all subscribers by client IP
    pub fn get_subscribers_by_ip(&self, client_ip: &str) -> Vec<(ConfigKey, ConfigSubscriber)> {
        let mut result = Vec::new();

        for entry in self.config_subscribers.iter() {
            let key_string = entry.key();
            for subscriber in entry.value().values() {
                if subscriber.client_ip == client_ip {
                    // Parse key string back to ConfigKey
                    if let Some(config_key) = Self::parse_key_string(key_string) {
                        result.push((config_key, subscriber.clone()));
                    }
                }
            }
        }

        result
    }

    /// Get all subscriptions with their listeners (for console display)
    pub fn get_all_subscriptions(&self) -> Vec<(ConfigKey, Vec<ConfigSubscriber>)> {
        self.config_subscribers
            .iter()
            .filter_map(|entry| {
                Self::parse_key_string(entry.key())
                    .map(|key| (key, entry.value().values().cloned().collect()))
            })
            .collect()
    }

    /// Get total number of subscriptions
    pub fn subscription_count(&self) -> usize {
        self.config_subscribers
            .iter()
            .map(|e| e.value().len())
            .sum()
    }

    /// Get number of unique configs being watched
    pub fn config_count(&self) -> usize {
        self.config_subscribers.len()
    }

    /// Get number of connections with active subscriptions
    pub fn connection_count(&self) -> usize {
        self.connection_configs.len()
    }

    /// Update the MD5 for a subscriber (when client confirms receipt of new config)
    pub fn update_md5(&self, connection_id: &str, config_key: &ConfigKey, md5: &str) {
        let key_string = config_key.to_key_string();

        if let Some(mut subscribers) = self.config_subscribers.get_mut(&key_string) {
            if let Some(subscriber) = subscribers.get_mut(connection_id) {
                subscriber.md5 = md5.to_string();
            }
        }
    }

    /// Parse a key string back to ConfigKey
    fn parse_key_string(key_string: &str) -> Option<ConfigKey> {
        let parts: Vec<&str> = key_string.splitn(3, "@@").collect();
        if parts.len() == 3 {
            Some(ConfigKey {
                tenant: parts[0].to_string(),
                group: parts[1].to_string(),
                data_id: parts[2].to_string(),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_key_to_string() {
        let key = ConfigKey::new("app.yaml", "DEFAULT_GROUP", "public");
        assert_eq!(key.to_key_string(), "public@@DEFAULT_GROUP@@app.yaml");
    }

    #[test]
    fn test_subscribe_and_get() {
        let manager = ConfigSubscriberManager::new();
        let config_key = ConfigKey::new("app.yaml", "DEFAULT_GROUP", "public");

        manager.subscribe("conn1", "192.168.1.1", &config_key, "md5-abc");
        manager.subscribe("conn2", "192.168.1.2", &config_key, "md5-def");

        let subscribers = manager.get_subscribers(&config_key);
        assert_eq!(subscribers.len(), 2);
    }

    #[test]
    fn test_unsubscribe() {
        let manager = ConfigSubscriberManager::new();
        let config_key = ConfigKey::new("app.yaml", "DEFAULT_GROUP", "public");

        manager.subscribe("conn1", "192.168.1.1", &config_key, "md5-abc");
        manager.subscribe("conn2", "192.168.1.2", &config_key, "md5-def");

        manager.unsubscribe("conn1", &config_key);

        let subscribers = manager.get_subscribers(&config_key);
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0].connection_id, "conn2");
    }

    #[test]
    fn test_unsubscribe_all() {
        let manager = ConfigSubscriberManager::new();
        let key1 = ConfigKey::new("app1.yaml", "DEFAULT_GROUP", "public");
        let key2 = ConfigKey::new("app2.yaml", "DEFAULT_GROUP", "public");

        manager.subscribe("conn1", "192.168.1.1", &key1, "md5-1");
        manager.subscribe("conn1", "192.168.1.1", &key2, "md5-2");
        manager.subscribe("conn2", "192.168.1.2", &key1, "md5-3");

        manager.unsubscribe_all("conn1");

        assert_eq!(manager.get_subscribers(&key1).len(), 1);
        assert_eq!(manager.get_subscribers(&key2).len(), 0);
        assert_eq!(manager.connection_count(), 1);
    }

    #[test]
    fn test_get_subscribers_by_ip() {
        let manager = ConfigSubscriberManager::new();
        let key1 = ConfigKey::new("app1.yaml", "DEFAULT_GROUP", "public");
        let key2 = ConfigKey::new("app2.yaml", "DEFAULT_GROUP", "public");

        manager.subscribe("conn1", "192.168.1.1", &key1, "md5-1");
        manager.subscribe("conn2", "192.168.1.1", &key2, "md5-2");
        manager.subscribe("conn3", "192.168.1.2", &key1, "md5-3");

        let by_ip = manager.get_subscribers_by_ip("192.168.1.1");
        assert_eq!(by_ip.len(), 2);
    }

    #[test]
    fn test_update_md5() {
        let manager = ConfigSubscriberManager::new();
        let config_key = ConfigKey::new("app.yaml", "DEFAULT_GROUP", "public");

        manager.subscribe("conn1", "192.168.1.1", &config_key, "old-md5");
        manager.update_md5("conn1", &config_key, "new-md5");

        let subscribers = manager.get_subscribers(&config_key);
        assert_eq!(subscribers[0].md5, "new-md5");
    }

    #[test]
    fn test_counts() {
        let manager = ConfigSubscriberManager::new();
        let key1 = ConfigKey::new("app1.yaml", "DEFAULT_GROUP", "public");
        let key2 = ConfigKey::new("app2.yaml", "DEFAULT_GROUP", "public");

        manager.subscribe("conn1", "192.168.1.1", &key1, "md5-1");
        manager.subscribe("conn1", "192.168.1.1", &key2, "md5-2");
        manager.subscribe("conn2", "192.168.1.2", &key1, "md5-3");

        assert_eq!(manager.config_count(), 2);
        assert_eq!(manager.connection_count(), 2);
        assert_eq!(manager.subscription_count(), 3);
    }
}
