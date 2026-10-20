//! 3-node naming consistency and read fan-out (distro layer).
//!
//! `cluster_convergence.rs` proves that a write reaches the other nodes. This
//! file pins down *what* reaches them and *how* a read is answered:
//!
//! - **consistency**: every field of an instance — not just its endpoint — is
//!   identical on all three nodes, including updates and health flips, and
//!   namespace / group / cluster boundaries are never crossed.
//! - **read fan-out**: distro replicates the whole set to every node, so a read
//!   is answered from local state and never has to visit a peer. That is why
//!   "any node can answer for any service" holds even with the other two nodes
//!   down, and why one sync round is enough for full fan-out.
//! - **no internal bookkeeping leaks**: `_distro_remote` / `_distro_source` are
//!   storage markers for the ownership gate and must never be visible to a
//!   caller reading instances back.
//!
//! Coverage limits are documented in `cluster/mod.rs` — no Raft, no true
//! network partition.

mod cluster;

use std::collections::HashMap;

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

fn inst_in_cluster(ip: &str, port: i32, cluster: &str) -> Instance {
    let mut i = Instance::new(ip.to_string(), port);
    i.cluster_name = cluster.to_string();
    i
}

/// A fully comparable view of one node's instances, independent of DashMap
/// iteration order.
fn state(c: &ClusterHarness, i: usize, ns: &str, group: &str, name: &str) -> Vec<String> {
    let mut v: Vec<String> = c
        .node(i)
        .svc
        .get_instances(ns, group, name, "", false)
        .iter()
        .map(|x| {
            let mut meta: Vec<String> =
                x.metadata.iter().map(|(k, v)| format!("{k}={v}")).collect();
            meta.sort();
            format!(
                "{}:{} w={} h={} e={} c={} [{}]",
                x.ip,
                x.port,
                x.weight,
                x.healthy,
                x.enabled,
                x.cluster_name,
                meta.join(",")
            )
        })
        .collect();
    v.sort();
    v
}

/// Every node must report exactly the same state.
fn consistent(c: &ClusterHarness, ns: &str, group: &str, name: &str) -> bool {
    let expected = state(c, 0, ns, group, name);
    (1..c.len()).all(|i| state(c, i, ns, group, name) == expected)
}

