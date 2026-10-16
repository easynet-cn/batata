//! On-disk snapshot persistence tests.
//!
//! Covers the JRaft-style `LocalSnapshotStorage` layout the state machine
//! writes: `temp/` staged first, published as `snapshot_{index}/`, with only
//! the newest snapshot kept.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use openraft::storage::RaftStateMachine;
use openraft::{CommittedLeaderId, LogId, RaftSnapshotBuilder, SnapshotMeta};
use tempfile::tempdir;

use batata_consistency::bincode;
use batata_consistency::raft::NamingApplyHook;
use batata_consistency::raft::state_machine::{
    CF_CONFIG, CF_INSTANCES, RocksStateMachine, StoredInstance,
};

/// Build a `SnapshotMeta` at the given log index, the way openraft would hand
/// it to `install_snapshot`.
fn meta_at(index: u64) -> SnapshotMeta<u64, openraft::BasicNode> {
    SnapshotMeta {
        last_log_id: Some(LogId::new(CommittedLeaderId::new(1, 0), index)),
        last_membership: Default::default(),
        snapshot_id: format!("snapshot-{}-1", index),
    }
}

async fn open_sm(
    root: &std::path::Path,
    name: &str,
    snapshots: &std::path::Path,
) -> RocksStateMachine {
    RocksStateMachine::with_options_and_cfs(root.join(name), None, None, &[])
        .await
        .unwrap()
        .with_snapshot_dir(snapshots.to_path_buf())
}

fn committed_snapshot_dirs(snapshots: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(snapshots)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("snapshot_"))
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn built_snapshot_is_persisted_and_reloadable() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    let mut sm = open_sm(dir.path(), "state", &snapshots).await;
    let db = sm.db();
    let cf = db.cf_handle(CF_CONFIG).unwrap();
    db.put_cf(&cf, b"cfg:key", b"cfg:value").unwrap();

    let snapshot = sm.build_snapshot().await.unwrap();
    let expected_id = snapshot.meta.snapshot_id.clone();

    // Staged writes must not survive: only the published directory remains.
    let dirs = committed_snapshot_dirs(&snapshots);
    assert_eq!(
        dirs.len(),
        1,
        "expected exactly one snapshot dir: {:?}",
        dirs
    );

    // A fresh node pointing at the same snapshot dir must recover from it.
    let mut reopened = open_sm(dir.path(), "state2", &snapshots).await;
    let loaded = reopened
        .get_current_snapshot()
        .await
        .unwrap()
        .expect("snapshot should be readable from disk");
    assert_eq!(loaded.meta.snapshot_id, expected_id);

    // The recovered bytes must actually restore the state machine.
    let mut restored = open_sm(dir.path(), "state3", &snapshots).await;
    restored
        .install_snapshot(&loaded.meta, loaded.snapshot)
        .await
        .unwrap();
    let db3 = restored.db();
    let cf3 = db3.cf_handle(CF_CONFIG).unwrap();
    assert_eq!(
        db3.get_cf(&cf3, b"cfg:key").unwrap().as_deref(),
        Some(b"cfg:value".as_slice())
    );
}

#[tokio::test]
async fn installed_snapshot_is_persisted_too() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    // Node A builds a snapshot with data in it.
    let mut source = open_sm(dir.path(), "source", &dir.path().join("source_snaps")).await;
    let source_db = source.db();
    let cf = source_db.cf_handle(CF_CONFIG).unwrap();
    source_db.put_cf(&cf, b"cfg:shared", b"v1").unwrap();
    let snapshot = source.build_snapshot().await.unwrap();
    let bytes = snapshot.snapshot.into_inner();

    // Node B installs it. The install must also land on disk, otherwise B has
    // nothing to recover from until it builds a snapshot itself.
    let meta = meta_at(11);
    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    target
        .install_snapshot(&meta, Box::new(Cursor::new(bytes)))
        .await
        .unwrap();

    let mut reopened = open_sm(dir.path(), "target2", &snapshots).await;
    let loaded = reopened
        .get_current_snapshot()
        .await
        .unwrap()
        .expect("installed snapshot should be on disk");
    assert_eq!(loaded.meta.snapshot_id, meta.snapshot_id);
}

