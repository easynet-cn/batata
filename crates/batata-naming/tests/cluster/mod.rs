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
use batata_naming::{Instance, NamingService};

/// One logical cluster node.
pub struct ClusterNode {
    /// Address used as the distro `source` for data produced by this node.
    pub addr: String,
    pub svc: Arc<NamingService>,
    pub handler: Arc<NamingInstanceDistroHandler>,
    online: bool,
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
        let nodes = (0..n)
            .map(|i| {
                let addr = format!("127.0.0.1:{}", 8848 + i * 10);
                let svc = Arc::new(NamingService::new());
                let handler = Arc::new(NamingInstanceDistroHandler::new(addr.clone(), svc.clone()));
                ClusterNode {
                    addr,
                    svc,
                    handler,
                    online: true,
                }
            })
            .collect();
        Self { nodes }
    }

    /// Three nodes — the canonical dev/test cluster size.
    pub fn three() -> Self {
        Self::new(3)
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
    pub fn stop(&mut self, i: usize) {
        self.nodes[i].online = false;
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
