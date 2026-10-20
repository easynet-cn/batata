//! Consistency semantics of the naming data path.
//!
//! Three areas that the instance-lifecycle suite (#66) does not cover:
//!
//! 1. **Distro sharding / replica ownership.** A node only ever garbage-
//!    collects the replicas it received *from a given source*, so a sync from
//!    node X can never delete node Y's replicas, and locally registered
//!    instances (no `_distro_remote` marker) are never reclaimed at all.
//!    Upstream anchor: `DistroClientDataProcessor#processData` reconciles per
//!    *clientId* — batata publishes per instance, so the same per-source
//!    scoping is applied to instances instead.
//! 2. **Distro version accounting.** The version is a per-node counter that
//!    only advances on *local* mutations. Replaying a peer's sync must leave it
//!    untouched, otherwise the two nodes ping-pong forever.
//! 3. **The AP/CP routing gates**, including what they must *not* do: a
//!    rejected write may not fix the service-level ephemeral flag, because
//!    `resolve_service_ephemeral` seeds the flag on first sight.
//!
//! The `_distro_remote` / `_distro_source` markers are `pub(crate)` in
//! `batata-naming`; the literals below mirror `DISTRO_REMOTE_MARKER` and
//! `DISTRO_SOURCE_MARKER` so this suite stays an external-crate test.

mod common;

use std::sync::Arc;

use batata_naming::service::{build_service_key, ClientOperationService};
use common::{GROUP, NS, instance, persistent_instance, query, service};

const SVC: &str = "svc";

/// Mirrors `DISTRO_REMOTE_MARKER`.
const REMOTE: &str = "_distro_remote";
/// Mirrors `DISTRO_SOURCE_MARKER`.
const SOURCE: &str = "_distro_source";

/// Two distinct source nodes, as `ip:port` like the distro transport uses.
const N1: &str = "10.0.0.1:8848";
const N2: &str = "10.0.0.2:8848";

fn snapshot(svc: &batata_naming::NamingService) -> Vec<batata_api::naming::model::Instance> {
    svc.get_instances_snapshot(NS, GROUP, SVC, "", false)
        .into_iter()
        .map(|arc| (*arc).clone())
        .collect()
}

fn find<'a>(
    instances: &'a [batata_api::naming::model::Instance],
    ip: &str,
) -> &'a batata_api::naming::model::Instance {
    instances
        .iter()
        .find(|i| i.ip == ip)
        .unwrap_or_else(|| panic!("instance {ip} is missing from {:?}", common::endpoints(instances)))
}

fn marker(
    instances: &[batata_api::naming::model::Instance],
    ip: &str,
    key: &str,
) -> Option<String> {
    find(instances, ip).metadata.get(key).cloned()
}

fn ips(svc: &batata_naming::NamingService) -> Vec<String> {
    let mut v: Vec<String> = snapshot(svc).iter().map(|i| i.ip.clone()).collect();
    v.sort();
    v
}

/// Merge `instances` as a distro sync arriving from `source`.
fn sync(
    svc: &batata_naming::NamingService,
    instances: Vec<batata_api::naming::model::Instance>,
    source: &str,
) {
    svc.merge_remote_instances(NS, GROUP, SVC, instances, source);
}

fn version(svc: &batata_naming::NamingService, name: &str) -> i64 {
    svc.get_distro_version(&build_service_key(NS, GROUP, name))
}

// ---------------------------------------------------------------------------
// 1) Distro replica ownership
// ---------------------------------------------------------------------------

/// A locally registered instance carries no replica marker, which is what keeps
/// it out of the replica garbage-collection filter.
#[test]
fn locally_registered_instance_is_not_marked_as_a_replica() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    let all = snapshot(&svc);
    assert_eq!(marker(&all, "10.0.0.1", REMOTE), None);
    assert_eq!(marker(&all, "10.0.0.1", SOURCE), None);
}

#[test]
fn synced_instances_carry_the_source_node() {
    let svc = service();
    sync(&svc, vec![instance("10.0.0.1", 8080)], N1);

    let all = snapshot(&svc);
    assert_eq!(marker(&all, "10.0.0.1", REMOTE).as_deref(), Some("true"));
    assert_eq!(marker(&all, "10.0.0.1", SOURCE).as_deref(), Some(N1));
}

