//! Quantified amortization of health-check work across a 3-node cluster.
//!
//! "Responsibility sharding + edge-triggered de-duplication saves work" used to
//! be an assertion in a unit test: the right node probes, an unchanged status
//! does not write. What it never said was *how much* work is saved, so a
//! regression that quietly turned either mechanism off would still have passed
//! every test. These tests put numbers on both and assert the ratios.
//!
//! What is measured, and where the numbers come from
//! -------------------------------------------------
//! - **Probes**: `InstanceCheckRegistry::probes_recorded()` — one increment per
//!   probe result a node records.
//! - **Raft entries**: `raft_writes()` — one increment per probe result that
//!   actually reached the replicator. In production that is one
//!   `RaftRequest::HealthCheckStatusUpdate` per increment.
//! - **Suppressed**: `deduped_writes()` — probe results the unchanged-status
//!   guard short-circuited.
//!
//! The replicator here is a counter, not real Raft: the question is how many
//! cluster writes the guard admits, and openraft's own heartbeat/membership
//! traffic would only add noise to that count.
//!
//! Sharding is exercised through the real
//! `HealthCheckInterceptorChain::cluster(..)` against a real `DistroMapper`
//! over three members, so the A/B is "chain with the responsible interceptor"
//! versus "chain without it" — not a re-implementation of the hash in the test.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use batata_core::service::distro::DistroMapper;
use batata_naming::healthcheck::config::HealthCheckConfig;
use batata_naming::healthcheck::interceptor::HealthCheckInterceptorChain;
use batata_naming::healthcheck::registry::{
    CheckStatus, CheckType, HealthStatusReplicator, InstanceCheckConfig, InstanceCheckRegistry,
};

const NODES: [&str; 3] = ["127.0.0.1:8848", "127.0.0.1:8858", "127.0.0.1:8868"];
/// Distinct backends to probe. Deliberately a multiple of the node count so a
/// perfect split is 10/10/10 and a broken split is visibly not.
const TARGETS: usize = 30;
/// Probe rounds per target.
const ROUNDS: u64 = 20;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Replicator that counts what would be Raft writes.
///
/// Returns `false` on purpose: that makes the registry fall back to
/// `apply_status_local`, which is exactly what the Raft apply hook does after a
/// commit. Without it the local status would never advance, every probe would
/// look like a flip, and the de-duplication measurement would read zero.
struct CountingReplicator {
    status_writes: AtomicU64,
    ttl_writes: AtomicU64,
}

impl CountingReplicator {
    fn new() -> Self {
        Self {
            status_writes: AtomicU64::new(0),
            ttl_writes: AtomicU64::new(0),
        }
    }
    fn total(&self) -> u64 {
        self.status_writes.load(Ordering::Relaxed) + self.ttl_writes.load(Ordering::Relaxed)
    }
}

// `#[async_trait]` is part of the trait's contract: `async fn` in a trait is
// desugared to `-> Pin<Box<dyn Future + '_>>`, so an impl without the same
// macro produces a method whose lifetimes do not match (E0195).
#[async_trait::async_trait]
impl HealthStatusReplicator for CountingReplicator {
    async fn replicate_status(
        &self,
        _check_key: &str,
        _success: bool,
        _output: &str,
        _response_time_ms: u64,
    ) -> bool {
        self.status_writes.fetch_add(1, Ordering::Relaxed);
        false
    }
    async fn replicate_ttl(&self, _check_key: &str, _status: &str, _output: Option<&str>) -> bool {
        self.ttl_writes.fetch_add(1, Ordering::Relaxed);
        false
    }
}

fn node_registry() -> (Arc<InstanceCheckRegistry>, Arc<CountingReplicator>) {
    let naming: Arc<dyn batata_api::naming::NamingServiceProvider> = Arc::new(common::service());
    let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming));
    let replicator = Arc::new(CountingReplicator::new());
    registry.set_replicator(replicator.clone());
    (registry, replicator)
}

/// `TARGETS` distinct backends, so each has its own `ip:port` sharding key.
fn target_addr(i: usize) -> (String, i32) {
    (format!("10.0.1.{}", i + 1), 9000 + i as i32)
}

fn check_config(id: &str, ip: &str, port: i32) -> InstanceCheckConfig {
    InstanceCheckConfig {
        check_id: id.to_string(),
        name: format!("check {id}"),
        check_type: CheckType::Tcp,
        namespace: common::NS.to_string(),
        group_name: common::GROUP.to_string(),
        service_name: "svc".to_string(),
        ip: ip.to_string(),
        port,
        cluster_name: "DEFAULT".to_string(),
        http_url: None,
        tcp_addr: None,
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
        node: "node-1".to_string(),
    }
}

