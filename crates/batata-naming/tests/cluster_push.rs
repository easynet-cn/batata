//! Multi-node subscription push (distro-driven notifications).
//!
//! In a real cluster an SDK subscribes on **one** node, but the instance
//! change it cares about can be registered on **any** node. The change
//! reaches the subscriber's node as a distro sync, and that node has to
//! translate the sync into a push — otherwise the SDK keeps serving a cached,
//! now-stale instance list. `NamingInstanceDistroHandler::notify_subscribers`
//! is that translation, and this file is its coverage.
//!
//! Upstream anchor: Nacos publishes a `ClientChangedEvent` on distro sync for
//! exactly this reason (`DistroClientDataProcessor` → `NotifyCenter`), so "a
//! sync must notify" is not a batata invention.
//!
//! Scope limits inherited from `cluster/mod.rs`: no Raft, no true partitions,
//! ephemeral instances only. What is *not* covered here is the **local**
//! registration push path (an SDK registered on the same node it subscribes
//! on) — that is driven by the gRPC handler, not by distro, and needs a real
//! connection manager.

mod cluster;

use std::sync::Arc;
use std::sync::Mutex;

use batata_api::grpc::Payload;
use batata_naming::Instance;
use batata_naming::service::SubscriberPusher;

use cluster::ClusterHarness;

const NS: &str = "public";
const GROUP: &str = "DEFAULT_GROUP";
const SVC: &str = "svc";
const OTHER: &str = "other-svc";

fn inst(ip: &str, port: i32) -> Instance {
    let mut i = Instance::new(ip.to_string(), port);
    i.cluster_name = "DEFAULT".to_string();
    i
}

/// One recorded push: who it went to, and what it said.
#[derive(Clone, Debug)]
struct Push {
    targets: Vec<String>,
    body: String,
}

/// Push sink that records everything a node would have sent.
///
/// Implements the same `SubscriberPusher` trait the production
/// `ConnectionManagerPusher` does, so the code under test is the real one.
#[derive(Default)]
struct RecordingPusher {
    events: Mutex<Vec<Push>>,
}

impl RecordingPusher {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn pushes(&self) -> Vec<Push> {
        self.events.lock().unwrap().clone()
    }

    fn count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Every connection id ever targeted, in order.
    fn targets(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .flat_map(|p| p.targets.iter().cloned())
            .collect()
    }

    fn last_body(&self) -> String {
        self.events
            .lock()
            .unwrap()
            .last()
            .map(|p| p.body.clone())
            .unwrap_or_default()
    }

    fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

#[async_trait::async_trait]
impl SubscriberPusher for RecordingPusher {
    async fn push_to_many(&self, connection_ids: &[String], payload: Payload) -> usize {
        let body = payload
            .body
            .as_ref()
            .map(|a| String::from_utf8_lossy(&a.value).to_string())
            .unwrap_or_default();
        self.events.lock().unwrap().push(Push {
            targets: connection_ids.to_vec(),
            body,
        });
        connection_ids.len()
    }
}

/// A register on node 0 must notify subscribers attached to nodes 1 and 2.
///
/// This is the whole point of the file: without distro-driven notification a
/// subscriber learns nothing about changes made on another node.
#[tokio::test]
async fn remote_registration_notifies_subscribers_on_other_nodes() {
    let p0 = RecordingPusher::new();
    let p1 = RecordingPusher::new();
    let p2 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![p0.clone(), p1.clone(), p2.clone()]);

    c.subscribe(1, "conn-on-1", NS, GROUP, SVC);
    c.subscribe(2, "conn-on-2", NS, GROUP, SVC);

    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.sync_round_flushed().await;

    assert!(
        p1.targets().contains(&"conn-on-1".to_string()),
        "node 1 should have pushed to its own subscriber; got {:?}",
        p1.targets()
    );
    assert!(
        p2.targets().contains(&"conn-on-2".to_string()),
        "node 2 should have pushed to its own subscriber; got {:?}",
        p2.targets()
    );
}

/// The notification must carry the *current* instance list, not just a
/// "something changed" ping — SDKs replace their cache from it.
#[tokio::test]
async fn notification_carries_the_full_current_instance_list() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "conn-on-1", NS, GROUP, SVC);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.sync_round_flushed().await;
    assert!(
        p1.last_body().contains("10.0.0.1"),
        "first push missing instance"
    );

    // A second instance registered on a *third* node must show up in the next
    // push together with the first one — the payload is a full view.
    c.register(2, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    let ok = c
        .converge_flushed(5, |_| {
            let b = p1.last_body();
            b.contains("10.0.0.1") && b.contains("10.0.0.2")
        })
        .await;
    assert!(
        ok,
        "push did not converge to the full instance list: {}",
        p1.last_body()
    );
}

