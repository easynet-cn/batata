//! Consul health check result handler
//!
//! Updates health status in ConsulNamingStore when checks change.
//! Also increments the blocking query index so watchers are notified.

use std::sync::Arc;

use batata_plugin::{HealthCheckResultHandler, PluginNamingStore};

use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::naming_store::ConsulNamingStore;

/// Consul-specific result handler.
///
/// When a health check result changes:
/// 1. Updates the instance health in ConsulNamingStore
/// 2. Increments the catalog index (for blocking query watchers)
///
/// When an instance should be deregistered (critical too long):
/// 1. Removes from ConsulNamingStore
/// 2. Increments the catalog index
pub struct ConsulResultHandler {
    naming_store: Arc<ConsulNamingStore>,
    index_provider: Arc<ConsulIndexProvider>,
}

impl ConsulResultHandler {
    pub fn new(
        naming_store: Arc<ConsulNamingStore>,
        index_provider: Arc<ConsulIndexProvider>,
    ) -> Self {
        Self {
            naming_store,
            index_provider,
        }
    }
}

impl HealthCheckResultHandler for ConsulResultHandler {
    fn on_health_changed(
        &self,
        _namespace: &str,
        _group: &str,
        _service: &str,
        ip: &str,
        port: i32,
        _cluster: &str,
        healthy: bool,
    ) {
        self.naming_store.update_health(ip, port, healthy);
        self.index_provider.increment(ConsulTable::Catalog);
    }

    fn on_deregister(
        &self,
        _namespace: &str,
        _group: &str,
        _service: &str,
        ip: &str,
        port: i32,
        _cluster: &str,
    ) {
        // Find and remove the service by IP:port
        let entries = PluginNamingStore::scan(&*self.naming_store, "");
        for (key, data) in &entries {
            if let Ok(reg) = serde_json::from_slice::<crate::model::AgentServiceRegistration>(data)
            {
                let reg_port = reg.port.unwrap_or(0) as i32;
                let reg_addr = reg.address.as_deref().unwrap_or("");
                if reg_addr == ip && reg_port == port {
                    let _ = PluginNamingStore::deregister(&*self.naming_store, key);
                    self.index_provider.increment(ConsulTable::Catalog);
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_plugin::PluginNamingStore;
    use bytes::Bytes;

    #[test]
    fn test_consul_result_handler_health_changed() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        // Register a service
        let data = serde_json::to_vec(&serde_json::json!({
            "Name": "web", "ID": "web-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/web/web-1", Bytes::from(data))
            .unwrap();

        // Initially healthy
        assert!(store.is_healthy("10.0.0.1", 8080));

        // Mark unhealthy
        handler.on_health_changed("", "", "web", "10.0.0.1", 8080, "", false);
        assert!(!store.is_healthy("10.0.0.1", 8080));

        // Mark healthy
        handler.on_health_changed("", "", "web", "10.0.0.1", 8080, "", true);
        assert!(store.is_healthy("10.0.0.1", 8080));
    }

    #[test]
    fn test_consul_result_handler_deregister() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        let data = serde_json::to_vec(&serde_json::json!({
            "Name": "web", "ID": "web-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/web/web-1", Bytes::from(data))
            .unwrap();
        assert_eq!(store.len(), 1);

        handler.on_deregister("", "", "web", "10.0.0.1", 8080, "");
        assert_eq!(store.len(), 0);
    }
}