/// Deregistration propagates as an *empty* sync from the owner: every replica
/// that owner had placed here disappears. This is the #81 path.
#[test]
fn empty_sync_from_the_owner_removes_its_replicas() {
    let svc = service();
    sync(
        &svc,
        vec![instance("10.0.0.1", 8080), instance("10.0.0.2", 8080)],
        N1,
    );
    assert_eq!(ips(&svc), vec!["10.0.0.1", "10.0.0.2"]);

    sync(&svc, vec![], N1);
    assert!(ips(&svc).is_empty(), "the owner's empty sync is the GC signal");
}

/// A sync from node Y must never garbage-collect node X's replicas — the
/// reconcile is scoped per source.
#[test]
fn sync_from_another_owner_keeps_the_first_owners_replicas() {
    let svc = service();
    sync(&svc, vec![instance("10.0.0.1", 8080)], N1);
    sync(&svc, vec![instance("10.0.0.2", 8080)], N2);

    sync(&svc, vec![], N2);

    assert_eq!(
        ips(&svc),
        vec!["10.0.0.1"],
        "only node N2's own replica is reclaimed"
    );
}

#[test]
fn sync_omitting_one_instance_removes_only_that_instance() {
    let svc = service();
    sync(
        &svc,
        vec![instance("10.0.0.1", 8080), instance("10.0.0.2", 8080)],
        N1,
    );

    sync(&svc, vec![instance("10.0.0.1", 8080)], N1);

    assert_eq!(ips(&svc), vec!["10.0.0.1"]);
}

/// `source == ""` is the dispossession path: every remote replica goes,
/// whichever node placed it.
#[test]
fn dispossession_sync_wipes_every_replica() {
    let svc = service();
    sync(&svc, vec![instance("10.0.0.1", 8080)], N1);
    sync(&svc, vec![instance("10.0.0.2", 8080)], N2);

    sync(&svc, vec![], "");

    assert!(ips(&svc).is_empty());
}

/// The whole point of #81: a locally registered instance is not a replica, so
/// no peer's empty sync may delete it.
#[test]
fn locally_registered_instance_survives_an_empty_sync() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    sync(&svc, vec![instance("10.0.0.2", 8080)], N1);

    sync(&svc, vec![], N1);

    assert_eq!(ips(&svc), vec!["10.0.0.1"]);
}

/// Ownership must be transferable: when a client reconnects to another node
/// that node's syncs re-tag the replica, and only then can *it* reclaim the
/// replica with an empty sync. Freezing the first owner would strand it.
#[test]
fn ownership_transfer_retags_the_replica() {
    let svc = service();
    sync(&svc, vec![instance("10.0.0.1", 8080)], N1);
    assert_eq!(marker(&snapshot(&svc), "10.0.0.1", SOURCE).as_deref(), Some(N1));

    sync(&svc, vec![instance("10.0.0.1", 8080)], N2);
    assert_eq!(
        marker(&snapshot(&svc), "10.0.0.1", SOURCE).as_deref(),
        Some(N2),
        "the new owner's sync must be able to adopt the replica"
    );

    sync(&svc, vec![], N2);
    assert!(ips(&svc).is_empty(), "and then reclaim it");
}

/// A sync must not replace a locally registered instance with a remote-marked
/// copy: doing so would let the next empty sync delete it.
#[test]
fn sync_never_overwrites_a_locally_registered_instance() {
    let svc = service();
    let mut local = instance("10.0.0.1", 8080);
    local.weight = 2.0;
    svc.register_instance(NS, GROUP, SVC, local);

    let mut remote = instance("10.0.0.1", 8080);
    remote.weight = 9.0;
    remote.healthy = false;
    sync(&svc, vec![remote], N1);

    let all = snapshot(&svc);
    assert_eq!(all.len(), 1);
    assert_eq!(find(&all, "10.0.0.1").weight, 2.0, "local values win");
    assert!(find(&all, "10.0.0.1").healthy);
    assert_eq!(
        marker(&all, "10.0.0.1", REMOTE),
        None,
        "the local instance must stay unmarked"
    );
}

