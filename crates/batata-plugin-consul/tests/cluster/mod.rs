//! In-process multi-node Consul cluster harness.
//!
//! Booting three real `batata-server` processes for every assertion is far too
//! slow and too flaky to be the primary home for cluster semantics, so this
//! harness models the cluster **at the Raft state-machine layer**: each node
//! owns a real `ConsulNamingStore`, a real `InstanceCheckRegistry`, a real
//! RocksDB with the Consul column families, and a real
//! `ConsulRaftPluginHandler` with the *production* `NamingStoreApplyHook`
//! installed. "Commit" is an explicit round that applies one serialized
//! `ConsulRaftRequest` to every online node — i.e. exactly what the state
//! machine does after Raft commits an entry, minus consensus.
//!
//! What this covers:
//! - `CatalogRegister` / `CatalogDeregister` reaching every node
//! - `HealthCheckRegister` / `HealthCheckDeregister` reaching every node
//! - the upsert-by-service-ID contract across nodes (a rename must not leave
//!   a ghost on followers)
//! - node-qualified check keys (two nodes registering `serfHealth` must not
//!   collapse into one entry)
//! - a node being offline (it simply receives no applies) and catching up
//! - **restart** (`restart()`): the node comes back with empty memory and
//!   recovers through the production replay path
//!   (`replay_catalog_from_db` + `restore_checks_from_db`)
//! - snapshot install as an alternative recovery route
//!
//! What this deliberately does **not** cover, and why:
//! - **Consensus / leader election**: no openraft here. Ordering is
//!   total (single log), which is what Raft guarantees; *which* node proposes
//!   is not modelled because it does not affect the state machine.
//! - **True network partitions**: `stop()`/`start()` model "receives no
//!   applies", not dropped packets. Tests must not claim partition coverage.
//! - **The HTTP handlers** (`agent::register_service` and friends): they need
//!   an actix request plus a live `RaftNode`. The request sequences here are
//!   the ones those handlers emit, kept in the same order — see
//!   [`ConsulCluster::register`].
#![allow(dead_code)]

use std::sync::Arc;

use batata_consistency::raft::plugin::RaftPluginHandler;
use batata_naming::healthcheck::registry::{
    CheckStatus, CheckType, InstanceCheckConfig, InstanceCheckRegistry, qualify_check_key,
};
use batata_plugin::HealthCheckResultHandler;
use bytes::Bytes;
use rocksdb::{DB, Options};
use tempfile::TempDir;

use batata_plugin_consul::index_provider::{ConsulTable, ConsulTableIndex};
use batata_plugin_consul::model::AgentServiceRegistration;
use batata_plugin_consul::naming_store::ConsulNamingStore;
use batata_plugin_consul::plugin::{
    NamingStoreApplyHook, replay_catalog_from_db, restore_checks_from_db,
};
use batata_plugin_consul::raft::{ConsulRaftPluginHandler, ConsulRaftRequest};

/// Result handler that swallows health transitions.
///
/// The cluster tests assert on registry contents, not on side effects
/// projected into a `NamingService`, so a sink keeps them independent of the
/// naming-side handler.
struct NoopResultHandler;

impl HealthCheckResultHandler for NoopResultHandler {
    fn on_health_changed(
        &self,
        _namespace: &str,
        _group: &str,
        _service: &str,
        _ip: &str,
        _port: i32,
        _cluster: &str,
        _healthy: bool,
    ) {
    }
}

/// One logical Consul node.
pub struct ConsulNode {
    /// Agent name. Consul identifies a check by `(node, check_id)`, so this
    /// is part of check identity, not decoration.
    pub node_name: String,
    /// Address, mirroring the dev cluster layout (`8848 / 8858 / 8868`).
    pub addr: String,
    pub store: Arc<ConsulNamingStore>,
    pub registry: Arc<InstanceCheckRegistry>,
    pub table_index: ConsulTableIndex,
    pub handler: Arc<ConsulRaftPluginHandler>,
    /// Committed state. Only the CFs Consul owns matter here.
    db: Arc<DB>,
    /// Kept alive so the RocksDB directory outlives the test.
    _tmp: TempDir,
    online: bool,
}

/// A Consul cluster of `N` nodes wired for Raft apply, with no sockets.
pub struct ConsulCluster {
    nodes: Vec<ConsulNode>,
    /// Monotonic Raft log index. One commit = one entry = one index.
    log_index: u64,
}

impl ConsulCluster {
    /// Build `n` online nodes.
    pub fn new(n: usize) -> Self {
        let nodes = (0..n)
            .map(|i| {
                Self::build_node(
                    &format!("node{}", i + 1),
                    format!("127.0.0.1:{}", 8848 + i * 10),
                )
            })
            .collect();
        Self {
            nodes,
            log_index: 0,
        }
    }

