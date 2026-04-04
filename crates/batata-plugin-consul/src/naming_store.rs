//! Consul-specific naming store — independent service data storage
//!
//! Stores Consul services in their native format (AgentService with tags,
//! weights, connect config, etc.) without converting to Nacos Instance.
//!
//! Key format: "{service_name}/{service_id}"
//! Data format: JSON-serialized AgentService

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;

use batata_plugin::{PluginNamingStore, PluginNamingStoreError};

use crate::model::AgentServiceRegistration;

/// Stored service entry with Consul-native data
#[derive(Debug, Clone)]
struct StoredEntry {
    data: Bytes,
    service_name: String,
    service_id: String,
}

/// Consul naming store — holds service registrations in Consul-native format
///
/// Services are stored as serialized JSON bytes, keyed by "{service_name}/{service_id}".
/// This avoids the Nacos metadata JSON packing/unpacking overhead.
#[derive(Clone)]
pub struct ConsulNamingStore {
    /// Key: "{service_name}/{service_id}", Value: serialized AgentService
    entries: Arc<DashMap<String, StoredEntry>>,
    /// Instance health status: "ip:port" → healthy (aggregated from checks)
    health_status: Arc<DashMap<String, bool>>,
    /// Monotonically increasing revision for change detection
    revision: Arc<AtomicU64>,
}

impl ConsulNamingStore {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            health_status: Arc::new(DashMap::new()),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Build health key from ip and port
    fn health_key(ip: &str, port: i32) -> String {
        format!("{ip}:{port}")
    }

    /// Get health status for an instance
    pub fn is_healthy(&self, ip: &str, port: i32) -> bool {
        self.health_status
            .get(&Self::health_key(ip, port))
            .map(|v| *v)
            .unwrap_or(true) // Default: healthy
    }

    /// Update health status for an instance
    pub fn update_health(&self, ip: &str, port: i32, healthy: bool) {
        self.health_status
            .insert(Self::health_key(ip, port), healthy);
        self.bump_revision();
    }

    /// Build a key from service name and service ID
    pub fn build_key(service_name: &str, service_id: &str) -> String {
        format!("{service_name}/{service_id}")
    }

    /// Get all entries for a service name
    pub fn get_service_entries(&self, service_name: &str) -> Vec<Bytes> {
        let prefix = format!("{service_name}/");
        self.entries
            .iter()
            .filter(|e| e.key().starts_with(&prefix))
            .map(|e| e.value().data.clone())
            .collect()
    }

    /// Get all unique service names
    pub fn service_names(&self) -> Vec<String> {
        let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in self.entries.iter() {
            names.insert(entry.value().service_name.clone());
        }
        let mut result: Vec<String> = names.into_iter().collect();
        result.sort();
        result
    }

    /// Get service names with their tags (for catalog list)
    pub fn service_names_with_tags(&self) -> std::collections::HashMap<String, Vec<String>> {
        let mut result: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for entry in self.entries.iter() {
            // Deserialize to extract tags
            if let Ok(reg) =
                serde_json::from_slice::<AgentServiceRegistration>(&entry.value().data)
            {
                let tags = result
                    .entry(entry.value().service_name.clone())
                    .or_default();
                if let Some(ref reg_tags) = reg.tags {
                    for tag in reg_tags {
                        if !tags.contains(tag) {
                            tags.push(tag.clone());
                        }
                    }
                }
            }
        }
        result
    }

    /// Get an entry by service_id (scans all entries)
    pub fn get_by_service_id(&self, service_id: &str) -> Option<Bytes> {
        self.entries
            .iter()
            .find(|e| e.value().service_id == service_id)
            .map(|e| e.value().data.clone())
    }

