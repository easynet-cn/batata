//! Consul-specific naming store — independent service data storage
//!
//! Stores Consul services in their native format (AgentService with tags,
//! weights, connect config, etc.) without converting to Batata Instance.
//!
//! Key format: "{namespace}/{service_name}/{service_id}"
//! Data format: JSON-serialized AgentService
//!
//! The namespace prefix enables Enterprise namespace isolation.
//! CE clients use "default" namespace transparently.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use dashmap::DashMap;

use batata_plugin::{PluginNamingStore, PluginNamingStoreError};

use crate::model::AgentServiceRegistration;
use crate::namespace::DEFAULT_NAMESPACE;

/// Stored service entry with Consul-native data
#[derive(Debug, Clone)]
struct StoredEntry {
    data: Bytes,
    namespace: String,
    service_name: String,
    service_id: String,
}

/// Consul naming store — holds service registrations in Consul-native format
///
/// Services are stored as serialized JSON bytes, keyed by "{namespace}/{service_name}/{service_id}".
/// This avoids the Batata metadata JSON packing/unpacking overhead.
#[derive(Clone)]
pub struct ConsulNamingStore {
    /// Key: "{namespace}/{service_name}/{service_id}", Value: serialized AgentService
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

    /// Count total registered service instances
    pub fn service_count(&self) -> usize {
        self.entries.len()
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

    /// Build a key from namespace, service name and service ID.
    /// Uses "default" namespace if ns is empty.
    pub fn build_key(ns: &str, service_name: &str, service_id: &str) -> String {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        format!("{ns}/{service_name}/{service_id}")
    }

    /// Get all entries for a service name within a namespace
    pub fn get_service_entries(&self, ns: &str, service_name: &str) -> Vec<Bytes> {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        let prefix = format!("{ns}/{service_name}/");
        self.entries
            .iter()
            .filter(|e| e.key().starts_with(&prefix))
            .map(|e| e.value().data.clone())
            .collect()
    }

    /// Get all known namespaces
    pub fn all_namespaces(&self) -> Vec<String> {
        let mut namespaces: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in self.entries.iter() {
            namespaces.insert(entry.value().namespace.clone());
        }
        namespaces.into_iter().collect()
    }

    /// Get all unique service names within a namespace
    pub fn service_names(&self, ns: &str) -> Vec<String> {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in self.entries.iter() {
            if entry.value().namespace == ns {
                names.insert(entry.value().service_name.clone());
            }
        }
        let mut result: Vec<String> = names.into_iter().collect();
        result.sort();
        result
    }

    /// Get service names with their tags within a namespace
    pub fn service_names_with_tags(
        &self,
        ns: &str,
    ) -> std::collections::HashMap<String, Vec<String>> {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        let mut result: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for entry in self.entries.iter() {
            if entry.value().namespace != ns {
                continue;
            }
            if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&entry.value().data)
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

    /// Get an entry by service_id within a namespace (scans entries in that namespace)
    pub fn get_by_service_id(&self, ns: &str, service_id: &str) -> Option<Bytes> {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        self.entries
            .iter()
            .find(|e| e.value().namespace == ns && e.value().service_id == service_id)
            .map(|e| e.value().data.clone())
    }

    /// Find the store key for a service_id within a namespace (without removing)
    pub fn find_key_by_service_id(&self, ns: &str, service_id: &str) -> Option<String> {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        self.entries
            .iter()
            .find(|e| e.value().namespace == ns && e.value().service_id == service_id)
            .map(|e| e.key().clone())
    }

    /// Remove by service_id within a namespace
    pub fn remove_by_service_id(&self, ns: &str, service_id: &str) -> bool {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        let key = self
            .entries
            .iter()
            .find(|e| e.value().namespace == ns && e.value().service_id == service_id)
            .map(|e| e.key().clone());
        if let Some(key) = key {
            self.entries.remove(&key);
            self.revision.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Scan all entries in a namespace (for internal/metrics)
    pub fn scan_ns(&self, ns: &str) -> Vec<(String, Bytes)> {
        let ns = if ns.is_empty() { DEFAULT_NAMESPACE } else { ns };
        self.entries
            .iter()
            .filter(|e| e.value().namespace == ns)
            .map(|e| (e.key().clone(), e.value().data.clone()))
            .collect()
    }

    fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::Relaxed);
    }

    /// Parse key into (namespace, service_name, service_id)
    fn parse_key(key: &str) -> Option<(String, String, String)> {
        let mut parts = key.splitn(3, '/');
        let ns = parts.next()?.to_string();
        let svc = parts.next()?.to_string();
        let id = parts.next()?.to_string();
        Some((ns, svc, id))
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
        let (namespace, service_name, service_id) = Self::parse_key(key)
            .ok_or_else(|| PluginNamingStoreError::Storage(format!("Invalid key format: {key}")))?;

        self.entries.insert(
            key.to_string(),
            StoredEntry {
                data,
                namespace,
                service_name,
                service_id,
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
            if let Some((ns, svc, id)) = Self::parse_key(&key) {
                self.entries.insert(
                    key,
                    StoredEntry {
                        data: Bytes::from(value),
                        namespace: ns,
                        service_name: svc,
                        service_id: id,
                    },
                );
            }
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
        let key = ConsulNamingStore::build_key("default", "web-api", "web-api-1");
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
        let key = ConsulNamingStore::build_key("default", "web-api", "web-api-1");
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
                &ConsulNamingStore::build_key("default", "web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("default", "web-api", "web-api-2"),
                sample_registration("web-api", "web-api-2"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("default", "db", "db-1"),
                sample_registration("db", "db-1"),
            )
            .unwrap();

        let web_entries = store.get_service_entries("default", "web-api");
        assert_eq!(web_entries.len(), 2);

        let db_entries = store.get_service_entries("default", "db");
        assert_eq!(db_entries.len(), 1);
    }

    #[test]
    fn test_service_names() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("default", "web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("default", "db", "db-1"),
                sample_registration("db", "db-1"),
            )
            .unwrap();

        let names = store.service_names("default");
        assert_eq!(names, vec!["db", "web-api"]);
    }

    #[test]
    fn test_namespace_isolation() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("default", "web", "web-1"),
                sample_registration("web", "web-1"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("staging", "web", "web-2"),
                sample_registration("web", "web-2"),
            )
            .unwrap();

        // Each namespace sees only its own services
        let default_entries = store.get_service_entries("default", "web");
        assert_eq!(default_entries.len(), 1);

        let staging_entries = store.get_service_entries("staging", "web");
        assert_eq!(staging_entries.len(), 1);

        // service_names is namespace-scoped
        assert_eq!(store.service_names("default"), vec!["web"]);
        assert_eq!(store.service_names("staging"), vec!["web"]);

        // get_by_service_id is namespace-scoped
        assert!(store.get_by_service_id("default", "web-1").is_some());
        assert!(store.get_by_service_id("default", "web-2").is_none());
        assert!(store.get_by_service_id("staging", "web-2").is_some());
        assert!(store.get_by_service_id("staging", "web-1").is_none());

        // remove is namespace-scoped
        assert!(store.remove_by_service_id("default", "web-1"));
        assert_eq!(store.len(), 1); // staging's web-2 still exists
    }

    #[test]
    fn test_get_by_service_id() {
        let store = ConsulNamingStore::new();
        let data = sample_registration("web-api", "web-api-1");
        store
            .register(
                &ConsulNamingStore::build_key("default", "web-api", "web-api-1"),
                data.clone(),
            )
            .unwrap();

        let result = store.get_by_service_id("default", "web-api-1");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);

        assert!(store.get_by_service_id("default", "nonexistent").is_none());
    }

    #[test]
    fn test_remove_by_service_id() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("default", "web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();

        assert!(store.remove_by_service_id("default", "web-api-1"));
        assert_eq!(store.len(), 0);
        assert!(!store.remove_by_service_id("default", "nonexistent"));
    }

    #[tokio::test]
    async fn test_snapshot_and_restore() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("default", "web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();
        store
            .register(
                &ConsulNamingStore::build_key("default", "db", "db-1"),
                sample_registration("db", "db-1"),
            )
            .unwrap();

        let snapshot = store.snapshot().await;

        let store2 = ConsulNamingStore::new();
        store2.restore(&snapshot).await.unwrap();

        assert_eq!(store2.len(), 2);
        assert!(store2.get_by_service_id("default", "web-api-1").is_some());
        assert!(store2.get_by_service_id("default", "db-1").is_some());
    }

    #[test]
    fn test_service_names_with_tags() {
        let store = ConsulNamingStore::new();
        store
            .register(
                &ConsulNamingStore::build_key("default", "web-api", "web-api-1"),
                sample_registration("web-api", "web-api-1"),
            )
            .unwrap();

        let names_tags = store.service_names_with_tags("default");
        assert_eq!(names_tags.len(), 1);
        let tags = names_tags.get("web-api").unwrap();
        assert!(tags.contains(&"web".to_string()));
        assert!(tags.contains(&"v1".to_string()));
    }

    #[test]
    fn test_empty_namespace_defaults_to_default() {
        let key = ConsulNamingStore::build_key("", "web", "web-1");
        assert!(key.starts_with("default/"));
    }

    #[test]
    fn test_find_key_by_service_id() {
        let store = ConsulNamingStore::new();
        let key = ConsulNamingStore::build_key("default", "web", "web-1");
        let data = br#"{"name":"web","id":"web-1"}"#;
        store.register(&key, bytes::Bytes::from(&data[..])).unwrap();

        // Should find the key
        let found = store.find_key_by_service_id("default", "web-1");
        assert!(found.is_some());
        assert_eq!(found.unwrap(), key);

        // Should not find non-existent service
        let not_found = store.find_key_by_service_id("default", "web-99");
        assert!(not_found.is_none());

        // Should not find in wrong namespace
        let wrong_ns = store.find_key_by_service_id("other", "web-1");
        assert!(wrong_ns.is_none());
    }
}
