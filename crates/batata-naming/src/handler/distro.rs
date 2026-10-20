//! Naming-specific distro protocol handler for ephemeral instance synchronization
//!
//! This handler bridges the naming service with the core distro protocol,
//! allowing ephemeral service instances to be synced across cluster nodes.

use std::sync::Arc;

use batata_api::naming::model::NotifySubscriberRequest;
use batata_api::remote::model::RequestTrait;
use batata_core::service::distro::{DistroData, DistroDataHandler, DistroDataType};
use batata_core::service::remote::ConnectionManager;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::service::instance::is_distro_replica;
use crate::service::{ConnectionManagerPusher, NamingService, SubscriberPusher};

// Re-export DistroProtocol for convenience
pub use batata_core::service::distro::DistroProtocol;

/// Instance data serialized for distro sync
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroInstanceData {
    /// The `namespace` value.
    pub namespace: String,
    /// The `group_name` value.
    pub group_name: String,
    /// The `service_name` value.
    pub service_name: String,
    /// The `instances` value.
    pub instances: Vec<DistroInstance>,
}

/// Single instance for distro sync
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroInstance {
    /// The `instance_id` value.
    pub instance_id: String,
    /// The `ip` value.
    pub ip: String,
    /// The `port` value.
    pub port: i32,
    /// The `weight` value.
    pub weight: f64,
    /// The `healthy` value.
    pub healthy: bool,
    /// The `enabled` value.
    pub enabled: bool,
    /// The `ephemeral` value.
    pub ephemeral: bool,
    /// The `cluster_name` value.
    pub cluster_name: String,
    /// The `metadata` value.
    pub metadata: std::collections::HashMap<String, String>,
}

/// Composite key of a service key plus the peer that produced the data.
///
/// `last_sync_versions` is keyed per (key, source) because `DistroData::version`
/// is a per-node counter: version 7 from node A and version 7 from node B say
/// nothing about each other. Nacos gets the same property by keeping revisions
/// on the per-client object rather than on the shared service.
fn sync_version_key(key: &str, source: &str) -> String {
    format!("{}@@{}", key, source)
}

/// Default implementation for naming instance data
pub struct NamingInstanceDistroHandler {
    local_address: String,
    naming_service: Arc<NamingService>,
    /// Sink used to push updates to subscribers connected to this node.
    ///
    /// Abstracted as a trait (rather than holding a concrete
    /// `ConnectionManager`) so the push path is testable without a gRPC
    /// runtime: `with_pusher` accepts any implementation. `None` means
    /// "no subscribers to notify" — standalone wiring and unit tests that
    /// only care about data movement.
    pusher: Option<Arc<dyn SubscriberPusher>>,
    /// Key: `sync_version_key(service_key, source)`, Value: version of the
    /// most recent sync applied from that source. Consumed by
    /// `process_verify_data` to answer "am I up to date with peer X?".
    last_sync_versions: Arc<DashMap<String, i64>>,
}

impl NamingInstanceDistroHandler {
    /// Creates a new instance.
    pub fn new(local_address: String, naming_service: Arc<NamingService>) -> Self {
        Self {
            local_address,
            naming_service,
            pusher: None,
            last_sync_versions: Arc::new(DashMap::new()),
        }
    }

    /// Creates a new instance that also notifies local subscribers when
    /// remote (distro) data is merged in.
    pub fn with_connection_manager(
        local_address: String,
        naming_service: Arc<NamingService>,
        connection_manager: Arc<ConnectionManager>,
    ) -> Self {
        Self {
            local_address,
            naming_service,
            pusher: Some(Arc::new(ConnectionManagerPusher::new(connection_manager))),
            last_sync_versions: Arc::new(DashMap::new()),
        }
    }

    /// Creates a new instance with an explicit push sink.
    ///
    /// Production wiring uses [`Self::with_connection_manager`]; this is the
    /// seam tests use to observe what a node would have pushed, without
    /// standing up a gRPC connection manager.
    pub fn with_pusher(
        local_address: String,
        naming_service: Arc<NamingService>,
        pusher: Arc<dyn SubscriberPusher>,
    ) -> Self {
        Self {
            local_address,
            naming_service,
            pusher: Some(pusher),
            last_sync_versions: Arc::new(DashMap::new()),
        }
    }