fn mapper() -> Arc<DistroMapper> {
    let m = Arc::new(DistroMapper::new());
    m.update_members(NODES.iter().map(|n| n.to_string()).collect());
    m
}

fn cluster_chain(addr: &str) -> HealthCheckInterceptorChain {
    HealthCheckInterceptorChain::cluster(
        Arc::new(HealthCheckConfig::default()),
        mapper(),
        addr.to_string(),
    )
}

fn standalone_chain() -> HealthCheckInterceptorChain {
    HealthCheckInterceptorChain::standalone(Arc::new(HealthCheckConfig::default()))
}

// ---------------------------------------------------------------------------
// 1. Sharding: one prober per target, not one per node
// ---------------------------------------------------------------------------

/// Every target is owned by exactly one node, and the ownership is spread
/// across all three.
///
/// This is the "same target is never probed by two nodes" claim made
/// measurable: with sharding the cluster-wide probe count for one round is
/// `TARGETS`; without it every node would probe everything (`3 * TARGETS`).
#[test]
fn sharding_gives_every_target_exactly_one_prober() {
    let chains: Vec<HealthCheckInterceptorChain> = NODES.iter().map(|n| cluster_chain(n)).collect();

    let mut owners_per_node = [0usize; 3];
    let mut multi_owner = 0usize;
    let mut unowned = 0usize;

    for i in 0..TARGETS {
        let (ip, port) = target_addr(i);
        let responsible_id = format!("{ip}:{port}");
        let owners: Vec<usize> = chains
            .iter()
            .enumerate()
            .filter(|(_, c)| c.should_execute(&responsible_id))
            .map(|(n, _)| n)
            .collect();
        match owners.len() {
            1 => owners_per_node[owners[0]] += 1,
            0 => unowned += 1,
            _ => multi_owner += 1,
        }
    }

    assert_eq!(multi_owner, 0, "a target must never have two probers");
    assert_eq!(unowned, 0, "a target must never be left without a prober");
    assert_eq!(owners_per_node.iter().sum::<usize>(), TARGETS);
    assert!(
        owners_per_node.iter().all(|c| *c > 0),
        "all three nodes must own something, got {owners_per_node:?}"
    );

    // The same workload through the standalone chain is the "off" baseline:
    // every node would probe every target.
    let off = standalone_chain();
    let total_without_sharding: usize = (0..TARGETS)
        .map(|i| {
            let (ip, port) = target_addr(i);
            NODES
                .iter()
                .filter(|_| off.should_execute(&format!("{ip}:{port}")))
                .count()
        })
        .sum();
    assert_eq!(total_without_sharding, NODES.len() * TARGETS);

    println!(
        "[sharding] targets={TARGETS} nodes={} owners={owners_per_node:?} \
         probes/round: sharded={TARGETS} unsharded={total_without_sharding} \
         ({}x fewer)",
        NODES.len(),
        total_without_sharding / TARGETS
    );
}

/// The end-to-end version: each node really only records probes for the
/// targets it owns, so the non-owners stay at zero.
#[tokio::test]
async fn cluster_probes_only_owned_targets() {
    let chains: Vec<HealthCheckInterceptorChain> = NODES.iter().map(|n| cluster_chain(n)).collect();
    let nodes: Vec<(Arc<InstanceCheckRegistry>, Arc<CountingReplicator>)> =
        (0..NODES.len()).map(|_| node_registry()).collect();

    // Register every target on every node — that is what Raft replication of
    // the check *config* does, so all three hold a full copy.
    let mut keys = Vec::with_capacity(TARGETS);
    for i in 0..TARGETS {
        let (ip, port) = target_addr(i);
        let mut key = String::new();
        for (registry, _) in &nodes {
            key = registry.register_check(check_config(&format!("c{i}"), &ip, port));
        }
        keys.push((key, format!("{ip}:{port}")));
    }

    for _round in 0..ROUNDS {
        for (node_idx, (registry, _)) in nodes.iter().enumerate() {
            for (key, responsible_id) in &keys {
                if !chains[node_idx].should_execute(responsible_id) {
                    continue;
                }
                // A stable backend: the probe outcome never changes, which is
                // the normal steady state and the case de-duplication targets.
                registry
                    .update_check_result(key, true, "ok".into(), 1)
                    .await;
            }
        }
    }

    let per_node: Vec<u64> = nodes.iter().map(|(r, _)| r.probes_recorded()).collect();
    let owned: Vec<usize> = per_node
        .iter()
        .map(|p| (*p as usize) / ROUNDS as usize)
        .collect();

    assert_eq!(
        per_node.iter().sum::<u64>(),
        (TARGETS as u64) * ROUNDS,
        "cluster-wide probes must be one per target per round, got {per_node:?}"
    );
    assert_eq!(
        owned.iter().sum::<usize>(),
        TARGETS,
        "ownership must cover every target exactly once, got {owned:?}"
    );

    // No Raft entry was produced at all: the status never changed, so the
    // edge-triggered guard suppressed every one of them.
    let writes: u64 = nodes.iter().map(|(_, c)| c.total()).sum();
    assert_eq!(writes, 0, "stable status must not produce any Raft write");

    println!(
        "[cluster] rounds={ROUNDS} targets={TARGETS} probes_per_node={per_node:?} \
         owned_per_node={owned:?} raft_writes={writes}"
    );
}

