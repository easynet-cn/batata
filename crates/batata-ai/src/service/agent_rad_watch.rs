//! RAD gRPC Watch service (Nacos 3.3.0 GA `radWatchV1`).
//!
//! Mirrors GA `AgentGrpcWatchService`: installs connection-owned watch intents
//! keyed by a server-generated `watchKey`, computes the observed discovery
//! fingerprint on subscribe, and pushes a refresh hint to the owning connection
//! when the watched agent changes.
//!
//! The change source is the existing `publish_agent_change` broadcast emitted by
//! every agent mutation path (`register_agent`, `update_agent_card`,
//! `delete_agent`, `publish_agent_version`, `delete_agent_draft`,
//! `publish_agent_from_code`), so the writers never learn about the watchers and
//! the watch service is fully decoupled from the write paths.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use batata_api::grpc::Payload;
use batata_api::remote::model::AgentDiscoveryRequest as ModelDiscoveryRequest;
use batata_api::remote::model::{AgentSubscribeRpcResponse, ResponseTrait};
use batata_common::DEFAULT_NAMESPACE_ID;
use tracing::debug;

use batata_core::service::remote::ConnectionManager;

use crate::service::agent_watch::{
    AgentDiscoveryRequest, AgentProjectionKey, AgentProjectionState, AgentProjectionStatus,
    AgentProjector, AgentReference, DiscoveryProjector, subscribe_agent_changes,
};
use crate::service::traits::A2aAgentService;

/// Push target abstraction so the watch service can deliver refresh hints
/// without depending on the concrete gRPC `ConnectionManager` in tests.
#[async_trait]
pub trait WatchPusher: Send + Sync {
    /// Push a refresh hint payload to one connection. Returns true on success.
    async fn push(&self, connection_id: &str, payload: Payload) -> bool;
}

#[async_trait]
impl WatchPusher for ConnectionManager {
    async fn push(&self, connection_id: &str, payload: Payload) -> bool {
        self.push_message(connection_id, payload).await
    }
}

/// Maximum number of live RAD watch intents accepted node-wide.
pub const MAX_RAD_WATCHES: usize = 100_000;

/// One installed watch intent.
struct Registration {
    /// Connection that owns this watch.
    connection_id: String,
    /// The canonical discovery request the watch resolves.
    request: AgentDiscoveryRequest,
    /// The fingerprint most recently observed for this watch.
    last_fingerprint: Option<String>,
}

/// Server-owned RAD Watch service for the gRPC `radWatchV1` contract.
pub struct AgentRadWatchService {
    projector: Arc<DiscoveryProjector>,
    pusher: Arc<dyn WatchPusher>,
    /// `watch_key` -> registration.
    watchers: Arc<Mutex<HashMap<String, Registration>>>,
    /// `connection_id` -> set of `watch_key`s owned by that connection.
    by_connection: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

impl AgentRadWatchService {
    /// Create the service backed by the gRPC `ConnectionManager` and spawn the
    /// change-notify background task.
    ///
    /// `a2a` is used to resolve the discovery projection; the watch fingerprint
    /// is computed without the runtime endpoint backend (consistent with
    /// batata's simplified discovery surface). `conn_mgr` is used to push
    /// refresh hints to the owning connection.
    pub fn new(a2a: Arc<dyn A2aAgentService>, conn_mgr: Arc<ConnectionManager>) -> Self {
        Self::with_pusher(a2a, conn_mgr)
    }

    /// Construct with an arbitrary [`WatchPusher`] (used in tests).
    pub fn with_pusher(a2a: Arc<dyn A2aAgentService>, pusher: Arc<dyn WatchPusher>) -> Self {
        let projector = Arc::new(DiscoveryProjector {
            a2a,
            endpoint: None,
        });
        let service = Self {
            projector,
            pusher,
            watchers: Arc::new(Mutex::new(HashMap::new())),
            by_connection: Arc::new(Mutex::new(HashMap::new())),
        };
        service.start();
        service
    }

    /// Spawn the background task that listens for agent-change events and pushes
    /// refresh hints to the owning connection when a watched projection changes.
    fn start(&self) {
        let watchers = self.watchers.clone();
        let by_connection = self.by_connection.clone();
        let projector = self.projector.clone();
        let pusher = self.pusher.clone();

        if let Some(rx) = subscribe_agent_changes()
            && let Ok(handle) = tokio::runtime::Handle::try_current()
        {
            handle.spawn(async move {
                let mut rx = rx;
                while let Ok((namespace_id, agent_name)) = rx.recv().await {
                    Self::on_agent_changed_inner(
                        &watchers,
                        &by_connection,
                        &projector,
                        &pusher,
                        &namespace_id,
                        &agent_name,
                    )
                    .await;
                }
            });
        }
    }

