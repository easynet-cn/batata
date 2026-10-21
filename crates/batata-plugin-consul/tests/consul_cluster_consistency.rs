//! Consul cross-node consistency (T4d).
//!
//! Consul state reaches other nodes through the shared Raft group, not
//! through distro, so the questions here are different from the naming-side
//! cluster tests: does a committed entry produce the same in-memory view on
//! every node, does a restart recover, and can two nodes' identically-named
//! checks coexist.
//!
//! Every test drives the real `ConsulRaftPluginHandler` + production
//! `NamingStoreApplyHook`; see `cluster/mod.rs` for what this models and what
//! it deliberately does not.

mod cluster;

use batata_plugin_consul::naming_store::ConsulNamingStore;
use batata_plugin_consul::raft::ConsulRaftRequest;

use cluster::{ConsulCluster, check, check_key, service};

const NS: &str = "default";

/// A registration committed on one node must be visible on all of them.
///
/// The writer's store is filled by `upsert_by_service_id`; every other node's
/// store is filled only by the apply hook. If the hook were missing or wired
/// to the wrong node, this is the assertion that fails.
#[test]
fn catalog_register_reaches_every_node() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));

    for i in 0..3 {
        assert!(
            c.has_service(i, NS, "web-1"),
            "node {i} never saw the registration"
        );
        assert_eq!(c.instance_count(i, NS, "web"), 1);
    }
    assert_eq!(c.service_names(0, NS), vec!["web".to_string()]);
    assert_eq!(c.service_names(2, NS), vec!["web".to_string()]);
}

/// A deregistration must reach every node, including the node that issued it
/// (which removes locally *and* applies its own entry).
#[test]
fn catalog_deregister_reaches_every_node() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));
    c.register(1, NS, service("web", "web-2", 8081));
    assert_eq!(c.instance_count(0, NS, "web"), 2);

    assert!(c.deregister(0, NS, "web-1"));

    for i in 0..3 {
        assert!(!c.has_service(i, NS, "web-1"), "node {i} kept the ghost");
        assert!(
            c.has_service(i, NS, "web-2"),
            "node {i} lost a live service"
        );
    }
}

/// Re-registering a service ID under a new name must not leave the old name
/// behind on any node.
///
/// This is the cross-node half of `upsert_by_service_id`: the leader cleans
/// up locally, and followers only converge because the caller forwards a
/// `CatalogDeregister` for the stale key.
#[test]
fn rename_leaves_no_ghost_on_any_node() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "svc-1", 8080));

    // Same service ID, new name.
    c.register(0, NS, service("api", "svc-1", 8081));

    for i in 0..3 {
        assert_eq!(
            c.service_names(i, NS),
            vec!["api".to_string()],
            "node {i} still lists the superseded name"
        );
        assert_eq!(c.instance_count(i, NS, "web"), 0);
        assert_eq!(c.instance_count(i, NS, "api"), 1);
        assert_eq!(
            c.store_key(i, NS, "svc-1"),
            Some(ConsulNamingStore::build_key(NS, "api", "svc-1"))
        );
    }
}

/// Characterization: the stale `CatalogDeregister` is the *only* thing that
/// removes the superseded entry on followers.
///
/// The apply hook inserts by key, so nothing downstream of a
/// `CatalogRegister` performs the ID-based upsert the leader does. Forwarding
/// the stale deregister is therefore mandatory, not belt-and-braces — this
/// test pins the consequence so a future refactor of `agent::register_service`
/// cannot quietly drop it.
#[test]
fn omitting_the_stale_deregister_leaves_a_ghost_on_followers() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "svc-1", 8080));
    c.register_ignoring_stale(0, NS, service("api", "svc-1", 8081));

    // The leader upserted by ID, so it is clean.
    assert_eq!(c.service_names(0, NS), vec!["api".to_string()]);
    // Followers were only told to add the new key, so they keep the old one.
    for i in 1..3 {
        assert_eq!(
            c.service_names(i, NS),
            vec!["api".to_string(), "web".to_string()],
            "node {i} should show the ghost the missing deregister would have removed"
        );
    }
}