    /// Remove by service_id (scans and removes)
    pub fn remove_by_service_id(&self, service_id: &str) -> bool {
        let key = self
            .entries
            .iter()
            .find(|e| e.value().service_id == service_id)
            .map(|e| e.key().clone());
        if let Some(key) = key {
            self.entries.remove(&key);
            self.revision.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for ConsulNamingStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PluginNamingStore for ConsulNamingStore {
    fn plugin_id(&self) -> &str {
        "consul"
    }

    fn register(&self, key: &str, data: Bytes) -> Result<(), PluginNamingStoreError> {
        // Extract service_name and service_id from key
        let (service_name, service_id) = key.split_once('/').ok_or_else(|| {
            PluginNamingStoreError::Storage(format!("Invalid key format: {key}"))
        })?;

        self.entries.insert(
            key.to_string(),
            StoredEntry {
                data,
                service_name: service_name.to_string(),
                service_id: service_id.to_string(),
            },
        );
        self.bump_revision();
        Ok(())
    }

    fn deregister(&self, key: &str) -> Result<(), PluginNamingStoreError> {
        self.entries.remove(key);
        self.bump_revision();
        Ok(())
    }

    fn get(&self, key: &str) -> Option<Bytes> {
        self.entries.get(key).map(|e| e.value().data.clone())
    }

    fn scan(&self, prefix: &str) -> Vec<(String, Bytes)> {
        self.entries
            .iter()
            .filter(|e| e.key().starts_with(prefix))
            .map(|e| (e.key().clone(), e.value().data.clone()))
            .collect()
    }

    fn keys(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.key().clone()).collect()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    async fn snapshot(&self) -> Bytes {
        // Serialize as Vec<(String, Vec<u8>)> since Bytes doesn't impl Serialize
        let all: Vec<(String, Vec<u8>)> = self
            .entries
            .iter()
            .map(|e| (e.key().clone(), e.value().data.to_vec()))
            .collect();
        Bytes::from(serde_json::to_vec(&all).unwrap_or_default())
    }

    async fn restore(&self, data: &[u8]) -> Result<(), PluginNamingStoreError> {
        let entries: Vec<(String, Vec<u8>)> = serde_json::from_slice(data)
            .map_err(|e| PluginNamingStoreError::Serialization(e.to_string()))?;

        self.entries.clear();
        for (key, value) in entries {
            let (service_name, service_id) = if let Some((sn, si)) = key.split_once('/') {
                (sn.to_string(), si.to_string())
            } else {
                (String::new(), key.clone())
            };
            self.entries.insert(
                key,
                StoredEntry {
                    data: Bytes::from(value),
                    service_name,
                    service_id,
                },
            );
        }
        self.bump_revision();
        Ok(())
    }

    fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_registration(name: &str, id: &str) -> Bytes {
        let reg = serde_json::json!({
            "Name": name,
            "ID": id,
            "Port": 8080,
            "Address": "10.0.0.1",
            "Tags": ["web", "v1"],
            "Meta": {"version": "1.0"}
        });
        Bytes::from(serde_json::to_vec(&reg).unwrap())
    }

    #[test]
    fn test_register_and_get() {
        let store = ConsulNamingStore::new();
        let key = ConsulNamingStore::build_key("web-api", "web-api-1");
        let data = sample_registration("web-api", "web-api-1");

        store.register(&key, data.clone()).unwrap();

        let result = store.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);
        assert_eq!(store.len(), 1);
        assert_eq!(store.revision(), 1);
    }

    #[test]
    fn test_deregister() {
        let store = ConsulNamingStore::new();
        let key = ConsulNamingStore::build_key("web-api", "web-api-1");
        store
            .register(&key, sample_registration("web-api", "web-api-1"))
            .unwrap();

        store.deregister(&key).unwrap();
        assert_eq!(store.len(), 0);
        assert!(store.get(&key).is_none());
    }

    #[test]
    fn test_scan_by_service() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-2"),
                sample_registration("web-api", "web-api-2"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("db", "db-1"),
                sample_registration("db", "db-1"),
            )
            .unwrap();

        let web_entries = store.scan("web-api/");
        assert_eq!(web_entries.len(), 2);

        let db_entries = store.scan("db/");
        assert_eq!(db_entries.len(), 1);
    }

    #[test]
    fn test_service_names() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-2"),
                sample_registration("web-api", "web-api-2"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("db", "db-1"),
                sample_registration("db", "db-1"),
            )
            .unwrap();

        let names = store.service_names();
        assert_eq!(names, vec!["db", "web-api"]);
    }

    #[test]
    fn test_get_by_service_id() {
        let store = ConsulNamingStore::new();
        let data = sample_registration("web-api", "web-api-1");
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-1"),
                data.clone(),
            )
            .unwrap();

        let result = store.get_by_service_id("web-api-1");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);

        assert!(store.get_by_service_id("nonexistent").is_none());
    }

    #[test]
    fn test_remove_by_service_id() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();

        assert!(store.remove_by_service_id("web-api-1"));
        assert_eq!(store.len(), 0);
        assert!(!store.remove_by_service_id("nonexistent"));
    }

    #[tokio::test]
    async fn test_snapshot_and_restore() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("db", "db-1"),
                sample_registration("db", "db-1"),
            )
            .unwrap();

        let snapshot = store.snapshot().await;

        let store2 = ConsulNamingStore::new();
        store2.restore(&snapshot).await.unwrap();

        assert_eq!(store2.len(), 2);
        assert!(store2.get_by_service_id("web-api-1").is_some());
        assert!(store2.get_by_service_id("db-1").is_some());
    }

    #[test]
    fn test_service_names_with_tags() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();

        let names_tags = store.service_names_with_tags();
        assert_eq!(names_tags.len(), 1);
        let tags = names_tags.get("web-api").unwrap();
        assert!(tags.contains(&"web".to_string()));
        assert!(tags.contains(&"v1".to_string()));
    }
}