    /// Three online nodes — the shape every cluster test starts from.
    pub fn three() -> Self {
        Self::new(3)
    }

    fn build_node(node_name: &str, addr: String) -> ConsulNode {
        let tmp = TempDir::new().expect("tempdir for consul test db");
        let cf_names = ConsulRaftPluginHandler::new(ConsulTableIndex::new()).column_families();
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let db =
            Arc::new(DB::open_cf(&opts, tmp.path(), cf_names).expect("open consul test rocksdb"));

        let store = Arc::new(ConsulNamingStore::new());
        let registry = Arc::new(InstanceCheckRegistry::new(Arc::new(NoopResultHandler)));
        let table_index = ConsulTableIndex::new();

        let handler = ConsulRaftPluginHandler::new_arc(table_index.clone());
        let hook: Arc<dyn batata_plugin_consul::raft::ConsulApplyHook> =
            Arc::new(NamingStoreApplyHook::new(store.clone(), registry.clone()));
        // Nothing else holds the slot during construction, so `try_write`
        // cannot fail here.
        handler
            .apply_hook()
            .try_write()
            .expect("apply hook slot uncontended at construction")
            .replace(hook);

        ConsulNode {
            node_name: node_name.to_string(),
            addr,
            store,
            registry,
            table_index,
            handler,
            db,
            _tmp: tmp,
            online: true,
        }
    }

    // ---------------------------------------------------------------- commit

    /// Apply one request to every online node, as the state machine would
    /// after Raft commits it. Returns how many nodes applied it.
    pub fn commit(&mut self, req: ConsulRaftRequest) -> usize {
        let payload = serde_json::to_vec(&req).expect("serialize ConsulRaftRequest");
        self.log_index += 1;
        let log_index = self.log_index;
        let mut applied = 0;
        for node in self.nodes.iter() {
            if !node.online {
                continue;
            }
            let resp = node
                .handler
                .apply(&node.db, "consul_write", &payload, log_index);
            assert!(
                resp.success,
                "apply failed for {:?}: {:?}",
                req, resp.message
            );
            applied += 1;
        }
        applied
    }

    /// Raft log index of the last committed entry.
    pub fn log_index(&self) -> u64 {
        self.log_index
    }

    // ----------------------------------------------------------- membership

    /// Take a node offline: it stops receiving applies.
    pub fn stop(&mut self, i: usize) {
        self.nodes[i].online = false;
    }

    /// Bring a node back. It does **not** replay what it missed — that is
    /// what `catch_up` / `restart` are for, and leaving it out is what makes
    /// the divergence visible instead of implicit.
    pub fn start(&mut self, i: usize) {
        self.nodes[i].online = true;
    }

    pub fn is_online(&self, i: usize) -> bool {
        self.nodes[i].online
    }

    // ------------------------------------------------- agent write sequences

    /// Register a service on `on`, mirroring the request sequence
    /// `agent::register_service` emits:
    ///
    /// 1. local `upsert_by_service_id`, which returns the stale key of a
    ///    replaced entry (a rename),
    /// 2. `CatalogDeregister` for that stale key — **before** the register,
    ///    because a follower must not be left holding the ghost,
    /// 3. `CatalogRegister` for the new key.
    ///
    /// Ordering is the property under test, so it lives in the harness next
    /// to the assertions about it rather than being re-derived per test.
    pub fn register(&mut self, on: usize, ns: &str, reg: AgentServiceRegistration) -> String {
        let service_id = reg.service_id();
        let store_key = ConsulNamingStore::build_key(ns, &reg.name, &service_id);
        let data = Bytes::from(serde_json::to_vec(&reg).expect("serialize registration"));

        let stale =
            self.nodes[on]
                .store
                .upsert_by_service_id(ns, &store_key, &service_id, data.clone());
        if let Some(stale_key) = stale {
            self.commit(ConsulRaftRequest::CatalogDeregister { key: stale_key });
        }
        let registration_json = String::from_utf8_lossy(&data).to_string();
        self.commit(ConsulRaftRequest::CatalogRegister {
            key: store_key.clone(),
            registration_json,
        });
        store_key
    }