/// Persistent replicas are deliberately *not* reclaimed. Persistent instances
/// are not supposed to travel over distro at all, so this is defensive: if one
/// ever does, it must not be silently deleted by a peer's empty sync.
#[test]
fn persistent_replicas_are_not_reclaimed_by_sync() {
    let svc = service();
    sync(&svc, vec![persistent_instance("10.0.0.1", 8080)], N1);

    sync(&svc, vec![], N1);

    assert_eq!(
        ips(&svc),
        vec!["10.0.0.1"],
        "the GC filter is `ephemeral && replica`"
    );
}

// ---------------------------------------------------------------------------
// 2) Distro version accounting
// ---------------------------------------------------------------------------

#[test]
fn an_unknown_service_reports_version_zero() {
    let svc = service();
    assert_eq!(version(&svc, SVC), 0);
}

#[test]
fn local_registration_advances_the_distro_version() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    assert_eq!(version(&svc, SVC), 1);

    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.2", 8080));
    assert_eq!(version(&svc, SVC), 2);
}

/// The ping-pong guard. If replaying a peer's data moved our own counter, the
/// peer would report a *higher* version back and the two nodes would sync each
/// other forever.
#[test]
fn remote_sync_does_not_advance_the_distro_version() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    let before = version(&svc, SVC);

    sync(&svc, vec![instance("10.0.0.2", 8080)], N1);
    sync(&svc, vec![], N1);

    assert_eq!(version(&svc, SVC), before);
}

#[test]
fn deregistration_advances_the_distro_version() {
    let svc = service();
    let inst = instance("10.0.0.1", 8080);
    svc.register_instance(NS, GROUP, SVC, inst.clone());
    svc.deregister_instance(NS, GROUP, SVC, &inst);

    assert_eq!(version(&svc, SVC), 2);
}

