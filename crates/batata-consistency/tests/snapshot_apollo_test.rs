use std::io::Cursor;

use openraft::storage::RaftStateMachine;
use openraft::RaftSnapshotBuilder;
use tempfile::tempdir;

use batata_consistency::raft::state_machine::{
    RocksStateMachine, CF_APOLLO_SERVER_CONFIG,
};

#[tokio::test]
async fn apollo_cf_survives_snapshot_round_trip() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let mut sm1 = RocksStateMachine::with_options_and_cfs(dir1.path(), None, None, &[])
        .await
        .unwrap();
    let db1 = sm1.db();
    let cf = db1.cf_handle(CF_APOLLO_SERVER_CONFIG).unwrap();
    db1.put_cf(&cf, b"server_config:test_key", b"test_value")
        .unwrap();

    // build snapshot from sm1
    let snapshot = sm1.build_snapshot().await.unwrap();
    let meta = snapshot.meta;
    let bytes = snapshot.snapshot.into_inner();

    // install into a fresh state machine sm2
    let mut sm2 = RocksStateMachine::with_options_and_cfs(dir2.path(), None, None, &[])
        .await
        .unwrap();
    sm2.install_snapshot(&meta, Box::new(Cursor::new(bytes)))
        .await
        .unwrap();

    // the apollo CF data must be present after restore
    let db2 = sm2.db();
    let cf2 = db2.cf_handle(CF_APOLLO_SERVER_CONFIG).unwrap();
    let got = db2.get_cf(&cf2, b"server_config:test_key").unwrap();
    assert_eq!(got.as_deref(), Some(b"test_value".as_slice()));
}
