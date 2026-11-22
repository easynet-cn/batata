//! Every ordered (write node, read node) pair on a real three-node Raft cluster.
//!
//! A config published through any node must be readable from any node. The two
//! halves of that sentence are independent: a follower accepts a write by
//! forwarding it to the leader over gRPC (`RaftManagementService`) and then
//! waiting for its own apply index to catch up, while the leader writes
//! locally. A test that always writes and reads on the same node (or only on
//! the leader) cannot see a broken forwarding path or a replica that never
//! applies what it acknowledged.
//!
//! This mirrors rnacos' `config_cluster_tests.rs`: enumerate the full
//! Cartesian product of writers and readers, not a hand-picked sample. The
//! cluster here is real openraft (three `RaftNode`s, real gRPC, real RocksDB
//! per node), so both code paths are exercised on every run. The test also
//! asserts that both paths actually occurred; with three writers and one
//! leader, a run that only ever took the local path would be a silent gap.

use std::collections::{BTreeMap, BTreeSet};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use batata_consistency::bincode;
use batata_consistency::raft::request::ConfigPublishPayload;
use batata_consistency::raft::state_machine::{CF_CONFIG, RocksStateMachine, StoredConfig};
use batata_consistency::raft::{
    RaftConfig, RaftGrpcService, RaftManagementGrpcService, RaftNode, RaftRequest, ServerState,
};
use batata_server_api::raft::raft_management_service_server::RaftManagementServiceServer;
use batata_server_api::raft::raft_service_server::RaftServiceServer;
use openraft::BasicNode;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tonic::transport::Server;

const NODE_COUNT: usize = 3;
const GROUP: &str = "DEFAULT_GROUP";
const TENANT: &str = "public";

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// The complete ordered product, not a sample. A missing pair is a coverage
/// regression even when the remaining pairs still pass.
fn all_pairs() -> Vec<(usize, usize)> {
    (0..NODE_COUNT)
        .flat_map(|writer| (0..NODE_COUNT).map(move |reader| (writer, reader)))
        .collect()
}

#[test]
fn ordered_pairs_cover_every_writer_and_reader() {
    let pairs = all_pairs();
    assert_eq!(pairs.len(), NODE_COUNT * NODE_COUNT);

    let unique: BTreeSet<(usize, usize)> = pairs.iter().copied().collect();
    assert_eq!(unique.len(), NODE_COUNT * NODE_COUNT);

    for node in 0..NODE_COUNT {
        assert_eq!(pairs.iter().filter(|(w, _)| *w == node).count(), NODE_COUNT);
        assert_eq!(pairs.iter().filter(|(_, r)| *r == node).count(), NODE_COUNT);
    }
    // Both the identity pairs (same writer/reader) and the cross pairs must be
    // present: dropping either half hides a different class of bug.
    assert_eq!(pairs.iter().filter(|(w, r)| w == r).count(), NODE_COUNT);
    assert_eq!(
        pairs.iter().filter(|(w, r)| w != r).count(),
        NODE_COUNT * (NODE_COUNT - 1)
    );
}

/// Coverage guard: deleting the Cartesian product, the management service or
/// the both-paths assertion would leave a green test that no longer covers
/// what this file exists to cover. Keep the source-shape check next to the
/// behavioural tests so a future "simplification" turns red here.
#[test]
fn ordered_pair_coverage_guards_are_intact() {
    let source = include_str!("config_cluster_ordered_pairs.rs");
    // Tokens are split with concat! so the guard list itself cannot satisfy
    // `contains`: the guard must find them in the real test code.
    for token in [
        concat!("NODE_COUNT * NODE", "_COUNT"),
        concat!("RaftManagement", "ServiceServer"),
        concat!("wait_for_config(&nodes[*", "reader]"),
        concat!("BTreeSet::from([\"leader-local\", ", "\"forwarded\"])"),
        concat!("published_config(&nodes[*", "writer]"),
        // Split on the comma so rustfmt wrapping the `assert_eq!` arguments
        // cannot silently starve this guard.
        concat!("stored", ".content, content"),
    ] {
        assert!(source.contains(token), "coverage guard missing: {token}");
    }
}

struct TestNode {
    id: u64,
    addr: String,
    node: Arc<RaftNode>,
    db: Arc<rocksdb::DB>,
    _dir: TempDir,
}

