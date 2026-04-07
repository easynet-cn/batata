//! Consul health check result handler
//!
//! Updates health status in ConsulNamingStore when checks change.
//! Also increments the blocking query index so watchers are notified.
//!
//! Key behaviors (matching Consul original):
//! - on_health_changed: update NamingStore health status
//! - on_deregister: remove service by check_id → service_id → store_key chain
//! - on_check_critical: invalidate sessions linked to the failing check

use std::sync::Arc;

use batata_plugin::{HealthCheckResultHandler, PluginNamingStore};
use tracing::{info, warn};

use crate::check_index::ConsulCheckIndex;
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::kv::ConsulKVService;
use crate::naming_store::ConsulNamingStore;
use crate::session::ConsulSessionService;

/// Services needed for session-check invalidation and service deregistration.
/// Set after construction via `set_services()` because session/kv/check_index
/// are created after the result handler is passed to InstanceCheckRegistry.
struct ConsulServices {
    check_index: Arc<ConsulCheckIndex>,
    session_service: Arc<ConsulSessionService>,
    kv_service: Arc<ConsulKVService>,
}

/// Consul-specific result handler.
///
/// When a health check result changes:
/// 1. Updates the instance health in ConsulNamingStore
/// 2. Increments the catalog index (for blocking query watchers)
///
/// When an individual check becomes Critical:
/// 3. Invalidates sessions linked to that check (Consul core behavior)
/// 4. Releases/deletes KV locks based on session behavior
///
/// When an instance should be deregistered (critical too long):
/// 5. Removes from ConsulNamingStore using check_id → service_id → store_key
/// 6. Increments the catalog index
pub struct ConsulResultHandler {
    naming_store: Arc<ConsulNamingStore>,
    index_provider: Arc<ConsulIndexProvider>,
    /// Lazily set services for session-check invalidation and deregistration.
    services: std::sync::OnceLock<ConsulServices>,
}

impl ConsulResultHandler {
    pub fn new(
        naming_store: Arc<ConsulNamingStore>,
        index_provider: Arc<ConsulIndexProvider>,
    ) -> Self {
        Self {
            naming_store,
            index_provider,
            services: std::sync::OnceLock::new(),
        }
    }

    /// Set the services needed for session-check invalidation and service deregistration.
    ///
    /// Must be called after plugin init when session/kv/check_index become available.
    pub fn set_services(
        &self,
        check_index: Arc<ConsulCheckIndex>,
        session_service: Arc<ConsulSessionService>,
        kv_service: Arc<ConsulKVService>,
    ) {
        let _ = self.services.set(ConsulServices {
            check_index,
            session_service,
            kv_service,
        });
    }

    /// Deregister a service using the check_id → service_id → store_key chain.
    ///
    /// This matches Consul's `reapServicesInternal()` which uses ServiceID
    /// to call `RemoveService()`, NOT IP:Port matching.
    fn deregister_by_check_id(&self, check_id: &str) -> bool {
        let Some(services) = self.services.get() else {
            warn!(
                "ConsulResultHandler: services not set, cannot deregister by check_id '{}'",
                check_id
            );
            return false;
        };

        // Step 1: check_id → service_id
        let Some(service_id) = services.check_index.lookup_service_id(check_id) else {
            warn!(
                "ConsulResultHandler: no service_id found for check_id '{}'",
                check_id
            );
            return false;
        };

        // Step 2: service_id → find store_key in NamingStore
        // Store key format: "{namespace}/{service_name}/{service_id}"
        let entries = PluginNamingStore::scan(&*self.naming_store, "");
        for (key, data) in &entries {
            if let Ok(reg) = serde_json::from_slice::<crate::model::AgentServiceRegistration>(data)
            {
                if reg.service_id() == service_id {
                    info!(
                        "Deregistering service '{}' (check_id='{}', store_key='{}')",
                        service_id, check_id, key
                    );
                    let _ = PluginNamingStore::deregister(&*self.naming_store, key);
                    // Clean up check_index mappings
                    services.check_index.remove(&service_id);
                    services.check_index.remove_check(check_id);
                    self.index_provider.increment(ConsulTable::Catalog);
                    return true;
                }
            }
        }

        warn!(
            "ConsulResultHandler: service_id '{}' not found in NamingStore (check_id='{}')",
            service_id, check_id
        );
        false
    }