/// Several subscribers on the same node must all be notified.
#[tokio::test]
async fn every_subscriber_on_a_node_is_notified() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "sub-a", NS, GROUP, SVC);
    c.subscribe(1, "sub-b", NS, GROUP, SVC);
    c.subscribe(1, "sub-c", NS, GROUP, SVC);

    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.sync_round_flushed().await;

    let push = p1
        .pushes()
        .into_iter()
        .find(|p| p.targets.len() == 3)
        .expect("expected one push addressing all three subscribers");
    for id in ["sub-a", "sub-b", "sub-c"] {
        assert!(
            push.targets.iter().any(|t| t == id),
            "{id} was not notified"
        );
    }
}

/// Subscribers of a *different* service must not be swept into the push.
#[tokio::test]
async fn subscribers_of_other_services_are_not_notified() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "watched-other", NS, GROUP, OTHER);
    c.subscribe(1, "watched-svc", NS, GROUP, SVC);

    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.sync_round_flushed().await;

    assert!(
        p1.targets().contains(&"watched-svc".to_string()),
        "subscriber of the changed service must be notified"
    );
    assert!(
        !p1.targets().contains(&"watched-other".to_string()),
        "subscriber of an unrelated service must NOT be notified: {:?}",
        p1.targets()
    );
}

/// Deregistration propagates as a push carrying what is left.
#[tokio::test]
async fn deregistration_notifies_with_the_remaining_view() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "conn-on-1", NS, GROUP, SVC);
    let a = inst("10.0.0.1", 8080);
    let b = inst("10.0.0.2", 8080);
    c.register(0, NS, GROUP, SVC, a.clone());
    c.register(0, NS, GROUP, SVC, b.clone());
    let ok = c
        .converge_flushed(5, |c| c.endpoints(1, NS, GROUP, SVC).len() == 2)
        .await;
    assert!(ok, "two instances did not reach node 1");

    c.deregister(0, NS, GROUP, SVC, &a);
    p1.clear();
    let ok = c
        .converge_flushed(5, |_| {
            let b = p1.last_body();
            !b.is_empty() && !b.contains("10.0.0.1") && b.contains("10.0.0.2")
        })
        .await;
    assert!(
        ok,
        "deregistration push should show only the survivor: {}",
        p1.last_body()
    );
}

/// A node that missed the change while offline must notify on catch-up.
///
/// Not "it eventually has the data" but "its subscribers are told" — an SDK
/// that reconnects after a blip would otherwise never refresh.
#[tokio::test]
async fn offline_node_catches_up_and_notifies() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "conn-on-1", NS, GROUP, SVC);
    c.stop(1);

    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.settle(3).await;
    c.flush_pushes().await;
    assert_eq!(
        p1.count(),
        0,
        "an offline node cannot push — it received no sync"
    );

    c.start(1);
    let ok = c
        .converge_flushed(5, |_| p1.last_body().contains("10.0.0.1"))
        .await;
    assert!(ok, "returning node never notified its subscriber");
}

/// A converged cluster must stop pushing.
///
/// Every extra push is a wasted wake-up for every SDK watching the service,
/// so "sync happened" must not mean "push happened" once nothing changed.
#[tokio::test]
async fn converged_cluster_stops_pushing() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "conn-on-1", NS, GROUP, SVC);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));

    // Reconcile until every node is caught up, then let it settle further.
    let ok = c
        .converge_flushed(5, |c| {
            (0..c.len()).all(|i| c.endpoints(i, NS, GROUP, SVC) == vec!["10.0.0.1:8080"])
        })
        .await;
    assert!(ok, "cluster did not converge");

    let before = p1.count();
    assert!(before > 0, "expected at least one push while converging");

    // Only the *verify* cycle runs on a quiet cluster, and it must find
    // nothing stale: no repair, no sync, no push.
    //
    // Deliberately not `sync_round` here: that re-publishes every key
    // unconditionally — it is this harness broadcasting, not a real cluster
    // churning — so it would push every round by construction.
    for round in 0..4 {
        let pushed = c.verify_round().await;
        assert_eq!(
            pushed, 0,
            "round {round}: converged cluster still had keys to repair"
        );
    }
    c.flush_pushes().await;
    assert_eq!(
        p1.count(),
        before,
        "converged cluster kept pushing — that is a notification storm"
    );
}

