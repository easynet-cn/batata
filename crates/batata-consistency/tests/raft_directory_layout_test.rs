//! Raft directory layout: consensus state is kept apart from the business DB.
//!
//! Regression cover for the two failure modes this layout rules out:
//!   * Raft log/snapshots leaking into the business database directory, and
//!   * the state machine being written under the Raft root, which made the
//!     standalone and cluster layouts disagree (switching mode orphaned data).

use std::net::TcpListener;

use batata_consistency::raft::{RaftConfig, RaftNode};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// Starting a node must place the log and snapshots under the Raft root while
/// the state machine lands on the shared business database path.
#[tokio::test]
async fn raft_state_and_business_db_do_not_share_a_directory() {
    let root = tempfile::tempdir().expect("root temp dir");

    let raft_root = root.path().join("protocol").join("raft");
    let business_db = root.path().join("batata_rocksdb");
    let addr = format!("127.0.0.1:{}", free_port());

    let config = RaftConfig {
        election_timeout_ms: 800,
        heartbeat_interval_ms: 200,
        data_dir: raft_root.clone(),
        state_machine_dir: Some(business_db.clone()),
        ..Default::default()
    };
    config.ensure_dirs().await.expect("ensure dirs");

    let (_node, _db) = RaftNode::new_with_db(1, addr, config.clone())
        .await
        .expect("create raft node");

    // Consensus-owned state sits under the Raft root.
    assert!(config.log_dir().exists(), "raft log dir was not created");
    assert!(
        config.snapshot_dir().exists(),
        "raft snapshot dir was not created"
    );
    assert_eq!(config.log_dir(), raft_root.join("log"));
    assert_eq!(config.snapshot_dir(), raft_root.join("snapshot"));

    // The database really opened on the business path: RocksDB drops CURRENT
    // there as soon as the instance exists.
    assert!(
        business_db.join("CURRENT").exists(),
        "state machine did not open on the business db path"
    );

    // And it is not nested under the Raft root -- that is what made the two
    // deployment modes disagree before.
    assert!(
        !business_db.starts_with(&raft_root),
        "business db must not live inside the raft root"
    );
    assert!(
        !raft_root.join("state").exists(),
        "state machine must not fall back to {{raft_root}}/state"
    );
}

/// Without an override the state machine stays at the Raft root, which is the
/// standalone-friendly default the library documents.
#[tokio::test]
async fn state_machine_defaults_under_the_raft_root() {
    let config = RaftConfig {
        data_dir: std::path::PathBuf::from("/tmp/raft"),
        ..Default::default()
    };
    assert_eq!(
        config.state_machine_dir(),
        std::path::PathBuf::from("/tmp/raft/state")
    );
}