// ---------------------------------------------------------------------------
// 2. De-duplication: probes versus Raft entries
// ---------------------------------------------------------------------------

/// A stable backend produces one Raft entry per *flip*, not per probe.
///
/// The paired baseline is the same number of updates through `ttl_update`,
/// which deliberately has no unchanged-status guard (see the test below for
/// why). Comparing the two is the A/B: same volume of work, guarded versus
/// level-triggered.
#[tokio::test]
async fn edge_dedup_collapses_steady_state_probes_into_zero_writes() {
    let (registry, counter) = node_registry();
    let (ip, port) = target_addr(0);
    let key = registry.register_check(check_config("c0", &ip, port));

    const PROBES: u64 = 100;
    for _ in 0..PROBES {
        registry
            .update_check_result(&key, true, "ok".into(), 1)
            .await;
    }

    assert_eq!(registry.probes_recorded(), PROBES);
    assert_eq!(
        counter.total(),
        0,
        "an unchanged status must not write Raft"
    );
    assert_eq!(registry.deduped_writes(), PROBES);

    // A real flip does write — once.
    registry
        .update_check_result(&key, false, "refused".into(), 1)
        .await;
    assert_eq!(counter.total(), 1);

    // ...and then the steady state is suppressed again.
    for _ in 0..PROBES {
        registry
            .update_check_result(&key, false, "refused".into(), 1)
            .await;
    }
    assert_eq!(counter.total(), 1);

    let probes = registry.probes_recorded();
    let saved = registry.deduped_writes();
    println!(
        "[dedup] probes={probes} raft_writes={} suppressed={saved} \
         ({:.1}% of probes produced no cluster write)",
        counter.total(),
        (saved as f64 * 100.0) / probes as f64
    );
    assert!(
        saved as f64 / probes as f64 > 0.95,
        "steady-state probes should be almost entirely suppressed, got {saved}/{probes}"
    );
}

/// The level-triggered baseline for the measurement above.
///
/// `ttl_update` is the documented exception to de-duplication: a TTL renewal
/// that carries the *same* status must still refresh `last_updated`, because
/// `TtlMonitor` derives expiry from that timestamp alone. Silence here would
/// make every other node see the check as expired — so this test is both the
/// A/B baseline and the guard against someone "optimising" the exception away.
#[tokio::test]
async fn ttl_renewal_is_the_documented_exception_and_still_writes() {
    let (registry, counter) = node_registry();
    let (ip, port) = target_addr(0);
    let mut cfg = check_config("c0", &ip, port);
    cfg.check_type = CheckType::Ttl;
    cfg.ttl = Some(std::time::Duration::from_secs(30));
    let key = registry.register_check(cfg);

    const RENEWALS: u64 = 50;
    for _ in 0..RENEWALS {
        registry.ttl_update(&key, CheckStatus::Passing, None).await;
    }

    assert_eq!(
        counter.total(),
        RENEWALS,
        "every TTL renewal must write, even when the status is unchanged"
    );
    assert_eq!(registry.deduped_writes(), 0);

    // The point of the exception: `last_updated` has to keep moving, because
    // `TtlMonitor` decides expiry from that timestamp alone. Assert it on a
    // fresh renewal rather than on the batch above, which can complete inside
    // one millisecond.
    let before = registry
        .get_check_status(&key)
        .expect("status exists")
        .last_updated;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    registry.ttl_update(&key, CheckStatus::Passing, None).await;
    let after = registry
        .get_check_status(&key)
        .expect("status exists")
        .last_updated;
    assert!(
        after > before,
        "a same-status TTL renewal must refresh last_updated ({before} -> {after})"
    );

    println!(
        "[ttl] renewals={RENEWALS} raft_writes={} (same-status TTL updates are never de-duped)",
        counter.total()
    );
}