/// A restarted node has no subscribers left, so it must not push.
///
/// Faithful, not a gap: subscriptions live in memory and the SDK re-subscribes
/// when it reconnects. The assertion pins the consequence — no phantom pushes
/// to connections that no longer exist.
#[tokio::test]
async fn restarted_node_has_no_subscribers_and_stops_pushing() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "conn-on-1", NS, GROUP, SVC);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    c.sync_round_flushed().await;
    assert!(p1.count() > 0);

    c.restart(1);
    p1.clear();

    // Re-register something so there is data to sync into the restarted node.
    c.register(2, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle_reconciling(3).await;
    c.flush_pushes().await;

    assert_eq!(
        p1.count(),
        0,
        "a restarted node has no subscribers until the SDK re-subscribes"
    );
    // ...but it still holds the data, so a fresh subscription can be served.
    assert!(
        c.endpoints(1, NS, GROUP, SVC)
            .contains(&"10.0.0.2:8080".to_string()),
        "restarted node should still receive the data"
    );
}

/// The verify repair path notifies too, not just the event-driven sync.
///
/// A node that came back empty has no subscribers at first, but a node that
/// merely *missed* one key is repaired by verify and must still push.
#[tokio::test]
async fn verify_driven_repair_also_notifies() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "conn-on-1", NS, GROUP, SVC);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    let ok = c
        .converge_flushed(5, |_| p1.last_body().contains("10.0.0.1"))
        .await;
    assert!(ok, "initial sync did not notify");

    // Node 1 misses the next change entirely (offline), then comes back. Only
    // the verify cycle can repair it — no event-driven sync will fire.
    c.stop(1);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.2", 8080));
    c.settle(2).await;
    c.start(1);
    p1.clear();

    let pushed = c.verify_round().await;
    c.flush_pushes().await;

    assert!(pushed > 0, "verify should have found a stale key to repair");
    assert!(
        p1.last_body().contains("10.0.0.2"),
        "verify-driven repair must notify subscribers: {}",
        p1.last_body()
    );
}

/// Two services must not cross-notify: a change on one must not wake the
/// other's subscribers.
#[tokio::test]
async fn two_services_do_not_cross_notify() {
    let p1 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        RecordingPusher::new(),
        p1.clone(),
        RecordingPusher::new(),
    ]);

    c.subscribe(1, "watch-svc", NS, GROUP, SVC);
    c.subscribe(1, "watch-other", NS, GROUP, OTHER);

    // Converge SVC first so it is quiet, then change only OTHER.
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));
    let ok = c
        .converge_flushed(5, |c| {
            c.endpoints(1, NS, GROUP, SVC) == vec!["10.0.0.1:8080"]
        })
        .await;
    assert!(ok, "SVC did not converge");
    p1.clear();

    c.register(0, NS, GROUP, OTHER, inst("10.0.0.9", 9090));
    let ok = c
        .converge_flushed(5, |_| p1.targets().contains(&"watch-other".to_string()))
        .await;
    assert!(ok, "OTHER change did not notify its subscriber");

    // A peer's empty re-publish (the GC signal) can legitimately wake the SVC
    // subscriber — but it must never hand it another service's instances.
    for push in p1.pushes() {
        if push.targets.iter().any(|t| t == "watch-svc") {
            assert!(
                !push.body.contains("10.0.0.9"),
                "SVC subscriber received another service's instances: {}",
                push.body
            );
        }
    }
}

/// Peers' empty re-publishes must not blind the node that owns the data.
///
/// Every peer re-publishes every key it knows on each round, and for most
/// keys the payload is **empty** — that empty payload is the garbage
/// collection signal (`get_all_keys` is deliberately unfiltered; see the
/// distro handler docs). It must not delete the owner's own locally
/// registered instance, and nothing the owner pushes may claim it is gone.
///
/// This is the notification-side view of the same invariant the data tests
/// assert: an owner's instance survives its peers' GC traffic.
#[tokio::test]
async fn owner_node_keeps_serving_its_own_instances_despite_peer_gc_payloads() {
    let p0 = RecordingPusher::new();
    let mut c = ClusterHarness::three_with_pushers(vec![
        p0.clone(),
        RecordingPusher::new(),
        RecordingPusher::new(),
    ]);

    c.subscribe(0, "conn-on-0", NS, GROUP, SVC);
    c.register(0, NS, GROUP, SVC, inst("10.0.0.1", 8080));

    c.settle(3).await;
    c.flush_pushes().await;

    assert_eq!(
        c.endpoints(0, NS, GROUP, SVC),
        vec!["10.0.0.1:8080"],
        "peer GC payloads must not delete a locally registered instance"
    );
    for push in p0.pushes() {
        assert!(
            push.body.contains("10.0.0.1"),
            "owner pushed a view without its own instance: {}",
            push.body
        );
    }
}
