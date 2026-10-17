use std::sync::Arc;

use batata_consistency::raft::SnapshotFile;
use batata_consistency::raft::plugin::RaftPluginHandler;
use batata_consistency::raft::request::RaftResponse;
use batata_consistency::raft::state_machine::{CF_APOLLO_SERVER_CONFIG, RocksStateMachine};
use openraft::RaftSnapshotBuilder;
use openraft::storage::RaftStateMachine;
use rocksdb::DB;
use tempfile::tempdir;

/// Stand-in for the real Apollo handler: it owns no data of its own, it only
/// declares which column families must travel in a snapshot.
///
/// The core state machine used to hardcode the Apollo CF list; this test pins
/// the contract that replaced it — a plugin's CFs are snapshotted because the
/// plugin says so, not because core knows about them.
struct DeclaringPluginHandler {
    column_families: Vec<String>,
}

impl RaftPluginHandler for DeclaringPluginHandler {
    fn plugin_id(&self) -> &str {
        "apollo"
    }

    fn column_families(&self) -> Vec<String> {
        self.column_families.clone()
    }

    fn apply(&self, _db: &DB, _op_type: &str, _payload: &[u8], _log_index: u64) -> RaftResponse {
        RaftResponse::success()
    }

    fn build_snapshot(&self, _db: &DB) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }

    fn install_snapshot(&self, _db: &DB, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn plugin_declared_cf_survives_snapshot_round_trip() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let mut sm1 = RocksStateMachine::with_options_and_cfs(dir1.path(), None, None, &[])
        .await
        .unwrap();
    sm1.register_plugin(Arc::new(DeclaringPluginHandler {
        column_families: vec![CF_APOLLO_SERVER_CONFIG.to_string()],
    }))
    .await
    .unwrap();

    let db1 = sm1.db();
    let cf = db1.cf_handle(CF_APOLLO_SERVER_CONFIG).unwrap();
    db1.put_cf(&cf, b"server_config:test_key", b"test_value")
        .unwrap();

    // build snapshot from sm1
    let snapshot = sm1.build_snapshot().await.unwrap();
    let meta = snapshot.meta;
    let body = snapshot.snapshot.path().to_path_buf();

    // install into a fresh state machine sm2
    let mut sm2 = RocksStateMachine::with_options_and_cfs(dir2.path(), None, None, &[])
        .await
        .unwrap();
    sm2.install_snapshot(&meta, Box::new(SnapshotFile::open(&body).await.unwrap()))
        .await
        .unwrap();

    // the plugin-declared CF data must be present after restore
    let db2 = sm2.db();
    let cf2 = db2.cf_handle(CF_APOLLO_SERVER_CONFIG).unwrap();
    let got = db2.get_cf(&cf2, b"server_config:test_key").unwrap();
    assert_eq!(got.as_deref(), Some(b"test_value".as_slice()));
}

/// The flip side of the contract above: a CF nobody declares is not part of
/// the snapshot, even though the column family exists in RocksDB.
///
/// This is what makes the mechanism worth having — before, adding a CF to a
/// plugin meant editing the core snapshot list, and forgetting that silently
/// dropped the data for any follower recovering from a snapshot.
#[tokio::test]
async fn undeclared_cf_is_not_snapshotted() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let mut sm1 = RocksStateMachine::with_options_and_cfs(dir1.path(), None, None, &[])
        .await
        .unwrap();
    let db1 = sm1.db();
    let cf = db1.cf_handle(CF_APOLLO_SERVER_CONFIG).unwrap();
    db1.put_cf(&cf, b"server_config:test_key", b"test_value")
        .unwrap();

    let snapshot = sm1.build_snapshot().await.unwrap();
    let meta = snapshot.meta;
    let body = snapshot.snapshot.path().to_path_buf();

    let mut sm2 = RocksStateMachine::with_options_and_cfs(dir2.path(), None, None, &[])
        .await
        .unwrap();
    sm2.install_snapshot(&meta, Box::new(SnapshotFile::open(&body).await.unwrap()))
        .await
        .unwrap();

    let db2 = sm2.db();
    let cf2 = db2.cf_handle(CF_APOLLO_SERVER_CONFIG).unwrap();
    assert_eq!(db2.get_cf(&cf2, b"server_config:test_key").unwrap(), None);
}
