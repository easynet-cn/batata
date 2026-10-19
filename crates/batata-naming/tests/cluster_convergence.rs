//! Cluster convergence semantics for ephemeral instances (distro layer).
//!
//! These are the invariants `scripts/test.sh sdk --matrix` can only check
//! indirectly ("the SDK didn't error"): that a write on one node actually
//! reaches the others, that removals propagate, and that a node which was
//! offline catches up when it returns.
//!
//! Coverage limits are documented in `cluster/mod.rs` — notably, no Raft and
//! no true network partition.

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

#[tokio::test]
async fn registration_on_one_node_reaches_the_others() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));

    assert_eq!(c.endpoints(0, NS, GROUP, SVC), vec!["10.0.0.1:8080"]);
    // Before any sync the peers legitimately know nothing.
    assert!(c.endpoints(1, NS, GROUP, SVC).is_empty());

    let ok = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == vec!["10.0.0.1:8080"])
        })
        .await;
    assert!(ok, "registration did not converge across the cluster");
}

#[tokio::test]
async fn registrations_from_different_nodes_merge_on_every_node() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.register(2, NS, GROUP, SVC, inst("10.0.0.3", 8080));

    let expected = vec!["10.0.0.1:8080", "10.0.0.2:8080", "10.0.0.3:8080"];
    let ok = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == expected)
        })
        .await;
    assert!(ok, "three-node registrations did not merge");
}

#[tokio::test]
async fn deregistration_propagates_to_peers() {
    let mut c = ClusterHarness::three();
    let a = inst("10.0.0.1", 8080);
    let b = inst("10.0.0.2", 8080);
    c.register(0, NS, GROUP, SVC, a.clone());
    c.register(0, NS, GROUP, SVC, b.clone());
    c.settle(3).await;
    assert_eq!(
        c.endpoints(1, NS, GROUP, SVC),
        vec!["10.0.0.1:8080", "10.0.0.2:8080"]
    );

    c.deregister(0, NS, GROUP, SVC, &a);
    let ok = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == vec!["10.0.0.2:8080"])
        })
        .await;
    assert!(ok, "deregistration did not propagate");
}

#[tokio::test]
async fn offline_node_misses_updates_then_catches_up_on_return() {
    let mut c = ClusterHarness::three();
    // Node 2 is down while two instances are registered and synced.
    c.stop(2);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;

    assert!(
        c.endpoints(2, NS, GROUP, SVC).is_empty(),
        "an offline node must not receive syncs"
    );
    assert_eq!(
        c.endpoints(0, NS, GROUP, SVC),
        vec!["10.0.0.1:8080", "10.0.0.2:8080"]
    );

    // Restart: distro re-syncs the full snapshot and the node catches up.
    c.start(2);
    let expected = vec!["10.0.0.1:8080", "10.0.0.2:8080"];
    let ok = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == expected)
        })
        .await;
    assert!(ok, "restarted node did not catch up");
}

#[tokio::test]
async fn repeated_sync_rounds_are_idempotent() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.settle(1).await;
    let after_one = c.endpoints(1, NS, GROUP, SVC);

    // Many extra rounds must not duplicate instances or drift the version.
    c.settle(10).await;
    assert_eq!(c.endpoints(1, NS, GROUP, SVC), after_one);
    assert_eq!(after_one.len(), 1);
}

/// Regression for the "deregistration resurrects the instance" bug.
///
/// Before the ownership gate in
/// `NamingInstanceDistroHandler::get_data`, nodes relayed the replicas they
/// received, re-tagging them with the relayer as `_distro_source`. The owner's
/// empty sync was then scoped to a source no replica carried any more, so
/// nothing was removed — while the relayer kept republishing the instance. One
/// round later every node had it back.
#[tokio::test]
async fn deregistration_does_not_resurrect_after_many_rounds() {
    let mut c = ClusterHarness::three();
    let a = inst("10.0.0.1", 8080);
    let b = inst("10.0.0.2", 8080);
    c.register(0, NS, GROUP, SVC, a.clone());
    c.register(0, NS, GROUP, SVC, b.clone());
    c.settle(3).await;

    c.deregister(0, NS, GROUP, SVC, &a);
    // Far more rounds than convergence needs: a resurrection bug shows up as
    // "converged at first, then the instance is back".
    c.settle(10).await;

    let expected = vec!["10.0.0.2:8080"];
    for i in 0..c.len() {
        assert_eq!(
            c.endpoints(i, NS, GROUP, SVC),
            expected,
            "node {i} resurrected the deregistered instance"
        );
    }
}

/// A node that was offline while an instance was deregistered must lose that
/// instance when it comes back, not reintroduce it to the cluster.
#[tokio::test]
async fn offline_node_does_not_resurrect_a_deregistered_instance() {
    let mut c = ClusterHarness::three();
    let a = inst("10.0.0.1", 8080);
    let b = inst("10.0.0.2", 8080);
    c.register(0, NS, GROUP, SVC, a.clone());
    c.register(0, NS, GROUP, SVC, b.clone());
    c.settle(3).await;

    // Node 1 misses the deregistration entirely and keeps its stale replica.
    c.stop(1);
    c.deregister(0, NS, GROUP, SVC, &a);
    c.settle(5).await;
    assert_eq!(
        c.endpoints(1, NS, GROUP, SVC),
        vec!["10.0.0.1:8080", "10.0.0.2:8080"],
        "the offline node must hold on to its stale replica"
    );

    // Back online: the owner's payload (a is gone) must win.
    c.start(1);
    let expected = vec!["10.0.0.2:8080"];
    let ok = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == expected)
        })
        .await;
    assert!(
        ok,
        "returning node resurrected a deregistered instance: node1={:?}",
        c.endpoints(1, NS, GROUP, SVC)
    );
}
