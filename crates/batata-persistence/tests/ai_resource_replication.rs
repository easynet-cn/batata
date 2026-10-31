//! AI resources replicate across a real two-node Raft cluster.
//!
//! The AI module keeps its resources in column families of the Raft state
//! machine, so a write made anywhere has to land everywhere. It used not to:
//! the distributed backend wrote those column families directly, which left
//! every node holding its own private copy — a `GET` answered by one node could
//! not see what another node had just accepted, and a snapshot install would
//! overwrite a follower's local writes with the leader's. These tests pin the
//! replicated behaviour down against an actual cluster rather than a mock.

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use batata_consistency::RocksDbReader;
use batata_consistency::raft::{
    RaftConfig, RaftGrpcService, RaftManagementGrpcService, RaftNode, ServerState,
};
use batata_persistence::model::{
    AiResourceInfo, AiResourceSearchChunkInfo, AiResourceTaskInfo, Page,
};
use batata_persistence::{AiResourcePersistence, DistributedPersistService};
use batata_server_api::raft::raft_management_service_server::RaftManagementServiceServer;
use batata_server_api::raft::raft_service_server::RaftServiceServer;
use openraft::BasicNode;
use tonic::transport::Server;

const NS: &str = "public";
const R_TYPE: &str = "mcp";

/// Reserve a free localhost port for a node's advertised address.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// One node of the test cluster: its Raft node, its own RocksDB, and the
/// persistence built on top of both.
struct TestNode {
    id: u64,
    addr: String,
    node: Arc<RaftNode>,
    store: Arc<DistributedPersistService>,
    _dir: tempfile::TempDir,
}

impl TestNode {
    async fn start(root: &std::path::Path, id: u64) -> Self {
        let dir = tempfile::tempdir_in(root).expect("temp dir");
        let addr = format!("127.0.0.1:{}", free_port());

        let config = RaftConfig {
            election_timeout_ms: 300,
            heartbeat_interval_ms: 100,
            data_dir: dir.path().join(format!("raft-{id}")),
            ..Default::default()
        };
        config.ensure_dirs().await.expect("ensure dirs");

        let (node, db) = RaftNode::new_with_db(id, addr.clone(), config)
            .await
            .expect("create raft node");
        let node = Arc::new(node);

        // Serve this node's Raft endpoint so the other one can reach it. The
        // management service is not optional for these tests: a follower
        // forwards its write to the leader through it.
        let slot: Arc<tokio::sync::RwLock<Option<Arc<RaftNode>>>> =
            Arc::new(tokio::sync::RwLock::new(Some(node.clone())));
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

        let store = Arc::new(DistributedPersistService::new(
            node.clone(),
            RocksDbReader::new(db),
        ));

        Self {
            id,
            addr,
            node,
            store,
            _dir: dir,
        }
    }
}

/// Wait until some node reports itself as leader and return its index.
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

