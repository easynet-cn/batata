//! On-disk snapshot persistence tests.
//!
//! Covers the JRaft-style `LocalSnapshotStorage` layout the state machine
//! writes: `temp/` staged first, published as `snapshot_{index}/`, with only
//! the newest snapshot kept. The body is a file (`SnapshotFile`) rather than an
//! in-memory buffer, so these tests move paths around instead of byte vectors.

use std::sync::{Arc, Mutex};

use openraft::RaftSnapshotBuilder;
use openraft::storage::RaftStateMachine;
use tempfile::tempdir;

use batata_consistency::bincode;
use batata_consistency::raft::NamingApplyHook;
use batata_consistency::raft::state_machine::{
    CF_CONFIG, CF_INSTANCES, RocksStateMachine, StoredInstance,
};
use batata_consistency::raft::types::{CommittedLeaderId, LogId, SnapshotMeta};
use batata_consistency::raft::{SnapshotFile, snapshot_format};

/// Build a `SnapshotMeta` at the given log index, the way openraft would hand
/// it to `install_snapshot`. Since 0.10 a snapshot is identified by the log
/// position it covers; the `snapshot_id` field no longer exists.
fn meta_at(index: u64) -> SnapshotMeta {
    SnapshotMeta {
        last_log_id: Some(LogId::new(
            CommittedLeaderId {
                term: 1,
                node_id: 0,
            },
            index,
        )),
        last_membership: Default::default(),
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

/// An empty snapshot body, standing in for a node that has no state yet.
async fn empty_snapshot(dir: &std::path::Path, name: &str) -> SnapshotFile {
    SnapshotFile::create(dir.join(name)).await.unwrap()
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
    let expected_last_log_id = snapshot.meta.last_log_id;

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
    assert_eq!(loaded.meta.last_log_id, expected_last_log_id);

    // The recovered body must actually restore the state machine.
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
    let body = snapshot.snapshot.path().to_path_buf();

    // Node B installs it. The install must also land on disk, otherwise B has
    // nothing to recover from until it builds a snapshot itself.
    let meta = meta_at(11);
    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    target
        .install_snapshot(&meta, SnapshotFile::open(&body).await.unwrap())
        .await
        .unwrap();

    let mut reopened = open_sm(dir.path(), "target2", &snapshots).await;
    let loaded = reopened
        .get_current_snapshot()
        .await
        .unwrap()
        .expect("installed snapshot should be on disk");
    assert_eq!(loaded.meta.last_log_id, meta.last_log_id);
}

#[tokio::test]
async fn only_the_newest_snapshot_is_kept() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    let mut sm = open_sm(dir.path(), "state", &snapshots).await;

    for index in [5u64, 9] {
        let empty = empty_snapshot(dir.path(), &format!("empty-{index}")).await;
        sm.install_snapshot(&meta_at(index), empty).await.unwrap();
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

    // Without `with_snapshot_dir` the behaviour is unchanged: nothing on disk.
    assert!(sm.get_current_snapshot().await.unwrap().is_none());
}

#[tokio::test]
async fn corrupted_snapshot_body_is_refused() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    let mut source = open_sm(dir.path(), "source", &dir.path().join("source_snaps")).await;
    let source_db = source.db();
    let cf = source_db.cf_handle(CF_CONFIG).unwrap();
    source_db.put_cf(&cf, b"cfg:key", b"cfg:value").unwrap();
    let snapshot = source.build_snapshot().await.unwrap();
    let body = snapshot.snapshot.path().to_path_buf();

    // The untouched body installs cleanly.
    let mut good = open_sm(dir.path(), "good", &snapshots).await;
    good.install_snapshot(&meta_at(4), SnapshotFile::open(&body).await.unwrap())
        .await
        .expect("a healthy snapshot must install");

    // Flip one byte in the middle: the checksum must catch it before anything
    // is written to the database.
    let mut bytes = tokio::fs::read(&body).await.unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    let corrupted = dir.path().join("corrupted");
    tokio::fs::write(&corrupted, &bytes).await.unwrap();
    snapshot_format::verify_file(&corrupted)
        .await
        .expect_err("checksum must reject a corrupted body");

    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    let result = target
        .install_snapshot(&meta_at(5), SnapshotFile::open(&corrupted).await.unwrap())
        .await;
    assert!(result.is_err(), "a corrupted snapshot must be refused");
}

/// A snapshot body far larger than one restore batch must round-trip exactly.
///
/// This is the regression guard for the streaming rewrite: the body is written
/// and applied in fixed-size batches, so correctness must not depend on the
/// snapshot fitting into a single batch.
#[tokio::test]
async fn large_snapshot_round_trips_in_batches() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    let mut source = open_sm(dir.path(), "source", &dir.path().join("source_snaps")).await;
    let source_db = source.db();
    let cf = source_db.cf_handle(CF_CONFIG).unwrap();
    let entries = 5_000u32;
    for i in 0..entries {
        source_db
            .put_cf(&cf, format!("cfg:key-{i}"), format!("value-{i}"))
            .unwrap();
    }
    let snapshot = source.build_snapshot().await.unwrap();
    let body = snapshot.snapshot.path().to_path_buf();
    let body_len = tokio::fs::metadata(&body).await.unwrap().len();
    assert!(body_len > 0, "snapshot body must have been written");

    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    target
        .install_snapshot(&meta_at(20), SnapshotFile::open(&body).await.unwrap())
        .await
        .expect("large snapshot must install");

    let target_db = target.db();
    let target_cf = target_db.cf_handle(CF_CONFIG).unwrap();
    for i in 0..entries {
        assert_eq!(
            target_db
                .get_cf(&target_cf, format!("cfg:key-{i}"))
                .unwrap()
                .as_deref(),
            Some(format!("value-{i}").as_bytes()),
            "entry {i} did not survive the round trip"
        );
    }
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
            bincode::serialize(&stored).unwrap(),
        )
        .unwrap();
    let body = source
        .build_snapshot()
        .await
        .unwrap()
        .snapshot
        .path()
        .to_path_buf();

    // Target installs it with a hook registered.
    let hook = Arc::new(RecordingHook {
        calls: Mutex::new(Vec::new()),
    });
    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    target.install_naming_hook(hook.clone()).await;

    target
        .install_snapshot(&meta_at(7), SnapshotFile::open(&body).await.unwrap())
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
    let empty = empty_snapshot(dir.path(), "empty").await;
    target.install_snapshot(&meta_at(3), empty).await.unwrap();

    let mut reopened = open_sm(dir.path(), "target2", &snapshots).await;
    assert!(reopened.get_current_snapshot().await.unwrap().is_some());
}

