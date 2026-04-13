//! NamingManager — unified entry point for naming services
//!
//! Holds the core Batata NamingService and any registered plugin naming stores.
//!
//! ```text
//! NamingManager
//! ├── core: NamingService          (Batata core, Nacos compatible)
//! └── plugins: Map<id, Store>      (Consul, Eureka, Apollo, ...)
//! ```
//!
//! - The core is always available and handles all SDK traffic (Nacos-compatible).
//! - Plugin stores are optional and handle their own protocol's traffic.
//! - Core and plugins are completely isolated — no data sharing.

use std::sync::Arc;

use dashmap::DashMap;

use batata_plugin::PluginNamingStore;

use crate::service::NamingService;

/// Unified naming manager.
///
/// Provides access to the core Batata naming service and any registered
/// plugin naming stores. Designed to be stored as `Arc<NamingManager>`
/// and shared across HTTP/gRPC handlers.
#[derive(Clone)]
pub struct NamingManager {
    /// Core naming service (Batata)
    core: Arc<NamingService>,
    /// Plugin naming stores, keyed by plugin_id
    plugins: Arc<DashMap<String, Arc<dyn PluginNamingStore>>>,
}

impl NamingManager {
    /// Create a new NamingManager with the given core naming service.
    pub fn new(core: Arc<NamingService>) -> Self {
        Self {
            core,
            plugins: Arc::new(DashMap::new()),
        }
    }

    /// Get the core Batata naming service.
    pub fn core(&self) -> &Arc<NamingService> {
        &self.core
    }

    /// Register a plugin naming store.
    ///
    /// The store is identified by its `plugin_id()`. If a store with the
    /// same ID already exists, it is replaced.
    pub fn register_plugin_store(&self, store: Arc<dyn PluginNamingStore>) {
        tracing::info!(
            "Registered plugin naming store: {} (revision={})",
            store.plugin_id(),
            store.revision()
        );
        self.plugins.insert(store.plugin_id().to_string(), store);
    }

    /// Get a plugin naming store by ID.
    pub fn plugin_store(&self, plugin_id: &str) -> Option<Arc<dyn PluginNamingStore>> {
        self.plugins.get(plugin_id).map(|r| r.clone())
    }

    /// List all registered plugin IDs.
    pub fn plugin_ids(&self) -> Vec<String> {
        self.plugins.iter().map(|e| e.key().clone()).collect()
    }

    /// Total number of registered plugin stores.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_plugin::PluginNamingStoreError;

    struct MockPluginStore {
        id: String,
    }

    #[async_trait::async_trait]
    impl PluginNamingStore for MockPluginStore {
        fn plugin_id(&self) -> &str {
            &self.id
        }
        fn register(&self, _key: &str, _data: bytes::Bytes) -> Result<(), PluginNamingStoreError> {
            Ok(())
        }
        fn deregister(&self, _key: &str) -> Result<(), PluginNamingStoreError> {
            Ok(())
        }
        fn get(&self, _key: &str) -> Option<bytes::Bytes> {
            None
        }
        fn scan(&self, _prefix: &str) -> Vec<(String, bytes::Bytes)> {
            Vec::new()
        }
        fn keys(&self) -> Vec<String> {
            Vec::new()
        }
        fn len(&self) -> usize {
            0
        }
        async fn snapshot(&self) -> bytes::Bytes {
            bytes::Bytes::new()
        }
        async fn restore(&self, _data: &[u8]) -> Result<(), PluginNamingStoreError> {
            Ok(())
        }
        fn revision(&self) -> u64 {
            0
        }
    }

    #[test]
    fn test_naming_manager_core() {
        let core = Arc::new(NamingService::new());
        let manager = NamingManager::new(core.clone());

        // Core is accessible
        assert!(Arc::ptr_eq(manager.core(), &core));
    }

    #[test]
    fn test_naming_manager_plugin_registration() {
        let manager = NamingManager::new(Arc::new(NamingService::new()));

        // No plugins initially
        assert_eq!(manager.plugin_count(), 0);
        assert!(manager.plugin_store("consul").is_none());

        // Register a plugin store
        let store = Arc::new(MockPluginStore {
            id: "consul".to_string(),
        });
        manager.register_plugin_store(store);

        // Plugin is now accessible
        assert_eq!(manager.plugin_count(), 1);
        assert!(manager.plugin_store("consul").is_some());
        assert_eq!(manager.plugin_ids(), vec!["consul"]);

        // Unknown plugin returns None
        assert!(manager.plugin_store("eureka").is_none());
    }

    #[test]
    fn test_naming_manager_multiple_plugins() {
        let manager = NamingManager::new(Arc::new(NamingService::new()));

        manager.register_plugin_store(Arc::new(MockPluginStore {
            id: "consul".to_string(),
        }));
        manager.register_plugin_store(Arc::new(MockPluginStore {
            id: "eureka".to_string(),
        }));

        assert_eq!(manager.plugin_count(), 2);
        assert!(manager.plugin_store("consul").is_some());
        assert!(manager.plugin_store("eureka").is_some());
    }

    #[test]
    fn test_naming_manager_replace_plugin() {
        let manager = NamingManager::new(Arc::new(NamingService::new()));

        let store1 = Arc::new(MockPluginStore {
            id: "consul".to_string(),
        });
        let store2 = Arc::new(MockPluginStore {
            id: "consul".to_string(),
        });

        manager.register_plugin_store(store1.clone());
        manager.register_plugin_store(store2.clone());

        // Still only 1 plugin (replaced)
        assert_eq!(manager.plugin_count(), 1);
    }
}