/// Poll until every node can read the resource, or fail.
///
/// Replication is asynchronous, so a single read right after the write would
/// make the test a race rather than an assertion.
async fn wait_for_resource(nodes: &[TestNode], name: &str) {
    for _ in 0..150 {
        let mut seen = 0;
        for n in nodes {
            if n.store
                .ai_resource_find(NS, name, R_TYPE)
                .await
                .expect("find resource")
                .is_some()
            {
                seen += 1;
            }
        }
        if seen == nodes.len() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("resource {name} did not replicate to every node");
}

fn resource(name: &str) -> AiResourceInfo {
    AiResourceInfo {
        id: 0,
        name: name.to_string(),
        resource_type: R_TYPE.to_string(),
        description: Some("replication probe".to_string()),
        status: Some("PUBLISHED".to_string()),
        namespace_id: NS.to_string(),
        biz_tags: None,
        ext: None,
        from: "user".to_string(),
        version_info: None,
        meta_version: 0,
        scope: "PUBLIC".to_string(),
        owner: "nacos".to_string(),
        download_count: 0,
        gmt_create: None,
        gmt_modified: None,
    }
}

fn chunk(name: &str, suffix: &str) -> AiResourceSearchChunkInfo {
    AiResourceSearchChunkInfo {
        id: 0,
        document_id: 0,
        namespace_id: NS.to_string(),
        resource_type: R_TYPE.to_string(),
        resource_name: name.to_string(),
        resource_version: "1.0.0".to_string(),
        chunk_type: "description".to_string(),
        chunk_text: format!("text-{suffix}"),
        canonical_text: format!("canonical-{suffix}"),
        language: None,
        chunk_hash: format!("hash-{suffix}"),
        metadata: None,
        status: "enabled".to_string(),
        gmt_create: None,
        gmt_modified: None,
    }
}

/// A resource written through one node is readable through the other.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ai_resource_written_on_one_node_is_readable_on_the_other() {
    let root = tempfile::tempdir().expect("root temp dir");
    let node1 = TestNode::start(root.path(), 1).await;
    let node2 = TestNode::start(root.path(), 2).await;
    let nodes = vec![node1, node2];

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

    // Write through the *follower* on purpose: it proves the write is both
    // forwarded to the leader and replicated back, rather than staying on the
    // node that happened to serve the request.
    let writer_idx = (leader_idx + 1) % nodes.len();
    let id = nodes[writer_idx]
        .store
        .ai_resource_insert(&resource("replicated-server"))
        .await
        .expect("insert resource");
    assert!(id > 0, "the insert must assign an id");

    wait_for_resource(&nodes, "replicated-server").await;

    for n in &nodes {
        let found = n
            .store
            .ai_resource_find(NS, "replicated-server", R_TYPE)
            .await
            .expect("find resource")
            .expect("every node must hold the replicated resource");
        assert_eq!(
            found.id, id,
            "the allocated id must be identical on every node"
        );
    }

    // Deletion has to travel too, otherwise a deleted server stays visible on
    // whichever node answers the next list.
    assert_eq!(
        nodes[leader_idx]
            .store
            .ai_resource_delete(NS, "replicated-server", R_TYPE)
            .await
            .expect("delete resource"),
        1
    );
    for _ in 0..150 {
        let mut gone = 0;
        for n in &nodes {
            if n.store
                .ai_resource_find(NS, "replicated-server", R_TYPE)
                .await
                .expect("find resource")
                .is_none()
            {
                gone += 1;
            }
        }
        if gone == nodes.len() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for n in &nodes {
        assert!(
            n.store
                .ai_resource_find(NS, "replicated-server", R_TYPE)
                .await
                .expect("find resource")
                .is_none(),
            "the delete must replicate to every node"
        );
    }

    for n in &nodes {
        n.node.shutdown().await.expect("shutdown raft node");
    }
}

/// Two nodes allocating IDs concurrently get different IDs.
///
/// Allocation happens during apply, so the read-modify-write on the counter is
/// covered by Raft's ordering. Were it done on the proposing node, both would
/// read the same counter value and hand out the same ID.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_inserts_on_different_nodes_get_distinct_ids() {
    let root = tempfile::tempdir().expect("root temp dir");
    let node1 = TestNode::start(root.path(), 1).await;
    let node2 = TestNode::start(root.path(), 2).await;
    let nodes = vec![node1, node2];

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
    let _leader_idx = wait_for_leader(&nodes).await;

    let names = ["ids-a", "ids-b", "ids-c", "ids-d"];
    let mut ids = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let node = &nodes[i % nodes.len()];
        ids.push(
            node.store
                .ai_resource_insert(&resource(name))
                .await
                .expect("insert resource"),
        );
    }

    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        ids.len(),
        "every insert must get its own id, got {ids:?}"
    );

    for n in &nodes {
        n.node.shutdown().await.expect("shutdown raft node");
    }
}

/// Chunk replacement replicates as one batch: the version is never left
/// half-replaced on a node.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chunk_replacement_replicates_as_a_whole() {
    let root = tempfile::tempdir().expect("root temp dir");
    let node1 = TestNode::start(root.path(), 1).await;
    let node2 = TestNode::start(root.path(), 2).await;
    let nodes = vec![node1, node2];

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
    let writer = &nodes[leader_idx];

    writer
        .store
        .search_chunk_replace(
            NS,
            R_TYPE,
            "chunk-server",
            "1.0.0",
            &[
                chunk("chunk-server", "one"),
                chunk("chunk-server", "two"),
                chunk("chunk-server", "three"),
            ],
        )
        .await
        .expect("replace chunks");

    writer
        .store
        .search_chunk_replace(
            NS,
            R_TYPE,
            "chunk-server",
            "1.0.0",
            &[chunk("chunk-server", "one")],
        )
        .await
        .expect("replace chunks again");

    for _ in 0..150 {
        let mut settled = 0;
        for n in &nodes {
            let len = n
                .store
                .search_chunk_list(NS, R_TYPE, "chunk-server", "1.0.0")
                .await
                .expect("list chunks")
                .len();
            if len == 1 {
                settled += 1;
            }
        }
        if settled == nodes.len() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    for n in &nodes {
        let chunks = n
            .store
            .search_chunk_list(NS, R_TYPE, "chunk-server", "1.0.0")
            .await
            .expect("list chunks");
        assert_eq!(
            chunks.len(),
            1,
            "the replacement must delete the stale chunks on every node"
        );
        assert_eq!(chunks[0].chunk_text, "text-one");
    }

    for n in &nodes {
        n.node.shutdown().await.expect("shutdown raft node");
    }
}

