//! In-process multi-node cluster harness for naming semantics.
//!
//! Booting three real `batata-server` processes for every assertion is far too
//! slow and too flaky to be the primary home for cluster semantics, so this
//! harness models the cluster **at the distro layer**: each node owns a real
//! `NamingService` plus a real `NamingInstanceDistroHandler`, and "the network"
//! is an explicit sync round that moves `DistroData` between them.
//!
//! A sync round drives the *production* publication path —
//! `get_all_keys()` + `get_data()` — so ownership filtering is exercised for
//! real rather than being modelled by the harness. That matters: publishing a
//! replica you received from a peer re-tags it with the relayer as
//! `_distro_source`, after which the original owner can no longer
//! garbage-collect it and deregistration silently fails to propagate (the
//! instance resurrects everywhere). That was a real bug, so the harness must
//! not paper over it by deciding ownership itself.
//!
//! What this covers:
//! - ephemeral instance convergence (register on A, visible on B/C)
//! - deregistration propagation
//! - a node being offline (it simply receives no syncs) and catching up on
//!   restart, which is what distro re-sync does after a partition heals
//! - **restart** (`restart()`): the node comes back with empty memory, and
//!   recovery is driven by the two production compensation paths modelled
//!   here — `load_from()` (startup snapshot load, first peer wins, partial by
//!   design) and `verify_round()` (periodic verify, the steady-state repair)
//!
//! What this deliberately does **not** cover, and why:
//! - **Raft / leader election**: that needs a real consensus group; see
//!   `crates/batata-consistency/tests/cluster_snapshot_transfer_test.rs` for
//!   the raft-level equivalent.
//! - **True network partitions**: an in-process harness cannot drop packets
//!   between two nodes while keeping both alive. "Node stopped" is the closest
//!   faithful model and is what `stop()`/`start()` provide. Tests must not
//!   claim partition coverage.
//! - Persistent (CP) instances: those travel over Raft, not distro.
#![allow(dead_code)]

use std::sync::Arc;

use batata_core::service::distro::{DistroData, DistroDataHandler};
use batata_naming::handler::distro::NamingInstanceDistroHandler;
use batata_naming::service::SubscriberPusher;
use batata_naming::{Instance, NamingService};

/// One logical cluster node.
pub struct ClusterNode {
    /// Address used as the distro `source` for data produced by this node.
    pub addr: String,
    pub svc: Arc<NamingService>,
    pub handler: Arc<NamingInstanceDistroHandler>,
    online: bool,
    /// Push sink carried across `restart()` so a restarted node keeps
    /// notifying through the same observer.
    pusher: Option<Arc<dyn SubscriberPusher>>,
}

/// A cluster of `N` nodes wired for distro sync, with no I/O.
pub struct ClusterHarness {
    nodes: Vec<ClusterNode>,
}

impl ClusterHarness {
    /// Build `n` online nodes. Addresses mirror the dev cluster layout
    /// (`scripts/cluster_lib.sh`: 8848 / 8858 / 8868) so logs and fixtures line
    /// up with the real thing; nothing binds a socket here.
    pub fn new(n: usize) -> Self {
        Self::with_pushers(n, vec![None; n])
    }

    /// Build `n` nodes, each optionally wired to a push sink.
    ///
    /// A node without a pusher still converges — it simply records no
    /// notifications. Passing `Some` is how subscription tests observe what a
    /// node pushed to the subscribers attached to it, without a gRPC runtime.
    pub fn with_pushers(n: usize, pushers: Vec<Option<Arc<dyn SubscriberPusher>>>) -> Self {
        assert_eq!(pushers.len(), n, "one pusher slot per node");
        let nodes = (0..n)
            .map(|i| {
                let addr = format!("127.0.0.1:{}", 8848 + i * 10);
                let svc = Arc::new(NamingService::new());
                let handler = match &pushers[i] {
                    Some(p) => Arc::new(NamingInstanceDistroHandler::with_pusher(
                        addr.clone(),
                        svc.clone(),
                        p.clone(),
                    )),
                    None => Arc::new(NamingInstanceDistroHandler::new(addr.clone(), svc.clone())),
                };
                ClusterNode {
                    addr,
                    svc,
                    handler,
                    online: true,
                    pusher: pushers[i].clone(),
                }
            })
            .collect();
        Self { nodes }
    }

    /// Three nodes — the canonical dev/test cluster size.
    pub fn three() -> Self {
        Self::new(3)
    }

    /// Three nodes, each with a push sink.
    pub fn three_with_pushers(pushers: Vec<Arc<dyn SubscriberPusher>>) -> Self {
        Self::with_pushers(3, pushers.into_iter().map(Some).collect())
    }

