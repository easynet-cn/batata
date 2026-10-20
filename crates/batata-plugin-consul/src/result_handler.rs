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
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};
use crate::session::ConsulSessionService;

/// Services needed for session-check invalidation and service deregistration.
/// Set after construction via `set_services()` because session/kv/check_index
/// are created after the result handler is passed to InstanceCheckRegistry.
struct ConsulServices {
    check_index: Arc<ConsulCheckIndex>,
    session_service: Arc<ConsulSessionService>,
    kv_service: Arc<ConsulKVService>,
    /// Raft writer for routing auto-deregister through the cluster.
    /// `None` in standalone mode; present in cluster mode.
    raft_writer: Option<Arc<ConsulRaftWriter>>,
    /// This agent's node name, used to tell "our" checks apart from another
    /// node's when a status update arrives through Raft replication.
    node_name: String,
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
    /// The `new` associated function.
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
        raft_writer: Option<Arc<ConsulRaftWriter>>,
        node_name: String,
    ) {
        let _ = self.services.set(ConsulServices {
            check_index,
            session_service,
            kv_service,
            raft_writer,
            node_name,
        });
    }

    /// Deregister a service using the check_id → service_id → store_key chain.
    ///
    /// This matches Consul's `reapServicesInternal()` which uses ServiceID
    /// to call `RemoveService()`, NOT IP:Port matching.
    ///
    /// Cluster mode: routes through `ConsulRaftRequest::CatalogDeregister`
    /// so the removal replicates to all nodes. The write is *not* gated on
    /// this node being the leader: `RaftNode::write_with_index` forwards to
    /// the leader on a follower, so gating here would strand the one node
    /// that can actually resolve the check (see `ReapOutcome::UnknownCheck`).
    fn deregister_by_check_id(&self, check_id: &str) -> ReapOutcome {
        let Some(services) = self.services.get() else {
            warn!(
                "ConsulResultHandler: services not set, cannot deregister by check_id '{}'",
                check_id
            );
            return ReapOutcome::UnknownCheck;
        };

        // Step 1: check_id → service_id
        let Some(service_id) = services.check_index.lookup_service_id(check_id) else {
            // This node does not own the check: the mapping is only built by
            // the agent that accepted the registration, while the check
            // itself replicates to every node. Guessing here is what used to
            // delete an unrelated service sharing the same IP:Port.
            warn!(
                "ConsulResultHandler: no service_id found for check_id '{}' \
                 (check is replicated from another agent; leaving reap to its owner)",
                check_id
            );
            return ReapOutcome::UnknownCheck;
        };

        // Step 2: service_id → find store_key in NamingStore
        // Store key format: "{namespace}/{service_name}/{service_id}"
        let entries = PluginNamingStore::scan(&*self.naming_store, "");
        for (key, data) in &entries {
            if let Ok(reg) = serde_json::from_slice::<crate::model::AgentServiceRegistration>(data)
                && reg.service_id() == service_id
            {
                if let Some(ref writer) = services.raft_writer {
                    info!(
                        "Deregistering service '{}' (check_id='{}', store_key='{}') via Raft",
                        service_id, check_id, key
                    );
                    let writer = writer.clone();
                    let key_owned = key.clone();
                    let service_id_owned = service_id.clone();
                    let check_id_owned = check_id.to_string();
                    let check_index = services.check_index.clone();
                    let idx = self.index_provider.clone();
                    tokio::spawn(async move {
                        match writer
                            .write(ConsulRaftRequest::CatalogDeregister {
                                key: key_owned.clone(),
                            })
                            .await
                        {
                            Ok(r) if r.success => {
                                // Hook-driven naming_store update already
                                // ran on every node. Clean up ancillary
                                // indexes that aren't part of the apply
                                // path.
                                check_index.remove(&service_id_owned);
                                check_index.remove_check(&check_id_owned);
                                idx.increment(ConsulTable::Catalog);
                            }
                            Ok(r) => {
                                warn!(
                                    "Raft CatalogDeregister rejected during auto-reap: {:?}",
                                    r.message
                                );
                            }
                            Err(e) => {
                                warn!("Raft CatalogDeregister failed during auto-reap: {}", e);
                            }
                        }
                    });
                    return ReapOutcome::Removed;
                }
                // Standalone mode: direct mutation.
                info!(
                    "Deregistering service '{}' (check_id='{}', store_key='{}')",
                    service_id, check_id, key
                );
                let _ = PluginNamingStore::deregister(&*self.naming_store, key);
                services.check_index.remove(&service_id);
                services.check_index.remove_check(check_id);
                self.index_provider.increment(ConsulTable::Catalog);
                return ReapOutcome::Removed;
            }
        }

        warn!(
            "ConsulResultHandler: service_id '{}' not found in NamingStore (check_id='{}')",
            service_id, check_id
        );
        ReapOutcome::MissingService
    }

    /// Invalidate sessions linked to a check that became Critical.
    ///
    /// Cluster-mode: delegates to
    /// `ConsulSessionService::invalidate_sessions_for_check_via_raft`
    /// which is leader-gated and writes through Raft so all nodes observe
    /// identical session destruction + KV release. Non-leader nodes
    /// observe the cascade via applied Raft entries rather than running
    /// the invalidation themselves. This eliminates the race where
    /// multiple nodes independently detected the same critical check and
    /// each raced to delete the session locally.
    ///
    /// Standalone-mode: the leader check is a no-op (no Raft configured),
    /// so the single node runs the full cascade.
    fn invalidate_sessions_for_check(&self, check_id: &str) {
        let Some(services) = self.services.get() else {
            return;
        };

        let check_id_owned = check_id.to_string();
        let session_service = services.session_service.clone();
        let kv_service = services.kv_service.clone();
        let idx = self.index_provider.clone();
        tokio::spawn(async move {
            let invalidated = session_service
                .invalidate_sessions_for_check_via_raft(&check_id_owned, &kv_service)
                .await;
            if invalidated.is_empty() {
                return;
            }
            info!(
                "Invalidated {} session(s) via Raft due to check '{}' becoming critical",
                invalidated.len(),
                check_id_owned
            );
            // Wake blocking queries watching KVS/Sessions tables
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
        // Edge-triggered: the reactor reports a result on every probe interval,
        // which is level-triggered by nature. Waking blocking queries on an
        // unchanged value would return identical payloads to every watcher
        // (Consul long-polling, `?index=` clients) once per interval.
        if !self
            .naming_store
            .update_health_if_changed(ip, port, healthy)
        {
            return;
        }
        self.index_provider.increment(ConsulTable::Catalog);
    }

    fn on_check_critical(&self, check_id: &str) {
        // `check_id` is the registry key, i.e. node-qualified for checks that
        // belong to a node. Sessions are bound to *this* node's checks
        // (`NodeChecks: ["serfHealth"]`), so another node's identically-named
        // check going critical must not invalidate them — that is exactly the
        // cross-node contamination the qualifier was introduced to prevent.
        let (node, bare_id) = batata_naming::healthcheck::registry::split_check_key(check_id);
        if let Some(node) = node
            && let Some(services) = self.services.get()
            && node != services.node_name
        {
            return;
        }
        self.invalidate_sessions_for_check(bare_id);
    }

    fn on_deregister(
        &self,
        check_id: &str,
        _namespace: &str,
        _group: &str,
        _service: &str,
        _ip: &str,
        _port: i32,
        _cluster: &str,
    ) {
        // Consul reaps strictly by ServiceID (`Agent.reapServicesInternal`):
        // a check with no resolvable ServiceID is skipped
        // (`if serviceID.ID == "" { continue }`). There is deliberately no
        // IP:Port fallback — in a cluster the check replicates to every node
        // while the check_id → service_id mapping stays on the agent that
        // accepted the registration, so address matching made every follower
        // delete whichever service happened to share the address.
        match self.deregister_by_check_id(check_id) {
            ReapOutcome::Removed => {}
            ReapOutcome::UnknownCheck => {
                warn!(
                    "ConsulResultHandler: skipping reap of check '{}' — \
                     no local service mapping",
                    check_id
                );
            }
            ReapOutcome::MissingService => {
                warn!(
                    "ConsulResultHandler: could not find service to deregister (check_id='{}')",
                    check_id
                );
            }
        }
    }
}

/// Result of a reap attempt.
///
/// The distinction matters because a miss must **not** fall back to address
/// matching: `UnknownCheck` means "another agent owns this check", which is a
/// normal, expected condition in a cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReapOutcome {
    /// The service was removed, or a Raft removal was initiated.
    Removed,
    /// No `check_id → service_id` mapping on this node.
    UnknownCheck,
    /// Mapped, but the service is no longer in the store.
    MissingService,
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
        handler.set_services(
            check_index.clone(),
            session_service,
            kv_service,
            None,
            "c1".to_string(),
        );

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
            0,         // wrong port
            "",
        );

        assert_eq!(
            store.len(),
            0,
            "Service should be deregistered via check_id chain"
        );
    }

    /// A check this node cannot resolve to a service must be skipped, never
    /// guessed at by address.
    ///
    /// Upstream `Agent.reapServicesInternal` does the same:
    /// `if serviceID.ID == "" { continue }`.
    #[test]
    fn test_deregister_unknown_check_does_not_guess_by_ip_port() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        // NO services set — no check_id → service_id mapping exists.

        let data = serde_json::to_vec(&serde_json::json!({
            "Name": "web", "ID": "web-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/web/web-1", Bytes::from(data))
            .unwrap();
        assert_eq!(store.len(), 1);

        handler.on_deregister("unknown-check", "", "", "web", "10.0.0.1", 8080, "");

        assert_eq!(
            store.len(),
            1,
            "an unresolvable check must not delete a service that matches by IP:Port"
        );
    }

    /// The regression this guards: in a cluster the check replicates to every
    /// node but the check_id → service_id mapping stays on the registering
    /// agent. Followers used to fall back to IP:Port and delete whichever
    /// service shared the address — here, the decoy.
    #[test]
    fn test_unknown_check_spares_unrelated_service_sharing_address() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        let check_index = Arc::new(ConsulCheckIndex::new());
        handler.set_services(
            check_index,
            Arc::new(ConsulSessionService::new()),
            Arc::new(ConsulKVService::new()),
            None,
            "c1".to_string(),
        );

        // Decoy registered first, so it is what an address scan hits first.
        let decoy = serde_json::to_vec(&serde_json::json!({
            "Name": "decoy", "ID": "decoy-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/decoy/decoy-1", Bytes::from(decoy))
            .unwrap();

        let target = serde_json::to_vec(&serde_json::json!({
            "Name": "web", "ID": "web-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/web/web-1", Bytes::from(target))
            .unwrap();

        // Replicated check: no local mapping, same address as both services.
        handler.on_deregister("service:web-1", "", "", "web", "10.0.0.1", 8080, "");

        assert_eq!(
            store.len(),
            2,
            "neither service may be deleted from a node that does not own the check"
        );
    }

    /// The positive counterpart: when the mapping *is* known, only the mapped
    /// service is removed even though another service shares its address.
    #[test]
    fn test_known_check_removes_only_the_mapped_service() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        let check_index = Arc::new(ConsulCheckIndex::new());
        handler.set_services(
            check_index.clone(),
            Arc::new(ConsulSessionService::new()),
            Arc::new(ConsulKVService::new()),
            None,
            "c1".to_string(),
        );

        let decoy = serde_json::to_vec(&serde_json::json!({
            "Name": "decoy", "ID": "decoy-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/decoy/decoy-1", Bytes::from(decoy))
            .unwrap();

        let target = serde_json::to_vec(&serde_json::json!({
            "Name": "web", "ID": "web-1", "Port": 8080, "Address": "10.0.0.1"
        }))
        .unwrap();
        store
            .register("default/web/web-1", Bytes::from(target))
            .unwrap();

        check_index.register_check("service:web-1", "web-1");

        handler.on_deregister("service:web-1", "", "", "web", "10.0.0.1", 8080, "");

        let remaining = PluginNamingStore::scan(&*store, "")
            .into_iter()
            .map(|(k, _)| k)
            .collect::<Vec<_>>();
        assert_eq!(
            remaining,
            vec!["default/decoy/decoy-1".to_string()],
            "only the mapped service may be removed"
        );
    }

    #[tokio::test]
    async fn test_on_check_critical_invalidates_sessions() {
        let store = Arc::new(ConsulNamingStore::new());
        let index = Arc::new(ConsulIndexProvider::new());
        let handler = ConsulResultHandler::new(store.clone(), index.clone());

        let session_service = Arc::new(ConsulSessionService::new());
        let kv_service = Arc::new(ConsulKVService::new());
        let check_index = Arc::new(ConsulCheckIndex::new());
        handler.set_services(
            check_index,
            session_service.clone(),
            kv_service,
            None,
            "c1".to_string(),
        );

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

        // The invalidation cascade is now async (spawned via tokio) so the
        // destroy doesn't land before this line. Yield a few times to let
        // the spawned task make progress. This matches the production
        // pattern where blocking-query indices get incremented slightly
        // after the initial check-critical event.
        for _ in 0..20 {
            tokio::task::yield_now().await;
            if session_service.list_sessions().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

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
        handler.set_services(
            check_index,
            session_service.clone(),
            kv_service,
            None,
            "c1".to_string(),
        );

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
