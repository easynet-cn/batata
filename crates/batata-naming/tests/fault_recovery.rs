//! Fault injection and recovery on the distro layer.
//!
//! `cluster_convergence.rs` covers the happy path: writes reach the peers and
//! removals propagate. This file covers what happens when that path is
//! *broken* — a node restarts with empty memory, a node is gone while data
//! changes, a client reconnects somewhere else — and asserts the cluster
//! rebuilds a correct view through the two compensation paths production
//! actually has:
//!
//! * startup snapshot load (`ClusterHarness::load_from`, first peer wins, so
//!   partial by design), and
//! * periodic verify (`ClusterHarness::verify_round`), the steady-state repair
//!   that heals anything the event-driven sync missed.
//!
//! The same coverage limits as `cluster/mod.rs` apply: no Raft, no true
//! partition, persistent instances out of scope.

mod cluster;

use batata_naming::Instance;

use cluster::ClusterHarness;

const NS: &str = "public";
const GROUP: &str = "DEFAULT_GROUP";
const SVC: &str = "svc";

fn inst(ip: &str, port: i32) -> Instance {
    let mut i = Instance::new(ip.to_string(), port);
    i.cluster_name = "DEFAULT".to_string();
    i
}

fn full(c: &ClusterHarness) -> bool {
    let expected = vec!["10.0.0.1:8080", "10.0.0.2:8080"];
    (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == expected)
}

/// A restarted node bootstraps from one peer's snapshot — which is partial by
/// upstream design — and the verify cycle completes the rest.
#[tokio::test]
async fn restarted_node_recovers_the_full_view() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;
    assert!(full(&c));

    // Restart wipes node 2 completely.
    c.restart(2);
    assert!(
        c.endpoints(2, NS, GROUP, SVC).is_empty(),
        "a restarted node starts empty: distro data is not persisted"
    );

    // Snapshot load stops at the first peer that answers (upstream
    // `loadAllDataSnapshotFromRemote` returns on the first success), and that
    // snapshot only carries what the peer owns.
    c.load_from(2, 0).await;
    assert_eq!(
        c.endpoints(2, NS, GROUP, SVC),
        vec!["10.0.0.1:8080"],
        "snapshot load from one peer yields that peer's instances only"
    );

    let ok = c.converge_reconciling(5, full).await;
    assert!(
        ok,
        "restarted node did not recover the full view: {:?}",
        (0..c.len())
            .map(|i| c.endpoints(i, NS, GROUP, SVC))
            .collect::<Vec<_>>()
    );
}

/// The verify cycle alone must be able to rebuild a node that loaded nothing
/// at all — in production this is the only thing standing between a node with
/// an empty store and permanently incomplete query results, because sync is
/// event-driven and no local event fires after a restart.
#[tokio::test]
async fn verify_alone_heals_a_node_that_loaded_nothing() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;

    // No snapshot load at all — the load failed or the node started alone.
    c.restart(2);
    for _ in 0..5 {
        c.verify_round().await;
    }

    assert!(
        full(&c),
        "verify must heal an empty node; got {:?}",
        c.endpoints(2, NS, GROUP, SVC)
    );
}

/// A restarted node owns nothing, so every key it publishes carries an empty
/// instance list — which is exactly the garbage-collection signal. It must
/// only ever collect *its own* replicas, never its peers'.
#[tokio::test]
async fn restarted_node_empty_payload_does_not_delete_peers_replicas() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;

    c.restart(2);
    c.load_from(2, 0).await;
    c.settle_reconciling(4).await;

    let expected = vec!["10.0.0.1:8080", "10.0.0.2:8080"];
    for i in 0..2 {
        assert_eq!(
            c.endpoints(i, NS, GROUP, SVC),
            expected,
            "node {i} lost instances to the restarted node's empty payload"
        );
    }
}