/// The instance set every node agrees on, sorted.
fn all_endpoints(c: &ClusterHarness, ns: &str, group: &str, name: &str) -> Vec<String> {
    let mut v: Vec<String> = c
        .node(0)
        .svc
        .get_instances(ns, group, name, "", false)
        .iter()
        .map(|x| format!("{}:{}", x.ip, x.port))
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------
// Consistency
// ---------------------------------------------------------------------------

/// Every field of a registration — not just the endpoint — reaches the peers.
#[tokio::test]
async fn all_nodes_report_identical_instance_state() {
    let mut c = ClusterHarness::three();
    let mut a = inst("10.0.0.1", 8080);
    a.weight = 2.5;
    a.enabled = false;
    a.metadata = HashMap::from([
        ("env".to_string(), "prod".to_string()),
        ("zone".to_string(), "a".to_string()),
    ]);
    c.register(0, NS, GROUP, SVC, a);

    let ok = c.converge(5, |c| consistent(c, NS, GROUP, SVC)).await;
    assert!(
        ok,
        "state did not converge: {:?}",
        state(&c, 2, NS, GROUP, SVC)
    );

    let expected = state(&c, 0, NS, GROUP, SVC);
    assert_eq!(expected.len(), 1);
    assert!(expected[0].contains("w=2.5"), "weight lost: {expected:?}");
    assert!(expected[0].contains("h=true"), "health lost: {expected:?}");
    assert!(
        expected[0].contains("e=false"),
        "enabled lost: {expected:?}"
    );
    assert!(
        expected[0].contains("env=prod") && expected[0].contains("zone=a"),
        "metadata lost: {expected:?}"
    );
}

/// An update on the owner replaces the value everywhere — the peers must not
/// keep serving the first version they ever saw.
#[tokio::test]
async fn metadata_and_weight_updates_reach_every_node() {
    let mut c = ClusterHarness::three();
    let mut a = inst("10.0.0.1", 8080);
    a.metadata = HashMap::from([("version".to_string(), "v1".to_string())]);
    c.register(0, NS, GROUP, SVC, a.clone());
    c.settle(3).await;

    let mut updated = a;
    updated.weight = 7.0;
    updated
        .metadata
        .insert("version".to_string(), "v2".to_string());
    updated
        .metadata
        .insert("canary".to_string(), "true".to_string());
    c.register(0, NS, GROUP, SVC, updated);

    let ok = c.converge(5, |c| consistent(c, NS, GROUP, SVC)).await;
    assert!(
        ok,
        "update did not converge: {:?}",
        state(&c, 1, NS, GROUP, SVC)
    );

    for i in 0..c.len() {
        let s = state(&c, i, NS, GROUP, SVC);
        assert!(s[0].contains("w=7"), "node {i} kept the old weight: {s:?}");
        assert!(
            s[0].contains("version=v2") && s[0].contains("canary=true"),
            "node {i} kept stale metadata: {s:?}"
        );
    }
}

/// A health flip on the owner is visible to `healthyOnly` readers on the peers.
#[tokio::test]
async fn health_flip_reaches_every_node() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(0, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(3).await;

    c.node(0)
        .svc
        .update_instance_health(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT", false);

    let ok = c.converge(5, |c| consistent(c, NS, GROUP, SVC)).await;
    assert!(ok, "health flip did not converge");

    for i in 0..c.len() {
        let healthy: Vec<String> = c
            .node(i)
            .svc
            .get_instances(NS, GROUP, SVC, "", true)
            .iter()
            .map(|x| format!("{}:{}", x.ip, x.port))
            .collect();
        assert_eq!(
            healthy,
            vec!["10.0.0.2:8080"],
            "node {i} must exclude the unhealthy instance from healthyOnly reads"
        );

        // The unhealthy instance is still there — just filtered out.
        assert_eq!(all_endpoints(&c, NS, GROUP, SVC).len(), 2);
    }
}

/// Namespace and group are part of the distro key and must never bleed across.
#[tokio::test]
async fn namespace_and_group_isolation_across_nodes() {
    let mut c = ClusterHarness::three();
    c.register(0, "public", GROUP, SVC, inst("10.1.0.1", 8080));
    c.register(1, "tenant-a", GROUP, SVC, inst("10.2.0.1", 8080));
    c.register(2, "public", "OTHER_GROUP", SVC, inst("10.3.0.1", 8080));
    c.settle(3).await;

    let expected_public = vec!["10.1.0.1:8080"];
    let expected_tenant = vec!["10.2.0.1:8080"];
    let expected_other = vec!["10.3.0.1:8080"];
    for i in 0..c.len() {
        assert_eq!(
            c.endpoints(i, "public", GROUP, SVC),
            expected_public,
            "node {i} mixed namespaces"
        );
        assert_eq!(
            c.endpoints(i, "tenant-a", GROUP, SVC),
            expected_tenant,
            "node {i} mixed namespaces"
        );
        assert_eq!(
            c.endpoints(i, "public", "OTHER_GROUP", SVC),
            expected_other,
            "node {i} mixed groups"
        );
    }
}

/// Cluster membership travels with the instance, so a cluster-scoped read on a
/// peer returns the same rows as on the owner.
#[tokio::test]
async fn cluster_membership_survives_sync() {
    let mut c = ClusterHarness::three();
    c.register(
        0,
        NS,
        GROUP,
        SVC,
        inst_in_cluster("10.0.0.1", 8080, "shard-a"),
    );
    c.register(
        1,
        NS,
        GROUP,
        SVC,
        inst_in_cluster("10.0.0.2", 8080, "shard-b"),
    );
    c.settle(3).await;

    for i in 0..c.len() {
        let a: Vec<String> = c
            .node(i)
            .svc
            .get_instances(NS, GROUP, SVC, "shard-a", false)
            .iter()
            .map(|x| format!("{}:{}", x.ip, x.port))
            .collect();
        let b: Vec<String> = c
            .node(i)
            .svc
            .get_instances(NS, GROUP, SVC, "shard-b", false)
            .iter()
            .map(|x| format!("{}:{}", x.ip, x.port))
            .collect();
        assert_eq!(a, vec!["10.0.0.1:8080"], "node {i} cluster shard-a: {a:?}");
        assert_eq!(b, vec!["10.0.0.2:8080"], "node {i} cluster shard-b: {b:?}");
    }
}

/// A client that moves from one node to another transfers ownership: the new
/// node must be able to publish — and therefore to deregister — the instance.
#[tokio::test]
async fn ownership_transfer_after_client_moves() {
    let mut c = ClusterHarness::three();
    let moved = inst("10.0.0.9", 8080);
    c.register(0, NS, GROUP, SVC, moved.clone());
    c.settle(3).await;

    // The client's connection moves: node 0 loses it, node 1 gains it.
    c.deregister(0, NS, GROUP, SVC, &moved);
    c.register(1, NS, GROUP, SVC, moved.clone());

    let ok = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == vec!["10.0.0.9:8080"])
        })
        .await;
    assert!(
        ok,
        "ownership did not transfer: node2={:?}",
        c.endpoints(2, NS, GROUP, SVC)
    );

    // The new owner's deregistration must reach everyone and stay gone.
    c.deregister(1, NS, GROUP, SVC, &moved);
    c.settle(10).await;
    for i in 0..c.len() {
        assert!(
            c.endpoints(i, NS, GROUP, SVC).is_empty(),
            "node {i} resurrected the instance owned by node 1"
        );
    }
}