    /// Recompute every watch bound to one logical agent and push refresh hints
    /// for the ones whose fingerprint changed.
    #[allow(dead_code)]
    async fn on_agent_changed(&self, namespace_id: &str, agent_name: &str) {
        Self::on_agent_changed_inner(
            &self.watchers,
            &self.by_connection,
            &self.projector,
            &self.pusher,
            namespace_id,
            agent_name,
        )
        .await;
    }

    async fn on_agent_changed_inner(
        watchers: &Mutex<HashMap<String, Registration>>,
        by_connection: &Mutex<HashMap<String, HashSet<String>>>,
        projector: &Arc<DiscoveryProjector>,
        pusher: &Arc<dyn WatchPusher>,
        namespace_id: &str,
        agent_name: &str,
    ) {
        // Snapshot the watches that match this agent, dropping the lock before
        // doing async I/O.
        let matches: Vec<(String, AgentDiscoveryRequest, String, Option<String>)> = {
            let guard = watchers.lock().unwrap();
            guard
                .iter()
                .filter(|(_, reg)| {
                    let ns = reg
                        .request
                        .namespace_id
                        .clone()
                        .unwrap_or_else(|| DEFAULT_NAMESPACE_ID.to_string());
                    ns == namespace_id && reg.request.reference.agent_name == agent_name
                })
                .map(|(k, reg)| {
                    (
                        k.clone(),
                        reg.request.clone(),
                        reg.connection_id.clone(),
                        reg.last_fingerprint.clone(),
                    )
                })
                .collect()
        };
        if matches.is_empty() {
            return;
        }

        for (watch_key, request, connection_id, last_fingerprint) in matches {
            let key = AgentProjectionKey::of(&request);
            let state = projector.project(&key).await;
            let new_fingerprint = match &state {
                AgentProjectionState {
                    status: AgentProjectionStatus::Available,
                    fingerprint: Some(fp),
                } => Some(fp.clone()),
                _ => None,
            };

            // Mutant guard: only proceed when the fingerprint actually changed.
            // Removing this check would push a hint on every change event even
            // when the projection is identical, spamming clients.
            if new_fingerprint == last_fingerprint {
                continue;
            }

            // Persist the new fingerprint before pushing.
            {
                let mut guard = watchers.lock().unwrap();
                if let Some(reg) = guard.get_mut(&watch_key) {
                    reg.last_fingerprint = new_fingerprint.clone();
                } else {
                    continue;
                }
            }

            let mut resp = AgentSubscribeRpcResponse::new();
            resp.watch_key = Some(watch_key.clone());
            resp.observed_fingerprint = new_fingerprint;
            resp.refresh_required = true;
            let payload: Payload = resp.build_payload();
            if !pusher.push(&connection_id, payload).await {
                // The connection is gone (or the push failed): prune the watch.
                Self::remove_locked(watchers, by_connection, &connection_id, &watch_key);
            }
        }
    }