/// A resource list is served from replicated state, so both nodes agree.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn both_nodes_agree_on_the_resource_list() {
    let root = tempfile::tempdir().expect("root temp dir");
    let node1 = TestNode::start(root.path(), 1).await;
    let node2 = TestNode::start(root.path(), 2).await;
    let nodes = vec![node1, node2];

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

    for name in ["list-a", "list-b"] {
        nodes[leader_idx]
            .store
            .ai_resource_insert(&resource(name))
            .await
            .expect("insert resource");
    }

    wait_for_resource(&nodes, "list-a").await;
    wait_for_resource(&nodes, "list-b").await;

    let filter = Default::default();
    for n in &nodes {
        let page: Page<AiResourceInfo> = n
            .store
            .ai_resource_list(NS, R_TYPE, &filter, 1, 100)
            .await
            .expect("list resources");
        assert_eq!(
            page.total_count, 2,
            "every node must serve the same list from replicated state"
        );
    }

    for n in &nodes {
        n.node.shutdown().await.expect("shutdown raft node");
    }
}

/// A due index task makes both nodes agree on its schedule.
///
/// Tasks are replicated state, so a task scheduled through one node has to be
/// visible through the other; without this the two-node claim test below could
/// only ever prove that one node could not see the other's work.
async fn wait_for_task(nodes: &[TestNode], task_key: &str) {
    for _ in 0..150 {
        let mut seen = 0;
        for n in nodes {
            if n.store
                .task_find(task_key)
                .await
                .expect("find task")
                .is_some()
            {
                seen += 1;
            }
        }
        if seen == nodes.len() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("task {task_key} did not replicate to every node");
}

/// Poll until every node reports the same non-zero lease token.
async fn wait_for_lease(nodes: &[TestNode], task_key: &str) {
    for _ in 0..150 {
        let mut tokens = Vec::new();
        for n in nodes {
            tokens.push(
                n.store
                    .task_find(task_key)
                    .await
                    .expect("read task")
                    .expect("task exists")
                    .lease_token,
            );
        }
        if tokens.iter().all(|t| *t == tokens[0] && *t != 0) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the winning lease did not replicate to every node");
}

fn index_task(task_key: &str, next_execute_at: i64) -> AiResourceTaskInfo {
    AiResourceTaskInfo {
        task_key: task_key.to_string(),
        namespace_id: NS.to_string(),
        task_type: "search_index".to_string(),
        task_stage: "base_index".to_string(),
        status: "pending".to_string(),
        task_payload: "{}".to_string(),
        task_result: None,
        retry_count: 0,
        revision: 1,
        lease_token: 0,
        next_execute_at,
        lease_expire_at: None,
        last_error: None,
        gmt_create: None,
        gmt_modified: None,
    }
}

/// Two nodes polling the same queue must not both take the lease.
///
/// The claim is a compare-and-set evaluated during the Raft apply, so the two
/// proposals are ordered and the loser's write is dropped. If the comparison
/// were made on the proposing node instead, both nodes would read the same due
/// task and both would hand themselves the lease.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_nodes_cannot_both_claim_one_index_task() {
    let root = tempfile::tempdir().expect("root temp dir");
    let node1 = TestNode::start(root.path(), 1).await;
    let node2 = TestNode::start(root.path(), 2).await;
    let nodes = vec![node1, node2];

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
    let _leader_idx = wait_for_leader(&nodes).await;

    let now = chrono::Utc::now().timestamp_millis();
    nodes[0]
        .store
        .task_upsert(&index_task("shared-task", now - 1_000))
        .await
        .expect("schedule task");
    wait_for_task(&nodes, "shared-task").await;

    // Both nodes read the same due task, then both try to claim it.
    let mut snapshots = Vec::new();
    for n in &nodes {
        snapshots.push(
            n.store
                .task_find("shared-task")
                .await
                .expect("read task")
                .expect("task replicated"),
        );
    }
    let (first, second) = tokio::join!(
        nodes[0].store.task_claim(&snapshots[0], 60_000, now),
        nodes[1].store.task_claim(&snapshots[1], 60_000, now)
    );
    let claims = [first, second];

    let winners = claims
        .iter()
        .filter(|c| c.as_ref().expect("claim must not error").is_some())
        .count();
    assert_eq!(
        winners, 1,
        "exactly one node may hold the lease, got {winners}"
    );

    // The winning lease is the replicated one; both nodes must agree on it.
    wait_for_lease(&nodes, "shared-task").await;
    for n in &nodes {
        let stored = n
            .store
            .task_find("shared-task")
            .await
            .expect("read task")
            .expect("task exists");
        assert_eq!(stored.status, "processing");
        assert_eq!(
            stored.lease_token, 1,
            "the winning claim must be the one every node stores"
        );
    }

    for n in &nodes {
        n.node.shutdown().await.expect("shutdown raft node");
    }
}

/// A renewed lease reaches the other node, and a lapsed one cannot be
/// renewed by anybody.
///
/// Renewal is a replicated write, so the expiry the owner pushes forward has to
/// be the one every node later compares against — otherwise a second node would
/// still see the original expiry, decide the task is overdue and start it a
/// second time while the first is still running.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_renewed_lease_replicates_and_a_lapsed_one_cannot_be_renewed() {
    let root = tempfile::tempdir().expect("root temp dir");
    let node1 = TestNode::start(root.path(), 1).await;
    let node2 = TestNode::start(root.path(), 2).await;
    let nodes = vec![node1, node2];

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
    let _leader_idx = wait_for_leader(&nodes).await;

    let now = chrono::Utc::now().timestamp_millis();
    nodes[0]
        .store
        .task_upsert(&index_task("renewed-task", now - 1_000))
        .await
        .expect("schedule task");
    wait_for_task(&nodes, "renewed-task").await;

    let snapshot = nodes[0]
        .store
        .task_find("renewed-task")
        .await
        .expect("read task")
        .expect("task replicated");
    let claimed = nodes[0]
        .store
        .task_claim(&snapshot, 60_000, now)
        .await
        .expect("claim")
        .expect("the first claim must win");
    wait_for_lease(&nodes, "renewed-task").await;

    // Renew from the node that owns the lease and check the new expiry lands on
    // the other node too.
    let renewed_at = now + 20_000;
    assert!(
        nodes[0]
            .store
            .task_renew_lease(&claimed, 60_000, renewed_at)
            .await
            .expect("renew"),
        "the owner must be able to renew"
    );
    let expected_expiry = renewed_at + 60_000;
    for _ in 0..150 {
        let mut agreed = true;
        for n in &nodes {
            let stored = n
                .store
                .task_find("renewed-task")
                .await
                .expect("read task")
                .expect("task exists");
            if stored.lease_expire_at != Some(expected_expiry) {
                agreed = false;
            }
        }
        if agreed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for n in &nodes {
        let stored = n
            .store
            .task_find("renewed-task")
            .await
            .expect("read task")
            .expect("task exists");
        assert_eq!(
            stored.lease_expire_at,
            Some(expected_expiry),
            "the renewed expiry must be the one every node compares against"
        );
    }

    // Once it has lapsed — before anyone has taken it over, so the token still
    // matches — the owner must not be able to renew it.
    let after_expiry = expected_expiry + 1;
    assert!(
        !nodes[0]
            .store
            .task_renew_lease(&claimed, 60_000, after_expiry)
            .await
            .expect("renew"),
        "a lapsed lease must not be renewable even before anyone takes over"
    );

    let taken = nodes[1]
        .store
        .task_claim(&claimed, 60_000, after_expiry)
        .await
        .expect("claim")
        .expect("an expired lease must be taken over");
    assert!(
        !nodes[0]
            .store
            .task_renew_lease(&claimed, 60_000, after_expiry)
            .await
            .expect("renew"),
        "a lapsed lease must not be renewable by its previous owner"
    );
    // And the successor's renewal is the one that counts.
    assert!(
        nodes[1]
            .store
            .task_renew_lease(&taken, 60_000, after_expiry)
            .await
            .expect("renew"),
        "the new owner must be able to renew"
    );

    for n in &nodes {
        n.node.shutdown().await.expect("shutdown raft node");
    }
}