// ---------------------------------------------------------------------------
// Read fan-out
// ---------------------------------------------------------------------------

/// Distro replicates the full set to every node, so a read never has to visit
/// a peer: with the other two nodes down, this node still answers completely.
#[tokio::test]
async fn reads_are_served_locally_without_peers() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.register(2, NS, GROUP, SVC, inst("10.0.0.3", 8080));
    c.settle(2).await;

    let expected = vec!["10.0.0.1:8080", "10.0.0.2:8080", "10.0.0.3:8080"];

    // Take every peer away and read anyway — no fan-out, no forwarding.
    c.stop(1);
    c.stop(2);
    assert_eq!(
        c.endpoints(0, NS, GROUP, SVC),
        expected,
        "a read must be answered from local state"
    );

    // A read is also side-effect free: peering with nobody changes nothing.
    c.sync_round().await;
    assert_eq!(c.endpoints(0, NS, GROUP, SVC), expected);
}

/// Any node can answer for any service, whichever node owns it.
#[tokio::test]
async fn any_node_can_answer_for_any_service() {
    let mut c = ClusterHarness::three();
    for (i, name) in ["svc-a", "svc-b", "svc-c"].into_iter().enumerate() {
        c.register(i, NS, GROUP, name, inst(&format!("10.0.1.{i}"), 8080));
    }
    c.settle(2).await;

    for i in 0..c.len() {
        for (j, name) in ["svc-a", "svc-b", "svc-c"].into_iter().enumerate() {
            assert_eq!(
                c.endpoints(i, NS, GROUP, name),
                vec![format!("10.0.1.{j}:8080")],
                "node {i} cannot answer for {name} (owned by node {j})"
            );
        }
    }
}

/// One sync round is enough for a write to reach every node — distro pushes to
/// all peers at once rather than gossiping hop by hop.
#[tokio::test]
async fn one_sync_round_is_enough_for_full_fanout() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.register(1, NS, GROUP, SVC, inst("10.0.0.2", 8080));

    c.settle(1).await;
    let expected = vec!["10.0.0.1:8080", "10.0.0.2:8080"];
    for i in 0..c.len() {
        assert_eq!(
            c.endpoints(i, NS, GROUP, SVC),
            expected,
            "node {i} did not receive both instances in one round"
        );
    }
}