async fn start_node(root: &std::path::Path, id: u64) -> TestNode {
    let dir = tempfile::tempdir_in(root).expect("temp dir");
    let addr = format!("127.0.0.1:{}", free_port());
    let config = RaftConfig {
        election_timeout_ms: 800,
        heartbeat_interval_ms: 200,
        snapshot_threshold: 20,
        data_dir: dir.path().join(format!("raft-{}", id)),
        ..Default::default()
    };
    config.ensure_dirs().await.expect("ensure dirs");

    let (node, db) = RaftNode::new_with_db(id, addr.clone(), config)
        .await
        .expect("create raft node");
    let node = Arc::new(node);

    // The management service is not optional: a follower forwards its write
    // to the leader through RaftManagementService. Serving only RaftService
    // would make every non-leader writer fail with "connection refused" and
    // the test would silently stop covering the forwarded path.
    let slot: Arc<RwLock<Option<Arc<RaftNode>>>> = Arc::new(RwLock::new(Some(node.clone())));
    let bind_addr = addr.clone();
    tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(RaftServiceServer::new(RaftGrpcService::new(slot.clone())))
            .add_service(RaftManagementServiceServer::new(
                RaftManagementGrpcService::new(slot),
            ))
            .serve(bind_addr.parse().expect("valid addr"))
            .await;
    });

    TestNode {
        id,
        addr,
        node,
        db,
        _dir: dir,
    }
}