#[tokio::test]
async fn only_the_newest_snapshot_is_kept() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    let mut sm = open_sm(dir.path(), "state", &snapshots).await;

    for index in [5u64, 9] {
        sm.install_snapshot(&meta_at(index), Box::new(Cursor::new(Vec::new())))
            .await
            .unwrap();
    }

    let dirs = committed_snapshot_dirs(&snapshots);
    assert_eq!(dirs, vec!["snapshot_9".to_string()]);

    let mut reopened = open_sm(dir.path(), "state2", &snapshots).await;
    let loaded = reopened.get_current_snapshot().await.unwrap().unwrap();
    assert_eq!(loaded.meta.last_log_id.unwrap().index, 9);
}

#[tokio::test]
async fn missing_snapshot_dir_means_no_snapshot() {
    let dir = tempdir().unwrap();
    let mut sm = RocksStateMachine::with_options_and_cfs(dir.path().join("state"), None, None, &[])
        .await
        .unwrap();

    // Without `with_snapshot_dir` the behaviour is unchanged: memory only.
    assert!(sm.get_current_snapshot().await.unwrap().is_none());
}

/// Records the instance id sets handed to `on_snapshot_loaded`.
struct RecordingHook {
    calls: Mutex<Vec<Vec<String>>>,
}

impl NamingApplyHook for RecordingHook {
    fn on_register(
        &self,
        _namespace_id: &str,
        _group_name: &str,
        _service_name: &str,
        _instance_id: &str,
        _ip: &str,
        _port: u16,
        _weight: f64,
        _healthy: bool,
        _enabled: bool,
        _metadata: &str,
        _cluster_name: &str,
    ) {
    }

    fn on_deregister(
        &self,
        _namespace_id: &str,
        _group_name: &str,
        _service_name: &str,
        _instance_id: &str,
    ) {
    }

    fn on_update(
        &self,
        _namespace_id: &str,
        _group_name: &str,
        _service_name: &str,
        _instance_id: &str,
        _ip: Option<&str>,
        _port: Option<u16>,
        _weight: Option<f64>,
        _healthy: Option<bool>,
        _enabled: Option<bool>,
        _metadata: Option<&str>,
    ) {
    }

    fn on_snapshot_loaded(&self, instances: Vec<StoredInstance>) {
        self.calls
            .lock()
            .unwrap()
            .push(instances.into_iter().map(|i| i.instance_id).collect());
    }
}

#[tokio::test]
async fn install_hands_restored_instances_to_the_naming_hook() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    // Source node holds one persistent instance.
    let mut source = open_sm(dir.path(), "source", &dir.path().join("source_snaps")).await;
    let source_db = source.db();
    let cf = source_db.cf_handle(CF_INSTANCES).unwrap();
    let stored = StoredInstance {
        namespace_id: "public".to_string(),
        group_name: "DEFAULT_GROUP".to_string(),
        service_name: "svc-a".to_string(),
        instance_id: "inst-1".to_string(),
        ip: "1.1.1.1".to_string(),
        port: 80,
        weight: 1.0,
        healthy: true,
        enabled: true,
        metadata: "{}".to_string(),
        cluster_name: "DEFAULT".to_string(),
        registered_time: 0,
        modified_time: 0,
    };
    source_db
        .put_cf(
            &cf,
            b"public@@DEFAULT_GROUP@@svc-a@@1.1.1.1#80#DEFAULT",
            &bincode::serialize(&stored).unwrap(),
        )
        .unwrap();
    let bytes = source.build_snapshot().await.unwrap().snapshot.into_inner();

    // Target installs it with a hook registered.
    let hook = Arc::new(RecordingHook {
        calls: Mutex::new(Vec::new()),
    });
    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    *target.naming_hook().write().await = Some(hook.clone());

    target
        .install_snapshot(&meta_at(7), Box::new(Cursor::new(bytes)))
        .await
        .unwrap();

    let calls = hook.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1, "hook must run once per install");
    assert_eq!(
        calls[0],
        vec!["inst-1".to_string()],
        "hook must receive the authoritative post-install instance set"
    );
}

#[tokio::test]
async fn install_without_a_naming_hook_still_succeeds() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    // No hook registered: the default no-op must keep the install working.
    target
        .install_snapshot(&meta_at(3), Box::new(Cursor::new(Vec::new())))
        .await
        .unwrap();

    let mut reopened = open_sm(dir.path(), "target2", &snapshots).await;
    assert!(reopened.get_current_snapshot().await.unwrap().is_some());
}