/// Fanning out the same data over and over changes nothing: no duplicates, no
/// drift between the nodes.
#[tokio::test]
async fn repeated_fanout_is_idempotent() {
    let mut c = ClusterHarness::three();
    for i in 0..c.len() {
        c.register(i, NS, GROUP, SVC, inst(&format!("10.0.2.{i}"), 8080));
    }
    c.settle(1).await;
    let after_one = all_endpoints(&c, NS, GROUP, SVC);

    c.settle(15).await;

    assert_eq!(after_one.len(), 3, "each node owns one instance");
    for i in 0..c.len() {
        assert_eq!(
            c.endpoints(i, NS, GROUP, SVC),
            after_one,
            "node {i} drifted after repeated fan-out"
        );
    }
}

// ---------------------------------------------------------------------------
// Storage bookkeeping must stay invisible
// ---------------------------------------------------------------------------

/// `_distro_remote` / `_distro_source` drive the ownership gate. They are
/// written into instance metadata on every peer, so readers see them unless
/// they are stripped — Nacos has no such keys and an SDK that echoes metadata
/// back on the next registration would silently mark its own instance as a
/// replica, making it invisible to the rest of the cluster.
#[tokio::test]
async fn replica_markers_never_reach_readers() {
    let mut c = ClusterHarness::three();
    let mut a = inst("10.0.0.1", 8080);
    a.metadata = HashMap::from([("env".to_string(), "prod".to_string())]);
    c.register(0, NS, GROUP, SVC, a);
    c.settle(3).await;

    for i in 0..c.len() {
        let instances = c.node(i).svc.get_instances(NS, GROUP, SVC, "", false);
        assert_eq!(instances.len(), 1, "node {i} lost the instance");
        for key in instances[0].metadata.keys() {
            assert!(
                !key.starts_with("_distro"),
                "node {i} leaked internal marker {key} to a reader: {:?}",
                instances[0].metadata
            );
        }
        assert_eq!(
            instances[0].metadata.get("env").map(String::as_str),
            Some("prod"),
            "node {i} lost or polluted the real metadata: {:?}",
            instances[0].metadata
        );
    }
}

/// The service view used for instance lists and subscribe pushes carries the
/// same guarantee.
#[tokio::test]
async fn service_view_hides_replica_markers() {
    let mut c = ClusterHarness::three();
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.settle(3).await;

    for i in 0..c.len() {
        let service = c.node(i).svc.get_service(NS, GROUP, SVC, "", false);
        assert_eq!(service.hosts.len(), 1, "node {i}: {:?}", service.hosts);
        assert!(
            !service.hosts[0]
                .metadata
                .keys()
                .any(|k| k.starts_with("_distro")),
            "node {i} leaked internal markers through the service view: {:?}",
            service.hosts[0].metadata
        );
    }
}

// ---------------------------------------------------------------------------
// Emptying and refilling a service
// ---------------------------------------------------------------------------

/// Dropping the last instance removes it everywhere, and registering again
/// brings it back everywhere.
#[tokio::test]
async fn emptying_and_refilling_a_service_converges() {
    let mut c = ClusterHarness::three();
    let a = inst("10.0.0.1", 8080);
    c.register(0, NS, GROUP, SVC, a.clone());
    c.settle(3).await;

    c.deregister(0, NS, GROUP, SVC, &a);
    let emptied = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC).is_empty())
        })
        .await;
    assert!(emptied, "emptying the service did not converge");

    c.register(0, NS, GROUP, SVC, a);
    let refilled = c
        .converge(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == vec!["10.0.0.1:8080"])
        })
        .await;
    assert!(refilled, "re-registering did not converge");
}