    /// Push the current instance list to subscribers connected to this node.
    ///
    /// Distro replicas arrive without any local registration event, so without
    /// this the SDKs subscribed here would keep serving their cached (now
    /// stale) instance list. Nacos publishes a client-changed event on distro
    /// sync for exactly the same reason.
    fn notify_subscribers(&self, namespace: &str, group_name: &str, service_name: &str) {
        let Some(ref pusher) = self.pusher else {
            return;
        };

        let subscribers = self
            .naming_service
            .get_subscribers(namespace, group_name, service_name);
        if subscribers.is_empty() {
            return;
        }

        let service_info =
            self.naming_service
                .get_service(namespace, group_name, service_name, "", false);
        let notification =
            NotifySubscriberRequest::for_service(namespace, group_name, service_name, service_info);
        let payload = notification.build_server_push_payload();

        let subscriber_count = subscribers.len();
        let pusher = pusher.clone();
        tokio::spawn(async move {
            let sent = pusher.push_to_many(&subscribers, payload).await;
            debug!(
                "Distro sync: pushed updated instances to {}/{} subscribers",
                sent, subscriber_count
            );
        });
    }

    /// Parse service key to (namespace, group_name, service_name)
    fn parse_service_key(key: &str) -> Option<(String, String, String)> {
        let parts: Vec<&str> = key.split("@@").collect();
        if parts.len() == 3 {
            Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ))
        } else {
            None
        }
    }
}

#[tonic::async_trait]
impl DistroDataHandler for NamingInstanceDistroHandler {
    fn data_type(&self) -> DistroDataType {
        DistroDataType::NamingInstance
    }

    async fn get_all_keys(&self) -> Vec<String> {
        // Deliberately NOT filtered to "keys we own". A node with no instances
        // of its own for a key still publishes it — with an empty instance
        // list, which is exactly the signal peers need to garbage-collect the
        // replicas they hold on this node's behalf (see `get_data`). Dropping
        // the key instead would strand those replicas: nothing would ever
        // contradict them. The empty payload is a no-op at peers that hold no
        // replica tagged with this node as `_distro_source`.
        self.naming_service.get_all_service_keys()
    }

    async fn get_data(&self, key: &str) -> Option<DistroData> {
        // Parse the service key
        let (namespace, group_name, service_name) = Self::parse_service_key(key)?;

        // Snapshot instances (zero-copy Arc clones). We filter ephemerals
        // before any deep-clone, so persistent instances cost nothing here.
        //
        // `*_raw` on purpose: the gate below reads the ownership markers, and
        // this is the one read path that must see them.
        let snapshot = self.naming_service.get_instances_snapshot_raw(
            &namespace,
            &group_name,
            &service_name,
            "",    // all clusters
            false, // include unhealthy
        );

        // Only sync ephemeral instances **this node owns** — i.e. the ones its
        // own clients registered, which are exactly the ones without the
        // `_distro_remote` marker. Replicas received from a peer must never be
        // republished.
        //
        // This is batata's counterpart of Nacos' `isResponsibleClient` gate
        // (`naming/.../distro/v2/DistroClientDataProcessor.java`). There the
        // distro key is a *client id*, and a node publishes a client only when
        // `ConnectionBasedClientManager.isResponsibleClient(client)` holds —
        // which is `client instanceof ConnectionBasedClient && client.isNative()`,
        // i.e. "the connection terminates on this node". The same gate guards
        // `syncToAllServer`/`syncToVerifyFailedServer` (via `isInvalidClient`)
        // and `getVerifyData()`, and `getDatumSnapshot()` skips
        // connection-based clients outright. Replicas are therefore never
        // relayed, and every piece of data has exactly one publisher.
        //
        // batata keys distro data by *service*, so the equivalent gate has to
        // be per instance. Without it two things break at once:
        //   1. a relayed replica is re-tagged with the relayer as
        //      `_distro_source`, so the real owner's later sync — scoped to its
        //      own source — can no longer garbage-collect it;
        //   2. the owner's empty sync (its deregistration signal) removes
        //      nothing, while the relayer keeps republishing the instance.
        // The observed result was that deregistering an instance on the owning
        // node resurrected it on every node within one sync round.
        let ephemeral_instances: Vec<DistroInstance> = snapshot
            .iter()
            .filter(|inst| inst.ephemeral && !is_distro_replica(inst))
            .map(|inst| DistroInstance {
                instance_id: inst.instance_id.clone(),
                ip: inst.ip.clone(),
                port: inst.port,
                weight: inst.weight,
                healthy: inst.healthy,
                enabled: inst.enabled,
                ephemeral: inst.ephemeral,
                cluster_name: inst.cluster_name.clone(),
                metadata: inst.metadata.clone(),
            })
            .collect();

        // Always return data even when instances list is empty.
        // An empty list signals to remote nodes that all ephemeral instances
        // have been deregistered and should be removed on their side.
        let data = DistroInstanceData {
            namespace,
            group_name,
            service_name,
            instances: ephemeral_instances,
        };

        let content = serde_json::to_vec(&data).ok()?;

        // Use the local-mutation-only distro version instead of the shared
        // service revision. The revision also moves when a peer's sync is
        // merged, which would make two nodes invalidate each other's version
        // forever (see `NamingService::distro_versions`).
        let version = self.naming_service.get_distro_version(key);

        Some(DistroData {
            data_type: DistroDataType::NamingInstance,
            key: key.to_string(),
            content,
            version,
            source: self.local_address.clone(),
        })
    }