async fn wait_for_leader(nodes: &[TestNode]) -> u64 {
    for _ in 0..150 {
        for node in nodes {
            if node.node.metrics().state == ServerState::Leader {
                return node.id;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no leader elected within 15s");
}

fn published_config(node: &TestNode, data_id: &str) -> Option<StoredConfig> {
    let cf = node.db.cf_handle(CF_CONFIG).expect("CF_CONFIG");
    let key = RocksStateMachine::config_key(data_id, GROUP, TENANT);
    node.db
        .get_cf(&cf, key.as_bytes())
        .ok()
        .flatten()
        .and_then(|bytes| bincode::deserialize::<StoredConfig>(&bytes).ok())
}

async fn wait_for_config(node: &TestNode, data_id: &str, expected: &str) {
    let mut seen = false;
    for _ in 0..150 {
        if published_config(node, data_id).is_some_and(|stored| stored.content == expected) {
            seen = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        seen,
        "node {} never applied config {} with content {}",
        node.id, data_id, expected
    );
    let stored = published_config(node, data_id).expect("config must still be present");
    assert_eq!(stored.data_id, data_id);
    assert_eq!(stored.group, GROUP);
    assert_eq!(stored.tenant, TENANT);
    assert_eq!(stored.md5, format!("md5-{}", data_id));
}

fn publish_request(data_id: &str, content: &str) -> RaftRequest {
    RaftRequest::ConfigPublish(Box::new(ConfigPublishPayload {
        data_id: data_id.to_string(),
        group: GROUP.to_string(),
        tenant: TENANT.to_string(),
        content: content.to_string(),
        md5: format!("md5-{}", data_id),
        config_type: Some("text".to_string()),
        app_name: None,
        tag: None,
        desc: None,
        src_user: Some("ordered-pairs-test".to_string()),
        src_ip: None,
        r#use: None,
        effect: None,
        schema: None,
        encrypted_data_key: None,
        cas_md5: None,
        history: None,
        // Leader wall time at propose time. apply() must never read the local
        // clock: all replicas have to persist byte-identical state.
        now_ms: chrono::Utc::now().timestamp_millis(),
    }))
}

/// A single initializer makes the cold-start leader deterministic.
///
/// openraft's `initialize()` writes the initial membership *and immediately
/// campaigns*. When every node initializes the same membership, all of them
/// campaign at boot and the winner depends on scheduling — that is the source
/// of the Config Write double peak. Initializing exactly one node (the lowest
/// id) leaves the others to learn membership from the leader's AppendEntries,
/// so the designated node wins every time. This test runs three independent
/// cold starts: a random winner would fail with high probability.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_initializer_elects_the_designated_leader_every_cold_start() {
    for attempt in 1..=3 {
        let root = tempfile::tempdir().expect("root temp dir");
        let mut nodes = Vec::new();
        for id in 1..=(NODE_COUNT as u64) {
            nodes.push(start_node(root.path(), id).await);
        }
        let nodes: Vec<TestNode> = nodes;

        let mut members = BTreeMap::new();
        for node in &nodes {
            members.insert(
                node.id,
                BasicNode {
                    addr: node.addr.clone(),
                },
            );
        }
        // Only the designated bootstrap leader initializes. Do NOT call
        // initialize() on the other two: that is the race being eliminated.
        let designated = *members.keys().min().expect("members must not be empty");
        let initializer = nodes
            .iter()
            .find(|n| n.id == designated)
            .expect("designated node must exist");
        initializer
            .node
            .initialize(members)
            .await
            .expect("initialize cluster");

        let leader = wait_for_leader(&nodes).await;
        assert_eq!(
            leader, designated,
            "cold start {attempt}: leader must be the single initializer"
        );

        // A deterministic leader is only useful if the cluster can still
        // commit: prove the followers joined the membership and applied.
        let response = initializer
            .node
            .write(publish_request(
                &format!("bootstrap-{}", attempt),
                &format!("boot-{}", attempt),
            ))
            .await
            .expect("leader write");
        assert!(response.success);
        for node in &nodes {
            wait_for_config(
                node,
                &format!("bootstrap-{}", attempt),
                &format!("boot-{}", attempt),
            )
            .await;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn config_written_on_any_node_is_readable_from_every_node() {
    let root = tempfile::tempdir().expect("root temp dir");
    let mut nodes = Vec::new();
    for id in 1..=(NODE_COUNT as u64) {
        nodes.push(start_node(root.path(), id).await);
    }
    let nodes: Vec<TestNode> = nodes;

    let mut members = BTreeMap::new();
    for node in &nodes {
        members.insert(
            node.id,
            BasicNode {
                addr: node.addr.clone(),
            },
        );
    }
    nodes[0]
        .node
        .initialize(members)
        .await
        .expect("initialize cluster");
    wait_for_leader(&nodes).await;

    let pairs = all_pairs();
    let mut observed_paths: BTreeSet<&'static str> = BTreeSet::new();

    for (writer, reader) in &pairs {
        let data_id = format!("ordered-pair-w{}-r{}", writer + 1, reader + 1);
        let content = format!("value-w{}-r{}", writer + 1, reader + 1);

        // Record which path this particular write took. With three writers
        // and one leader, both paths are guaranteed to appear.
        let leader_id = nodes
            .iter()
            .find(|n| n.node.metrics().state == ServerState::Leader)
            .map(|n| n.id)
            .expect("cluster must still have a leader");
        let path = if nodes[*writer].id == leader_id {
            "leader-local"
        } else {
            "forwarded"
        };
        observed_paths.insert(path);

        let write_result = nodes[*writer]
            .node
            .write(publish_request(&data_id, &content))
            .await;
        assert!(
            write_result.is_ok(),
            "write via node {} failed: {:?}",
            nodes[*writer].id,
            write_result.err()
        );
        let response = write_result.expect("checked above");
        assert!(
            response.success,
            "write w{}-r{} was rejected: {:?}",
            writer + 1,
            reader + 1,
            response.message
        );

        // `write_with_index` promises that a follower waits for its own apply
        // index before returning, so a subsequent local read sees the write.
        // Assert it *without* polling: a forwarding path that skips the wait
        // would still converge eventually and this is the only guard that
        // catches it.
        let local = published_config(&nodes[*writer], &data_id);
        assert!(
            local.is_some(),
            "writer node {} did not apply {} before returning",
            nodes[*writer].id,
            data_id
        );
        let local = local.expect("checked above");
        assert_eq!(local.content, content);
        assert_eq!(local.md5, format!("md5-{}", data_id));

        // The specific reader in the pair must see it. A write acknowledged
        // by a follower is worthless if that follower (or the designated
        // reader) never applies the entry.
        wait_for_config(&nodes[*reader], &data_id, &content).await;
    }

    assert_eq!(
        observed_paths,
        BTreeSet::from(["leader-local", "forwarded"]),
        "the run must exercise both the local and the forwarded write path"
    );

    // Convergence check independent of the pair loop: every replica must hold
    // every published config, not just the one it was asked to read from.
    for node in &nodes {
        for (writer, reader) in &pairs {
            let data_id = format!("ordered-pair-w{}-r{}", writer + 1, reader + 1);
            let content = format!("value-w{}-r{}", writer + 1, reader + 1);
            let stored = published_config(node, &data_id);
            assert!(stored.is_some(), "node {} is missing {}", node.id, data_id);
            let stored = stored.expect("checked above");
            assert_eq!(
                stored.content, content,
                "node {} diverged on {}",
                node.id, data_id
            );
        }
    }
}