/// A deregistration that happened while a node was down must not be undone by
/// that node coming back: the restarted node's empty payload is the *correct*
/// state, and it must never resurrect a removed instance.
#[tokio::test]
async fn restart_does_not_resurrect_a_deregistered_instance() {
    let mut c = ClusterHarness::three();
    let a = inst("10.0.0.1", 8080);
    let b = inst("10.0.0.2", 8080);
    c.register(0, NS, GROUP, SVC, a.clone());
    c.register(0, NS, GROUP, SVC, b.clone());
    c.settle(3).await;

    // Node 2 restarts and misses the deregistration entirely.
    c.stop(2);
    c.deregister(0, NS, GROUP, SVC, &a);
    c.settle(3).await;

    c.restart(2);
    c.load_from(2, 1).await;
    let expected = vec!["10.0.0.2:8080"];
    let ok = c
        .converge_reconciling(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == expected)
        })
        .await;
    assert!(
        ok,
        "restarted node resurrected a deregistered instance: {:?}",
        (0..c.len())
            .map(|i| c.endpoints(i, NS, GROUP, SVC))
            .collect::<Vec<_>>()
    );

    // And it stays gone — many more rounds must not bring it back.
    c.settle_reconciling(10).await;
    for i in 0..c.len() {
        assert_eq!(
            c.endpoints(i, NS, GROUP, SVC),
            expected,
            "node {i} resurrected the instance after settling"
        );
    }
}

/// When a client reconnects to a different node after its old node restarted,
/// ownership must follow it: exactly one publisher, no duplicates, and the new
/// owner must be able to deregister.
#[tokio::test]
async fn ownership_transfers_when_a_client_reconnects_elsewhere() {
    let mut c = ClusterHarness::three();
    let x = inst("10.0.0.1", 8080);
    c.register(0, NS, GROUP, SVC, x.clone());
    c.settle(3).await;

    // Node 0 restarts; its client reconnects to node 1 instead.
    c.restart(0);
    c.register(1, NS, GROUP, SVC, x.clone());

    let ok = c
        .converge_reconciling(6, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == vec!["10.0.0.1:8080"])
        })
        .await;
    assert!(
        ok,
        "ownership did not transfer cleanly: {:?}",
        (0..c.len())
            .map(|i| c.endpoints(i, NS, GROUP, SVC))
            .collect::<Vec<_>>()
    );

    // The new owner's deregistration must propagate and stick.
    c.deregister(1, NS, GROUP, SVC, &x);
    c.settle_reconciling(10).await;
    for i in 0..c.len() {
        assert!(
            c.endpoints(i, NS, GROUP, SVC).is_empty(),
            "node {i} kept an instance the new owner deregistered: {:?}",
            c.endpoints(i, NS, GROUP, SVC)
        );
    }
}

/// A node that never comes back leaves its instances visible: responsibility
/// is recomputed, but data is never deleted by a membership change. Nacos keeps
/// a full replica set on every node for exactly this reason.
#[tokio::test]
async fn a_node_that_never_returns_leaves_its_instances_visible() {
    let mut c = ClusterHarness::three();
    c.register(2, NS, GROUP, SVC, inst("10.0.0.3", 8080));
    c.settle(3).await;

    c.stop(2);
    c.settle_reconciling(5).await;

    for i in 0..2 {
        assert_eq!(
            c.endpoints(i, NS, GROUP, SVC),
            vec!["10.0.0.3:8080"],
            "node {i} dropped the replica of a permanently lost node"
        );
    }
}

/// Convergence must be quiet. A cluster that still reports "needs sync" on
/// every verify cycle is re-syncing forever, which is a storm, not repair.
#[tokio::test]
async fn a_converged_cluster_does_no_verify_repair_work() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;
    c.settle_reconciling(3).await;
    assert!(full(&c));

    assert_eq!(
        c.verify_round().await,
        0,
        "a converged cluster must report every key as in sync"
    );
}

/// Rolling restarts with healthy clients: each restarted node gets its client
/// back, and the cluster must end up with exactly one copy of every instance —
/// no duplicates from the re-registration, no stragglers from the restart.
#[tokio::test]
async fn repeated_restarts_do_not_duplicate_or_lose_instances() {
    let mut c = ClusterHarness::three();
    // Node 0 and node 1 each own one instance; node 2 owns none.
    let owners = [(0usize, "10.0.0.1"), (1usize, "10.0.0.2")];
    for (node, ip) in owners {
        c.register(node, NS, GROUP, SVC, inst(ip, 8080));
    }
    c.settle(3).await;

    for round in 0..4 {
        let (node, ip) = owners[round % owners.len()];
        c.restart(node);
        // The client reconnects once its node is back.
        c.register(node, NS, GROUP, SVC, inst(ip, 8080));
        // Startup load from a single peer, so recovery has real work to do.
        c.load_from(node, if node == 0 { 1 } else { 0 }).await;
        c.settle_reconciling(5).await;
        assert!(
            full(&c),
            "round {round}: cluster did not recover; {:?}",
            (0..c.len())
                .map(|i| c.endpoints(i, NS, GROUP, SVC))
                .collect::<Vec<_>>()
        );

        // No duplicates: one entry per endpoint on every node.
        for i in 0..c.len() {
            let eps = c.endpoints(i, NS, GROUP, SVC);
            assert_eq!(
                eps.len(),
                eps.iter().collect::<std::collections::HashSet<_>>().len(),
                "round {round}: node {i} has duplicate instances: {eps:?}"
            );
        }
    }
}