    async fn process_sync_data(&self, data: DistroData) -> Result<(), String> {
        debug!("Processing naming instance sync data: key={}", data.key);

        // NOTE: no "is this newer than what I already applied?" guard here, and
        // that is deliberate — it matches upstream. Nacos'
        // `DistroClientDataProcessor.upgradeClient` applies the received client
        // data unconditionally: it adds/updates every published instance, then
        // removes every service the client holds that is absent from the sync
        // (`client.removeServiceInstance`) and finally stores the revision.
        // The revision is only ever *compared* — in
        // `ClientManager.verifyClient` — never used to reject a sync.
        //
        // Adding an ordering guard on top of `last_sync_versions` would be
        // actively harmful here: `DistroData::version` is a per-node in-memory
        // counter, so a peer that restarts legitimately starts again at 0 and
        // every one of its syncs would be discarded as "older". Convergence
        // relies on the payload being authoritative for its source, which is
        // guaranteed by the ownership gate in `get_data`.

        // Parse the content
        let instance_data: DistroInstanceData =
            serde_json::from_slice(&data.content).map_err(|e| format!("Failed to parse: {}", e))?;

        let instances: Vec<crate::Instance> = instance_data
            .instances
            .into_iter()
            .map(|inst| crate::Instance {
                instance_id: inst.instance_id,
                ip: inst.ip,
                port: inst.port,
                weight: inst.weight,
                healthy: inst.healthy,
                enabled: inst.enabled,
                ephemeral: inst.ephemeral,
                cluster_name: inst.cluster_name,
                service_name: instance_data.service_name.clone(),
                metadata: inst.metadata,
            })
            .collect();

        // Merge-and-reconcile: use the instance keys from the sync data as the
        // authoritative set from the source node. Add/update instances present in
        // the sync, remove instances that are in our local store AND match keys
        // in the sync set's "namespace" (i.e., came from the same source).
        // Since distro sync is per-service, and each sync contains ALL instances
        // the source node knows about for this service, we can safely replace.
        // But we must NOT remove instances registered locally by this node's
        // own clients -- only merge from remote.
        let count = instances.len();
        self.naming_service.merge_remote_instances(
            &instance_data.namespace,
            &instance_data.group_name,
            &instance_data.service_name,
            instances,
            &data.source,
        );

        // Remember which version we just applied *from this source*, so the
        // next verify round can answer "am I up to date with that peer?".
        // This is the batata counterpart of Nacos storing the synced revision
        // on the replicated client object (`upgradeClient -> setRevision`).
        if !data.source.is_empty() {
            self.last_sync_versions
                .insert(sync_version_key(&data.key, &data.source), data.version);
        }

        info!(
            "Synced {} instances for service {}//{}/{} (merge)",
            count, instance_data.namespace, instance_data.group_name, instance_data.service_name
        );

        // Notify subscribers connected to THIS node about the merged data.
        self.notify_subscribers(
            &instance_data.namespace,
            &instance_data.group_name,
            &instance_data.service_name,
        );

        Ok(())
    }