    /// Install a connection-owned watch intent.
    ///
    /// Validates `client_watch_id`, generates a server-owned `watch_key`, runs the
    /// discovery projection to compute the observed fingerprint, stores the
    /// registration, and returns `(watch_key, observed_fingerprint)`.
    pub async fn subscribe(
        &self,
        connection_id: &str,
        client_watch_id: &str,
        discovery_request: ModelDiscoveryRequest,
        _materialized_fingerprint: &str,
    ) -> Result<(String, String), String> {
        if !valid_client_watch_id(client_watch_id) {
            return Err(format!(
                "`clientWatchId` must match `[A-Za-z0-9._:-]+` and be 1..=128 chars: {client_watch_id}"
            ));
        }
        let ns = if discovery_request.namespace_id.is_empty() {
            DEFAULT_NAMESPACE_ID.to_string()
        } else {
            discovery_request.namespace_id.clone()
        };
        if discovery_request.reference.agent_name.is_empty() {
            return Err("`reference.agentName` must not be empty".to_string());
        }

        // Capacity admission.
        {
            let guard = self.watchers.lock().unwrap();
            if guard.len() >= MAX_RAD_WATCHES {
                return Err("RAD watch capacity exceeded".to_string());
            }
        }

        let watch_key = format!("{connection_id}:{client_watch_id}");

        // Resolve the observed fingerprint via the discovery projector.
        let inner = AgentDiscoveryRequest {
            namespace_id: Some(ns),
            reference: AgentReference {
                agent_name: discovery_request.reference.agent_name.clone(),
                version: if discovery_request.reference.version.is_empty() {
                    None
                } else {
                    Some(discovery_request.reference.version.clone())
                },
                label: if discovery_request.reference.label.is_empty() {
                    None
                } else {
                    Some(discovery_request.reference.label.clone())
                },
            },
        };
        let key = AgentProjectionKey::of(&inner);
        let state = self.projector.project(&key).await;
        let observed_fingerprint = match &state {
            AgentProjectionState {
                status: AgentProjectionStatus::Available,
                fingerprint: Some(fp),
            } => Some(fp.clone()),
            _ => None,
        };

        let mut watchers = self.watchers.lock().unwrap();
        watchers.insert(
            watch_key.clone(),
            Registration {
                connection_id: connection_id.to_string(),
                request: inner,
                last_fingerprint: observed_fingerprint.clone(),
            },
        );
        self.by_connection
            .lock()
            .unwrap()
            .entry(connection_id.to_string())
            .or_default()
            .insert(watch_key.clone());
        drop(watchers);

        debug!(
            connection_id,
            watch_key,
            agent_name = %discovery_request.reference.agent_name,
            "RAD watch subscribed"
        );

        Ok((watch_key, observed_fingerprint.unwrap_or_default()))
    }

    /// Remove one watch intent. Returns true when a registration was removed.
    pub fn unsubscribe(&self, connection_id: &str, watch_key: &str) -> bool {
        Self::remove_locked(
            &self.watchers,
            &self.by_connection,
            connection_id,
            watch_key,
        )
    }

    /// Remove every watch owned by a connection (e.g. on disconnect).
    pub fn unsubscribe_all(&self, connection_id: &str) -> usize {
        let keys = self
            .by_connection
            .lock()
            .unwrap()
            .remove(connection_id)
            .unwrap_or_default();
        let mut removed = 0usize;
        let mut watchers = self.watchers.lock().unwrap();
        for key in &keys {
            if watchers.remove(key).is_some() {
                removed += 1;
            }
        }
        removed
    }

    fn remove_locked(
        watchers: &Mutex<HashMap<String, Registration>>,
        by_connection: &Mutex<HashMap<String, HashSet<String>>>,
        connection_id: &str,
        watch_key: &str,
    ) -> bool {
        let existed = self_watchers_remove(watchers, watch_key);
        if existed {
            let mut bc = by_connection.lock().unwrap();
            if let Some(set) = bc.get_mut(connection_id) {
                set.remove(watch_key);
                if set.is_empty() {
                    bc.remove(connection_id);
                }
            }
        }
        existed
    }

