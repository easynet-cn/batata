//! AI search index on the distributed (Raft) backend.
//!
//! The compat doc used to say no test drove the cluster path. These do, with a
//! real single-node Raft cluster and the real `DistributedPersistService`.
//!
//! What they pin down is narrower than the embedded round-trip: that the
//! distributed backend implements the index at all rather than falling through
//! to the trait's "not supported by this storage backend" default, and that
//! upsert / chunk replacement behave the same way they do on the other two
//! backends.
//!
//! Worth knowing before reading the assertions: on this backend the index is
//! written **straight into RocksDB**, not through a Raft write. That is why the
//! consumer loop runs on every node and each one builds its own copy — the
//! index is node-local state derived from replicated data, not replicated
//! state itself. The test documents that rather than fighting it.

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use batata_consistency::RocksDbReader;
use batata_consistency::raft::{RaftConfig, RaftNode};
use batata_persistence::model::{
    AiResourceSearchChunkInfo, AiResourceSearchDocumentInfo, AiResourceTaskInfo,
};
use batata_persistence::{AiResourcePersistence, DistributedPersistService};
use openraft::BasicNode;

const NS: &str = "public";
const R_TYPE: &str = "mcp";
const R_NAME: &str = "distributed-probe";
const R_VERSION: &str = "1.0.0";

/// Reserve a free localhost port for the node's advertised address.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// Start a one-node Raft cluster and hand back the persistence built on it.
///
/// The Raft node is kept alive for as long as the store: the temp dir lives in
/// the returned guard, and dropping the node closes the state machine's
/// RocksDB out from under the reader.
struct Cluster {
    node: Arc<RaftNode>,
    store: Arc<DistributedPersistService>,
    _dir: tempfile::TempDir,
}

impl Cluster {
    async fn start() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let addr = format!("127.0.0.1:{}", free_port());

        let config = RaftConfig {
            election_timeout_ms: 300,
            heartbeat_interval_ms: 100,
            data_dir: dir.path().join("raft-1"),
            ..Default::default()
        };
        config.ensure_dirs().await.expect("ensure dirs");

        let (node, db) = RaftNode::new_with_db(1, addr.clone(), config)
            .await
            .expect("create raft node");
        let node = Arc::new(node);

        let mut members = BTreeMap::new();
        members.insert(1, BasicNode { addr });
        node.initialize(members).await.expect("initialize cluster");

        let leader = node
            .wait_for_leader(Duration::from_secs(10))
            .await
            .expect("wait for leader");
        assert_eq!(leader, Some(1), "the single node must elect itself");

        let store = Arc::new(DistributedPersistService::new(
            node.clone(),
            RocksDbReader::new(db),
        ));

        Self {
            node,
            store,
            _dir: dir,
        }
    }
}

fn document() -> AiResourceSearchDocumentInfo {
    AiResourceSearchDocumentInfo {
        id: 0,
        namespace_id: NS.to_string(),
        resource_type: R_TYPE.to_string(),
        resource_name: R_NAME.to_string(),
        resource_version: R_VERSION.to_string(),
        display_name: "Distributed Probe".to_string(),
        description: Some("initial".to_string()),
        tags: Some("[\"a\"]".to_string()),
        capabilities: None,
        representative_queries: None,
        metadata: None,
        source_digest: "digest-1".to_string(),
        status: "pending".to_string(),
        generate_mode: "auto".to_string(),
        gmt_create: None,
        gmt_modified: None,
    }
}

fn chunk(suffix: &str) -> AiResourceSearchChunkInfo {
    AiResourceSearchChunkInfo {
        id: 0,
        document_id: 0,
        namespace_id: NS.to_string(),
        resource_type: R_TYPE.to_string(),
        resource_name: R_NAME.to_string(),
        resource_version: R_VERSION.to_string(),
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

fn task(key: &str, next_execute_at: i64) -> AiResourceTaskInfo {
    AiResourceTaskInfo {
        task_key: key.to_string(),
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

#[tokio::test]
async fn distributed_backend_stores_the_search_index() {
    let cluster = Cluster::start().await;
    let db = cluster.store.clone();

    // ---- document: insert, then upsert in place -----------------------------
    let id = db
        .search_document_upsert(&document())
        .await
        .expect("insert document");
    assert!(id > 0, "the insert must assign an id");

    let found = db
        .search_document_find(NS, R_TYPE, R_NAME, R_VERSION)
        .await
        .expect("find document")
        .expect("the document must be readable");
    assert_eq!(found.id, id);
    assert_eq!(found.source_digest, "digest-1");

    let mut updated = document();
    updated.source_digest = "digest-2".to_string();
    let same_id = db
        .search_document_upsert(&updated)
        .await
        .expect("upsert document");
    assert_eq!(
        same_id, id,
        "an upsert must reuse the id rather than insert a second row"
    );
    assert_eq!(
        db.search_document_find(NS, R_TYPE, R_NAME, R_VERSION)
            .await
            .expect("find document")
            .expect("document must still exist")
            .source_digest,
        "digest-2",
        "the upsert must overwrite the digest"
    );

    // ---- chunks: replacement drops the ones no longer present ---------------
    let written = db
        .search_chunk_replace(NS, R_TYPE, R_NAME, R_VERSION, &[chunk("one"), chunk("two")])
        .await
        .expect("replace chunks");
    assert_eq!(written, 2, "both chunks must be written");
    assert_eq!(
        db.search_chunk_list(NS, R_TYPE, R_NAME, R_VERSION)
            .await
            .expect("list chunks")
            .len(),
        2
    );

    db.search_chunk_replace(NS, R_TYPE, R_NAME, R_VERSION, &[chunk("one")])
        .await
        .expect("replace chunks again");
    let chunks = db
        .search_chunk_list(NS, R_TYPE, R_NAME, R_VERSION)
        .await
        .expect("list chunks");
    assert_eq!(chunks.len(), 1, "replacement must delete the stale chunk");
    assert_eq!(chunks[0].chunk_text, "text-one");

    // ---- keyword search over the stored chunks ------------------------------
    let hits = db
        .search_chunk_search(NS, "text-one", &[R_TYPE], 10)
        .await
        .expect("search chunks");
    assert!(
        hits.iter().any(|hit| hit.resource_name == R_NAME),
        "the stored chunk must be searchable: {hits:?}"
    );

    // ---- tasks: schedule, find, then delete ---------------------------------
    let key = "mcp:public:distributed-probe";
    db.task_upsert(&task(key, 1_000))
        .await
        .expect("upsert task");
    let stored = db
        .task_find(key)
        .await
        .expect("find task")
        .expect("the task must be readable");
    assert_eq!(stored.task_type, "search_index");

    let due = db
        .task_find_due("search_index", 2_000, 10)
        .await
        .expect("find due tasks");
    assert!(
        due.iter().any(|t| t.task_key == key),
        "a task whose time has come must be due: {due:?}"
    );

    assert_eq!(
        db.task_delete(key).await.expect("delete task"),
        1,
        "deleting an existing task must remove one row"
    );
    assert!(
        db.task_find(key).await.expect("find task").is_none(),
        "the task must be gone"
    );

    // ---- document deletion --------------------------------------------------
    assert_eq!(
        db.search_document_delete(NS, R_TYPE, R_NAME, R_VERSION)
            .await
            .expect("delete document"),
        1
    );
    assert!(
        db.search_document_find(NS, R_TYPE, R_NAME, R_VERSION)
            .await
            .expect("find document")
            .is_none(),
        "the document must be gone"
    );

    cluster.node.shutdown().await.expect("shutdown raft node");
}