    async fn process_verify_data(&self, data: &DistroData) -> Result<bool, String> {
        // "version" is a per-node counter, so it is only comparable against
        // the value this same source sent us last time. Comparing it with our
        // own counter (the previous behaviour) is meaningless: the two
        // counters advance independently, so a replica whose counter happens
        // to be ahead would never request a sync and silently stay divergent.
        //
        // Equality, not ordering — Nacos compares `client.getRevision()` with
        // the received `DistroClientVerifyInfo.revision` the same way.
        match self
            .last_sync_versions
            .get(&sync_version_key(&data.key, &data.source))
        {
            Some(applied) => Ok(*applied.value() == data.version),
            // Nothing applied from that source yet (or our copy was dropped):
            // report out of sync so the peer pushes its data.
            None => Ok(false),
        }
    }

    async fn get_snapshot(&self) -> Vec<DistroData> {
        // Get all service keys and their data
        let keys = self.get_all_keys().await;
        let mut snapshot = Vec::new();

        for key in keys {
            if let Some(data) = self.get_data(&key).await {
                snapshot.push(data);
            }
        }

        snapshot
    }

    async fn remove_data(&self, key: &str) -> Result<(), String> {
        // Called by DistroProtocol::cleanup_non_responsible_keys when the
        // local node is no longer responsible for this service_key after a
        // cluster membership change. We only drop remotely-synced ephemeral
        // instances — instances registered by local gRPC clients must be
        // preserved (they are tracked by connection_instances and will be
        // cleaned up when the client disconnects). Persistent instances
        // live in Raft and must not be touched by the Distro layer.
        //
        // Use merge_remote_instances with an empty list: it only removes
        // instances marked with `_distro_remote=true`, leaving locally-
        // registered instances intact. This matches Nacos 3.x behavior
        // where gRPC-registered instances are accepted at any node and
        // never deleted by distro responsibility changes.
        let Some((namespace, group_name, service_name)) = Self::parse_service_key(key) else {
            return Err(format!("Invalid service key: {}", key));
        };
        self.naming_service.merge_remote_instances(
            &namespace,
            &group_name,
            &service_name,
            Vec::new(),
            "",
        );
        // Drop the recorded versions too: we no longer hold any data for this
        // key, so claiming "still at version N" would suppress the next
        // resync and leave the replica permanently empty.
        let prefix = format!("{}@@", key);
        self.last_sync_versions
            .retain(|k, _| !k.starts_with(&prefix));
        debug!(
            "Removed remote ephemeral instances for non-responsible key: {}",
            key
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_core::service::distro::{DistroData, DistroDataType};

    fn build_sync_data(namespace: &str, group: &str, service: &str, ip: &str) -> DistroData {
        let content = DistroInstanceData {
            namespace: namespace.to_string(),
            group_name: group.to_string(),
            service_name: service.to_string(),
            instances: vec![DistroInstance {
                instance_id: format!("{}-{}", service, ip),
                ip: ip.to_string(),
                port: 8080,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                metadata: std::collections::HashMap::new(),
            }],
        };
        DistroData {
            data_type: DistroDataType::NamingInstance,
            key: format!("{}@@{}@@{}", namespace, group, service),
            content: serde_json::to_vec(&content).unwrap(),
            version: 1,
            source: "10.0.0.2:8848".to_string(),
        }
    }

    /// Gap A — after dispossession via `remove_data`, the service's
    /// remote-synced ephemeral instances must be gone from the naming store.
    #[tokio::test]
    async fn test_dispossession_via_remove_data() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());

        // Simulate: we received a sync for svc-foo from another node.
        let data = build_sync_data("public", "DEFAULT_GROUP", "svc-foo", "192.168.1.1");
        handler.process_sync_data(data).await.unwrap();

        let key = "public@@DEFAULT_GROUP@@svc-foo";
        let before = naming.get_instances_snapshot("public", "DEFAULT_GROUP", "svc-foo", "", false);
        assert_eq!(before.len(), 1, "sync must have populated one instance");
        assert_eq!(before[0].ip, "192.168.1.1");

        // Peer verify-failure / membership change: dispossess.
        handler.remove_data(key).await.unwrap();

        let after = naming.get_instances_snapshot("public", "DEFAULT_GROUP", "svc-foo", "", false);
        assert!(
            after.is_empty(),
            "dispossession must drop remote ephemeral instances; found {:?}",
            after.iter().map(|i| i.ip.clone()).collect::<Vec<_>>()
        );
    }