    pub fn node(&self, i: usize) -> &ClusterNode {
        &self.nodes[i]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Register an instance on node `i` — as far as that node is concerned it
    /// is now the owner (its own client registered it).
    pub fn register(
        &mut self,
        i: usize,
        ns: &str,
        group: &str,
        name: &str,
        mut instance: Instance,
    ) {
        if instance.cluster_name.is_empty() {
            instance.cluster_name = "DEFAULT".to_string();
        }
        self.nodes[i]
            .svc
            .register_instance(ns, group, name, instance);
    }

    /// Deregister an instance on node `i`.
    pub fn deregister(&mut self, i: usize, ns: &str, group: &str, name: &str, instance: &Instance) {
        self.nodes[i]
            .svc
            .deregister_instance(ns, group, name, instance);
    }

    /// Sorted `ip:port` view of one node — order-independent assertions.
    pub fn endpoints(&self, i: usize, ns: &str, group: &str, name: &str) -> Vec<String> {
        let mut v: Vec<String> = self.nodes[i]
            .svc
            .get_instances(ns, group, name, "", false)
            .iter()
            .map(|x| format!("{}:{}", x.ip, x.port))
            .collect();
        v.sort();
        v
    }

    /// Take a node offline: it stops receiving syncs until `start()`.
    ///
    /// This models "the node is unreachable / its process is down". It is not a
    /// true partition — both sides of a partition would still be alive — so
    /// tests must not claim partition coverage.
    pub fn stop(&mut self, i: usize) {
        self.nodes[i].online = false;
    }

    /// Simulate a process restart of node `i`.
    ///
    /// A restarted node comes back with an **empty** memory: every instance it
    /// held — including the replicas of its peers — is gone. That is faithful:
    /// distro data is AP and is not persisted, so recovery is entirely up to
    /// the two compensation paths below (`load_from` and `verify_round`).
    pub fn restart(&mut self, i: usize) {
        let addr = self.nodes[i].addr.clone();
        let svc = Arc::new(NamingService::new());
        let handler = match &self.nodes[i].pusher {
            Some(p) => Arc::new(NamingInstanceDistroHandler::with_pusher(
                addr.clone(),
                svc.clone(),
                p.clone(),
            )),
            None => Arc::new(NamingInstanceDistroHandler::new(addr.clone(), svc.clone())),
        };
        let pusher = self.nodes[i].pusher.clone();
        self.nodes[i] = ClusterNode {
            addr,
            svc,
            handler,
            online: true,
            pusher,
        };
    }

    /// Attach a subscriber connection to a service on node `i`.
    ///
    /// Models an SDK that opened its bi-stream against node `i`: notifications
    /// for this service can only arrive through node `i`'s push sink.
    pub fn subscribe(&self, i: usize, conn: &str, ns: &str, group: &str, name: &str) {
        self.nodes[i].svc.subscribe(conn, ns, group, name);
    }

    /// Let any `tokio::spawn`ed push land.
    ///
    /// `notify_subscribers` spawns rather than awaiting, so a sync round
    /// returns before the notification has been recorded. Tests must flush
    /// before asserting, otherwise they race the spawned task.
    pub async fn flush_pushes(&self) {
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }

    /// One sync round, then flush spawned pushes.
    pub async fn sync_round_flushed(&self) {
        self.sync_round().await;
        self.flush_pushes().await;
    }

    /// Sync until `pred` holds, flushing pushes before every check.
    pub async fn converge_flushed<F>(&self, max_rounds: usize, pred: F) -> bool
    where
        F: Fn(&ClusterHarness) -> bool,
    {
        for _ in 0..max_rounds {
            self.flush_pushes().await;
            if pred(self) {
                return true;
            }
            self.sync_round().await;
        }
        self.flush_pushes().await;
        pred(self)
    }

    /// Snapshot load: `target` pulls a full snapshot from `source`, the way
    /// `DistroProtocol::load_initial_data` bootstraps a starting node.
    ///
    /// Deliberately **partial**. Upstream stops at the first peer that answers
    /// — `DistroLoadDataTask.loadAllDataSnapshotFromRemote` returns as soon as
    /// one `processSnapshot` succeeds (`return true` inside the member loop) —
    /// and the snapshot itself only carries what that peer owns (Nacos'
    /// `getDatumSnapshot` skips connection-based clients, and our
    /// `get_data` publishes only locally-owned instances). So a loaded node
    /// holds one peer's instances and must rely on the verify cycle for the
    /// rest. Tests assert the partial state and then the completed one.
    pub async fn load_from(&self, target: usize, source: usize) {
        for data in self.nodes[source].handler.get_snapshot().await {
            let _ = self.nodes[target].handler.process_sync_data(data).await;
        }
    }

    /// One verify round — the steady-state repair path.
    ///
    /// Every online node sends each peer the (key, version) pairs it currently
    /// holds; the peer answers through `process_verify_data`, and every key it
    /// reports as stale is pushed back to it. This is what heals a node that
    /// missed an event-driven sync (a restart, a dropped message, a brief
    /// partition) without waiting for the next local mutation.
    ///
    /// Returns how many key pushes were needed, so a test can assert that a
    /// converged cluster does **no** repair work — a cluster that keeps
    /// re-syncing on every cycle is a sync storm, not convergence.
    pub async fn verify_round(&self) -> usize {
        let online: Vec<usize> = (0..self.nodes.len())
            .filter(|i| self.nodes[*i].online)
            .collect();
        let mut pushed = 0usize;

        for i in &online {
            let mut verify: Vec<DistroData> = Vec::new();
            for key in self.nodes[*i].handler.get_all_keys().await {
                if let Some(data) = self.nodes[*i].handler.get_data(&key).await {
                    verify.push(data);
                }
            }
            for j in &online {
                if i == j {
                    continue;
                }
                let mut stale: Vec<String> = Vec::new();
                for data in &verify {
                    let in_sync = self.nodes[*j]
                        .handler
                        .process_verify_data(data)
                        .await
                        .unwrap_or(false);
                    if !in_sync {
                        stale.push(data.key.clone());
                    }
                }
                for key in stale {
                    if let Some(data) = self.nodes[*i].handler.get_data(&key).await {
                        let _ = self.nodes[*j].handler.process_sync_data(data).await;
                        pushed += 1;
                    }
                }
            }
        }
        pushed
    }

    /// One round of "verify, then sync" — the pair a real cluster runs.
    pub async fn reconcile_round(&self) -> usize {
        let pushed = self.verify_round().await;
        self.sync_round().await;
        pushed
    }

    /// Run `rounds` reconcile rounds.
    pub async fn settle_reconciling(&self, rounds: usize) {
        for _ in 0..rounds {
            self.reconcile_round().await;
        }
    }

    /// Reconcile until `pred` holds, up to `max_rounds`.
    pub async fn converge_reconciling<F>(&self, max_rounds: usize, pred: F) -> bool
    where
        F: Fn(&ClusterHarness) -> bool,
    {
        for _ in 0..max_rounds {
            if pred(self) {
                return true;
            }
            self.reconcile_round().await;
        }
        pred(self)
    }

    /// Bring a node back online. The next sync round resends the owned
    /// snapshots to it, which is how distro covers a restarted node.
    pub fn start(&mut self, i: usize) {
        self.nodes[i].online = true;
    }

    pub fn is_online(&self, i: usize) -> bool {
        self.nodes[i].online
    }

    /// Everything node `i` publishes: one payload per known service key,
    /// holding only the instances that node owns.
    async fn published(&self, i: usize) -> Vec<DistroData> {
        let handler = &self.nodes[i].handler;
        let mut out = Vec::new();
        for key in handler.get_all_keys().await {
            if let Some(data) = handler.get_data(&key).await {
                out.push(data);
            }
        }
        out
    }

    /// One distro sync round: each online node publishes to every other online
    /// node through the real handler API. Deterministic — no timers involved.
    ///
    /// Senders run sequentially and each payload is applied before the next is
    /// built, so a node never publishes a view that is stale by more than one
    /// hop.
    pub async fn sync_round(&self) {
        let online: Vec<usize> = (0..self.nodes.len())
            .filter(|i| self.nodes[*i].online)
            .collect();
        for i in &online {
            let payload = self.published(*i).await;
            for j in &online {
                if i == j {
                    continue;
                }
                for data in &payload {
                    let _ = self.nodes[*j].handler.process_sync_data(data.clone()).await;
                }
            }
        }
    }

    /// Run `rounds` sync rounds.
    pub async fn settle(&self, rounds: usize) {
        for _ in 0..rounds {
            self.sync_round().await;
        }
    }

    /// Sync until `pred` holds, up to `max_rounds`. Returns whether it
    /// converged — tests should assert on this rather than sleeping, so a
    /// failure reports "did not converge" instead of a race.
    pub async fn converge<F>(&self, max_rounds: usize, pred: F) -> bool
    where
        F: Fn(&ClusterHarness) -> bool,
    {
        for _ in 0..max_rounds {
            if pred(self) {
                return true;
            }
            self.sync_round().await;
        }
        pred(self)
    }
}
