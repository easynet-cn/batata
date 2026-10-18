//! End-to-end snapshot transfer across a real three-node Raft cluster.
//!
//! Regression cover for the snapshot path being broken end to end: the leader
//! builds and streams a snapshot, the follower's gRPC endpoint reassembles it
//! and hands it to openraft, and the membership rides along so the follower
//! does not lose its cluster config.

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use openraft::network::{RPCOption, RaftNetwork};
use openraft::{BasicNode, Snapshot, Vote};
use tempfile::TempDir;
use tokio::sync::RwLock;
use tonic::transport::Server;

use batata_consistency::bincode;
use batata_consistency::raft::network::RaftNetworkConnection;
use batata_consistency::raft::state_machine::{CF_NAMESPACE, RocksStateMachine};
use batata_consistency::raft::{
    RaftConfig, RaftGrpcService, RaftNode, RaftRequest, ServerState, SnapshotFile,
};
use batata_server_api::raft::raft_service_server::RaftServiceServer;

/// Reserve a free localhost port. The listener is dropped immediately; the
/// window before the server binds is tiny and this avoids pulling in an extra
/// dev-dependency just to convert a listener into a stream.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

struct TestNode {
    id: u64,
    addr: String,
    node: Arc<RaftNode>,
    db: Arc<rocksdb::DB>,
    snapshot_dir: std::path::PathBuf,
    _dir: TempDir,
}

async fn start_node(root: &std::path::Path, id: u64) -> TestNode {
    let dir = tempfile::tempdir_in(root).expect("temp dir");
    let addr = format!("127.0.0.1:{}", free_port());
    let data_dir = dir.path().join(format!("raft-{}", id));

    let config = RaftConfig {
        election_timeout_ms: 800,
        heartbeat_interval_ms: 200,
        // Snapshot after a handful of entries so the test stays fast.
        snapshot_threshold: 20,
        data_dir: data_dir.clone(),
        ..Default::default()
    };
    config.ensure_dirs().await.expect("ensure dirs");

    let (node, db) = RaftNode::new_with_db(id, addr.clone(), config.clone())
        .await
        .expect("create raft node");
    let node = Arc::new(node);

    let slot: Arc<RwLock<Option<Arc<RaftNode>>>> = Arc::new(RwLock::new(Some(node.clone())));
    let service = RaftGrpcService::new(slot);
    let bind_addr = addr.clone();
    tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(RaftServiceServer::new(service))
            .serve(bind_addr.parse().expect("valid addr"))
            .await;
    });

    TestNode {
        id,
        addr,
        node,
        db,
        snapshot_dir: config.snapshot_dir(),
        _dir: dir,
    }
}

async fn wait_for_leader(nodes: &[TestNode]) -> usize {
    for _ in 0..150 {
        for (i, n) in nodes.iter().enumerate() {
            if n.node.metrics().state == ServerState::Leader {
                return i;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no leader elected within 15s");
}

fn has_namespace(db: &Arc<rocksdb::DB>, id: &str) -> bool {
    let cf = db.cf_handle(CF_NAMESPACE).expect("CF_NAMESPACE");
    db.get_cf(&cf, RocksStateMachine::namespace_key(id))
        .ok()
        .flatten()
        .is_some()
}

async fn wait_for_namespace(nodes: &[TestNode], id: &str) {
    for _ in 0..150 {
        if nodes.iter().all(|n| has_namespace(&n.db, id)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("namespace {} did not replicate to every node", id);
}

async fn wait_for_snapshot_file(snapshot_dir: &std::path::Path) -> std::path::PathBuf {
    for _ in 0..150 {
        if let Ok(mut entries) = tokio::fs::read_dir(snapshot_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("snapshot_") {
                    return entry.path();
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("leader never persisted a snapshot to {:?}", snapshot_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn follower_installs_streamed_snapshot_and_keeps_membership() {
    let root = tempfile::tempdir().expect("root temp dir");

    let node1 = start_node(root.path(), 1).await;
    let node2 = start_node(root.path(), 2).await;
    let node3 = start_node(root.path(), 3).await;
    let nodes = vec![node1, node2, node3];

    let mut members = BTreeMap::new();
    for n in &nodes {
        members.insert(
            n.id,
            BasicNode {
                addr: n.addr.clone(),
            },
        );
    }
    nodes[0]
        .node
        .initialize(members)
        .await
        .expect("initialize cluster");

    let leader_idx = wait_for_leader(&nodes).await;
    let leader = &nodes[leader_idx];

    // Replicate one namespace so the snapshot has something to carry.
    leader
        .node
        .write(RaftRequest::NamespaceCreate {
            namespace_id: "ns-snapshot".to_string(),
            namespace_name: "snapshot".to_string(),
            namespace_desc: None,
            now_ms: chrono::Utc::now().timestamp_millis(),
        })
        .await
        .expect("replicate namespace");
    wait_for_namespace(&nodes, "ns-snapshot").await;

    // Force a snapshot on the leader so one exists on disk.
    leader
        .node
        .raft()
        .trigger()
        .snapshot()
        .await
        .expect("trigger snapshot");
    let snapshot_path = wait_for_snapshot_file(&leader.snapshot_dir).await;

    let meta_bytes = tokio::fs::read(snapshot_path.join("__raft_snapshot_meta"))
        .await
        .expect("read snapshot meta");
    let meta: openraft::SnapshotMeta<u64, BasicNode> =
        bincode::deserialize(&meta_bytes).expect("decode snapshot meta");
    // The body stays on disk: the sender streams it from the file, so nothing
    // has to be read into memory to start the transfer.
    let data_path = snapshot_path.join("data");
    let data_len = tokio::fs::metadata(&data_path)
        .await
        .expect("stat snapshot data")
        .len();
    assert!(data_len > 0, "snapshot body must not be empty");

    // A fresh, uninitialized node stands in for a follower that has fallen so
    // far behind that only a snapshot can bring it back. Its state machine is
    // empty, so openraft cannot dismiss the snapshot as already-applied.
    let fresh = start_node(root.path(), 4).await;
    assert!(!has_namespace(&fresh.db, "ns-snapshot"));

    // Stream the snapshot over real gRPC, exactly the way the leader does.
    let vote: Vote<u64> = leader.node.metrics().vote;
    let snapshot = Snapshot {
        meta: meta.clone(),
        snapshot: Box::new(
            SnapshotFile::open(&data_path)
                .await
                .expect("open snapshot body"),
        ),
    };

    let mut connection = RaftNetworkConnection::new(fresh.addr.clone());
    connection
        .full_snapshot(
            vote,
            snapshot,
            std::future::pending(),
            RPCOption::new(Duration::from_secs(30)),
        )
        .await
        .expect("snapshot transfer over gRPC");

    // The receiving node must now hold the data...
    let mut recovered = false;
    for _ in 0..150 {
        if has_namespace(&fresh.db, "ns-snapshot") {
            recovered = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        recovered,
        "fresh node did not restore data from the snapshot"
    );

    // ...and must have picked up the membership that rides along with the
    // snapshot body. Before the fix the sender dropped `last_membership`, so an
    // install silently wiped the receiver's cluster config.
    let mut voters: Vec<u64> = Vec::new();
    for _ in 0..150 {
        voters = fresh
            .node
            .metrics()
            .membership_config
            .membership()
            .voter_ids()
            .collect();
        if voters == vec![1, 2, 3] {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(voters, vec![1, 2, 3], "membership must survive install");
}
