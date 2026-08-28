mod common;

use batata_plugin_apollo::persistence::traits::ReleaseMessagePersistence;
use batata_plugin_apollo::service::notification_hub::hub;
use batata_plugin_apollo::service::release_message_service::{
    ReleaseMessageService, CLUSTER_NAMESPACE_SEPARATOR,
};

#[tokio::test]
async fn send_message_persists_plain_watch_key_with_row_pk() {
    let p = common::make_embedded_persistence().await;
    let svc = ReleaseMessageService::new(p.clone());

    let m1 = svc.send_message("app1", "default", "ns1").await.unwrap();
    assert_eq!(
        m1.message,
        format!("app1{}default{}ns1", CLUSTER_NAMESPACE_SEPARATOR, CLUSTER_NAMESPACE_SEPARATOR)
    );
    assert!(m1.id > 0, "notification id is the persisted row pk");

    let m2 = svc.send_message("app1", "default", "ns2").await.unwrap();
    assert!(m2.id > m1.id, "ids are globally monotonic");
}

#[tokio::test]
async fn latest_per_key_and_prune() {
    let p = common::make_embedded_persistence().await;
    let svc = ReleaseMessageService::new(p.clone());

    let first = svc.send_message("app1", "default", "ns1").await.unwrap();
    // unrelated key must not interfere
    svc.send_message("other", "default", "ns1").await.unwrap();

    let latest = svc.find_latest_by_key(&first.message).await.unwrap().unwrap();
    assert_eq!(latest.id, first.id);

    // republish same key → only newest retained per key
    let second = svc.send_message("app1", "default", "ns1").await.unwrap();
    assert!(second.id > first.id);
    let latest = svc.find_latest_by_key(&first.message).await.unwrap().unwrap();
    assert_eq!(latest.id, second.id);

    let all = <dyn ReleaseMessagePersistence>::list_all(&*p).await.unwrap();
    assert_eq!(all.len(), 2, "stale rows are pruned on rewrite");
}

#[tokio::test]
async fn ids_survive_restart_via_recovery() {
    // First "process": publish messages.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    {
        let p = common::make_embedded_persistence_at(&path).await;
        let svc = ReleaseMessageService::new(p.clone());
        let m1 = svc.send_message("a", "default", "n").await.unwrap();
        let _m2 = svc.send_message("a", "default", "n").await.unwrap();
        assert!(m1.id > 0);
    }
    // Second "process": reopened store must continue after the max id.
    let db = open_rocksdb_at(&path).await;
    let p = std::sync::Arc::new(
        batata_plugin_apollo::persistence::EmbeddedApolloPersistence::new(db),
    );
    let svc = ReleaseMessageService::new(p);
    let m3 = svc.send_message("a", "default", "n").await.unwrap();
    assert!(m3.id >= 3, "recovered id continues monotonically, got {}", m3.id);
}

async fn open_rocksdb_at(path: &std::path::Path) -> std::sync::Arc<rocksdb::DB> {
    let sm = batata_consistency::raft::state_machine::RocksStateMachine::with_options_and_cfs(
        path,
        None,
        None,
        &[],
    )
    .await
    .unwrap();
    sm.db()
}

#[tokio::test]
async fn hub_wakes_registered_waiter_on_notify() {
    let h = hub();
    let keys = vec![format!("a{}default{}n", CLUSTER_NAMESPACE_SEPARATOR, CLUSTER_NAMESPACE_SEPARATOR)];
    let notify = h.register(&keys);
    let waker = notify.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        h.notify(&keys[0]);
        drop(waker);
    });
    let woken = tokio::time::timeout(std::time::Duration::from_secs(2), notify.notified())
        .await
        .is_ok();
    assert!(woken, "waiter should be woken by hub notify");
}