#[tokio::test]
async fn install_clears_stale_keys_before_applying_snapshot() {
    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");

    let mut source = open_sm(dir.path(), "source", &dir.path().join("source_snaps")).await;
    let source_db = source.db();
    let source_cf = source_db.cf_handle(CF_CONFIG).unwrap();
    source_db.put_cf(&source_cf, b"cfg:kept", b"new").unwrap();
    let snapshot = source.build_snapshot().await.unwrap();
    let body = snapshot.snapshot.path().to_path_buf();

    let mut target = open_sm(dir.path(), "target", &snapshots).await;
    let target_db = target.db();
    let target_cf = target_db.cf_handle(CF_CONFIG).unwrap();
    target_db.put_cf(&target_cf, b"cfg:stale", b"old").unwrap();

    target
        .install_snapshot(&meta_at(6), SnapshotFile::open(&body).await.unwrap())
        .await
        .unwrap();

    assert!(
        target_db
            .get_cf(&target_cf, b"cfg:stale")
            .unwrap()
            .is_none(),
        "the first restore batch must still clear the target column family"
    );
    assert_eq!(
        target_db
            .get_cf(&target_cf, b"cfg:kept")
            .unwrap()
            .as_deref(),
        Some(b"new".as_slice()),
        "snapshot entries must still be applied after the range delete"
    );
}

/// A naming hook that deliberately blocks inside `on_snapshot_loaded`. If
/// snapshot install ever runs this callback inline on the runtime thread, the
/// heartbeat test below observes a large scheduling gap.
struct SlowRecordingHook {
    called: Arc<Mutex<bool>>,
    delay: std::time::Duration,
}

impl NamingApplyHook for SlowRecordingHook {
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

    fn on_snapshot_loaded(&self, _instances: Vec<StoredInstance>) {
        std::thread::sleep(self.delay);
        *self.called.lock().unwrap() = true;
    }
}

/// Targeted responsiveness test: a slow post-install reconciliation must not
/// stall the single-threaded Raft runtime. A heartbeat task keeps ticking on
/// the same current-thread runtime while `install_snapshot` is awaited.
#[tokio::test(flavor = "current_thread")]
async fn snapshot_install_keeps_the_runtime_responsive() {
    use std::time::Duration;
    use tokio::time::{Instant, interval};

    let dir = tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");
    let mut target = open_sm(dir.path(), "target", &snapshots).await;

    let called = Arc::new(Mutex::new(false));
    let hook = Arc::new(SlowRecordingHook {
        called: called.clone(),
        delay: Duration::from_millis(600),
    });
    target
        .install_naming_hook(hook as Arc<dyn NamingApplyHook>)
        .await;

    let max_gap = Arc::new(Mutex::new(Duration::ZERO));
    let heartbeat_max = max_gap.clone();
    let heartbeat = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(10));
        let mut last = Instant::now();
        // Skip the immediate first tick.
        ticker.tick().await;
        for _ in 0..120 {
            ticker.tick().await;
            let now = Instant::now();
            let gap = now.duration_since(last);
            last = now;
            let mut guard = heartbeat_max.lock().unwrap();
            if gap > *guard {
                *guard = gap;
            }
        }
    });

    let empty = empty_snapshot(dir.path(), "empty").await;
    target.install_snapshot(&meta_at(1), empty).await.unwrap();

    heartbeat.await.unwrap();
    assert!(
        *called.lock().unwrap(),
        "naming hook must run during snapshot install"
    );
    let observed = *max_gap.lock().unwrap();
    assert!(
        observed < Duration::from_millis(400),
        "snapshot install stalled the runtime for {observed:?}"
    );
}