/// Ephemeral semantics across a restart: if the owner dies and its client does
/// *not* come back, the instance must disappear everywhere. A restarted node's
/// version counter restarts at zero, so this also pins down that the
/// GC-by-empty-payload still fires (verify compares against "what I last got
/// from that source", not against a monotonically growing number).
#[tokio::test]
async fn restarting_an_owner_without_reconnection_removes_its_instances() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;
    assert!(full(&c));

    // Node 0 restarts and its client never comes back.
    c.restart(0);
    c.load_from(0, 1).await;

    let expected = vec!["10.0.0.2:8080"];
    let ok = c
        .converge_reconciling(6, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == expected)
        })
        .await;
    assert!(
        ok,
        "an instance whose owner never came back must be reclaimed: {:?}",
        (0..c.len())
            .map(|i| c.endpoints(i, NS, GROUP, SVC))
            .collect::<Vec<_>>()
    );
}

/// The ownership markers are a storage detail: a node rebuilt entirely from
/// replicas must still serve a clean, marker-free view to clients.
#[tokio::test]
async fn a_node_rebuilt_from_replicas_serves_a_clean_view() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;

    c.restart(2);
    c.load_from(2, 0).await;
    c.settle_reconciling(5).await;
    assert!(full(&c));

    for instance in c.node(2).svc.get_instances(NS, GROUP, SVC, "", false) {
        assert!(
            !instance.metadata.keys().any(|k| k.starts_with("_distro_")),
            "replica markers leaked into a client-facing view: {:?}",
            instance.metadata
        );
    }
}

/// An offline node must catch up through verify alone once it returns — sync
/// is event-driven, so without verify a returning node would stay stale until
/// somebody happened to write again.
#[tokio::test]
async fn a_returning_node_catches_up_through_verify_only() {
    let mut c = ClusterHarness::three();
    c.stop(1);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(2, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle_reconciling(3).await;
    assert!(
        c.endpoints(1, NS, GROUP, SVC).is_empty(),
        "an offline node must not receive anything"
    );

    c.start(1);
    for _ in 0..5 {
        c.verify_round().await;
    }
    assert!(
        full(&c),
        "returning node did not catch up via verify: {:?}",
        c.endpoints(1, NS, GROUP, SVC)
    );
}

/// Two services must recover independently — a restart must not mix them up,
/// and a service that only exists on one node must not be stranded on the
/// others.
#[tokio::test]
async fn recovery_is_scoped_per_service() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, "svc-a", inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, "svc-b", inst("10.0.0.2", 8080));
    c.settle(3).await;

    c.restart(2);
    c.load_from(2, 0).await;

    // Right after a partial load node 2 holds svc-a but not svc-b.
    assert_eq!(
        c.endpoints(2, NS, GROUP, "svc-a"),
        vec!["10.0.0.1:8080"],
        "svc-a came from the peer it loaded from"
    );
    assert!(
        c.endpoints(2, NS, GROUP, "svc-b").is_empty(),
        "svc-b belongs to a peer it did not load from yet"
    );

    c.settle_reconciling(5).await;
    for i in 0..c.len() {
        assert_eq!(
            c.endpoints(i, NS, GROUP, "svc-a"),
            vec!["10.0.0.1:8080"],
            "node {i}: svc-a wrong after recovery"
        );
        assert_eq!(
            c.endpoints(i, NS, GROUP, "svc-b"),
            vec!["10.0.0.2:8080"],
            "node {i}: svc-b wrong after recovery"
        );
    }
}