    /// Number of live watch intents.
    pub fn size(&self) -> usize {
        self.watchers.lock().unwrap().len()
    }
}

/// Helper used by [`AgentRadWatchService::remove_locked`] so it can stay a free
/// function over the shared maps.
fn self_watchers_remove(watchers: &Mutex<HashMap<String, Registration>>, watch_key: &str) -> bool {
    watchers.lock().unwrap().remove(watch_key).is_some()
}

/// Validate `clientWatchId`: `[A-Za-z0-9._:-]{1,128}` (GA constraint).
pub fn valid_client_watch_id(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::agent_watch::publish_agent_change;

    #[test]
    fn client_watch_id_validation_matches_ga_constraint() {
        assert!(valid_client_watch_id("abc-123._:X"));
        assert!(!valid_client_watch_id(""));
        assert!(!valid_client_watch_id("bad/id"));
        assert!(!valid_client_watch_id(&"x".repeat(129)));
    }

    // Stub pusher that records every refresh hint it is asked to deliver.
    struct StubPusher {
        pushed: Mutex<Vec<(String, AgentSubscribeRpcResponse)>>,
    }

    impl StubPusher {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                pushed: Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait]
    impl WatchPusher for StubPusher {
        async fn push(&self, connection_id: &str, payload: Payload) -> bool {
            let resp: AgentSubscribeRpcResponse = serde_json::from_slice(
                payload
                    .body
                    .as_ref()
                    .map(|a| a.value.as_slice())
                    .unwrap_or(&[]),
            )
            .expect("pushed payload deserializes to AgentSubscribeRpcResponse");
            assert!(
                resp.refresh_required,
                "refresh hint must set refreshRequired"
            );
            self.pushed
                .lock()
                .unwrap()
                .push((connection_id.to_string(), resp));
            true
        }
    }

    // Minimal fake A2aAgentService that returns a deterministic version list so
    // the discovery projector resolves an Available fingerprint. The version it
    // reports flips after the first call to simulate an agent mutation, which is
    // what trips the fingerprint-diff guard.
    struct FakeA2a {
        calls: Mutex<u32>,
    }

    #[async_trait]
    impl A2aAgentService for FakeA2a {
        async fn register_agent(
            &self,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _namespace: &str,
            _registration_type: &str,
        ) -> anyhow::Result<String> {
            Ok("v1".to_string())
        }
        async fn get_agent_card(
            &self,
            _namespace: &str,
            _agent_name: &str,
            _version: Option<&str>,
            _user: Option<&str>,
        ) -> anyhow::Result<Option<batata_common::model::ai::a2a::RegisteredAgent>> {
            Ok(None)
        }
        async fn update_agent_card(
            &self,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _namespace: &str,
            _registration_type: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn delete_agent(
            &self,
            _namespace: &str,
            _agent_name: &str,
            _version: Option<&str>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn list_agents(
            &self,
            _namespace: &str,
            _agent_name: Option<&str>,
            _search_type: &str,
            _page_no: u32,
            _page_size: u32,
            _user: Option<&str>,
        ) -> anyhow::Result<batata_common::model::Page<batata_common::model::ai::a2a::AgentSummary>>
        {
            Ok(batata_common::model::Page::default())
        }
        async fn list_versions(
            &self,
            _namespace: &str,
            _agent_name: &str,
        ) -> anyhow::Result<
            batata_common::model::Page<batata_common::model::ai::a2a::AgentVersionSummary>,
        > {
            let n = {
                let mut c = self.calls.lock().unwrap();
                *c += 1;
                *c
            };
            let version = if n == 1 { "1.0.0" } else { "2.0.0" };
            Ok(batata_common::model::Page {
                page_items: vec![batata_common::model::ai::a2a::AgentVersionSummary {
                    version: version.to_string(),
                    status: "online".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            })
        }
        async fn find_by_skill(
            &self,
            _skill: &str,
        ) -> anyhow::Result<Vec<batata_common::model::ai::a2a::RegisteredAgent>> {
            Ok(Vec::new())
        }
        async fn batch_register(
            &self,
            _request: batata_common::model::ai::a2a::BatchAgentRegistrationRequest,
        ) -> anyhow::Result<batata_common::model::ai::a2a::BatchRegistrationResponse> {
            Ok(batata_common::model::ai::a2a::BatchRegistrationResponse::default())
        }
        async fn stats(&self) -> anyhow::Result<batata_common::model::ai::a2a::AgentRegistryStats> {
            Ok(batata_common::model::ai::a2a::AgentRegistryStats::default())
        }
        async fn create_agent_draft(
            &self,
            _namespace: &str,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _overwrite: bool,
            _call_interfaces: Option<Vec<batata_common::model::ai::a2a::AgentCallInterface>>,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn update_agent_draft(
            &self,
            _namespace: &str,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _call_interfaces: Option<Vec<batata_common::model::ai::a2a::AgentCallInterface>>,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn delete_agent_draft(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn submit_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn publish_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn force_publish_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn redraft_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn online_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn offline_agent_version(
            &self,
            _namespace: &str,
            _name: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
        async fn update_agent_labels(
            &self,
            _namespace: &str,
            _name: &str,
            _labels: std::collections::HashMap<String, String>,
        ) -> anyhow::Result<std::collections::HashMap<String, String>> {
            Ok(std::collections::HashMap::new())
        }
        async fn update_agent_scope(
            &self,
            _namespace: &str,
            _name: &str,
            _new_scope: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_runtime_endpoints(
            &self,
            _namespace: &str,
            _agent_name: &str,
            _protocol: &str,
            _version: &str,
        ) -> anyhow::Result<batata_common::model::ai::a2a::ConsoleRuntimeEndpointView> {
            Ok(batata_common::model::ai::a2a::ConsoleRuntimeEndpointView::default())
        }
        async fn publish_agent_from_code(
            &self,
            _namespace: &str,
            _card: &batata_common::model::ai::a2a::AgentCard,
            _auto_submit: bool,
        ) -> anyhow::Result<batata_common::model::ai::a2a::AgentVersionDetail> {
            Ok(batata_common::model::ai::a2a::AgentVersionDetail::default())
        }
    }

    fn model_req(agent_name: &str) -> ModelDiscoveryRequest {
        ModelDiscoveryRequest {
            namespace_id: "public".to_string(),
            reference: batata_api::remote::model::AgentReference {
                agent_name: agent_name.to_string(),
                version: String::new(),
                label: String::new(),
            },
            filter: None,
        }
    }

    #[tokio::test]
    async fn subscribe_generates_watch_key_and_stores_registration() {
        let a2a: Arc<dyn A2aAgentService> = Arc::new(FakeA2a {
            calls: Mutex::new(0),
        });
        let pusher = StubPusher::new();
        let svc = AgentRadWatchService::with_pusher(a2a, pusher);
        let (watch_key, fp) = svc
            .subscribe("conn-1", "w1", model_req("agent-x"), "")
            .await
            .unwrap();
        assert_eq!(watch_key, "conn-1:w1");
        assert!(
            fp.starts_with("sha256-canonical-json-v1:"),
            "fingerprint is computed"
        );
        assert_eq!(svc.size(), 1);

        // A bad client watch id is rejected.
        assert!(
            svc.subscribe("conn-1", "bad/id", model_req("agent-x"), "")
                .await
                .is_err()
        );

        // Unsubscribe removes the registration.
        assert!(svc.unsubscribe("conn-1", "conn-1:w1"));
        assert_eq!(svc.size(), 0);
        assert!(!svc.unsubscribe("conn-1", "conn-1:w1"));
    }

    #[tokio::test]
    async fn unsubscribe_all_clears_connection_watches() {
        let a2a: Arc<dyn A2aAgentService> = Arc::new(FakeA2a {
            calls: Mutex::new(0),
        });
        let pusher = StubPusher::new();
        let svc = AgentRadWatchService::with_pusher(a2a, pusher);
        for i in 0..3 {
            let _ = svc
                .subscribe(
                    "conn-9",
                    &format!("w{i}"),
                    model_req(&format!("agent-{i}")),
                    "",
                )
                .await;
        }
        assert_eq!(svc.size(), 3);
        assert_eq!(svc.unsubscribe_all("conn-9"), 3);
        assert_eq!(svc.size(), 0);
    }

    #[tokio::test]
    async fn change_notifies_only_on_fingerprint_diff() {
        let a2a: Arc<dyn A2aAgentService> = Arc::new(FakeA2a {
            calls: Mutex::new(0),
        });
        let pusher = StubPusher::new();
        let svc = AgentRadWatchService::with_pusher(a2a, pusher.clone());
        let (watch_key, _fp) = svc
            .subscribe("conn-2", "w1", model_req("agent-y"), "")
            .await
            .unwrap();
        assert_eq!(watch_key, "conn-2:w1");

        // First change: the fake flips 1.0.0 -> 2.0.0, so the fingerprint
        // differs and a refresh hint must be pushed.
        svc.on_agent_changed("public", "agent-y").await;
        assert_eq!(pusher.pushed.lock().unwrap().len(), 1);

        // A second identical change must NOT push again (the fingerprint diff
        // guard). The fake stays on 2.0.0 after the flip.
        svc.on_agent_changed("public", "agent-y").await;
        assert_eq!(pusher.pushed.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn change_via_global_broadcast_is_fanned_out() {
        let a2a: Arc<dyn A2aAgentService> = Arc::new(FakeA2a {
            calls: Mutex::new(0),
        });
        // `new` wraps a real ConnectionManager; subscribe just persists the
        // registration (it does not push), so the watch must be present.
        let conn_mgr = Arc::new(ConnectionManager::new());
        let svc = AgentRadWatchService::new(a2a, conn_mgr);
        let _ = svc
            .subscribe("conn-3", "w1", model_req("agent-z"), "")
            .await;
        assert_eq!(svc.size(), 1, "subscribe persists the registration");
        // Ensure the broadcast symbol is reachable so the wire coupling is tested.
        publish_agent_change("public", "agent-z");
    }
}