    /// Register *without* forwarding the stale key's `CatalogDeregister`.
    ///
    /// Not a production path — it exists so tests can show what the stale
    /// deregister actually buys. See the caller for why the answer is
    /// "everything": the apply hook inserts by key, so nothing on a follower
    /// ever removes the superseded entry on its own.
    pub fn register_ignoring_stale(&mut self, on: usize, ns: &str, reg: AgentServiceRegistration) {
        let service_id = reg.service_id();
        let store_key = ConsulNamingStore::build_key(ns, &reg.name, &service_id);
        let data = Bytes::from(serde_json::to_vec(&reg).expect("serialize registration"));
        self.nodes[on]
            .store
            .upsert_by_service_id(ns, &store_key, &service_id, data.clone());
        let registration_json = String::from_utf8_lossy(&data).to_string();
        self.commit(ConsulRaftRequest::CatalogRegister {
            key: store_key,
            registration_json,
        });
    }

    /// Deregister a service on `on`, mirroring `agent::deregister_service`:
    /// remove locally, then replicate the removal by key.
    ///
    /// Returns `false` when the node had no such service — the caller's 404
    /// path, which must *not* replicate anything.
    pub fn deregister(&mut self, on: usize, ns: &str, service_id: &str) -> bool {
        let store_key = self.nodes[on].store.find_key_by_service_id(ns, service_id);
        let removed = self.nodes[on].store.remove_by_service_id(ns, service_id);
        if removed && let Some(key) = store_key {
            self.commit(ConsulRaftRequest::CatalogDeregister { key });
        }
        removed
    }

    /// Persist a check config and replicate it, mirroring the
    /// `HealthCheckRegister` write in `agent::register_service`.
    ///
    /// The key is the config's own `registry_key()` — node-qualified — which
    /// is what stops two nodes' identically-named checks from colliding.
    pub fn register_check(&mut self, on: usize, config: InstanceCheckConfig) -> String {
        let check_key = config.registry_key();
        self.nodes[on].registry.register_check(config.clone());
        let config_json = serde_json::to_string(&config).expect("serialize check config");
        self.commit(ConsulRaftRequest::HealthCheckRegister {
            check_id: check_key.clone(),
            config_json,
        });
        check_key
    }

    /// Deregister a check and replicate, mirroring `agent::deregister_service`.
    pub fn deregister_check(&mut self, on: usize, check_key: &str) {
        self.nodes[on].registry.deregister_check(check_key);
        self.commit(ConsulRaftRequest::HealthCheckDeregister {
            check_id: check_key.to_string(),
        });
    }

    // ------------------------------------------------------------- recovery

    /// Re-apply the whole log to node `i`. Models a node that missed entries
    /// and is now replaying them — distro re-sync and Raft catch-up both end
    /// up here.
    ///
    /// The log is not retained entry-by-entry (this harness applies eagerly),
    /// so callers pass the entries to replay explicitly. That is deliberate:
    /// it makes each test say *what* the lagging node was fed.
    pub fn replay(&mut self, i: usize, entries: Vec<ConsulRaftRequest>) {
        for req in entries {
            let payload = serde_json::to_vec(&req).expect("serialize ConsulRaftRequest");
            self.log_index += 1;
            let node = &self.nodes[i];
            let resp = node
                .handler
                .apply(&node.db, "consul_write", &payload, self.log_index);
            assert!(resp.success, "replay apply failed: {:?}", resp.message);
        }
    }

    /// Restart node `i`: it comes back with **empty** in-memory state, then
    /// recovers through the production replay path.
    ///
    /// RocksDB survives (that is the whole point of it), so replaying the
    /// committed CFs is sufficient and is exactly what
    /// `ConsulPlugin::build_inner_cluster` does at startup.
    pub fn restart(&mut self, i: usize) {
        let node = &mut self.nodes[i];
        node.store = Arc::new(ConsulNamingStore::new());
        node.registry = Arc::new(InstanceCheckRegistry::new(Arc::new(NoopResultHandler)));

        // The hook holds `Arc`s to the old store/registry — reinstall it
        // against the fresh ones, or the node would keep filling memory that
        // nobody reads any more.
        let hook: Arc<dyn batata_plugin_consul::raft::ConsulApplyHook> = Arc::new(
            NamingStoreApplyHook::new(node.store.clone(), node.registry.clone()),
        );
        node.handler
            .apply_hook()
            .try_write()
            .expect("apply hook slot uncontended")
            .replace(hook);

        let db = node.db.clone();
        let store = node.store.clone();
        let registry = node.registry.clone();
        replay_catalog_from_db(&db, &store);
        restore_checks_from_db(&db, &registry);
    }