    /// Invalidate sessions linked to a check that became Critical.
    /// Matches Consul's `checkSessionsTxn()` behavior.
    fn invalidate_sessions_for_check(&self, check_id: &str) {
        let Some(services) = self.services.get() else {
            return;
        };

        let invalidated = services
            .session_service
            .invalidate_sessions_for_check(check_id);

        if invalidated.is_empty() {
            return;
        }

        info!(
            "Invalidated {} session(s) due to check '{}' becoming critical",
            invalidated.len(),
            check_id
        );

        // Handle KV locks based on session behavior (matches Consul session.go)
        let kv_service = services.kv_service.clone();
        let idx = self.index_provider.clone();
        tokio::spawn(async move {
            for session in &invalidated {
                if session.behavior == "delete" {
                    kv_service.delete_session_keys(&session.id).await;
                } else {
                    // "release" (default) — release locks but keep keys
                    kv_service.release_session(&session.id).await;
                }
            }
            // Increment KVS and Sessions indexes to wake blocking queries
            idx.increment(ConsulTable::KVS);
            idx.increment(ConsulTable::Sessions);
        });
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

    fn on_check_critical(&self, check_id: &str) {
        self.invalidate_sessions_for_check(check_id);
    }

    fn on_deregister(
        &self,
        check_id: &str,
        _namespace: &str,
        _group: &str,
        _service: &str,
        ip: &str,
        port: i32,
        _cluster: &str,
    ) {
        // Primary: use check_id → service_id → store_key chain (Consul approach)
        if self.deregister_by_check_id(check_id) {
            return;
        }

        // Fallback: IP:Port match (for checks registered outside the index)
        let entries = PluginNamingStore::scan(&*self.naming_store, "");
        for (key, data) in &entries {
            if let Ok(reg) = serde_json::from_slice::<crate::model::AgentServiceRegistration>(data)
            {
                let reg_port = reg.port.unwrap_or(0) as i32;
                let reg_addr = reg.effective_address();
                if reg_addr == ip && reg_port == port {
                    info!(
                        "Deregistering service by IP:Port fallback: key='{}', addr={}:{}",
                        key, ip, port
                    );
                    let _ = PluginNamingStore::deregister(&*self.naming_store, key);
                    self.index_provider.increment(ConsulTable::Catalog);
                    return;
                }
            }
        }

        warn!(
            "ConsulResultHandler: could not find service to deregister (check_id='{}', ip='{}', port={})",
            check_id, ip, port
        );
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
    fn test_deregister_by_check_id() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        // Set up services
        let check_index = Arc::new(ConsulCheckIndex::new());
        let session_service = Arc::new(ConsulSessionService::new());
        let kv_service = Arc::new(ConsulKVService::new());
        handler.set_services(check_index.clone(), session_service, kv_service);

        // Register a service in NamingStore
        let data = serde_json::to_vec(&serde_json::json!({
            "Name": "web", "ID": "web-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/web/web-1", Bytes::from(data))
            .unwrap();
        assert_eq!(store.len(), 1);

        // Register check_id → service_id mapping
        check_index.register_check("service:web-1", "web-1");

        // Deregister by check_id — should find via check_id → service_id → store_key
        handler.on_deregister(
            "service:web-1",
            "",
            "",
            "web",
            "0.0.0.0", // wrong IP — doesn't matter because primary path uses check_id
            0,          // wrong port
            "",
        );

        assert_eq!(store.len(), 0, "Service should be deregistered via check_id chain");
    }

    #[test]
    fn test_deregister_fallback_ip_port() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        // NO services set — primary path will fail, fallback to IP:Port

        let data = serde_json::to_vec(&serde_json::json!({
            "Name": "web", "ID": "web-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/web/web-1", Bytes::from(data))
            .unwrap();
        assert_eq!(store.len(), 1);

        handler.on_deregister("unknown-check", "", "", "web", "10.0.0.1", 8080, "");

        assert_eq!(store.len(), 0, "Service should be deregistered via IP:Port fallback");
    }

    #[tokio::test]
    async fn test_on_check_critical_invalidates_sessions() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        let session_service = Arc::new(ConsulSessionService::new());
        let kv_service = Arc::new(ConsulKVService::new());
        let check_index = Arc::new(ConsulCheckIndex::new());
        handler.set_services(check_index, session_service.clone(), kv_service);

        // Create a session linked to serfHealth (default)
        let _session = session_service
            .create_session(crate::model::SessionCreateRequest {
                name: Some("test-session".to_string()),
                ttl: Some("60s".to_string()),
                ..Default::default()
            })
            .await;
        assert_eq!(session_service.list_sessions().len(), 1);

        // Trigger serfHealth check going critical
        handler.on_check_critical("serfHealth");

        // Session should be invalidated
        assert_eq!(
            session_service.list_sessions().len(),
            0,
            "Session linked to serfHealth should be invalidated"
        );
    }

    #[tokio::test]
    async fn test_on_check_critical_skips_unlinked_sessions() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        let session_service = Arc::new(ConsulSessionService::new());
        let kv_service = Arc::new(ConsulKVService::new());
        let check_index = Arc::new(ConsulCheckIndex::new());
        handler.set_services(check_index, session_service.clone(), kv_service);

        let _session = session_service
            .create_session(crate::model::SessionCreateRequest {
                name: Some("serf-session".to_string()),
                ttl: Some("60s".to_string()),
                ..Default::default()
            })
            .await;

        // Trigger a different check going critical
        handler.on_check_critical("service:web-1");

        assert_eq!(
            session_service.list_sessions().len(),
            1,
            "Session not linked to failing check should survive"
        );
    }
}