/// A node that receives no applies diverges, and converges again once the
/// missed entries are replayed.
#[test]
fn offline_node_misses_entries_and_catches_up_on_replay() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));

    // Node 3 goes down and misses the next registration.
    c.stop(2);
    let reg = service("api", "api-1", 9090);
    let store_key = ConsulNamingStore::build_key(NS, "api", "api-1");
    let entries = vec![ConsulRaftRequest::CatalogRegister {
        key: store_key,
        registration_json: serde_json::to_string(&reg).expect("serialize"),
    }];
    for e in &entries {
        assert_eq!(c.commit(e.clone()), 2, "only online nodes apply");
    }

    assert!(
        !c.has_service(2, NS, "api-1"),
        "offline node must not see it"
    );

    c.start(2);
    c.replay(2, entries);

    assert!(
        c.has_service(2, NS, "api-1"),
        "replay must repair the lagging node"
    );
    assert!(c.has_service(2, NS, "web-1"), "old state must survive");
}

/// A missed *deregister* is the dangerous case: without the replay the node
/// keeps serving a service that no longer exists anywhere else.
#[test]
fn offline_node_keeps_a_removed_service_until_replay() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));

    c.stop(1);
    let stale = c.store_key(0, NS, "web-1").expect("present before removal");
    let entries = vec![ConsulRaftRequest::CatalogDeregister { key: stale }];
    for e in &entries {
        c.commit(e.clone());
    }

    assert!(!c.has_service(0, NS, "web-1"));
    assert!(!c.has_service(2, NS, "web-1"));
    assert!(
        c.has_service(1, NS, "web-1"),
        "the offline node still believes it exists"
    );

    c.start(1);
    c.replay(1, entries);
    assert!(!c.has_service(1, NS, "web-1"));
}

/// A check registered on one node must be visible on all of them.
///
/// Without the `HealthCheckRegister` replication a follower would report the
/// instance as healthy no matter what the owning node's probe found.
#[test]
fn check_register_replicates_to_every_node() {
    let mut c = ConsulCluster::three();
    let key = c.register_check(0, check("node1", "svc-check", "web", "10.0.0.1", 8080));

    assert_eq!(key, check_key("node1", "svc-check"));
    for i in 0..3 {
        assert!(c.has_check(i, &key), "node {i} is missing the check");
    }
}

/// Check deregistration must replicate too, or a follower keeps applying a
/// check that no longer exists.
#[test]
fn check_deregister_replicates_to_every_node() {
    let mut c = ConsulCluster::three();
    let key = c.register_check(0, check("node1", "svc-check", "web", "10.0.0.1", 8080));
    assert_eq!(c.check_count(1), 1);

    c.deregister_check(0, &key);

    for i in 0..3 {
        assert!(!c.has_check(i, &key), "node {i} kept a removed check");
        assert_eq!(c.check_count(i), 0);
    }
}

/// Two nodes registering a check with the same ID must not collide.
///
/// This is the whole reason check keys are node-qualified: every Consul agent
/// registers `serfHealth`, so a shared key space would have one node's status
/// overwrite the other's.
#[test]
fn node_qualified_check_keys_do_not_collide_across_nodes() {
    let mut c = ConsulCluster::three();
    let a = c.register_check(0, check("node1", "serfHealth", "", "10.0.0.1", 8300));
    let b = c.register_check(1, check("node2", "serfHealth", "", "10.0.0.2", 8300));

    assert_ne!(a, b, "same check_id on different nodes must differ");
    assert_eq!(a, check_key("node1", "serfHealth"));
    assert_eq!(b, check_key("node2", "serfHealth"));

    for i in 0..3 {
        assert!(c.has_check(i, &a), "node {i} lost node1's serfHealth");
        assert!(c.has_check(i, &b), "node {i} lost node2's serfHealth");
        assert_eq!(
            c.check_count(i),
            2,
            "node {i} collapsed two node checks into one"
        );
    }
}