    /// Gap B — a 50-item batch (one item per service) must populate all 50
    /// services when applied through the handler's `process_sync_data`.
    #[tokio::test]
    async fn test_batch_of_50_items_all_applied() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());

        const N: usize = 50;
        for i in 0..N {
            let service = format!("svc-{:02}", i);
            let ip = format!("192.168.1.{}", i + 1);
            let data = build_sync_data("public", "DEFAULT_GROUP", &service, &ip);
            handler
                .process_sync_data(data)
                .await
                .expect("each item must apply");
        }

        let keys = handler.get_all_keys().await;
        assert_eq!(keys.len(), N, "all {} services must be present", N);

        for i in 0..N {
            let service = format!("svc-{:02}", i);
            let instances =
                naming.get_instances_snapshot("public", "DEFAULT_GROUP", &service, "", false);
            assert_eq!(
                instances.len(),
                1,
                "service {} must have one instance",
                service
            );
            assert_eq!(instances[0].ip, format!("192.168.1.{}", i + 1));
            assert!(instances[0].ephemeral);
            assert_eq!(instances[0].cluster_name, "DEFAULT");
        }
    }

    /// A `DistroData::version` is a per-node counter, so it may only ever be
    /// compared against the version the *same* peer sent last time.
    #[tokio::test]
    async fn test_verify_data_is_compared_per_source() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());
        let key = "public@@DEFAULT_GROUP@@svc-verify";

        let probe = |source: &str, version: i64| DistroData {
            data_type: DistroDataType::NamingInstance,
            key: key.to_string(),
            content: Vec::new(),
            version,
            source: source.to_string(),
        };

        // Nothing applied from that peer yet.
        assert!(
            !handler
                .process_verify_data(&probe("10.0.0.2:8848", 7))
                .await
                .unwrap()
        );

        let mut applied = build_sync_data("public", "DEFAULT_GROUP", "svc-verify", "10.2.2.2");
        applied.version = 7;
        applied.source = "10.0.0.2:8848".to_string();
        handler.process_sync_data(applied).await.unwrap();

        assert!(
            handler
                .process_verify_data(&probe("10.0.0.2:8848", 7))
                .await
                .unwrap(),
            "same peer, same version -> in sync"
        );
        assert!(
            !handler
                .process_verify_data(&probe("10.0.0.2:8848", 8))
                .await
                .unwrap(),
            "peer moved on -> stale"
        );
        assert!(
            !handler
                .process_verify_data(&probe("10.0.0.3:8848", 7))
                .await
                .unwrap(),
            "version 7 from a different node says nothing about this one"
        );
    }

    /// Dispossessing a key must forget the recorded versions too, otherwise the
    /// replica claims to be current while holding no data at all.
    #[tokio::test]
    async fn test_dispossession_forgets_recorded_versions() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());
        let key = "public@@DEFAULT_GROUP@@svc-forget";

        let mut applied = build_sync_data("public", "DEFAULT_GROUP", "svc-forget", "10.3.3.3");
        applied.version = 3;
        applied.source = "10.0.0.2:8848".to_string();
        handler.process_sync_data(applied).await.unwrap();

        let probe = DistroData {
            data_type: DistroDataType::NamingInstance,
            key: key.to_string(),
            content: Vec::new(),
            version: 3,
            source: "10.0.0.2:8848".to_string(),
        };
        assert!(handler.process_verify_data(&probe).await.unwrap());

        handler.remove_data(key).await.unwrap();
        assert!(
            !handler.process_verify_data(&probe).await.unwrap(),
            "after dispossession the replica must request a resync"
        );
    }

    /// The version we advertise for a key must be stable while we only replay
    /// somebody else's data — otherwise two nodes bump each other's version on
    /// every apply and ping-pong syncs forever.
    #[tokio::test]
    async fn test_distro_version_ignores_remote_syncs() {
        let naming = Arc::new(NamingService::new());
        let key = "public@@DEFAULT_GROUP@@svc-version";
        assert_eq!(naming.get_distro_version(key), 0);

        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-version",
            crate::Instance {
                instance_id: "local#9000#DEFAULT#svc-version".to_string(),
                ip: "10.1.0.1".to_string(),
                port: 9000,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: "svc-version".to_string(),
                metadata: std::collections::HashMap::new(),
            },
        );
        assert_eq!(
            naming.get_distro_version(key),
            1,
            "a local registration must move the version"
        );

        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());
        let mut remote = build_sync_data("public", "DEFAULT_GROUP", "svc-version", "10.9.9.9");
        remote.version = 42;
        remote.source = "10.0.0.2:8848".to_string();
        handler.process_sync_data(remote).await.unwrap();

        assert_eq!(
            naming.get_distro_version(key),
            1,
            "merging a peer's sync must NOT move our own version"
        );
    }

    /// Verify-failure flow end-to-end on the naming layer: dispossess drops
    /// previously-synced data, then a fresh sync repopulates cleanly.
    #[tokio::test]
    async fn test_dispossess_then_resync_is_clean() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());

        let d1 = build_sync_data("public", "DEFAULT_GROUP", "svc-x", "10.1.1.1");
        handler.process_sync_data(d1).await.unwrap();

        let key = "public@@DEFAULT_GROUP@@svc-x";
        handler.remove_data(key).await.unwrap();
        assert!(
            naming
                .get_instances_snapshot("public", "DEFAULT_GROUP", "svc-x", "", false)
                .is_empty()
        );

        let d2 = build_sync_data("public", "DEFAULT_GROUP", "svc-x", "10.1.1.2");
        handler.process_sync_data(d2).await.unwrap();
        let after = naming.get_instances_snapshot("public", "DEFAULT_GROUP", "svc-x", "", false);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].ip, "10.1.1.2", "re-sync must populate the new IP");
    }

    /// A node publishes only what its own clients registered. Replicas carry
    /// the `_distro_remote` marker and must never leave the node again — see
    /// the ownership note on `get_data`.
    #[tokio::test]
    async fn get_data_publishes_only_locally_owned_instances() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());
        let key = "public@@DEFAULT_GROUP@@svc-owned";

        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-owned",
            crate::Instance {
                instance_id: "local#9000#DEFAULT#svc-owned".to_string(),
                ip: "10.1.0.1".to_string(),
                port: 9000,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: "svc-owned".to_string(),
                metadata: std::collections::HashMap::new(),
            },
        );
        // A replica that arrived from a peer.
        handler
            .process_sync_data(build_sync_data(
                "public",
                "DEFAULT_GROUP",
                "svc-owned",
                "10.9.9.9",
            ))
            .await
            .unwrap();
        assert_eq!(
            naming
                .get_instances_snapshot("public", "DEFAULT_GROUP", "svc-owned", "", false)
                .len(),
            2,
            "locally owned instance plus one replica"
        );

        let data = handler.get_data(key).await.expect("payload must exist");
        let parsed: DistroInstanceData = serde_json::from_slice(&data.content).unwrap();
        assert_eq!(
            parsed.instances.len(),
            1,
            "a node must publish only what it owns; got {:?}",
            parsed
                .instances
                .iter()
                .map(|i| i.ip.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(parsed.instances[0].ip, "10.1.0.1");
        assert!(
            !parsed.instances[0]
                .metadata
                .contains_key(crate::service::instance::DISTRO_REMOTE_MARKER),
            "published instances must not carry the replica marker"
        );
    }

    /// Push every key `from` publishes into `to`, exactly like one distro sync
    /// round would.
    async fn push_all(from: &NamingInstanceDistroHandler, to: &NamingInstanceDistroHandler) {
        for key in from.get_all_keys().await {
            if let Some(data) = from.get_data(&key).await {
                let _ = to.process_sync_data(data).await;
            }
        }
    }

    fn endpoints(naming: &NamingService, service: &str) -> Vec<String> {
        let mut v: Vec<String> = naming
            .get_instances_snapshot("public", "DEFAULT_GROUP", service, "", false)
            .iter()
            .map(|i| i.ip.clone())
            .collect();
        v.sort();
        v
    }

    /// Regression: deregistering on the owning node must survive a peer that
    /// would otherwise relay the instance back.
    ///
    /// This is the two-node projection of the bug: B receives X from A, then
    /// republishes it — which used to re-tag X with `_distro_source = B`, so
    /// A's "X is gone" sync no longer matched anything and B's relay
    /// re-inserted X on A.
    #[tokio::test]
    async fn deregistration_on_owner_is_not_resurrected_by_a_peer() {
        let a_svc = Arc::new(NamingService::new());
        let b_svc = Arc::new(NamingService::new());
        let a = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), a_svc.clone());
        let b = NamingInstanceDistroHandler::new("10.0.0.2:8848".to_string(), b_svc.clone());

        a_svc.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-relay",
            crate::Instance {
                instance_id: "x#8080#DEFAULT#svc-relay".to_string(),
                ip: "10.0.0.9".to_string(),
                port: 8080,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: "svc-relay".to_string(),
                metadata: std::collections::HashMap::new(),
            },
        );

        push_all(&a, &b).await;
        assert_eq!(endpoints(&b_svc, "svc-relay"), vec!["10.0.0.9"]);

        // B publishes back — it owns nothing, so this must be a no-op.
        push_all(&b, &a).await;
        assert_eq!(endpoints(&a_svc, "svc-relay"), vec!["10.0.0.9"]);

        // Owner deregisters and the removal must stick on both sides.
        a_svc.deregister_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-relay",
            &crate::Instance {
                instance_id: "x#8080#DEFAULT#svc-relay".to_string(),
                ip: "10.0.0.9".to_string(),
                port: 8080,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: "svc-relay".to_string(),
                metadata: std::collections::HashMap::new(),
            },
        );
        push_all(&a, &b).await;
        assert!(
            endpoints(&b_svc, "svc-relay").is_empty(),
            "the replica must be garbage-collected"
        );
        push_all(&b, &a).await;
        assert!(
            endpoints(&a_svc, "svc-relay").is_empty(),
            "the instance must not be resurrected on the owner"
        );
    }

    /// Empty payloads are the garbage-collection channel: a node keeps
    /// publishing keys it owns nothing for so peers can drop its replicas.
    #[tokio::test]
    async fn key_without_owned_instances_is_published_empty() {
        let naming = Arc::new(NamingService::new());
        let handler = NamingInstanceDistroHandler::new("10.0.0.1:8848".to_string(), naming.clone());

        handler
            .process_sync_data(build_sync_data(
                "public",
                "DEFAULT_GROUP",
                "svc-empty",
                "10.9.9.9",
            ))
            .await
            .unwrap();

        let data = handler
            .get_data("public@@DEFAULT_GROUP@@svc-empty")
            .await
            .expect("the key must still be published");
        let parsed: DistroInstanceData = serde_json::from_slice(&data.content).unwrap();
        assert!(
            parsed.instances.is_empty(),
            "a node holding only replicas publishes an empty list, which is the GC signal"
        );
    }
}