#[test]
fn health_flip_advances_the_distro_version() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    assert!(svc.update_instance_health(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT", false));
    assert_eq!(version(&svc, SVC), 2);

    // A no-op flip is not a local mutation, so the version must stay put.
    assert!(svc.update_instance_health(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT", false));
    assert_eq!(version(&svc, SVC), 2);
}

#[test]
fn distro_version_is_per_service() {
    let svc = service();
    svc.register_instance(NS, GROUP, "a", instance("10.0.0.1", 8080));
    svc.register_instance(NS, GROUP, "a", instance("10.0.0.2", 8080));
    svc.register_instance(NS, GROUP, "b", instance("10.0.0.3", 8080));

    assert_eq!(version(&svc, "a"), 2);
    assert_eq!(version(&svc, "b"), 1);
}

// ---------------------------------------------------------------------------
// 3) Dirty-data repair on the sync path
// ---------------------------------------------------------------------------

#[test]
fn sync_normalises_a_blank_cluster_to_default() {
    let svc = service();
    let mut inst = instance("10.0.0.1", 8080);
    inst.cluster_name = String::new();
    sync(&svc, vec![inst], N1);

    let all = snapshot(&svc);
    assert_eq!(find(&all, "10.0.0.1").cluster_name, "DEFAULT");
}

#[test]
fn sync_clamps_a_negative_weight() {
    let svc = service();
    let mut inst = instance("10.0.0.1", 8080);
    inst.weight = -3.0;
    sync(&svc, vec![inst], N1);

    assert_eq!(find(&snapshot(&svc), "10.0.0.1").weight, 1.0);
}

#[test]
fn sync_rewrites_the_service_name() {
    let svc = service();
    let mut inst = instance("10.0.0.1", 8080);
    inst.service_name = "stale".to_string();
    sync(&svc, vec![inst], N1);

    assert_eq!(find(&snapshot(&svc), "10.0.0.1").service_name, SVC);
}

/// A peer broadcasts an empty instance list for services it no longer owns. If
/// that materialised a service entry it would resurrect every service the
/// empty-service reaper just reclaimed.
#[test]
fn empty_sync_does_not_resurrect_a_reaped_service() {
    let svc = service();
    let inst = instance("10.0.0.1", 8080);
    svc.register_instance(NS, GROUP, SVC, inst.clone());
    svc.deregister_instance(NS, GROUP, SVC, &inst);
    assert!(svc.clean_empty_services(0) > 0);

    sync(&svc, vec![], N1);

    let (_total, page) = svc.list_services(NS, GROUP, 1, 10);
    assert!(!page.iter().any(|n| n == SVC));
}

#[test]
fn repeated_sync_is_idempotent() {
    let svc = service();
    let payload = || vec![instance("10.0.0.1", 8080), instance("10.0.0.2", 8080)];

    sync(&svc, payload(), N1);
    sync(&svc, payload(), N1);
    sync(&svc, payload(), N1);

    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 2);
}

// ---------------------------------------------------------------------------
// 4) AP / CP routing gates
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ap_path_refuses_a_persistent_instance() {
    let svc = Arc::new(service());
    assert!(!common::register_ap(&svc, SVC, persistent_instance("10.0.0.1", 8080)).await);
    assert_eq!(query(&svc, SVC, "", false).len(), 0);
}

#[tokio::test]
async fn cp_path_refuses_an_ephemeral_instance() {
    let svc = Arc::new(service());
    assert!(!common::register_cp(&svc, SVC, instance("10.0.0.1", 8080)).await);
    assert_eq!(query(&svc, SVC, "", false).len(), 0);
}

/// Regression: `resolve_service_ephemeral` fixes the flag on first sight, so a
/// *rejected* write used to brand the service persistent (the AP gate passed
/// the instance's own `ephemeral=false` in) — after which every ordinary
/// ephemeral registration for that service was refused too.
#[tokio::test]
async fn a_rejected_ap_write_leaves_the_service_unresolved() {
    let svc = Arc::new(service());
    assert!(!common::register_ap(&svc, SVC, persistent_instance("10.0.0.1", 8080)).await);

    assert_eq!(
        svc.service_ephemeral(NS, GROUP, SVC),
        None,
        "a refused write must not fix the service flag"
    );
    assert_eq!(version(&svc, SVC), 0, "and must not touch the data at all");

    assert!(
        common::register_ap(&svc, SVC, instance("10.0.0.2", 8080)).await,
        "the next conforming registration still works"
    );
    assert_eq!(svc.service_ephemeral(NS, GROUP, SVC), Some(true));
}

#[tokio::test]
async fn a_rejected_cp_write_leaves_the_service_unresolved() {
    let svc = Arc::new(service());
    assert!(!common::register_cp(&svc, SVC, instance("10.0.0.1", 8080)).await);

    assert_eq!(svc.service_ephemeral(NS, GROUP, SVC), None);
    assert_eq!(version(&svc, SVC), 0);

    assert!(common::register_cp(&svc, SVC, persistent_instance("10.0.0.2", 8080)).await);
    assert_eq!(svc.service_ephemeral(NS, GROUP, SVC), Some(false));
}

/// Upstream builds the `Service` handed to `getSingleton` from the *route*
/// (`InstanceRequestHandler` uses `Service.newService(..., true)`), so the flag
/// a route fixes must be the route's own, not the caller's instance flag.
#[tokio::test]
async fn the_route_fixes_the_service_flag_not_the_instance() {
    let ap = Arc::new(service());
    assert!(common::register_ap(&ap, SVC, instance("10.0.0.1", 8080)).await);
    assert_eq!(ap.service_ephemeral(NS, GROUP, SVC), Some(true));

    let cp = Arc::new(service());
    assert!(common::register_cp(&cp, SVC, persistent_instance("10.0.0.1", 8080)).await);
    assert_eq!(cp.service_ephemeral(NS, GROUP, SVC), Some(false));
}

/// Neither upstream implementation checks the flag on deregister, so an
/// instance is removable through whichever path asks.
#[tokio::test]
async fn deregistration_ignores_the_ap_cp_split() {
    let svc = Arc::new(service());
    let inst = persistent_instance("10.0.0.1", 8080);
    svc.register_instance(NS, GROUP, SVC, inst.clone());

    use batata_naming::service::EphemeralClientOperationService;
    assert!(
        EphemeralClientOperationService::new(Arc::clone(&svc), None)
            .deregister_instance(NS, GROUP, SVC, &inst)
            .await
    );
    assert!(query(&svc, SVC, "", false).is_empty());
}