/// Restart: the node comes back with empty memory and must rebuild both the
/// catalog and the check registry from committed RocksDB state.
///
/// This is the production replay path (`replay_catalog_from_db` +
/// `restore_checks_from_db`), not a copy of it.
#[test]
fn restart_recovers_catalog_and_checks_from_committed_state() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));
    c.register(1, NS, service("api", "api-1", 9090));
    let check_key_0 = c.register_check(0, check("node1", "svc-check", "web", "10.0.0.1", 8080));

    // Wipe node 3's memory and let production recovery refill it.
    c.restart(2);

    assert!(
        c.has_service(2, NS, "web-1"),
        "catalog did not survive the restart"
    );
    assert!(c.has_service(2, NS, "api-1"));
    assert_eq!(c.instance_count(2, NS, "web"), 1);
    assert_eq!(c.instance_count(2, NS, "api"), 1);
    assert!(
        c.has_check(2, &check_key_0),
        "check config did not survive the restart"
    );
}

/// A restarted node must stay wired for replication — the reinstalled hook
/// has to point at the *new* store, or later applies fill memory nobody reads.
#[test]
fn restarted_node_keeps_receiving_applies() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));
    c.restart(2);

    // Register something *after* the restart.
    c.register(0, NS, service("api", "api-1", 9090));

    assert!(
        c.has_service(2, NS, "api-1"),
        "the restarted node stopped applying: hook points at the wrong store"
    );
}

/// Snapshot install is the other recovery route (a follower far enough behind
/// that the log has been purged). It must rebuild memory too — installing
/// only the RocksDB state would leave the query path empty.
#[test]
fn snapshot_install_recovers_catalog_and_checks() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));
    let check_key_0 = c.register_check(0, check("node1", "svc-check", "web", "10.0.0.1", 8080));

    // Snapshot from node 1 onto node 3.
    c.install_snapshot(2, 0).expect("snapshot install");

    assert!(c.has_service(2, NS, "web-1"));
    assert!(c.has_check(2, &check_key_0));
}

/// Deregistering a service a node never had must not replicate anything:
/// the 404 path in `agent::deregister_service` has no key to send.
#[test]
fn deregistering_an_unknown_service_replicates_nothing() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));
    let before = c.log_index();

    assert!(!c.deregister(1, NS, "does-not-exist"));

    assert_eq!(c.log_index(), before, "no entry may be committed");
    assert!(c.has_service(0, NS, "web-1"));
}

/// Every node applies the same entries with the same log index, so their
/// catalog indices must agree — that is what blocking queries wait on.
#[test]
fn catalog_index_agrees_across_nodes() {
    let mut c = ConsulCluster::three();
    c.register(0, NS, service("web", "web-1", 8080));
    c.register(1, NS, service("api", "api-1", 9090));

    let idx = c.catalog_index(0);
    assert!(idx > 1, "index must advance past its initial value");
    for i in 1..3 {
        assert_eq!(
            c.catalog_index(i),
            idx,
            "node {i} disagrees on the catalog index"
        );
    }
}

/// Namespaces stay separate on every node — a registration in one namespace
/// must not surface in another, including after replication.
#[test]
fn namespace_isolation_holds_on_every_node() {
    let mut c = ConsulCluster::three();
    c.register(0, "team-a", service("web", "web-1", 8080));
    c.register(1, "team-b", service("web", "web-1", 8080));

    for i in 0..3 {
        assert_eq!(c.instance_count(i, "team-a", "web"), 1);
        assert_eq!(c.instance_count(i, "team-b", "web"), 1);
        // Same service ID, different namespaces: both must survive.
        assert!(c.has_service(i, "team-a", "web-1"));
        assert!(c.has_service(i, "team-b", "web-1"));
    }
}