    /// Install node `from`'s snapshot onto node `to` and replay it into
    /// memory. Models snapshot-based catch-up (a follower far enough behind
    /// that the log has been purged).
    pub fn install_snapshot(&mut self, to: usize, from: usize) -> Result<(), String> {
        // Snapshot bytes are produced and consumed by the same handler, so
        // the two nodes must be borrowed disjointly.
        let snapshot = {
            let src = &self.nodes[from];
            src.handler.build_snapshot(&src.db)?
        };
        let dst = &self.nodes[to];
        dst.handler.install_snapshot(&dst.db, &snapshot)?;
        let (db, store, registry) = (dst.db.clone(), dst.store.clone(), dst.registry.clone());
        replay_catalog_from_db(&db, &store);
        restore_checks_from_db(&db, &registry);
        Ok(())
    }

    // --------------------------------------------------------------- queries

    /// Service names visible on node `i` in `ns`, sorted.
    pub fn service_names(&self, i: usize, ns: &str) -> Vec<String> {
        let mut names = self.nodes[i].store.service_names(ns);
        names.sort();
        names
    }

    /// Number of registrations visible on node `i` for `service_name`.
    pub fn instance_count(&self, i: usize, ns: &str, service_name: &str) -> usize {
        self.nodes[i]
            .store
            .get_service_entries(ns, service_name)
            .len()
    }

    /// Whether node `i` can resolve `service_id` in `ns`.
    pub fn has_service(&self, i: usize, ns: &str, service_id: &str) -> bool {
        self.nodes[i]
            .store
            .get_by_service_id(ns, service_id)
            .is_some()
    }

    /// The store key node `i` holds for `service_id`, if any.
    pub fn store_key(&self, i: usize, ns: &str, service_id: &str) -> Option<String> {
        self.nodes[i].store.find_key_by_service_id(ns, service_id)
    }

    /// Whether node `i`'s registry knows `check_key`.
    pub fn has_check(&self, i: usize, check_key: &str) -> bool {
        self.nodes[i].registry.has_check(check_key)
    }

    /// Total checks in node `i`'s registry.
    pub fn check_count(&self, i: usize) -> usize {
        self.nodes[i].registry.check_count()
    }

    /// Consul catalog index on node `i`, used for blocking-query assertions.
    pub fn catalog_index(&self, i: usize) -> u64 {
        self.nodes[i]
            .table_index
            .current_index(ConsulTable::Catalog)
    }

    /// Node name, for building node-qualified check keys.
    pub fn node_name(&self, i: usize) -> &str {
        &self.nodes[i].node_name
    }

    /// Direct access to a node's store, for assertions the helpers don't cover.
    pub fn node(&self, i: usize) -> &ConsulNode {
        &self.nodes[i]
    }
}

impl ConsulNode {
    /// The RocksDB backing this node.
    pub fn db(&self) -> &Arc<DB> {
        &self.db
    }

    /// The naming store backing this node.
    pub fn store(&self) -> &Arc<ConsulNamingStore> {
        &self.store
    }

    /// The check registry backing this node.
    pub fn registry(&self) -> &Arc<InstanceCheckRegistry> {
        &self.registry
    }
}

// ------------------------------------------------------------------ fixtures

/// Build a service registration with the given name / id / port.
pub fn service(name: &str, id: &str, port: u16) -> AgentServiceRegistration {
    AgentServiceRegistration {
        id: Some(id.to_string()),
        name: name.to_string(),
        port: Some(port),
        address: Some("127.0.0.1".to_string()),
        tags: Some(vec![]),
        ..Default::default()
    }
}

/// Build a check config owned by `node`, attached to `service_name`.
///
/// `node` is part of the identity: `registry_key()` yields `node#check_id`,
/// so two nodes registering the same `check_id` get independent entries.
pub fn check(
    node: &str,
    check_id: &str,
    service_name: &str,
    ip: &str,
    port: i32,
) -> InstanceCheckConfig {
    InstanceCheckConfig {
        check_id: check_id.to_string(),
        name: check_id.to_string(),
        check_type: CheckType::Tcp,
        namespace: "consul".to_string(),
        group_name: "DEFAULT_GROUP".to_string(),
        service_name: service_name.to_string(),
        ip: ip.to_string(),
        port,
        cluster_name: "DEFAULT".to_string(),
        http_url: None,
        tcp_addr: Some(format!("{ip}:{port}")),
        grpc_addr: None,
        db_url: None,
        script: None,
        args: None,
        docker_container_id: None,
        interval: std::time::Duration::from_secs(10),
        timeout: std::time::Duration::from_secs(1),
        ttl: None,
        success_before_passing: 0,
        failures_before_critical: 0,
        deregister_critical_after: None,
        initial_status: CheckStatus::Passing,
        notes: String::new(),
        service_tags: vec![],
        node: node.to_string(),
    }
}

/// Node-qualified check key — re-exported so tests don't re-derive the
/// `node#check_id` shape by hand.
pub fn check_key(node: &str, check_id: &str) -> String {
    qualify_check_key(node, check_id)
}
