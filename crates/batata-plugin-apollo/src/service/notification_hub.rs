//! Process-local wake-up accelerator for Apollo long-polling.
//!
//! The **truth source** for notification ids is the persisted
//! `ReleaseMessage` store (see [`super::release_message_service`]) — exactly
//! like upstream, where `NotificationControllerV2` reads ids from the DB cache
//! and only uses `DeferredResult`s to suspend requests. This hub plays the
//! DeferredResult role: it registers waiters under watch keys
//! (`appId+cluster+namespace`) and wakes them when this node publishes.
//! Restart-safe: ids survive in the store, so a client that missed a wake-up
//! still sees the newer id on its next poll.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::Notify;

#[derive(Default)]
pub struct ApolloNotificationHub {
    waiters: Mutex<HashMap<String, Vec<Arc<Notify>>>>,
}

impl ApolloNotificationHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one waiter under every watch key. Call BEFORE reading the
    /// persisted latest-ids to avoid a lost-wakeup race (upstream registers
    /// deferred results before checking the DB for the same reason).
    pub fn register(&self, keys: &[String]) -> Arc<Notify> {
        let notify = Arc::new(Notify::new());
        let mut waiters = self.waiters.lock().unwrap();
        for key in keys {
            waiters.entry(key.clone()).or_default().push(notify.clone());
        }
        notify
    }

    fn unregister(&self, keys: &[String], notify: &Arc<Notify>) {
        let mut waiters = self.waiters.lock().unwrap();
        for key in keys {
            if let Some(list) = waiters.get_mut(key) {
                list.retain(|n| !Arc::ptr_eq(n, notify));
                if list.is_empty() {
                    waiters.remove(key);
                }
            }
        }
    }

    /// Suspend until any watched key is notified or `timeout` elapses.
    /// Returns `true` when woken by a publish, `false` on timeout (304).
    pub async fn wait_any(&self, keys: &[String], timeout: Duration) -> bool {
        if keys.is_empty() {
            let _ = tokio::time::timeout(timeout, std::future::pending::<()>()).await;
            return false;
        }
        let notify = self.register(keys);
        let woken = tokio::time::timeout(timeout, notify.notified()).await.is_ok();
        self.unregister(keys, &notify);
        woken
    }

    /// Wake every poller watching this watch key ("appId+cluster+namespace").
    pub fn notify(&self, watch_key: &str) {
        let list = self.waiters.lock().unwrap().remove(watch_key);
        if let Some(list) = list {
            for notify in list {
                notify.notify_one();
            }
        }
    }

    /// Wake pollers for an appId/cluster/namespace triple.
    pub fn notify_namespace(&self, app_id: &str, cluster: &str, namespace_name: &str) {
        self.notify(&format!(
            "{}{}{}{}{}",
            app_id,
            crate::service::release_message_service::CLUSTER_NAMESPACE_SEPARATOR,
            cluster,
            crate::service::release_message_service::CLUSTER_NAMESPACE_SEPARATOR,
            namespace_name
        ));
    }
}

static HUB: OnceLock<Arc<ApolloNotificationHub>> = OnceLock::new();

/// Process-global notification hub.
pub fn hub() -> Arc<ApolloNotificationHub> {
    HUB.get_or_init(|| Arc::new(ApolloNotificationHub::new()))
        .clone()
}
